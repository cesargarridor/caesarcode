pub mod commands;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::services::process_tree::kill_tree;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TerminalEvent {
    Data { data: String },
    Exit { code: Option<i32> },
}

/// Envía un evento; devuelve `false` si el receptor desapareció.
pub type TerminalSink = Box<dyn Fn(TerminalEvent) -> bool + Send + Sync>;

struct Term {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    pid: Option<u32>,
}

#[derive(Default)]
pub struct TerminalManager {
    terms: Arc<Mutex<HashMap<String, Term>>>,
}

/// Decodificación UTF-8 con arrastre: un carácter partido entre lecturas se conserva
/// para la siguiente en lugar de convertirse en U+FFFD.
#[derive(Default)]
pub struct Utf8Carry {
    carry: Vec<u8>,
}

impl Utf8Carry {
    pub fn push(&mut self, data: &[u8]) -> String {
        self.carry.extend_from_slice(data);
        let mut out = String::new();
        let mut rest: &[u8] = &self.carry;
        loop {
            match std::str::from_utf8(rest) {
                Ok(s) => {
                    out.push_str(s);
                    rest = &[];
                    break;
                }
                Err(e) => {
                    let (ok, bad) = rest.split_at(e.valid_up_to());
                    out.push_str(std::str::from_utf8(ok).unwrap_or(""));
                    match e.error_len() {
                        None => {
                            rest = bad; // secuencia incompleta al final: arrastrar
                            break;
                        }
                        Some(n) => {
                            out.push('\u{FFFD}');
                            rest = &bad[n..];
                        }
                    }
                }
            }
        }
        let keep = rest.to_vec();
        self.carry = keep;
        out
    }

    /// EOF: lo que quede sin completar se emite como U+FFFD.
    pub fn finish(&mut self) -> String {
        if self.carry.is_empty() {
            String::new()
        } else {
            self.carry.clear();
            "\u{FFFD}".to_string()
        }
    }
}

fn pick_shell() -> (PathBuf, Vec<String>) {
    #[cfg(unix)]
    {
        let mut cands: Vec<PathBuf> = Vec::new();
        if let Ok(s) = std::env::var("SHELL") {
            if !s.is_empty() {
                cands.push(PathBuf::from(s));
            }
        }
        cands.push(PathBuf::from("/bin/zsh"));
        cands.push(PathBuf::from("/bin/bash"));
        cands.push(PathBuf::from("/bin/sh"));
        let sh = cands
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| PathBuf::from("/bin/sh"));
        (sh, vec!["-l".to_string()])
    }
    #[cfg(windows)]
    {
        if let Ok(p) = which::which("powershell.exe") {
            return (p, vec!["-NoLogo".to_string()]);
        }
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into());
        (PathBuf::from(comspec), Vec::new())
    }
}

impl TerminalManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Abre un PTY con el shell de login. Devuelve el id.
    pub fn open(&self, cwd: &Path, cols: u16, rows: u16, sink: TerminalSink) -> AppResult<String> {
        let (shell, args) = pick_shell();
        self.spawn(cwd, shell, args, cols, rows, sink)
    }

    /// Abre un PTY ejecutando directamente `program` con `args` (agentes IA: vacío). Mismo gestor:
    /// `write/resize/close` sirven igual.
    pub fn open_program(&self, cwd: &Path, program: &Path, args: Vec<String>, cols: u16, rows: u16, sink: TerminalSink) -> AppResult<String> {
        self.spawn(cwd, program.to_path_buf(), args, cols, rows, sink)
    }

    fn spawn(&self, cwd: &Path, shell: PathBuf, args: Vec<String>, cols: u16, rows: u16, sink: TerminalSink) -> AppResult<String> {
        let pair = native_pty_system()
            .openpty(PtySize { rows: rows.max(1), cols: cols.max(1), pixel_width: 0, pixel_height: 0 })
            .map_err(|e| AppError::Pty(e.to_string()))?;
        let mut cmd = CommandBuilder::new(shell);
        cmd.args(args);
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| AppError::Pty(e.to_string()))?;
        drop(pair.slave); // sin esto el lector no recibe EOF
        let pid = child.process_id();
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| AppError::Pty(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| AppError::Pty(e.to_string()))?;
        let id = uuid::Uuid::new_v4().to_string();
        self.terms
            .lock()
            .map_err(|_| AppError::Pty("lock envenenado".into()))?
            .insert(id.clone(), Term { writer, master: pair.master, child, pid });

        let terms = Arc::clone(&self.terms);
        let tid = id.clone();
        std::thread::Builder::new()
            .name(format!("term-reader-{tid}"))
            .spawn(move || {
                let mut buf = [0u8; 16 * 1024];
                let mut utf8 = Utf8Carry::default();
                let mut receiver_gone = false;
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let s = utf8.push(&buf[..n]);
                            if !s.is_empty() && !sink(TerminalEvent::Data { data: s }) {
                                receiver_gone = true;
                                break;
                            }
                        }
                    }
                }
                if receiver_gone {
                    // Webview recargado/cerrado: nadie puede usar esta terminal, no dejar el shell vivo.
                    let term = terms.lock().ok().and_then(|mut g| g.remove(&tid));
                    if let Some(t) = term {
                        kill_term(t);
                    }
                    return;
                }
                let tail = utf8.finish();
                if !tail.is_empty() {
                    sink(TerminalEvent::Data { data: tail });
                }
                // Código de salida: el proceso puede tardar un instante en ser reapeable.
                let mut code: Option<i32> = None;
                let end = Instant::now() + Duration::from_millis(1000);
                loop {
                    let Ok(mut g) = terms.lock() else { break };
                    match g.get_mut(&tid) {
                        None => break, // cerrado por el usuario
                        Some(t) => match t.child.try_wait() {
                            Ok(Some(st)) => {
                                code = Some(st.exit_code() as i32);
                                g.remove(&tid);
                                break;
                            }
                            Ok(None) if Instant::now() < end => {}
                            _ => {
                                g.remove(&tid);
                                break;
                            }
                        },
                    }
                    drop(g);
                    std::thread::sleep(Duration::from_millis(25));
                }
                sink(TerminalEvent::Exit { code });
            })
            .map_err(|e| AppError::Pty(e.to_string()))?;
        Ok(id)
    }

    pub fn write(&self, id: &str, data: &str) -> AppResult<()> {
        let mut g = self.terms.lock().map_err(|_| AppError::Pty("lock envenenado".into()))?;
        let t = g.get_mut(id).ok_or_else(|| AppError::NotFound(format!("terminal {id}")))?;
        t.writer.write_all(data.as_bytes())?;
        t.writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> AppResult<()> {
        let g = self.terms.lock().map_err(|_| AppError::Pty("lock envenenado".into()))?;
        let t = g.get(id).ok_or_else(|| AppError::NotFound(format!("terminal {id}")))?;
        t.master
            .resize(PtySize { rows: rows.max(1), cols: cols.max(1), pixel_width: 0, pixel_height: 0 })
            .map_err(|e| AppError::Pty(e.to_string()))
    }

    pub fn close(&self, id: &str) -> AppResult<()> {
        let term = self
            .terms
            .lock()
            .map_err(|_| AppError::Pty("lock envenenado".into()))?
            .remove(id);
        if let Some(t) = term {
            kill_term(t);
        }
        Ok(())
    }

    /// Cierra todas (shutdown), en paralelo.
    pub fn close_all(&self) {
        let all: Vec<Term> = match self.terms.lock() {
            Ok(mut g) => g.drain().map(|(_, t)| t).collect(),
            Err(_) => return,
        };
        std::thread::scope(|s| {
            for t in all {
                s.spawn(move || kill_term(t));
            }
        });
    }

    #[cfg(test)]
    fn pid_of(&self, id: &str) -> Option<u32> {
        self.terms.lock().ok()?.get(id)?.pid
    }

    #[cfg(test)]
    fn count(&self) -> usize {
        self.terms.lock().map(|g| g.len()).unwrap_or(0)
    }
}

fn kill_term(mut t: Term) {
    if let Some(pid) = t.pid {
        kill_tree(pid, Duration::from_millis(500), true);
    }
    let _ = t.child.kill();
    let _ = t.child.wait();
    // `t.master` y `t.writer` se dropean aquí: cierra el PTY (necesario en ConPTY).
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::services::process_tree::{is_alive, snapshot_system, tree_pids};

    #[test]
    fn utf8_carry_joins_split_chars_and_replaces_invalid() {
        let mut c = Utf8Carry::default();
        let bytes = "añ€😀".as_bytes();
        let mut out = String::new();
        for b in bytes.chunks(1) {
            out.push_str(&c.push(b));
        }
        assert_eq!(out, "añ€😀");
        assert_eq!(c.push(&[b'a', 0xFF, b'b']), "a\u{FFFD}b");
        assert_eq!(c.push(&[0xE2, 0x82]), "");
        assert_eq!(c.finish(), "\u{FFFD}");
    }

    fn wait_for(mut f: impl FnMut() -> bool, ms: u64) -> bool {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        f()
    }

    #[test]
    fn interactive_shell_echo_resize_exit_and_close() {
        let tm = TerminalManager::new();
        let out: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let exited: Arc<Mutex<Option<Option<i32>>>> = Arc::new(Mutex::new(None));
        let (o2, e2) = (Arc::clone(&out), Arc::clone(&exited));
        let id = tm
            .open(
                &std::env::temp_dir(),
                100,
                30,
                Box::new(move |ev| {
                    match ev {
                        TerminalEvent::Data { data } => o2.lock().unwrap().push_str(&data),
                        TerminalEvent::Exit { code } => *e2.lock().unwrap() = Some(code),
                    }
                    true
                }),
            )
            .unwrap();
        tm.write(&id, "echo cc_$((20+22))_ok\n").unwrap();
        assert!(
            wait_for(|| out.lock().unwrap().contains("cc_42_ok"), 8000),
            "sin eco: {:?}",
            out.lock().unwrap()
        );
        tm.resize(&id, 120, 40).unwrap();
        tm.write(&id, "stty size\n").unwrap();
        assert!(wait_for(|| out.lock().unwrap().contains("40 120"), 8000), "resize no aplicado");
        tm.write(&id, "exit 7\n").unwrap();
        assert!(wait_for(|| exited.lock().unwrap().is_some(), 5000));
        assert_eq!(*exited.lock().unwrap(), Some(Some(7)));
        assert_eq!(tm.count(), 0);

        // close mata el shell y sus hijos
        let id2 = tm.open(&std::env::temp_dir(), 80, 24, Box::new(|_| true)).unwrap();
        tm.write(&id2, "sleep 321\n").unwrap();
        let pid = tm.pid_of(&id2).unwrap();
        let mut tree = Vec::new();
        assert!(wait_for(
            || {
                tree = tree_pids(&snapshot_system(), pid);
                tree.len() >= 2
            },
            5000
        ));
        tm.close(&id2).unwrap();
        for p in tree {
            assert!(!is_alive(p), "pid {p} sigue vivo tras close");
        }
        assert!(tm.write(&id2, "x").is_err());
    }
}

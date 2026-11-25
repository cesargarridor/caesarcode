//! En macOS/Linux una app GUI lanzada desde Finder/lanzador no hereda el PATH del shell.
//! Se pregunta al shell de login interactivo y se fija `PATH` en el proceso.

#[cfg(unix)]
pub fn fix_path() {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let child = Command::new(&shell)
        .args(["-ilc", "printf \"%s\" \"$PATH\""])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut stdout = match child.stdout.take() {
        Some(s) => s,
        None => return,
    };
    // Lectura en hilo para poder aplicar timeout de 3 s.
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        let _ = tx.send(buf);
    });
    let start = Instant::now();
    let result = loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(s) => break Some(s),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if start.elapsed() > Duration::from_secs(3) {
                    break None;
                }
            }
            Err(_) => break None,
        }
    };
    match result {
        Some(path) => {
            let _ = child.wait();
            // La última línea evita ruido que el rc del shell pueda imprimir antes.
            let path = path.lines().last().unwrap_or("").trim().to_string();
            if !path.is_empty() {
                std::env::set_var("PATH", path);
            }
        }
        None => {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(not(unix))]
pub fn fix_path() {}

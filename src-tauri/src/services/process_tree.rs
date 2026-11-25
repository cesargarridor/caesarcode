use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use sysinfo::{Pid, ProcessesToUpdate, System};

/// Foto de todos los procesos (solo relaciones padre/hijo).
pub fn snapshot_system() -> System {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys
}

/// Descendientes de `root` (BFS), sin incluir `root`.
pub fn descendants(sys: &System, root: u32) -> Vec<u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, p) in sys.processes() {
        if let Some(parent) = p.parent() {
            children.entry(parent.as_u32()).or_default().push(pid.as_u32());
        }
    }
    let mut out = Vec::new();
    let mut q = VecDeque::from([root]);
    while let Some(cur) = q.pop_front() {
        if let Some(ch) = children.get(&cur) {
            for c in ch {
                if *c != root && !out.contains(c) {
                    out.push(*c);
                    q.push_back(*c);
                }
            }
        }
    }
    out
}

/// `root` y sus descendientes.
pub fn tree_pids(sys: &System, root: u32) -> Vec<u32> {
    let mut v = vec![root];
    v.extend(descendants(sys, root));
    v
}

/// ¿Sigue vivo? Un zombi (muerto sin reapear) cuenta como muerto.
pub fn is_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // SAFETY: kill con señal 0 solo comprueba existencia.
        if unsafe { libc::kill(pid as i32, 0) } != 0 {
            return false;
        }
    }
    let mut sys = System::new();
    let p = Pid::from_u32(pid);
    sys.refresh_processes(ProcessesToUpdate::Some(&[p]), true);
    match sys.process(p) {
        Some(proc_) => proc_.status() != sysinfo::ProcessStatus::Zombie,
        None => false,
    }
}

/// Mata `root` y todo su árbol. La foto del árbol se toma ANTES de matar
/// (los huérfanos se re-parentan a launchd/init). `group`: además `killpg(root)`
/// (válido solo si `root` es líder de sesión/grupo, como el hijo de portable-pty).
pub fn kill_tree(root: u32, grace: Duration, group: bool) {
    if root <= 1 || root == std::process::id() {
        return;
    }
    let sys = snapshot_system();
    let pids = tree_pids(&sys, root);
    drop(sys);
    kill_pids(root, &pids, grace, group);
}

#[cfg(unix)]
fn kill_pids(root: u32, pids: &[u32], grace: Duration, group: bool) {
    let signal_all = |sig: i32, only: &[u32]| {
        // SAFETY: llamadas libc simples con pids/pgids obtenidos del sistema.
        unsafe {
            if group {
                libc::killpg(root as i32, sig);
            }
            for p in only {
                libc::kill(*p as i32, sig);
            }
        }
    };
    signal_all(libc::SIGTERM, pids);
    let deadline = std::time::Instant::now() + grace;
    let mut alive: Vec<u32> = pids.iter().copied().filter(|p| is_alive(*p)).collect();
    while !alive.is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        alive.retain(|p| is_alive(*p));
    }
    if !alive.is_empty() {
        signal_all(libc::SIGKILL, &alive);
        let end = std::time::Instant::now() + Duration::from_millis(500);
        while alive.iter().any(|p| is_alive(*p)) && std::time::Instant::now() < end {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[cfg(windows)]
fn kill_pids(root: u32, pids: &[u32], _grace: Duration, _group: bool) {
    use crate::util::proc::no_window;
    let _ = no_window(std::process::Command::new("taskkill").args(["/PID", &root.to_string(), "/T", "/F"])).output();
    for p in pids {
        if *p != root && is_alive(*p) {
            let _ = no_window(std::process::Command::new("taskkill").args(["/PID", &p.to_string(), "/F"])).output();
        }
    }
}

/// Mata el grupo de procesos de `pgid` (best-effort, p. ej. nietos que sobreviven
/// a la salida natural del líder).
#[cfg(unix)]
pub fn kill_group_best_effort(pgid: u32) {
    if pgid > 1 && pgid != std::process::id() {
        // SAFETY: ver arriba. ESRCH si el grupo ya no existe.
        unsafe {
            libc::killpg(pgid as i32, libc::SIGTERM);
        }
    }
}

#[cfg(not(unix))]
pub fn kill_group_best_effort(_pgid: u32) {}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};

    fn spawn(script: &str) -> (Box<dyn portable_pty::Child + Send + Sync>, Box<dyn portable_pty::MasterPty + Send>) {
        let pair = native_pty_system()
            .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
            .expect("openpty");
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", script]);
        let child = pair.slave.spawn_command(cmd).expect("spawn");
        drop(pair.slave);
        (child, pair.master)
    }

    fn wait_for(mut f: impl FnMut() -> bool, ms: u64) -> bool {
        let end = std::time::Instant::now() + Duration::from_millis(ms);
        while std::time::Instant::now() < end {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        f()
    }

    #[test]
    fn kill_tree_leaves_no_orphans() {
        // Árbol: sh -> (sh -> sleep) & sleep, con nietos.
        let (mut child, _master) = spawn("sh -c 'sleep 301; true' & sleep 302 & sleep 303 & wait");
        let root = child.process_id().expect("pid");
        let mut tree: Vec<u32> = Vec::new();
        assert!(
            wait_for(
                || {
                    tree = tree_pids(&snapshot_system(), root);
                    tree.len() >= 5 // sh + sh + sleep301 + sleep302 + sleep303
                },
                3000
            ),
            "árbol incompleto: {tree:?}"
        );
        kill_tree(root, Duration::from_secs(3), true);
        let _ = child.try_wait();
        for p in &tree {
            assert!(!is_alive(*p), "pid {p} sigue vivo tras kill_tree");
        }
        let left: Vec<u32> = descendants(&snapshot_system(), root);
        assert!(left.is_empty(), "huérfanos: {left:?}");
    }

    #[test]
    fn kill_tree_escalates_to_sigkill_when_sigterm_ignored() {
        let (mut child, _master) = spawn("trap '' TERM; sleep 304 & wait; sleep 304");
        let root = child.process_id().expect("pid");
        std::thread::sleep(Duration::from_millis(400));
        let tree = tree_pids(&snapshot_system(), root);
        kill_tree(root, Duration::from_millis(600), true);
        let _ = child.try_wait();
        for p in &tree {
            assert!(!is_alive(*p), "pid {p} sigue vivo");
        }
    }
}

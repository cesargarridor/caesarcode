use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use super::process_tree::kill_tree;
use crate::util::proc::no_window;

/// ¿Alguien escucha en `port`? Prueba IPv4 e IPv6 loopback.
pub fn is_port_open(port: u16) -> bool {
    let t = Duration::from_millis(150);
    let v4: SocketAddr = ([127, 0, 0, 1], port).into();
    if TcpStream::connect_timeout(&v4, t).is_ok() {
        return true;
    }
    let v6: SocketAddr = (std::net::Ipv6Addr::LOCALHOST, port).into();
    TcpStream::connect_timeout(&v6, t).is_ok()
}

#[cfg(unix)]
pub fn pids_on_port(port: u16) -> Vec<u32> {
    let arg = format!("-iTCP:{port}");
    for bin in ["lsof", "/usr/sbin/lsof", "/usr/bin/lsof"] {
        let mut cmd = std::process::Command::new(bin);
        cmd.args(["-nP", &arg, "-sTCP:LISTEN", "-t"]);
        if let Ok(out) = no_window(&mut cmd).output() {
            let mut v: Vec<u32> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|l| l.trim().parse().ok())
                .collect();
            v.sort_unstable();
            v.dedup();
            return v;
        }
    }
    Vec::new()
}

#[cfg(windows)]
pub fn pids_on_port(port: u16) -> Vec<u32> {
    let mut cmd = std::process::Command::new("netstat");
    cmd.args(["-ano", "-p", "tcp"]);
    let Ok(out) = no_window(&mut cmd).output() else {
        return Vec::new();
    };
    let suffix = format!(":{port}");
    let mut v: Vec<u32> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.contains("LISTENING"))
        .filter_map(|l| {
            let cols: Vec<&str> = l.split_whitespace().collect();
            // Proto, Local, Foreign, State, PID
            if cols.len() >= 5 && cols[1].ends_with(&suffix) {
                cols[4].parse().ok()
            } else {
                None
            }
        })
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Mata el árbol de cada proceso que escucha en `port`. Devuelve los pids afectados.
pub fn port_kill(port: u16) -> Vec<u32> {
    let me = std::process::id();
    let pids: Vec<u32> = pids_on_port(port).into_iter().filter(|p| *p != me).collect();
    for p in &pids {
        kill_tree(*p, Duration::from_secs(2), false);
    }
    pids
}

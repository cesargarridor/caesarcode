use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use regex::Regex;

use super::{now_ms, LogLevel, LogLine};

/// Máximo de líneas guardadas por servicio.
pub const CAP: usize = 20_000;
/// Una línea sin `\n` se emite igualmente si pasa este tiempo sin datos nuevos.
const STALE: Duration = Duration::from_millis(200);

fn re_error() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)\b(error|err|fatal|panic|exception|traceback)\b").expect("regex"))
}

fn re_warn() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)\b(warn|warning)\b").expect("regex"))
}

pub fn detect_level(text: &str) -> LogLevel {
    if re_error().is_match(text) {
        LogLevel::Error
    } else if re_warn().is_match(text) {
        LogLevel::Warn
    } else {
        LogLevel::Info
    }
}

/// Quita ANSI y decodifica UTF-8 (lossy).
fn clean(raw: &[u8]) -> String {
    let stripped = strip_ansi_escapes::strip(raw);
    String::from_utf8_lossy(&stripped).into_owned()
}

/// Convierte una línea cruda (sin `\n`) a texto: quita `\r` final y se queda con
/// el último segmento tras un `\r` intermedio (barras de progreso que se sobrescriben).
fn finish_line(mut raw: &[u8]) -> String {
    while raw.last() == Some(&b'\r') {
        raw = &raw[..raw.len() - 1];
    }
    if let Some(i) = raw.iter().rposition(|b| *b == b'\r') {
        raw = &raw[i + 1..];
    }
    clean(raw)
}

/// Acumula bytes y separa por `\n`.
#[derive(Default)]
pub struct LineSplitter {
    buf: Vec<u8>,
    last_data: Option<Instant>,
}

impl LineSplitter {
    pub fn push(&mut self, data: &[u8], now: Instant) -> Vec<String> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        let mut start = 0;
        while let Some(rel) = self.buf[start..].iter().position(|b| *b == b'\n') {
            let end = start + rel;
            out.push(finish_line(&self.buf[start..end]));
            start = end + 1;
        }
        if start > 0 {
            self.buf.drain(..start);
        }
        self.last_data = if self.buf.is_empty() { None } else { Some(now) };
        out
    }

    /// Emite la parte pendiente si lleva más de 200 ms sin datos.
    pub fn flush_stale(&mut self, now: Instant) -> Option<String> {
        match self.last_data {
            Some(t) if !self.buf.is_empty() && now.duration_since(t) >= STALE => self.finish(),
            _ => None,
        }
    }

    /// Emite lo que quede (EOF).
    pub fn finish(&mut self) -> Option<String> {
        self.last_data = None;
        if self.buf.is_empty() {
            return None;
        }
        let line = finish_line(&self.buf);
        self.buf.clear();
        Some(line)
    }
}

#[derive(Default)]
pub struct LogStore {
    per_service: HashMap<String, VecDeque<LogLine>>,
    splitters: HashMap<String, LineSplitter>,
    pending: Vec<LogLine>,
    seq: u64,
}

impl LogStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_line(&mut self, service_id: &str, text: String) {
        self.seq += 1;
        let line = LogLine {
            seq: self.seq,
            service_id: service_id.to_string(),
            ts: now_ms(),
            level: detect_level(&text),
            text,
        };
        let q = self.per_service.entry(service_id.to_string()).or_default();
        q.push_back(line.clone());
        while q.len() > CAP {
            q.pop_front();
        }
        self.pending.push(line);
    }

    /// Bytes crudos del PTY de un servicio.
    pub fn feed(&mut self, service_id: &str, data: &[u8]) {
        let lines = self
            .splitters
            .entry(service_id.to_string())
            .or_default()
            .push(data, Instant::now());
        for l in lines {
            self.push_line(service_id, l);
        }
    }

    /// Emite las líneas parciales que llevan >200 ms paradas.
    pub fn flush_stale(&mut self) {
        let now = Instant::now();
        let mut out: Vec<(String, String)> = Vec::new();
        for (id, sp) in self.splitters.iter_mut() {
            if let Some(l) = sp.flush_stale(now) {
                out.push((id.clone(), l));
            }
        }
        for (id, l) in out {
            self.push_line(&id, l);
        }
    }

    /// EOF del PTY de un servicio: emite el resto.
    pub fn finish(&mut self, service_id: &str) {
        let rest = self.splitters.get_mut(service_id).and_then(|s| s.finish());
        if let Some(l) = rest {
            self.push_line(service_id, l);
        }
    }

    /// Vacía el lote pendiente (máx `max` líneas).
    pub fn take_pending(&mut self, max: usize) -> Vec<LogLine> {
        if self.pending.len() <= max {
            std::mem::take(&mut self.pending)
        } else {
            self.pending.drain(..max).collect()
        }
    }

    pub fn drop_pending(&mut self) {
        self.pending.clear();
    }

    /// Últimas `limit` líneas de un servicio, o de todos ordenadas por `seq`.
    pub fn backlog(&self, service_id: Option<&str>, limit: usize) -> Vec<LogLine> {
        match service_id {
            Some(id) => self
                .per_service
                .get(id)
                .map(|q| {
                    let skip = q.len().saturating_sub(limit);
                    q.iter().skip(skip).cloned().collect()
                })
                .unwrap_or_default(),
            None => {
                let mut all: Vec<LogLine> = self.per_service.values().flat_map(|q| q.iter().cloned()).collect();
                all.sort_by_key(|l| l.seq);
                let skip = all.len().saturating_sub(limit);
                all.drain(..skip);
                all
            }
        }
    }

    pub fn clear(&mut self, service_id: Option<&str>) {
        match service_id {
            Some(id) => {
                if let Some(q) = self.per_service.get_mut(id) {
                    q.clear();
                }
                self.pending.retain(|l| l.service_id != id);
            }
            None => {
                self.per_service.clear();
                self.pending.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        assert_eq!(detect_level("Traceback (most recent call last)"), LogLevel::Error);
        assert_eq!(detect_level("[ERROR] boom"), LogLevel::Error);
        assert_eq!(detect_level("Warning: deprecated"), LogLevel::Warn);
        assert_eq!(detect_level("terrorist alert"), LogLevel::Info);
        assert_eq!(detect_level("listening on 3000"), LogLevel::Info);
    }

    #[test]
    fn splits_strips_ansi_and_carriage_returns() {
        let mut sp = LineSplitter::default();
        let now = Instant::now();
        let out = sp.push(b"\x1b[31mhello\x1b[0m\r\nwor", now);
        assert_eq!(out, vec!["hello".to_string()]);
        let out = sp.push(b"ld\r\n10%\r50%\r100%\r\n", now);
        assert_eq!(out, vec!["world".to_string(), "100%".to_string()]);
    }

    #[test]
    fn utf8_split_across_reads_is_kept_whole() {
        let mut sp = LineSplitter::default();
        let now = Instant::now();
        let bytes = "ñandú\n".as_bytes();
        assert!(sp.push(&bytes[..2], now).is_empty());
        assert_eq!(sp.push(&bytes[2..], now), vec!["ñandú".to_string()]);
    }

    #[test]
    fn stale_partial_line_is_flushed() {
        let mut sp = LineSplitter::default();
        let t0 = Instant::now();
        assert!(sp.push(b"prompt> ", t0).is_empty());
        assert!(sp.flush_stale(t0 + Duration::from_millis(50)).is_none());
        assert_eq!(sp.flush_stale(t0 + Duration::from_millis(250)), Some("prompt> ".into()));
        assert!(sp.flush_stale(t0 + Duration::from_millis(500)).is_none());
    }

    #[test]
    fn store_backlog_order_cap_and_clear() {
        let mut st = LogStore::new();
        st.feed("a", b"one\n");
        st.feed("b", b"two\n");
        st.feed("a", b"three\n");
        let all = st.backlog(None, 10);
        assert_eq!(all.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["one", "two", "three"]);
        assert_eq!(st.backlog(Some("a"), 1)[0].text, "three");
        assert_eq!(st.take_pending(2).len(), 2);
        assert_eq!(st.take_pending(10).len(), 1);
        st.clear(Some("a"));
        assert!(st.backlog(Some("a"), 10).is_empty());
        assert_eq!(st.backlog(None, 10).len(), 1);
        for i in 0..(CAP + 50) {
            st.push_line("c", format!("l{i}"));
        }
        assert_eq!(st.backlog(Some("c"), CAP + 100).len(), CAP);
    }
}

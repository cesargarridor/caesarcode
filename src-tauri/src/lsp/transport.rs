//! Framing LSP: `Content-Length: <bytes>\r\n\r\n<cuerpo JSON>`.

use std::io;

use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Límite de cordura para un mensaje (256 MiB).
const MAX_BODY: usize = 256 * 1024 * 1024;

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// Escribe un mensaje con cabecera; la longitud es en BYTES del cuerpo UTF-8.
pub async fn write_message<W: AsyncWrite + Unpin>(w: &mut W, msg: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(msg).map_err(|e| invalid(e.to_string()))?;
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend_from_slice(&body);
    w.write_all(&frame).await?;
    w.flush().await
}

/// Lee un mensaje. `Ok(None)` = EOF limpio antes de empezar una cabecera.
/// Acepta cabeceras en cualquier capitalización e ignora `Content-Type`.
pub async fn read_message<R: AsyncBufRead + AsyncRead + Unpin>(r: &mut R) -> io::Result<Option<Value>> {
    let mut content_length: Option<usize> = None;
    let mut first = true;
    loop {
        let mut line = String::new();
        let n = r.read_line(&mut line).await?;
        if n == 0 {
            return if first {
                Ok(None)
            } else {
                Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF dentro de la cabecera"))
            };
        }
        first = false;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break; // fin de cabeceras
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| invalid(format!("Content-Length inválido: {value}")))?,
                );
            }
        }
    }
    let len = content_length.ok_or_else(|| invalid("falta Content-Length"))?;
    if len > MAX_BODY {
        return Err(invalid(format!("mensaje demasiado grande: {len} bytes")));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await?;
    serde_json::from_slice::<Value>(&body).map(Some).map_err(|e| invalid(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn roundtrip_uses_byte_length_not_chars() {
        let msg = json!({"jsonrpc": "2.0", "method": "x", "params": {"t": "ñandú → 日本語 🦀"}});
        let mut buf: Vec<u8> = Vec::new();
        write_message(&mut buf, &msg).await.unwrap();
        let text = String::from_utf8(buf.clone()).unwrap();
        let body_len = serde_json::to_vec(&msg).unwrap().len();
        assert!(text.starts_with(&format!("Content-Length: {body_len}\r\n\r\n")));
        assert!(body_len > serde_json::to_string(&msg).unwrap().chars().count());
        let mut r = BufReader::new(&buf[..]);
        assert_eq!(read_message(&mut r).await.unwrap(), Some(msg));
        assert_eq!(read_message(&mut r).await.unwrap(), None);
    }

    #[tokio::test]
    async fn reads_extra_headers_any_case_and_back_to_back_messages() {
        let a = r#"{"id":1}"#;
        let b = r#"{"id":2}"#;
        let raw = format!(
            "content-type: application/vscode-jsonrpc; charset=utf-8\r\nCONTENT-LENGTH: {}\r\n\r\n{}Content-Length: {}\r\n\r\n{}",
            a.len(),
            a,
            b.len(),
            b
        );
        let mut r = BufReader::new(raw.as_bytes());
        assert_eq!(read_message(&mut r).await.unwrap(), Some(json!({"id": 1})));
        assert_eq!(read_message(&mut r).await.unwrap(), Some(json!({"id": 2})));
        assert_eq!(read_message(&mut r).await.unwrap(), None);
    }

    #[tokio::test]
    async fn rejects_missing_length_and_truncated_body() {
        let mut r = BufReader::new(&b"Content-Type: x\r\n\r\n{}"[..]);
        assert!(read_message(&mut r).await.is_err());
        let mut r = BufReader::new(&b"Content-Length: 50\r\n\r\n{}"[..]);
        assert!(read_message(&mut r).await.is_err());
    }
}

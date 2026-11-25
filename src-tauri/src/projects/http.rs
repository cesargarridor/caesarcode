//! Descargas a hosts fijos (HTTPS, timeout, tope 50 MB) y extracción segura de zips (zip-slip).

use std::io::{Cursor, Read};
use std::path::Path;
use std::time::Duration;

use reqwest::Url;
use tokio::sync::watch;

use crate::error::{AppError, AppResult};

pub const SPRING_HOST: &str = "start.spring.io";
pub const QUARKUS_HOST: &str = "code.quarkus.io";
pub const MICRONAUT_HOST: &str = "launch.micronaut.io";
const ALLOWED_HOSTS: &[&str] = &[SPRING_HOST, QUARKUS_HOST, MICRONAUT_HOST];

pub const MAX_DOWNLOAD: usize = 50 * 1024 * 1024;
pub const MAX_UNZIPPED: u64 = 200 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 20_000;

fn net(msg: impl Into<String>) -> AppError {
    AppError::Other(msg.into())
}

/// `https://<host><path>?<query>` construida con `query_pairs_mut` (jamás concatenando) y solo para hosts fijos.
pub fn build_url(host: &str, path: &str, query: &[(&str, &str)]) -> AppResult<Url> {
    if !ALLOWED_HOSTS.contains(&host) {
        return Err(net(format!("host no permitido: {host}")));
    }
    let mut url = Url::parse(&format!("https://{host}/"))
        .map_err(|e| net(format!("URL inválida: {e}")))?;
    url.path_segments_mut()
        .map_err(|_| net("URL inválida"))?
        .clear()
        .extend(path.split('/').filter(|s| !s.is_empty()));
    if !query.is_empty() {
        let mut q = url.query_pairs_mut();
        for (k, v) in query {
            q.append_pair(k, v);
        }
    }
    Ok(url)
}

fn is_allowed(url: &Url) -> bool {
    url.scheme() == "https" && url.host_str().is_some_and(|h| ALLOWED_HOSTS.contains(&h))
}

/// Cliente con timeouts y redirecciones solo a hosts permitidos.
pub fn client(total_timeout: Duration) -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("CaesarCode")
        .connect_timeout(Duration::from_secs(6))
        .timeout(total_timeout)
        .redirect(reqwest::redirect::Policy::custom(|a| {
            if a.previous().len() < 5 && is_allowed(a.url()) {
                a.follow()
            } else {
                a.stop()
            }
        }))
        .build()
        .map_err(|e| net(format!("cliente HTTP: {e}")))
}

/// GET con tope de tamaño y cancelación. `accept` opcional.
pub async fn fetch(
    client: &reqwest::Client,
    url: Url,
    accept: Option<&str>,
    mut cancel: Option<watch::Receiver<bool>>,
) -> AppResult<Vec<u8>> {
    if !is_allowed(&url) {
        return Err(net("URL fuera de los hosts permitidos"));
    }
    let mut req = client.get(url.clone());
    if let Some(a) = accept {
        req = req.header(reqwest::header::ACCEPT, a);
    }
    let work = async {
        let mut resp = req.send().await.map_err(|e| net(format!("{}: {e}", url.host_str().unwrap_or("red"))))?;
        if !resp.status().is_success() {
            return Err(net(format!("{} respondió {}", url.host_str().unwrap_or("servidor"), resp.status())));
        }
        if resp.content_length().is_some_and(|l| l as usize > MAX_DOWNLOAD) {
            return Err(net("la descarga supera el tamaño máximo (50 MB)"));
        }
        let mut buf = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(|e| net(format!("descarga interrumpida: {e}")))? {
            buf.extend_from_slice(&chunk);
            if buf.len() > MAX_DOWNLOAD {
                return Err(net("la descarga supera el tamaño máximo (50 MB)"));
            }
        }
        Ok(buf)
    };
    match cancel.as_mut() {
        Some(rx) => tokio::select! {
            r = work => r,
            _ = super::wait_cancel(rx) => Err(net("Cancelado")),
        },
        None => work.await,
    }
}

/// Extrae `bytes` (zip) en `dest` con guardas: sin rutas absolutas/`..`/symlinks, ≤ 20 000 entradas, ≤ 200 MB.
/// Si todas las entradas cuelgan de una única carpeta raíz, se descarta ese nivel (`strip_root`).
/// Devuelve el número de archivos escritos.
pub fn extract_zip(bytes: &[u8], dest: &Path, strip_root: bool) -> AppResult<usize> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| net(format!("zip inválido: {e}")))?;
    if zip.len() > MAX_ENTRIES {
        return Err(net("el zip tiene demasiadas entradas"));
    }
    // Primera pasada: validar todo y decidir el prefijo común antes de tocar el disco.
    let mut names: Vec<(usize, std::path::PathBuf, bool)> = Vec::with_capacity(zip.len());
    let mut total: u64 = 0;
    for i in 0..zip.len() {
        let f = zip.by_index(i).map_err(|e| net(format!("zip corrupto: {e}")))?;
        let rel = f
            .enclosed_name()
            .ok_or_else(|| net(format!("entrada de zip insegura rechazada: {}", f.name())))?;
        if let Some(mode) = f.unix_mode() {
            if mode & 0o170000 == 0o120000 {
                return Err(net(format!("el zip contiene un enlace simbólico: {}", f.name())));
            }
        }
        total = total.saturating_add(f.size());
        if total > MAX_UNZIPPED {
            return Err(net("el zip descomprimido supera 200 MB"));
        }
        names.push((i, rel, f.is_dir()));
    }
    let root = if strip_root {
        let first = names.iter().find_map(|(_, p, _)| p.components().next().map(|c| c.as_os_str().to_owned()));
        match first {
            Some(f) if names.iter().all(|(_, p, is_dir)| {
                p.components().next().map(|c| c.as_os_str() == f).unwrap_or(false)
                    && (p.components().count() > 1 || *is_dir)
            }) =>
            {
                Some(f)
            }
            _ => None,
        }
    } else {
        None
    };
    std::fs::create_dir_all(dest)?;
    let mut written = 0;
    for (i, rel, is_dir) in names {
        let rel = match &root {
            Some(r) => rel.strip_prefix(r).unwrap_or(&rel).to_path_buf(),
            None => rel,
        };
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = dest.join(&rel);
        if !out.starts_with(dest) {
            return Err(net("entrada de zip fuera del destino"));
        }
        if is_dir {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut f = zip.by_index(i).map_err(|e| net(format!("zip corrupto: {e}")))?;
        let mut data = Vec::with_capacity(f.size() as usize);
        (&mut f).take(MAX_UNZIPPED + 1).read_to_end(&mut data)?;
        std::fs::write(&out, &data)?;
        #[cfg(unix)]
        if let Some(mode) = f.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode & 0o777));
        }
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_of(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        for (name, body) in entries {
            if name.ends_with('/') {
                w.add_directory(*name, opts).unwrap();
            } else {
                w.start_file(*name, opts).unwrap();
                w.write_all(body.as_bytes()).unwrap();
            }
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn rejects_traversal_and_absolute() {
        let d = tempfile::tempdir().unwrap();
        let dest = d.path().join("out");
        assert!(extract_zip(&zip_of(&[("../evil.txt", "x")]), &dest, false).is_err());
        assert!(extract_zip(&zip_of(&[("/abs/evil.txt", "x")]), &dest, false).is_err());
        assert!(extract_zip(&zip_of(&[("ok/a.txt", "1"), ("ok/../../evil", "x")]), &dest, false).is_err());
        assert!(!d.path().join("evil.txt").exists() && !d.path().join("evil").exists());
        assert!(!dest.exists(), "no se toca el disco si hay entradas inseguras");
    }

    #[test]
    fn extracts_valid_zip_and_strips_root() {
        let d = tempfile::tempdir().unwrap();
        let z = zip_of(&[("proj/", ""), ("proj/pom.xml", "<p/>"), ("proj/src/Main.java", "class M{}")]);
        let n = extract_zip(&z, &d.path().join("a"), true).unwrap();
        assert_eq!(n, 2);
        assert_eq!(std::fs::read_to_string(d.path().join("a/pom.xml")).unwrap(), "<p/>");
        assert!(d.path().join("a/src/Main.java").is_file());
        // sin strip se conserva la raíz
        extract_zip(&z, &d.path().join("b"), false).unwrap();
        assert!(d.path().join("b/proj/pom.xml").is_file());
        // sin raíz común no se recorta
        let z2 = zip_of(&[("pom.xml", "1"), ("src/A.java", "2")]);
        extract_zip(&z2, &d.path().join("c"), true).unwrap();
        assert!(d.path().join("c/pom.xml").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn keeps_exec_bit() {
        use std::os::unix::fs::PermissionsExt;
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        w.start_file("mvnw", zip::write::SimpleFileOptions::default().unix_permissions(0o755)).unwrap();
        w.write_all(b"#!/bin/sh").unwrap();
        let bytes = w.finish().unwrap().into_inner();
        let d = tempfile::tempdir().unwrap();
        extract_zip(&bytes, d.path(), false).unwrap();
        assert_eq!(std::fs::metadata(d.path().join("mvnw")).unwrap().permissions().mode() & 0o111, 0o111);
    }

    #[test]
    fn urls_only_for_fixed_hosts() {
        let u = build_url(SPRING_HOST, "starter.zip", &[("name", "a b&c=d"), ("type", "maven-project")]).unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host_str(), Some(SPRING_HOST));
        assert!(u.query().unwrap().contains("name=a+b%26c%3Dd"));
        assert!(build_url("evil.example.com", "x", &[]).is_err());
        assert!(!is_allowed(&Url::parse("http://start.spring.io/").unwrap()));
    }

    #[tokio::test]
    #[ignore = "red real"]
    async fn spring_metadata_online() {
        let c = client(Duration::from_secs(6)).unwrap();
        let url = build_url(SPRING_HOST, "metadata/client", &[]).unwrap();
        let b = fetch(&c, url, Some("application/vnd.initializr.v2.2+json"), None).await.unwrap();
        assert!(String::from_utf8_lossy(&b).contains("bootVersion"));
    }
}

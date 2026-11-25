use std::io::Write;
use std::path::{Path, PathBuf};

use super::FileContent;
use crate::error::{AppError, AppResult};

const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// Por encima de esto solo se lee el principio (`truncated`).
const TRUNCATE_BYTES: u64 = 5 * 1024 * 1024;

/// Valida un nombre de archivo/carpeta (sin separadores, sin `..`, no vacío).
pub fn validate_name(name: &str) -> AppResult<()> {
    let t = name.trim();
    if t.is_empty()
        || t != name
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || name == "."
        || name == ".."
    {
        return Err(AppError::InvalidArgument(format!("nombre no válido: {name:?}")));
    }
    Ok(())
}

/// Decodifica `bytes`; si `cut` y el final parte un carácter UTF-8, descarta la cola incompleta.
fn decode(bytes: Vec<u8>, cut: bool) -> String {
    if cut {
        if let Err(e) = std::str::from_utf8(&bytes) {
            if e.error_len().is_none() {
                let ok = e.valid_up_to();
                return String::from_utf8_lossy(&bytes[..ok]).into_owned();
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn read_file(path: &Path) -> AppResult<FileContent> {
    use std::io::Read;
    let p = path.to_string_lossy().into_owned();
    let meta = std::fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AppError::NotFound(p.clone()),
        _ => AppError::Io(e),
    })?;
    if meta.is_dir() {
        return Err(AppError::InvalidArgument(format!("es una carpeta: {p}")));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Ok(FileContent { path: p, text: String::new(), is_binary: false, too_large: true, truncated: false });
    }
    let truncated = meta.len() > TRUNCATE_BYTES;
    let mut bytes = Vec::new();
    if truncated {
        std::fs::File::open(path)?.take(TRUNCATE_BYTES).read_to_end(&mut bytes)?;
    } else {
        bytes = std::fs::read(path)?;
    }
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return Ok(FileContent { path: p, text: String::new(), is_binary: true, too_large: false, truncated: false });
    }
    Ok(FileContent { path: p, text: decode(bytes, truncated), is_binary: false, too_large: false, truncated })
}

/// Escritura atómica: temporal en el mismo directorio + rename.
pub fn write_file(path: &Path, text: &str) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::InvalidArgument(path.display().to_string()))?;
    let name = path
        .file_name()
        .ok_or_else(|| AppError::InvalidArgument(path.display().to_string()))?
        .to_string_lossy();
    let tmp = parent.join(format!("{name}.cc-tmp-{}", uuid::Uuid::new_v4().simple()));
    let result = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        if let Ok(meta) = std::fs::metadata(path) {
            let _ = std::fs::set_permissions(&tmp, meta.permissions());
        }
        std::fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::Io(e));
    }
    Ok(())
}

fn new_child(parent: &Path, name: &str) -> AppResult<PathBuf> {
    validate_name(name)?;
    if !parent.is_dir() {
        return Err(AppError::NotFound(parent.display().to_string()));
    }
    let target = parent.join(name);
    if target.symlink_metadata().is_ok() {
        return Err(AppError::AlreadyExists(target.display().to_string()));
    }
    Ok(target)
}

pub fn create_file(parent: &Path, name: &str) -> AppResult<PathBuf> {
    let target = new_child(parent, name)?;
    std::fs::OpenOptions::new().write(true).create_new(true).open(&target)?;
    Ok(target)
}

pub fn create_dir(parent: &Path, name: &str) -> AppResult<PathBuf> {
    let target = new_child(parent, name)?;
    std::fs::create_dir(&target)?;
    Ok(target)
}

pub fn rename(path: &Path, new_name: &str) -> AppResult<PathBuf> {
    validate_name(new_name)?;
    let parent = path
        .parent()
        .ok_or_else(|| AppError::InvalidArgument(path.display().to_string()))?;
    let target = parent.join(new_name);
    if target == path {
        return Ok(target);
    }
    // Cambio solo de mayúsculas en FS insensibles: el destino "existe" pero es el mismo archivo.
    let case_only = path.to_string_lossy().to_lowercase() == target.to_string_lossy().to_lowercase();
    if !case_only && target.symlink_metadata().is_ok() {
        return Err(AppError::AlreadyExists(target.display().to_string()));
    }
    std::fs::rename(path, &target)?;
    Ok(target)
}

pub fn delete(path: &Path) -> AppResult<()> {
    if path.symlink_metadata().is_err() {
        return Err(AppError::NotFound(path.display().to_string()));
    }
    trash::delete(path).map_err(|e| AppError::Other(format!("papelera: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_file_not_truncated_and_big_is_cut_on_utf8_boundary() {
        let d = tempfile::tempdir().unwrap();
        let small = d.path().join("s.txt");
        std::fs::write(&small, "hola").unwrap();
        let c = read_file(&small).unwrap();
        assert_eq!((c.text.as_str(), c.truncated, c.too_large), ("hola", false, false));

        // 6 MB de "€" (3 bytes): 5 MiB no es múltiplo de 3 -> parte un carácter.
        let big = d.path().join("b.txt");
        std::fs::write(&big, "€".repeat(2 * 1024 * 1024)).unwrap();
        let c = read_file(&big).unwrap();
        assert!(c.truncated && !c.too_large && !c.is_binary);
        assert!(!c.text.contains('\u{FFFD}'));
        assert!(c.text.len() as u64 <= TRUNCATE_BYTES && c.text.len() as u64 > TRUNCATE_BYTES - 3);

        let huge = d.path().join("h.txt");
        std::fs::write(&huge, vec![b'a'; 11 * 1024 * 1024]).unwrap();
        let c = read_file(&huge).unwrap();
        assert!(c.too_large && !c.truncated);
    }
}

//! Persistencia de las raíces adjuntas en `<app_data>/workspaces.json`.
//! Clave = ruta canónica de la raíz primaria; valor = raíces adjuntas (sin la primaria).
//! Escritura atómica (tmp + rename), LRU de 100 entradas, JSON corrupto → `.bak` y vacío.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub const MAX_ENTRIES: usize = 100;
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct Entry {
    #[serde(default)]
    roots: Vec<String>,
    #[serde(default)]
    updated_at: u64,
}

#[derive(Serialize, Deserialize)]
struct StoreFile {
    #[serde(default = "one")]
    version: u32,
    #[serde(default)]
    workspaces: HashMap<String, Entry>,
}

fn one() -> u32 {
    1
}

pub fn file(dir: &Path) -> PathBuf {
    dir.join("workspaces.json")
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn key(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn read(dir: &Path) -> StoreFile {
    let p = file(dir);
    let empty = StoreFile { version: 1, workspaces: HashMap::new() };
    let Ok(text) = std::fs::read_to_string(&p) else { return empty };
    match serde_json::from_str::<StoreFile>(&text) {
        Ok(f) => f,
        Err(_) => {
            let _ = std::fs::copy(&p, dir.join("workspaces.json.bak"));
            empty
        }
    }
}

fn write(dir: &Path, f: &mut StoreFile) -> AppResult<()> {
    while f.workspaces.len() > MAX_ENTRIES {
        let oldest = f.workspaces.iter().min_by_key(|(_, e)| e.updated_at).map(|(k, _)| k.clone());
        match oldest {
            Some(k) => {
                f.workspaces.remove(&k);
            }
            None => break,
        }
    }
    std::fs::create_dir_all(dir)?;
    let p = file(dir);
    let tmp = dir.join(format!("workspaces.json.cc-tmp-{}", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string_pretty(f)?)?;
    if let Err(e) = std::fs::rename(&tmp, &p) {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::Io(e));
    }
    Ok(())
}

/// Raíces adjuntas persistidas para `primary` (puede incluir carpetas que ya no existen).
pub fn load(dir: &Path, primary: &Path) -> Vec<PathBuf> {
    read(dir)
        .workspaces
        .get(&key(primary))
        .map(|e| e.roots.iter().map(PathBuf::from).filter(|p| p != primary).collect())
        .unwrap_or_default()
}

/// Guarda las raíces adjuntas de `primary` (la primaria, si viene, se descarta).
pub fn save(dir: &Path, primary: &Path, roots: &[PathBuf]) -> AppResult<()> {
    let _g = LOCK.lock().map_err(|_| AppError::Other("lock envenenado".into()))?;
    let mut f = read(dir);
    let e = f.workspaces.entry(key(primary)).or_default();
    e.roots = roots.iter().filter(|p| p.as_path() != primary).map(|p| key(p)).collect();
    e.updated_at = now_ms();
    write(dir, &mut f)
}

/// Nº de raíces adjuntas por clave primaria (para `RecentProject.extra_roots`).
pub fn extra_counts(dir: &Path) -> HashMap<String, u32> {
    read(dir).workspaces.into_iter().map(|(k, e)| (k, e.roots.len() as u32)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_no_tmp() {
        let d = tempfile::tempdir().unwrap();
        let p = PathBuf::from("/w/primary");
        assert!(load(d.path(), &p).is_empty());
        save(d.path(), &p, &[PathBuf::from("/w/a"), PathBuf::from("/w/b")]).unwrap();
        assert_eq!(load(d.path(), &p), vec![PathBuf::from("/w/a"), PathBuf::from("/w/b")]);
        save(d.path(), &p, &[PathBuf::from("/w/a")]).unwrap();
        assert_eq!(extra_counts(d.path())["/w/primary"], 1);
        assert!(std::fs::read_dir(d.path()).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().contains("tmp")));
    }

    #[test]
    fn corrupt_json_backup_and_recover() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(file(d.path()), "{ nope").unwrap();
        assert!(load(d.path(), Path::new("/x")).is_empty());
        assert!(d.path().join("workspaces.json.bak").is_file());
        save(d.path(), Path::new("/x"), &[PathBuf::from("/y")]).unwrap();
        assert_eq!(load(d.path(), Path::new("/x")), vec![PathBuf::from("/y")]);
    }

    #[test]
    fn lru_caps_entries() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..(MAX_ENTRIES + 5) {
            save(d.path(), Path::new(&format!("/p{i}")), &[PathBuf::from("/r")]).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let f = read(d.path());
        assert_eq!(f.workspaces.len(), MAX_ENTRIES);
        assert!(!f.workspaces.contains_key("/p0"));
        assert!(f.workspaces.contains_key(&format!("/p{}", MAX_ENTRIES + 4)));
    }
}

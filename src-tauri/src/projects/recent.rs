//! Proyectos recientes en `app_data_dir/recent-projects.json` (escritura atómica tmp+rename).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use super::RecentProject;
use crate::error::{AppError, AppResult};

pub const MAX_RECENT: usize = 50;
static LOCK: Mutex<()> = Mutex::new(());

pub fn file(dir: &Path) -> PathBuf {
    dir.join("recent-projects.json")
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Lee la lista cruda. JSON corrupto: copia a `.bak` y lista vacía.
pub fn load(dir: &Path) -> Vec<RecentProject> {
    let p = file(dir);
    let Ok(text) = std::fs::read_to_string(&p) else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<RecentProject>>(&text) {
        Ok(v) => v,
        Err(_) => {
            let _ = std::fs::copy(&p, dir.join("recent-projects.json.bak"));
            Vec::new()
        }
    }
}

pub fn save(dir: &Path, list: &[RecentProject]) -> AppResult<()> {
    std::fs::create_dir_all(dir)?;
    let p = file(dir);
    let tmp = dir.join(format!("recent-projects.json.cc-tmp-{}", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string_pretty(list)?)?;
    if let Err(e) = std::fs::rename(&tmp, &p) {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::Io(e));
    }
    Ok(())
}

fn sort(list: &mut [RecentProject]) {
    list.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.last_opened.cmp(&a.last_opened)));
}

fn key(p: &str) -> String {
    p.trim_end_matches(['/', '\\']).to_string()
}

/// Registra `root` como abierto ahora (conserva `pinned`). Tope de 50: se descartan los no fijados más antiguos.
pub fn touch(dir: &Path, root: &Path) -> AppResult<()> {
    let _g = LOCK.lock().map_err(|_| AppError::Other("lock envenenado".into()))?;
    let mut list = load(dir);
    let path = root.to_string_lossy().into_owned();
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
    let kind = crate::fs::tree::module_of(root);
    let now = now_ms();
    if let Some(e) = list.iter_mut().find(|e| key(&e.path) == key(&path)) {
        e.last_opened = now;
        e.name = name;
        e.kind = kind;
        e.path = path;
    } else {
        list.push(RecentProject { path, name, last_opened: now, kind, pinned: false, exists: true, extra_roots: 0 });
    }
    sort(&mut list);
    while list.len() > MAX_RECENT {
        match list.iter().rposition(|e| !e.pinned) {
            Some(i) => {
                list.remove(i);
            }
            None => {
                list.truncate(MAX_RECENT);
            }
        }
    }
    save(dir, &list)
}

/// Lista ordenada (fijados primero, luego recientes) con `exists` calculado.
pub fn list(dir: &Path) -> Vec<RecentProject> {
    let mut l = load(dir);
    let extras = crate::workspace::store::extra_counts(dir);
    for e in &mut l {
        e.exists = Path::new(&e.path).is_dir();
        e.extra_roots = extras.get(e.path.trim_end_matches(['/', '\\'])).copied().unwrap_or(0);
    }
    sort(&mut l);
    l
}

pub fn remove(dir: &Path, path: &str) -> AppResult<()> {
    let _g = LOCK.lock().map_err(|_| AppError::Other("lock envenenado".into()))?;
    let mut l = load(dir);
    l.retain(|e| key(&e.path) != key(path));
    save(dir, &l)
}

pub fn pin(dir: &Path, path: &str, pinned: bool) -> AppResult<()> {
    let _g = LOCK.lock().map_err(|_| AppError::Other("lock envenenado".into()))?;
    let mut l = load(dir);
    for e in l.iter_mut().filter(|e| key(&e.path) == key(path)) {
        e.pinned = pinned;
    }
    save(dir, &l)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proj(base: &Path, name: &str) -> PathBuf {
        let p = base.join(name);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn touch_list_remove_pin() {
        let d = tempfile::tempdir().unwrap();
        let data = d.path().join("data");
        let a = proj(d.path(), "a");
        let b = proj(d.path(), "b");
        std::fs::write(b.join("pom.xml"), "<project/>").unwrap();
        touch(&data, &a).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        touch(&data, &b).unwrap();
        let l = list(&data);
        assert_eq!(l.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        assert_eq!(l[0].kind.as_deref(), Some("maven"));
        assert!(l.iter().all(|e| e.exists));

        pin(&data, &a.to_string_lossy(), true).unwrap();
        assert_eq!(list(&data)[0].name, "a");
        // touch conserva pinned y no duplica
        touch(&data, &a).unwrap();
        let l = list(&data);
        assert_eq!(l.len(), 2);
        assert!(l[0].pinned);

        remove(&data, &a.to_string_lossy()).unwrap();
        assert_eq!(list(&data).len(), 1);
        assert!(a.is_dir(), "quitar de recientes nunca borra del disco");
    }

    #[test]
    fn missing_path_marked_not_existing() {
        let d = tempfile::tempdir().unwrap();
        let data = d.path().join("data");
        let a = proj(d.path(), "gone");
        touch(&data, &a).unwrap();
        std::fs::remove_dir_all(&a).unwrap();
        let l = list(&data);
        assert_eq!(l.len(), 1);
        assert!(!l[0].exists);
    }

    #[test]
    fn cap_is_50_keeping_pinned() {
        let d = tempfile::tempdir().unwrap();
        let data = d.path().join("data");
        let first = proj(d.path(), "p0");
        touch(&data, &first).unwrap();
        pin(&data, &first.to_string_lossy(), true).unwrap();
        for i in 1..=55 {
            touch(&data, &proj(d.path(), &format!("p{i}"))).unwrap();
        }
        let l = load(&data);
        assert_eq!(l.len(), MAX_RECENT);
        assert!(l.iter().any(|e| e.name == "p0" && e.pinned));
    }

    #[test]
    fn corrupt_json_gives_empty_and_backup() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(file(d.path()), "{ no es json").unwrap();
        assert!(load(d.path()).is_empty());
        assert!(d.path().join("recent-projects.json.bak").is_file());
        // se puede volver a escribir
        touch(d.path(), d.path()).unwrap();
        assert_eq!(load(d.path()).len(), 1);
    }

    #[test]
    fn atomic_write_leaves_no_tmp() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), d.path()).unwrap();
        assert!(std::fs::read_dir(d.path()).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().contains("tmp")));
    }
}

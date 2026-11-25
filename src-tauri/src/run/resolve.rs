//! Resolver de "Current File": archivo -> línea de comando + cwd, sin ejecutar nada.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::ResolvedRun;
use crate::error::{AppError, AppResult};
use crate::util::paths::ensure_in_workspace;

/// Comprobación de binarios disponibles en PATH (inyectable para tests).
pub trait Probe {
    fn which(&self, bin: &str) -> bool;
}

pub struct SystemProbe;

impl Probe for SystemProbe {
    fn which(&self, bin: &str) -> bool {
        which::which(bin).is_ok()
    }
}

pub fn shell_quote_for(s: &str, windows: bool) -> String {
    if windows {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Cita una ruta/argumento para `sh -c` (unix) o `cmd /C` (Windows).
pub fn shell_quote(s: &str) -> String {
    shell_quote_for(s, cfg!(windows))
}

fn q(p: &Path) -> String {
    shell_quote(&p.to_string_lossy())
}

fn rel_slash(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(Path::new(""))
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Id de runtime de Current File: `file:<ruta relativa con />`.
pub fn runtime_id(root: &Path, abs: &Path) -> String {
    format!("file:{}", rel_slash(root, abs))
}

/// Primer ancestro (desde `from` hasta `root` inclusive) con un archivo que cumpla `pick`.
fn ascend_find(root: &Path, from: &Path, pick: impl Fn(&Path) -> Option<PathBuf>) -> Option<PathBuf> {
    let mut cur = Some(from);
    while let Some(d) = cur {
        if let Some(found) = pick(d) {
            return Some(found);
        }
        if d == root {
            break;
        }
        cur = d.parent();
    }
    None
}

fn venv_python(root: &Path, from: &Path) -> Option<PathBuf> {
    let rels: &[&str] = if cfg!(windows) {
        &[".venv/Scripts/python.exe", "venv/Scripts/python.exe"]
    } else {
        &[".venv/bin/python", "venv/bin/python"]
    };
    ascend_find(root, from, |d| rels.iter().map(|r| d.join(r)).find(|p| p.is_file()))
}

fn local_tsx(root: &Path, from: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "node_modules/.bin/tsx.cmd" } else { "node_modules/.bin/tsx" };
    ascend_find(root, from, |d| Some(d.join(name)).filter(|p| p.is_file()))
}

fn tmp_exe(abs: &Path) -> AppResult<PathBuf> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    abs.hash(&mut h);
    let dir = std::env::temp_dir().join("caesarcode-run");
    std::fs::create_dir_all(&dir)?;
    let stem = abs.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into());
    let ext = if cfg!(windows) { ".exe" } else { "" };
    Ok(dir.join(format!("{stem}-{:08x}{ext}", h.finish() as u32)))
}

const KNOWN_EXT: &[&str] = &["py", "ts", "mts", "cts", "js", "mjs", "cjs", "go", "rs", "java", "sh", "zsh", "c", "cpp", "cc", "cxx"];

struct Out {
    command: String,
    cwd: PathBuf,
    runner: String,
}

fn simple(cmd: &str, file: &Path, cwd: &Path) -> Out {
    Out { command: format!("{cmd} {}", q(file)), cwd: cwd.to_path_buf(), runner: cmd.split_whitespace().next().unwrap_or(cmd).to_string() }
}

fn compiled(cc: &str, file: &Path, cwd: &Path) -> AppResult<Out> {
    let tmp = tmp_exe(file)?;
    Ok(Out {
        command: format!("{cc} {} -o {} && {}", q(file), q(&tmp), q(&tmp)),
        cwd: cwd.to_path_buf(),
        runner: cc.to_string(),
    })
}

fn shebang(file: &Path) -> Option<String> {
    use std::io::Read;
    let mut buf = Vec::with_capacity(512);
    std::fs::File::open(file).ok()?.take(512).read_to_end(&mut buf).ok()?;
    let first = buf.split(|b| *b == b'\n').next()?;
    let line = String::from_utf8(first.to_vec()).ok()?;
    let rest = line.trim().strip_prefix("#!")?.trim();
    let mut toks = rest.split_whitespace();
    let prog = toks.next()?;
    let mut parts: Vec<&str> = Vec::new();
    if prog.ends_with("/env") || prog == "env" {
        parts.extend(toks.filter(|t| *t != "-S"));
    } else {
        parts.push(prog);
        parts.extend(toks);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn apply_user_template(tpl: &str, root: &Path, file: &Path, dir: &Path) -> String {
    let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    tpl.replace("{file}", &q(file))
        .replace("{fileDir}", &q(dir))
        .replace("{fileStem}", &stem)
        .replace("{root}", &q(root))
}

fn rust_out(root: &Path, file: &Path, dir: &Path, probe: &dyn Probe) -> AppResult<Out> {
    let _ = probe;
    let crate_dir = ascend_find(root, dir, |d| {
        std::fs::read_to_string(d.join("Cargo.toml"))
            .ok()
            .filter(|t| t.contains("[package]"))
            .map(|_| d.to_path_buf())
    });
    let Some(cd) = crate_dir else { return compiled_rustc(file, dir) };
    let rel = rel_slash(&cd, file);
    let command = if let Some(rest) = rel.strip_prefix("src/bin/") {
        let bin = rest.split('/').next().unwrap_or(rest).trim_end_matches(".rs");
        format!("cargo run --bin {bin}")
    } else {
        "cargo run".to_string()
    };
    Ok(Out { command, cwd: cd, runner: "cargo run".into() })
}

fn compiled_rustc(file: &Path, dir: &Path) -> AppResult<Out> {
    compiled("rustc", file, dir)
}

fn by_extension(root: &Path, file: &Path, ext: &str, dir: &Path, probe: &dyn Probe) -> AppResult<Option<Out>> {
    let out = match ext {
        "py" => match venv_python(root, dir) {
            Some(py) => Out { command: format!("{} {}", q(&py), q(file)), cwd: dir.to_path_buf(), runner: "python (.venv)".into() },
            None => simple(if cfg!(windows) { "python" } else { "python3" }, file, dir),
        },
        "ts" | "mts" | "cts" => {
            if probe.which("bun") {
                simple("bun", file, dir)
            } else if let Some(tsx) = local_tsx(root, dir) {
                Out { command: format!("{} {}", q(&tsx), q(file)), cwd: dir.to_path_buf(), runner: "tsx (local)".into() }
            } else if probe.which("tsx") {
                simple("tsx", file, dir)
            } else {
                Out { command: format!("npx --yes tsx {}", q(file)), cwd: dir.to_path_buf(), runner: "npx tsx".into() }
            }
        }
        "js" | "mjs" | "cjs" => simple("node", file, dir),
        "go" => simple("go run", file, dir),
        "rs" => rust_out(root, file, dir, probe)?,
        "java" => simple("java", file, dir),
        "sh" => simple("bash", file, dir),
        "zsh" => simple("zsh", file, dir),
        "c" => {
            let cc = if probe.which("clang") {
                "clang"
            } else if probe.which("gcc") {
                "gcc"
            } else {
                return Err(AppError::NotFound("compilador C (clang o gcc)".into()));
            };
            compiled(cc, file, dir)?
        }
        "cpp" | "cc" | "cxx" => {
            let cc = if probe.which("clang++") {
                "clang++"
            } else if probe.which("g++") {
                "g++"
            } else {
                return Err(AppError::NotFound("compilador C++ (clang++ o g++)".into()));
            };
            compiled(cc, file, dir)?
        }
        _ => return Ok(None),
    };
    Ok(Some(out))
}

/// Resuelve cómo ejecutar `file` (ruta absoluta dentro de `root`, que debe estar canonicalizado).
pub fn resolve_file(
    root: &Path,
    file: &Path,
    runners: &HashMap<String, String>,
    probe: &dyn Probe,
) -> AppResult<ResolvedRun> {
    let abs = ensure_in_workspace(root, file)?;
    if !abs.is_file() {
        return Err(AppError::NotFound(abs.display().to_string()));
    }
    let dir = abs.parent().unwrap_or(root).to_path_buf();
    let filename = abs.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = abs.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();

    let out = if let Some(tpl) = runners.get(&ext).filter(|_| !ext.is_empty()) {
        Out {
            command: apply_user_template(tpl, root, &abs, &dir),
            cwd: dir.clone(),
            runner: format!("fileRunners.{ext}"),
        }
    } else {
        let known = KNOWN_EXT.contains(&ext.as_str());
        let from_shebang = if ext == "sh" || !known { shebang(&abs) } else { None };
        match from_shebang {
            Some(s) => {
                let runner = s.split_whitespace().next().unwrap_or("").rsplit('/').next().unwrap_or("").to_string();
                Out { command: format!("{s} {}", q(&abs)), cwd: dir.clone(), runner }
            }
            None => match by_extension(root, &abs, &ext, &dir, probe)? {
                Some(o) => o,
                None => {
                    return Err(AppError::InvalidArgument(format!(
                        "No hay runner para .{ext}; añade fileRunners en .caesar/run-configs.json"
                    )))
                }
            },
        }
    };
    Ok(ResolvedRun {
        command: out.command,
        cwd: rel_slash(root, &out.cwd),
        label: format!("Run: {filename}"),
        runner: out.runner,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::collections::HashSet;

    struct FakeProbe(HashSet<&'static str>);
    impl Probe for FakeProbe {
        fn which(&self, bin: &str) -> bool {
            self.0.contains(bin)
        }
    }
    fn probe(bins: &[&'static str]) -> FakeProbe {
        FakeProbe(bins.iter().copied().collect())
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-resolve-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::canonicalize(&d).unwrap()
    }

    fn put(d: &Path, rel: &str, content: &str) -> PathBuf {
        let p = d.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, content).unwrap();
        p
    }

    fn res(root: &Path, f: &Path, bins: &[&'static str]) -> ResolvedRun {
        resolve_file(root, f, &HashMap::new(), &probe(bins)).unwrap()
    }

    #[test]
    fn python_with_and_without_venv() {
        let d = tmp("py");
        let f = put(&d, "pkg/sub/main.py", "print(1)");
        let r = res(&d, &f, &[]);
        assert_eq!(r.command, format!("python3 '{}'", f.display()));
        assert_eq!((r.cwd.as_str(), r.label.as_str()), ("pkg/sub", "Run: main.py"));
        let py = put(&d, ".venv/bin/python", "");
        let r = res(&d, &f, &[]);
        assert_eq!(r.command, format!("'{}' '{}'", py.display(), f.display()));
        assert_eq!(r.runner, "python (.venv)");
    }

    #[test]
    fn typescript_runners() {
        let d = tmp("ts");
        let f = put(&d, "app/a.ts", "");
        assert_eq!(res(&d, &f, &["bun", "tsx"]).command, format!("bun '{}'", f.display()));
        let tsx = put(&d, "node_modules/.bin/tsx", "");
        assert_eq!(res(&d, &f, &[]).command, format!("'{}' '{}'", tsx.display(), f.display()));
        std::fs::remove_file(&tsx).unwrap();
        assert_eq!(res(&d, &f, &["tsx"]).command, format!("tsx '{}'", f.display()));
        assert_eq!(res(&d, &f, &[]).command, format!("npx --yes tsx '{}'", f.display()));
    }

    #[test]
    fn rust_in_crate_and_loose() {
        let d = tmp("rs");
        put(&d, "c/Cargo.toml", "[package]\nname='c'");
        let m = put(&d, "c/src/main.rs", "");
        let b = put(&d, "c/src/bin/tool.rs", "");
        let o = put(&d, "c/src/util.rs", "");
        let r = res(&d, &m, &[]);
        assert_eq!((r.command.as_str(), r.cwd.as_str()), ("cargo run", "c"));
        assert_eq!(res(&d, &b, &[]).command, "cargo run --bin tool");
        assert_eq!(res(&d, &o, &[]).command, "cargo run");
        let loose = put(&d, "loose/x.rs", "");
        let r = res(&d, &loose, &[]);
        assert!(r.command.starts_with("rustc ") && r.command.contains(" && "), "{}", r.command);
        assert_eq!(r.cwd, "loose");
    }

    #[test]
    fn c_cpp_compilers() {
        let d = tmp("c");
        let f = put(&d, "a.c", "");
        assert!(res(&d, &f, &["clang", "gcc"]).command.starts_with("clang "));
        assert!(res(&d, &f, &["gcc"]).command.starts_with("gcc "));
        assert!(resolve_file(&d, &f, &HashMap::new(), &probe(&[])).is_err());
        let g = put(&d, "b.cpp", "");
        let r = res(&d, &g, &["g++"]);
        assert!(r.command.starts_with("g++ ") && r.command.contains("-o ") && r.command.contains(" && "));
    }

    #[test]
    fn simple_extensions() {
        let d = tmp("simple");
        for (name, expect) in [("a.js", "node"), ("a.go", "go run"), ("a.java", "java"), ("a.sh", "bash"), ("a.zsh", "zsh")] {
            let f = put(&d, name, "");
            let r = res(&d, &f, &[]);
            assert_eq!(r.command, format!("{expect} '{}'", f.display()), "{name}");
        }
    }

    #[test]
    fn shebang_resolution() {
        let d = tmp("sb");
        let f = put(&d, "tool", "#!/usr/bin/env node --harmony\nconsole.log(1)");
        assert_eq!(res(&d, &f, &[]).command, format!("node --harmony '{}'", f.display()));
        let g = put(&d, "run", "#!/bin/bash -e\n");
        assert_eq!(res(&d, &g, &[]).command, format!("/bin/bash -e '{}'", g.display()));
        let s = put(&d, "x.sh", "#!/usr/bin/env zsh\n");
        assert_eq!(res(&d, &s, &[]).command, format!("zsh '{}'", s.display()));
    }

    #[test]
    fn user_runners_win_and_unknown_errors() {
        let d = tmp("user");
        let f = put(&d, "s.lua", "");
        let mut runners = HashMap::new();
        runners.insert("lua".to_string(), "lua {file} # {fileStem}".to_string());
        let r = resolve_file(&d, &f, &runners, &probe(&[])).unwrap();
        assert_eq!(r.command, format!("lua '{}' # s", f.display()));
        let e = resolve_file(&d, &f, &HashMap::new(), &probe(&[])).unwrap_err();
        assert!(e.to_string().contains("No hay runner para .lua"), "{e}");
        // fileRunners gana incluso a un tipo conocido
        let p = put(&d, "m.py", "");
        runners.insert("py".to_string(), "mypy-run {file}".to_string());
        assert!(resolve_file(&d, &p, &runners, &probe(&[])).unwrap().command.starts_with("mypy-run "));
    }

    #[test]
    fn quoting_and_outside_root() {
        assert_eq!(shell_quote_for("a b", false), "'a b'");
        assert_eq!(shell_quote_for("it's", false), "'it'\\''s'");
        assert_eq!(shell_quote_for("a b", true), "\"a b\"");
        let d = tmp("quote");
        let f = put(&d, "my dir/it's.py", "");
        let r = res(&d, &f, &[]);
        assert!(r.command.contains(&format!("'{}/my dir/it'\\''s.py'", d.display())), "{}", r.command);
        let other = tmp("quote-out");
        let out = put(&other, "x.py", "");
        assert!(resolve_file(&d, &out, &HashMap::new(), &probe(&[])).is_err());
    }
}

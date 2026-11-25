//! Generadores locales con plantillas embebidas (`src-tauri/templates/**`) y los pasos
//! posteriores opcionales que usan CLIs (gradle wrapper, cargo init, go mod init, venv, npm install).

use std::path::Path;

use super::template::{self, Vars};
use super::{BuildSystem, JvmOpts, ProjectRequest, Reporter};
use crate::error::{AppError, AppResult};

macro_rules! t {
    ($f:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/", $f))
    };
}

pub type Files = &'static [(&'static str, &'static str)];

/// Un conjunto de plantillas: `(ruta con {{vars}}, contenido con {{vars}})`.
pub struct TplSet {
    #[allow(dead_code)] // identifica el conjunto en los mensajes de los tests
    pub name: &'static str,
    pub files: Files,
}

const JAVA_MAIN: &str = "src/main/java/{{package_path}}/Main.java";

pub const GITIGNORE_JAVA: &str = t!("common/gitignore-java");
pub const GITIGNORE_NODE: &str = t!("common/gitignore-node");
pub const GITIGNORE_PYTHON: &str = t!("common/gitignore-python");
pub const GITIGNORE_GO: &str = t!("common/gitignore-go");
pub const GITIGNORE_RUST: &str = t!("common/gitignore-rust");
pub const GITIGNORE_GENERIC: &str = t!("common/gitignore-generic");
pub const README: &str = t!("common/README.md");
pub const DOCKERFILE_MAVEN: &str = t!("spring/Dockerfile-maven");
pub const DOCKERFILE_GRADLE: &str = t!("spring/Dockerfile-gradle");

#[cfg(test)]
pub static ALL_SETS: &[&TplSet] = &[
    &JAVA_PLAIN_SAMPLE, &JAVA_MAVEN, &JAVA_MAVEN_SAMPLE, &JAVA_GRADLE_KTS, &JAVA_GRADLE_GROOVY,
    &JAVA_GRADLE_SAMPLE, &KOTLIN_MAVEN, &KOTLIN_GRADLE_KTS, &KOTLIN_GRADLE_GROOVY, &KOTLIN_SAMPLE_MAVEN,
    &KOTLIN_SAMPLE_GRADLE, &GROOVY_MAVEN, &GROOVY_GRADLE_KTS, &GROOVY_GRADLE_GROOVY, &GROOVY_SAMPLE_MAVEN,
    &GROOVY_SAMPLE_GRADLE, &JAVAFX, &JAKARTA, &KTOR, &HTML, &EXPRESS_JS, &EXPRESS_TS, &NODE_JS, &NODE_TS,
    &PYTHON, &GO, &RUST_BIN, &RUST_LIB, &GO_NO_MOD,
];

static JAVA_PLAIN_SAMPLE: TplSet = TplSet { name: "java-plain-sample", files: &[("src/{{package_path}}/Main.java", t!("java/Main.java"))] };
static JAVA_MAVEN: TplSet = TplSet { name: "java-maven", files: &[("pom.xml", t!("java/pom.xml"))] };
static JAVA_MAVEN_SAMPLE: TplSet = TplSet { name: "java-maven-sample", files: &[(JAVA_MAIN, t!("java/Main.java"))] };
static JAVA_GRADLE_KTS: TplSet = TplSet {
    name: "java-gradle-kts",
    files: &[("build.gradle.kts", t!("java/build.gradle.kts")), ("settings.gradle.kts", t!("java/settings.gradle.kts"))],
};
static JAVA_GRADLE_GROOVY: TplSet = TplSet {
    name: "java-gradle-groovy",
    files: &[("build.gradle", t!("java/build.gradle")), ("settings.gradle", t!("java/settings.gradle"))],
};
static JAVA_GRADLE_SAMPLE: TplSet = TplSet { name: "java-gradle-sample", files: &[(JAVA_MAIN, t!("java/Main.java"))] };
static KOTLIN_MAVEN: TplSet = TplSet { name: "kotlin-maven", files: &[("pom.xml", t!("kotlin/pom.xml"))] };
static KOTLIN_GRADLE_KTS: TplSet = TplSet {
    name: "kotlin-gradle-kts",
    files: &[("build.gradle.kts", t!("kotlin/build.gradle.kts")), ("settings.gradle.kts", t!("java/settings.gradle.kts"))],
};
static KOTLIN_GRADLE_GROOVY: TplSet = TplSet {
    name: "kotlin-gradle-groovy",
    files: &[("build.gradle", t!("kotlin/build.gradle")), ("settings.gradle", t!("java/settings.gradle"))],
};
static KOTLIN_SAMPLE_MAVEN: TplSet = TplSet { name: "kotlin-sample-maven", files: &[("src/main/kotlin/{{package_path}}/Main.kt", t!("kotlin/Main.kt"))] };
static KOTLIN_SAMPLE_GRADLE: TplSet = TplSet { name: "kotlin-sample-gradle", files: &[("src/main/kotlin/{{package_path}}/Main.kt", t!("kotlin/Main.kt"))] };
static GROOVY_MAVEN: TplSet = TplSet { name: "groovy-maven", files: &[("pom.xml", t!("groovy/pom.xml"))] };
static GROOVY_GRADLE_KTS: TplSet = TplSet {
    name: "groovy-gradle-kts",
    files: &[("build.gradle.kts", t!("groovy/build.gradle.kts")), ("settings.gradle.kts", t!("java/settings.gradle.kts"))],
};
static GROOVY_GRADLE_GROOVY: TplSet = TplSet {
    name: "groovy-gradle-groovy",
    files: &[("build.gradle", t!("groovy/build.gradle")), ("settings.gradle", t!("java/settings.gradle"))],
};
static GROOVY_SAMPLE_MAVEN: TplSet = TplSet { name: "groovy-sample-maven", files: &[("src/main/groovy/{{package_path}}/Main.groovy", t!("groovy/Main.groovy"))] };
static GROOVY_SAMPLE_GRADLE: TplSet = TplSet { name: "groovy-sample-gradle", files: &[("src/main/groovy/{{package_path}}/Main.groovy", t!("groovy/Main.groovy"))] };
static JAVAFX: TplSet = TplSet {
    name: "javafx",
    files: &[("pom.xml", t!("javafx/pom.xml")), (JAVA_APP, t!("javafx/App.java"))],
};
const JAVA_APP: &str = "src/main/java/{{package_path}}/App.java";
static JAKARTA: TplSet = TplSet {
    name: "jakarta",
    files: &[
        ("pom.xml", t!("jakarta/pom.xml")),
        ("src/main/java/{{package_path}}/HelloApplication.java", t!("jakarta/HelloApplication.java")),
        ("src/main/java/{{package_path}}/HelloResource.java", t!("jakarta/HelloResource.java")),
        ("src/main/webapp/index.html", t!("jakarta/index.html")),
    ],
};
static KTOR: TplSet = TplSet {
    name: "ktor",
    files: &[
        ("build.gradle.kts", t!("ktor/build.gradle.kts")),
        ("settings.gradle.kts", t!("ktor/settings.gradle.kts")),
        ("src/main/kotlin/{{package_path}}/Application.kt", t!("ktor/Application.kt")),
        ("src/main/resources/logback.xml", t!("ktor/logback.xml")),
    ],
};
static HTML: TplSet = TplSet {
    name: "html",
    files: &[("index.html", t!("html/index.html")), ("style.css", t!("html/style.css")), ("main.js", t!("html/main.js"))],
};
static EXPRESS_JS: TplSet = TplSet { name: "express-js", files: &[("package.json", t!("express/package.json")), ("src/index.js", t!("express/index.js"))] };
static EXPRESS_TS: TplSet = TplSet {
    name: "express-ts",
    files: &[("package.json", t!("express/package-ts.json")), ("tsconfig.json", t!("express/tsconfig.json")), ("src/index.ts", t!("express/index.ts"))],
};
static NODE_JS: TplSet = TplSet { name: "node-js", files: &[("package.json", t!("node/package.json")), ("src/index.js", t!("node/index.js"))] };
static NODE_TS: TplSet = TplSet {
    name: "node-ts",
    files: &[("package.json", t!("node/package-ts.json")), ("tsconfig.json", t!("node/tsconfig.json")), ("src/index.ts", t!("node/index.ts"))],
};
static PYTHON: TplSet = TplSet {
    name: "python",
    files: &[
        ("pyproject.toml", t!("python/pyproject.toml")),
        ("src/{{py_pkg}}/__init__.py", t!("python/__init__.py")),
        ("src/{{py_pkg}}/main.py", t!("python/main.py")),
    ],
};
/// Go completo (con `go.mod`) / solo el código (cuando `go mod init` lo crea).
static GO: TplSet = TplSet { name: "go", files: &[("go.mod", t!("go/go.mod")), ("main.go", t!("go/main.go"))] };
static GO_NO_MOD: TplSet = TplSet { name: "go-no-mod", files: &[("main.go", t!("go/main.go"))] };
static RUST_BIN: TplSet = TplSet { name: "rust-bin", files: &[("Cargo.toml", t!("rust/Cargo.toml")), ("src/main.rs", t!("rust/main.rs"))] };
static RUST_LIB: TplSet = TplSet { name: "rust-lib", files: &[("Cargo.toml", t!("rust/Cargo.toml")), ("src/lib.rs", t!("rust/lib.rs"))] };

/// Variables de ejemplo para el test que recorre todas las plantillas.
#[cfg(test)]
pub fn sample_vars() -> Vars {
    let mut v = Vars::new();
    for k in ["name", "group_id", "artifact_id", "package", "package_path", "java_version", "javafx_version", "pkg_name", "py_pkg", "module"] {
        v.insert(k, format!("x-{k}"));
    }
    v
}

fn pkg_name(name: &str) -> String {
    name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect()
}

fn py_pkg(name: &str) -> String {
    let mut s: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        s.insert(0, '_');
    }
    s
}

pub fn jvm_vars(name: &str, j: &JvmOpts) -> Vars {
    let mut v = Vars::new();
    v.insert("name", name.to_string());
    v.insert("group_id", j.group_id.clone());
    v.insert("artifact_id", j.artifact_id.clone());
    v.insert("package", j.package_name.clone());
    v.insert("package_path", j.package_name.replace('.', "/"));
    v.insert("java_version", j.java_version.to_string());
    v.insert("javafx_version", if j.java_version >= 21 { "21.0.5" } else { "17.0.13" }.to_string());
    v
}

fn simple_vars(name: &str, module: &str) -> Vars {
    let mut v = Vars::new();
    v.insert("name", name.to_string());
    v.insert("pkg_name", pkg_name(name));
    v.insert("py_pkg", py_pkg(name));
    v.insert("module", module.to_string());
    v
}

fn put(target: &Path, rel: &str, text: &str) -> AppResult<()> {
    template::write_text(target, rel, text)
}

fn write_sets(target: &Path, sets: &[&TplSet], vars: &Vars) -> AppResult<usize> {
    let mut n = 0;
    for s in sets {
        n += template::write_all(target, s.files, vars)?;
    }
    Ok(n)
}

/// Qué CLIs complementarios se usarán tras la plantilla (si existen).
#[derive(Clone, Copy, Debug, Default)]
pub struct Cli {
    pub cargo: bool,
    pub go: bool,
}

/// Pasos que el llamador ejecuta después (async, con streaming).
#[derive(Clone, Debug, PartialEq)]
pub enum Post {
    GradleWrapper,
    CargoInit { lib: bool, fallback: bool },
    GoModInit { module: String },
    PythonVenv,
    NpmInstall,
}

/// Escribe el proyecto local de `req` en `target` (que ya existe). Devuelve los pasos posteriores.
pub fn generate(req: &ProjectRequest, target: &Path, cli: Cli, rep: &Reporter) -> AppResult<Vec<Post>> {
    use ProjectRequest::*;
    rep.alive()?;
    let mut post = Vec::new();
    match req {
        Java { common, jvm } => {
            let v = jvm_vars(&common.name, jvm);
            let sample = jvm.sample_code;
            let gradle = matches!(jvm.build, BuildSystem::GradleKotlin | BuildSystem::GradleGroovy);
            let mut sets: Vec<&TplSet> = match jvm.build {
                BuildSystem::Intellij => vec![],
                BuildSystem::Maven => vec![&JAVA_MAVEN],
                BuildSystem::GradleKotlin => vec![&JAVA_GRADLE_KTS],
                BuildSystem::GradleGroovy => vec![&JAVA_GRADLE_GROOVY],
            };
            if sample {
                sets.push(match jvm.build {
                    BuildSystem::Intellij => &JAVA_PLAIN_SAMPLE,
                    BuildSystem::Maven => &JAVA_MAVEN_SAMPLE,
                    _ => &JAVA_GRADLE_SAMPLE,
                });
            } else if jvm.build == BuildSystem::Intellij {
                std::fs::create_dir_all(target.join("src"))?;
            }
            write_sets(target, &sets, &v)?;
            put(target, ".gitignore", GITIGNORE_JAVA)?;
            put(target, "README.md", &template::render(README, &v)?)?;
            if gradle {
                post.push(Post::GradleWrapper);
            }
        }
        Kotlin { common, jvm } | Groovy { common, jvm } => {
            let kotlin = matches!(req, Kotlin { .. });
            let v = jvm_vars(&common.name, jvm);
            // IntelliJ no tiene "build system" propio para Kotlin/Groovy: se usa Gradle (Kotlin DSL).
            let build = if jvm.build == BuildSystem::Intellij { BuildSystem::GradleKotlin } else { jvm.build };
            let sets: Vec<&TplSet> = match (kotlin, build) {
                (true, BuildSystem::Maven) => vec![&KOTLIN_MAVEN, &KOTLIN_SAMPLE_MAVEN],
                (true, BuildSystem::GradleGroovy) => vec![&KOTLIN_GRADLE_GROOVY, &KOTLIN_SAMPLE_GRADLE],
                (true, _) => vec![&KOTLIN_GRADLE_KTS, &KOTLIN_SAMPLE_GRADLE],
                (false, BuildSystem::Maven) => vec![&GROOVY_MAVEN, &GROOVY_SAMPLE_MAVEN],
                (false, BuildSystem::GradleKotlin) => vec![&GROOVY_GRADLE_KTS, &GROOVY_SAMPLE_GRADLE],
                (false, _) => vec![&GROOVY_GRADLE_GROOVY, &GROOVY_SAMPLE_GRADLE],
            };
            // El código de muestra es el último conjunto; se omite si no se pidió.
            let take = if jvm.sample_code { sets.len() } else { sets.len() - 1 };
            write_sets(target, &sets[..take], &v)?;
            put(target, ".gitignore", GITIGNORE_JAVA)?;
            put(target, "README.md", &template::render(README, &v)?)?;
            if build != BuildSystem::Maven {
                post.push(Post::GradleWrapper);
            }
        }
        Empty { common } => {
            let v = simple_vars(&common.name, "");
            put(target, ".gitignore", GITIGNORE_GENERIC)?;
            put(target, "README.md", &template::render(README, &v)?)?;
        }
        JavaFx { common, jvm } => {
            if jvm.java_version < 17 {
                rep.warn("JavaFX 21 requiere Java 17 o superior; se usa JavaFX 17");
            }
            let v = jvm_vars(&common.name, jvm);
            write_sets(target, &[&JAVAFX], &v)?;
            put(target, ".gitignore", GITIGNORE_JAVA)?;
            put(target, "README.md", &template::render(README, &v)?)?;
        }
        JakartaEe { common, jvm } => {
            let v = jvm_vars(&common.name, jvm);
            write_sets(target, &[&JAKARTA], &v)?;
            put(target, ".gitignore", GITIGNORE_JAVA)?;
            put(target, "README.md", &template::render(README, &v)?)?;
        }
        Ktor { common, jvm } => {
            let v = jvm_vars(&common.name, jvm);
            write_sets(target, &[&KTOR], &v)?;
            put(target, ".gitignore", GITIGNORE_JAVA)?;
            put(target, "README.md", &template::render(README, &v)?)?;
            post.push(Post::GradleWrapper);
        }
        Html { common } => {
            let v = simple_vars(&common.name, "");
            write_sets(target, &[&HTML], &v)?;
            put(target, ".gitignore", GITIGNORE_GENERIC)?;
            put(target, "README.md", &template::render(README, &v)?)?;
        }
        Express { common, typescript } | Node { common, typescript } => {
            let express = matches!(req, Express { .. });
            let v = simple_vars(&common.name, "");
            let set = match (express, *typescript) {
                (true, false) => &EXPRESS_JS,
                (true, true) => &EXPRESS_TS,
                (false, false) => &NODE_JS,
                (false, true) => &NODE_TS,
            };
            write_sets(target, &[set], &v)?;
            put(target, ".gitignore", GITIGNORE_NODE)?;
            put(target, "README.md", &template::render(README, &v)?)?;
            post.push(Post::NpmInstall);
        }
        Python { common, venv } => {
            let v = simple_vars(&common.name, "");
            write_sets(target, &[&PYTHON], &v)?;
            put(target, ".gitignore", GITIGNORE_PYTHON)?;
            put(target, "README.md", &template::render(README, &v)?)?;
            if *venv {
                post.push(Post::PythonVenv);
            }
        }
        Go { common, module } => {
            let v = simple_vars(&common.name, module);
            write_sets(target, &[if cli.go { &GO_NO_MOD } else { &GO }], &v)?;
            put(target, ".gitignore", GITIGNORE_GO)?;
            put(target, "README.md", &template::render(README, &v)?)?;
            if cli.go {
                post.push(Post::GoModInit { module: module.clone() });
            }
        }
        Rust { common, bin } => {
            let v = simple_vars(&common.name, "");
            put(target, ".gitignore", GITIGNORE_RUST)?;
            put(target, "README.md", &template::render(README, &v)?)?;
            if cli.cargo {
                post.push(Post::CargoInit { lib: !*bin, fallback: false });
            } else {
                write_rust_template(target, &common.name, *bin)?;
            }
        }
        other => {
            return Err(AppError::Other(format!("generador no local: {:?}", std::mem::discriminant(other))));
        }
    }
    Ok(post)
}

/// Plantilla de Rust (fallback de `cargo init` o cuando no hay cargo).
pub fn write_rust_template(target: &Path, name: &str, bin: bool) -> AppResult<()> {
    let v = simple_vars(name, "");
    write_sets(target, &[if bin { &RUST_BIN } else { &RUST_LIB }], &v)?;
    Ok(())
}

/// Plantilla de Go (fallback de `go mod init`): solo `go.mod`.
pub fn write_go_mod(target: &Path, module: &str) -> AppResult<()> {
    put(target, "go.mod", &format!("module {module}\n\ngo 1.22\n"))
}

pub fn rust_pkg_name(name: &str) -> String {
    pkg_name(name).replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{CommonOpts, GenEvent};
    use std::sync::Arc;

    fn rep() -> Reporter {
        Reporter::detached(Arc::new(|_: GenEvent| true))
    }

    fn common(n: &str) -> CommonOpts {
        CommonOpts { name: n.into(), location: String::new(), git: false }
    }

    fn jvm(build: BuildSystem, sample: bool) -> JvmOpts {
        JvmOpts {
            build,
            java_version: 21,
            jdk_home: None,
            group_id: "com.example".into(),
            artifact_id: "demo".into(),
            package_name: "com.example.demo".into(),
            sample_code: sample,
        }
    }

    #[test]
    fn no_unresolved_placeholders_in_any_template() {
        let v = sample_vars();
        for set in ALL_SETS {
            for (path, body) in set.files {
                let p = template::render(path, &v).unwrap_or_else(|e| panic!("{}: {path}: {e}", set.name));
                let b = template::render(body, &v).unwrap_or_else(|e| panic!("{}: {path}: {e}", set.name));
                assert!(!p.contains("{{") && !b.contains("{{"), "{}: {path}", set.name);
            }
        }
        for dockerfile in [DOCKERFILE_MAVEN, DOCKERFILE_GRADLE] {
            let mut sv = Vars::new();
            sv.insert("java_version", "21".into());
            sv.insert("port", "8081".into());
            assert!(!template::render(dockerfile, &sv).unwrap().contains("{{"));
        }
    }

    #[test]
    fn java_maven_with_sample() {
        let d = tempfile::tempdir().unwrap();
        let req = ProjectRequest::Java { common: common("demo"), jvm: jvm(BuildSystem::Maven, true) };
        let post = generate(&req, d.path(), Cli::default(), &rep()).unwrap();
        assert!(post.is_empty());
        let pom = std::fs::read_to_string(d.path().join("pom.xml")).unwrap();
        assert!(pom.contains("<groupId>com.example</groupId>") && pom.contains("<artifactId>demo</artifactId>"));
        assert!(pom.contains("<maven.compiler.release>21</maven.compiler.release>"));
        let main = std::fs::read_to_string(d.path().join("src/main/java/com/example/demo/Main.java")).unwrap();
        assert!(main.starts_with("package com.example.demo;"));
        assert!(d.path().join(".gitignore").is_file() && d.path().join("README.md").is_file());
    }

    #[test]
    fn java_intellij_and_gradle_variants() {
        let d = tempfile::tempdir().unwrap();
        let req = ProjectRequest::Java { common: common("demo"), jvm: jvm(BuildSystem::Intellij, true) };
        generate(&req, d.path(), Cli::default(), &rep()).unwrap();
        assert!(d.path().join("src/com/example/demo/Main.java").is_file());
        assert!(!d.path().join("pom.xml").exists());

        let d = tempfile::tempdir().unwrap();
        let req = ProjectRequest::Java { common: common("demo"), jvm: jvm(BuildSystem::GradleKotlin, false) };
        let post = generate(&req, d.path(), Cli::default(), &rep()).unwrap();
        assert_eq!(post, vec![Post::GradleWrapper]);
        let b = std::fs::read_to_string(d.path().join("build.gradle.kts")).unwrap();
        assert!(b.contains("group = \"com.example\"") && b.contains("com.example.demo.Main"));
        assert!(std::fs::read_to_string(d.path().join("settings.gradle.kts")).unwrap().contains("rootProject.name = \"demo\""));
        assert!(!d.path().join("src/main/java").exists(), "sin código de muestra");

        let d = tempfile::tempdir().unwrap();
        let req = ProjectRequest::Java { common: common("demo"), jvm: jvm(BuildSystem::GradleGroovy, true) };
        generate(&req, d.path(), Cli::default(), &rep()).unwrap();
        assert!(std::fs::read_to_string(d.path().join("build.gradle")).unwrap().contains("mainClass = 'com.example.demo.Main'"));
    }

    #[test]
    fn kotlin_groovy_javafx_jakarta_ktor() {
        let cases: Vec<(ProjectRequest, &str)> = vec![
            (ProjectRequest::Kotlin { common: common("k"), jvm: jvm(BuildSystem::Maven, true) }, "src/main/kotlin/com/example/demo/Main.kt"),
            (ProjectRequest::Kotlin { common: common("k"), jvm: jvm(BuildSystem::GradleKotlin, true) }, "src/main/kotlin/com/example/demo/Main.kt"),
            (ProjectRequest::Groovy { common: common("g"), jvm: jvm(BuildSystem::GradleGroovy, true) }, "src/main/groovy/com/example/demo/Main.groovy"),
            (ProjectRequest::Groovy { common: common("g"), jvm: jvm(BuildSystem::Maven, true) }, "src/main/groovy/com/example/demo/Main.groovy"),
            (ProjectRequest::JavaFx { common: common("f"), jvm: jvm(BuildSystem::Maven, true) }, "src/main/java/com/example/demo/App.java"),
            (ProjectRequest::JakartaEe { common: common("j"), jvm: jvm(BuildSystem::Maven, true) }, "src/main/webapp/index.html"),
            (ProjectRequest::Ktor { common: common("kt"), jvm: jvm(BuildSystem::GradleKotlin, true) }, "src/main/kotlin/com/example/demo/Application.kt"),
        ];
        for (req, expected) in cases {
            let d = tempfile::tempdir().unwrap();
            generate(&req, d.path(), Cli::default(), &rep()).unwrap();
            assert!(d.path().join(expected).is_file(), "{expected}");
            let text = std::fs::read_to_string(d.path().join(expected)).unwrap();
            assert!(!text.contains("{{"));
        }
        let d = tempfile::tempdir().unwrap();
        let req = ProjectRequest::JakartaEe { common: common("j"), jvm: jvm(BuildSystem::Maven, true) };
        generate(&req, d.path(), Cli::default(), &rep()).unwrap();
        let pom = std::fs::read_to_string(d.path().join("pom.xml")).unwrap();
        assert!(pom.contains("<packaging>war</packaging>") && pom.contains("<scope>provided</scope>"));
    }

    #[test]
    fn web_and_scripting_projects() {
        let d = tempfile::tempdir().unwrap();
        generate(&ProjectRequest::Html { common: common("site") }, d.path(), Cli::default(), &rep()).unwrap();
        assert!(d.path().join("index.html").is_file() && d.path().join("style.css").is_file());

        let d = tempfile::tempdir().unwrap();
        let post = generate(&ProjectRequest::Express { common: common("My.Api"), typescript: true }, d.path(), Cli::default(), &rep()).unwrap();
        assert_eq!(post, vec![Post::NpmInstall]);
        let pj = std::fs::read_to_string(d.path().join("package.json")).unwrap();
        assert!(pj.contains("\"name\": \"my-api\"") && pj.contains("tsx"));
        assert!(d.path().join("src/index.ts").is_file() && d.path().join("tsconfig.json").is_file());

        let d = tempfile::tempdir().unwrap();
        generate(&ProjectRequest::Node { common: common("n"), typescript: false }, d.path(), Cli::default(), &rep()).unwrap();
        assert!(d.path().join("src/index.js").is_file());

        let d = tempfile::tempdir().unwrap();
        let post = generate(&ProjectRequest::Python { common: common("my-app"), venv: true }, d.path(), Cli::default(), &rep()).unwrap();
        assert_eq!(post, vec![Post::PythonVenv]);
        assert!(d.path().join("src/my_app/main.py").is_file() && d.path().join("pyproject.toml").is_file());
    }

    #[test]
    fn go_and_rust_with_and_without_cli() {
        let d = tempfile::tempdir().unwrap();
        let req = ProjectRequest::Go { common: common("svc"), module: "github.com/me/svc".into() };
        assert!(generate(&req, d.path(), Cli::default(), &rep()).unwrap().is_empty());
        assert!(std::fs::read_to_string(d.path().join("go.mod")).unwrap().contains("module github.com/me/svc"));

        let d = tempfile::tempdir().unwrap();
        let post = generate(&req, d.path(), Cli { go: true, cargo: false }, &rep()).unwrap();
        assert_eq!(post, vec![Post::GoModInit { module: "github.com/me/svc".into() }]);
        assert!(!d.path().join("go.mod").exists() && d.path().join("main.go").is_file());

        let d = tempfile::tempdir().unwrap();
        let req = ProjectRequest::Rust { common: common("r"), bin: true };
        generate(&req, d.path(), Cli::default(), &rep()).unwrap();
        assert!(d.path().join("Cargo.toml").is_file() && d.path().join("src/main.rs").is_file());
        let d = tempfile::tempdir().unwrap();
        let req = ProjectRequest::Rust { common: common("r"), bin: false };
        let post = generate(&req, d.path(), Cli { cargo: true, go: false }, &rep()).unwrap();
        assert_eq!(post, vec![Post::CargoInit { lib: true, fallback: false }]);
        assert!(!d.path().join("Cargo.toml").exists());
    }

    #[test]
    fn empty_project() {
        let d = tempfile::tempdir().unwrap();
        generate(&ProjectRequest::Empty { common: common("e") }, d.path(), Cli::default(), &rep()).unwrap();
        assert!(d.path().join("README.md").is_file() && d.path().join(".gitignore").is_file());
    }
}

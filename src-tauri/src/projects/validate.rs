//! Validación de entradas del asistente. La autoridad es Rust; el frontend replica para feedback.
//! Nada de lo validado aquí se interpola jamás en una línea de shell.

use std::collections::HashSet;
use std::path::PathBuf;

use regex::Regex;

use super::{BuildSystem, ProjectRequest, SpringRequest};
use crate::error::{AppError, AppResult};
use crate::util::paths::clean_canonicalize;

const RESERVED_WIN: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1",
    "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

const JAVA_KEYWORDS: &[&str] = &[
    "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class", "const", "continue",
    "default", "do", "double", "else", "enum", "extends", "final", "finally", "float", "for", "goto", "if",
    "implements", "import", "instanceof", "int", "interface", "long", "native", "new", "package", "private",
    "protected", "public", "return", "short", "static", "strictfp", "super", "switch", "synchronized", "this",
    "throw", "throws", "transient", "try", "void", "volatile", "while", "true", "false", "null", "_",
];

pub const VITE_TEMPLATES: &[&str] = &[
    "vanilla", "vanilla-ts", "react", "react-ts", "vue", "vue-ts", "svelte", "svelte-ts", "preact", "preact-ts",
    "lit", "lit-ts", "solid", "solid-ts",
];
pub const ANGULAR_STYLES: &[&str] = &["css", "scss", "sass", "less"];
pub const MAX_SERVICES: usize = 30;

fn bad(msg: impl Into<String>) -> AppError {
    AppError::InvalidArgument(msg.into())
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("regex válida")
}

/// Nombre de proyecto o carpeta: `^[A-Za-z][A-Za-z0-9._-]{0,63}$`, sin `..`, no reservado en Windows.
pub fn project_name(name: &str) -> AppResult<()> {
    if !re(r"^[A-Za-z][A-Za-z0-9._-]{0,63}$").is_match(name) {
        return Err(bad(format!(
            "nombre de proyecto inválido '{name}': empieza por letra y usa solo letras, dígitos, '.', '_' o '-' (máx. 64)"
        )));
    }
    if name.contains("..") || name.ends_with('.') || name.ends_with(' ') {
        return Err(bad(format!("nombre de proyecto inválido '{name}'")));
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    if RESERVED_WIN.contains(&stem.as_str()) {
        return Err(bad(format!("'{name}' es un nombre reservado en Windows")));
    }
    Ok(())
}

/// Nombre de microservicio: nombre de proyecto + kebab `^[a-z][a-z0-9-]{0,40}$`.
pub fn service_name(name: &str) -> AppResult<()> {
    project_name(name)?;
    if !re(r"^[a-z][a-z0-9-]{0,40}$").is_match(name) || name.ends_with('-') || name.contains("--") {
        return Err(bad(format!("nombre de microservicio inválido '{name}': usa kebab-case (orders-service)")));
    }
    Ok(())
}

/// Paquete Java (también groupId).
pub fn package(pkg: &str) -> AppResult<()> {
    if !re(r"^[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)*$").is_match(pkg) {
        return Err(bad(format!("paquete/groupId inválido '{pkg}': minúsculas, dígitos y '_' separados por '.'")));
    }
    if let Some(k) = pkg.split('.').find(|s| JAVA_KEYWORDS.contains(s)) {
        return Err(bad(format!("'{k}' es una palabra reservada de Java (en '{pkg}')")));
    }
    Ok(())
}

pub fn artifact_id(a: &str) -> AppResult<()> {
    if !re(r"^[a-z][a-z0-9-]*$").is_match(a) || a.len() > 80 {
        return Err(bad(format!("artifactId inválido '{a}': minúsculas, dígitos y '-'")));
    }
    Ok(())
}

pub fn boot_version(v: &str) -> AppResult<()> {
    if !re(r"^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?$").is_match(v) {
        return Err(bad(format!("versión de Spring Boot inválida '{v}'")));
    }
    Ok(())
}

pub fn go_module(m: &str) -> AppResult<()> {
    if !re(r"^[A-Za-z0-9.\-_/~]+$").is_match(m) || m.contains("..") || m.starts_with('-') || m.len() > 200 {
        return Err(bad(format!("módulo de Go inválido '{m}'")));
    }
    Ok(())
}

pub fn java_version(v: u32) -> AppResult<()> {
    if (8..=40).contains(&v) {
        Ok(())
    } else {
        Err(bad(format!("versión de Java no soportada: {v}")))
    }
}

pub fn port(p: u16) -> AppResult<()> {
    if p >= 1024 {
        Ok(())
    } else {
        Err(bad(format!("puerto {p} fuera de rango (1024-65535)")))
    }
}

/// Id de dependencia / extensión / feature: nunca empieza por '-' ni contiene espacios.
pub fn token_id(id: &str, what: &str) -> AppResult<()> {
    if !re(r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,100}$").is_match(id) {
        return Err(bad(format!("{what} inválido '{id}'")));
    }
    Ok(())
}

/// Coordenadas Maven (group/artifact/version de un arquetipo).
pub fn maven_coord(s: &str, what: &str) -> AppResult<()> {
    if !re(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,100}$").is_match(s) {
        return Err(bad(format!("{what} de arquetipo inválido '{s}'")));
    }
    Ok(())
}

fn jvm(j: &super::JvmOpts, need_pkg: bool) -> AppResult<()> {
    java_version(j.java_version)?;
    package(&j.group_id)?;
    artifact_id(&j.artifact_id)?;
    if need_pkg {
        package(&j.package_name)?;
    }
    if let Some(h) = &j.jdk_home {
        if h.starts_with('-') || h.contains('\0') {
            return Err(bad("JDK inválido"));
        }
    }
    Ok(())
}

/// Puertos únicos y válidos (servicios + infraestructura).
pub fn spring(s: &SpringRequest) -> AppResult<()> {
    if s.services.is_empty() || s.services.len() > MAX_SERVICES {
        return Err(bad(format!("define entre 1 y {MAX_SERVICES} microservicios")));
    }
    if s.single && (s.services.len() != 1 || s.gateway.is_some() || s.discovery.is_some() || s.config_server.is_some()) {
        return Err(bad("un proyecto Spring Boot simple tiene un solo servicio y sin infraestructura"));
    }
    package(&s.group_id)?;
    package(&s.base_package)?;
    java_version(s.java_version)?;
    if let Some(b) = &s.boot_version {
        boot_version(b)?;
    }
    let mut names = HashSet::new();
    let mut ports = HashSet::new();
    let mut check_port = |p: u16, who: &str| -> AppResult<()> {
        port(p)?;
        if !ports.insert(p) {
            return Err(bad(format!("puerto {p} duplicado ({who})")));
        }
        Ok(())
    };
    for svc in &s.services {
        service_name(&svc.name)?;
        if !names.insert(svc.name.clone()) {
            return Err(bad(format!("microservicio duplicado: {}", svc.name)));
        }
        check_port(svc.port, &svc.name)?;
        for d in &svc.dependencies {
            token_id(d, "id de dependencia")?;
        }
        package(&format!("{}.{}", s.base_package, super::spring::generate::pkg_segment(&svc.name)))?;
    }
    for (spec, name) in [
        (&s.gateway, super::spring::generate::GATEWAY),
        (&s.discovery, super::spring::generate::EUREKA),
        (&s.config_server, super::spring::generate::CONFIG),
    ] {
        if let Some(i) = spec {
            if names.contains(name) {
                return Err(bad(format!("el nombre '{name}' está reservado para la infraestructura")));
            }
            check_port(i.port, name)?;
        }
    }
    if s.build == BuildSystem::Intellij {
        return Err(bad("Spring requiere Maven o Gradle"));
    }
    Ok(())
}

/// Valida toda la petición. Devuelve `(location canonicalizada, target)`.
pub fn request(req: &ProjectRequest) -> AppResult<(PathBuf, PathBuf)> {
    let c = req.common();
    project_name(&c.name)?;
    let location = location(&c.location)?;
    let target = location.join(&c.name);
    if target.parent() != Some(location.as_path()) {
        return Err(bad("destino fuera de la carpeta elegida"));
    }
    if target.exists() {
        let empty = target.is_dir() && std::fs::read_dir(&target).map(|mut d| d.next().is_none()).unwrap_or(false);
        if !empty {
            return Err(AppError::AlreadyExists(target.display().to_string()));
        }
    }
    use ProjectRequest::*;
    match req {
        Java { jvm: j, .. } | Kotlin { jvm: j, .. } | Groovy { jvm: j, .. } | JavaFx { jvm: j, .. }
        | JakartaEe { jvm: j, .. } | Ktor { jvm: j, .. } => jvm(j, true)?,
        MavenArchetype { jvm: j, archetype, .. } => {
            jvm(j, true)?;
            maven_coord(&archetype.group, "groupId")?;
            maven_coord(&archetype.artifact, "artifactId")?;
            maven_coord(&archetype.version, "versión")?;
        }
        Quarkus { jvm: j, extensions, .. } => {
            jvm(j, true)?;
            for e in extensions {
                token_id(e, "extensión")?;
            }
        }
        Micronaut { jvm: j, features, .. } => {
            jvm(j, true)?;
            for f in features {
                token_id(f, "feature")?;
            }
        }
        SpringMicroservices { spring: s, .. } => spring(s)?,
        Vite { template, .. } => {
            if !VITE_TEMPLATES.contains(&template.as_str()) {
                return Err(bad(format!("plantilla de Vite no permitida: {template}")));
            }
        }
        Angular { style, .. } => {
            if !ANGULAR_STYLES.contains(&style.as_str()) {
                return Err(bad(format!("estilo de Angular no permitido: {style}")));
            }
            if !re(r"^[A-Za-z][A-Za-z0-9-]*$").is_match(&c.name) {
                return Err(bad("Angular solo admite letras, dígitos y '-' en el nombre"));
            }
        }
        Go { module, .. } => go_module(module)?,
        Empty { .. } | Html { .. } | Express { .. } | Node { .. } | React { .. } | Vue { .. } | Python { .. }
        | Rust { .. } => {}
    }
    Ok((location, target))
}

/// Carpeta padre: existe, es carpeta, canonicalizada.
pub fn location(loc: &str) -> AppResult<PathBuf> {
    if loc.trim().is_empty() || loc.contains('\0') {
        return Err(bad("elige una carpeta de destino"));
    }
    let p = PathBuf::from(loc);
    if !p.is_dir() {
        return Err(AppError::NotFound(format!("la carpeta de destino no existe: {loc}")));
    }
    Ok(clean_canonicalize(&p)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{CommonOpts, InfraSpec, MicroserviceSpec};

    #[test]
    fn project_names() {
        for ok in ["app", "My-App_1.2", "a"] {
            assert!(project_name(ok).is_ok(), "{ok}");
        }
        for bad in ["", "-x", "../evil", "a..b", "CON", "con.txt", "x.", "1abc", "a b", "a/b", &"a".repeat(65)] {
            assert!(project_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn packages() {
        for ok in ["com.example.app", "a", "com.ex_1.x2"] {
            assert!(package(ok).is_ok(), "{ok}");
        }
        for bad in ["com.class.x", "com..x", "-x", "Com.x", "com.example.", "1a.b", "com.new"] {
            assert!(package(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn service_and_misc() {
        assert!(service_name("orders-service").is_ok());
        assert!(service_name("Orders").is_err());
        assert!(service_name("a--b").is_err());
        assert!(artifact_id("demo-app").is_ok() && artifact_id("Demo").is_err());
        assert!(boot_version("3.4.5").is_ok() && boot_version("3.4.5-RC1").is_ok() && boot_version("3.4").is_err());
        assert!(go_module("github.com/me/app").is_ok() && go_module("../x").is_err() && go_module("a b").is_err());
        assert!(token_id("data-jpa", "dep").is_ok() && token_id("-evil", "dep").is_err());
    }

    fn svc(n: &str, p: u16) -> MicroserviceSpec {
        MicroserviceSpec { name: n.into(), port: p, dependencies: vec!["web".into()], layers: true }
    }

    fn base_spring() -> SpringRequest {
        SpringRequest {
            group_id: "com.example".into(),
            base_package: "com.example.shop".into(),
            java_version: 21,
            boot_version: None,
            build: BuildSystem::Maven,
            services: vec![svc("orders", 8081), svc("users", 8082)],
            gateway: Some(InfraSpec { port: 8080 }),
            discovery: None,
            config_server: None,
            docker_compose: false,
            dockerfiles: false,
            prefer_online: false,
            single: false,
        }
    }

    #[test]
    fn spring_ports_and_names() {
        assert!(spring(&base_spring()).is_ok());
        let mut s = base_spring();
        s.services[1].port = 8081;
        assert!(spring(&s).is_err(), "puerto duplicado");
        let mut s = base_spring();
        s.gateway = Some(InfraSpec { port: 8081 });
        assert!(spring(&s).is_err(), "gateway colisiona");
        let mut s = base_spring();
        s.services[0].port = 80;
        assert!(spring(&s).is_err(), "puerto bajo");
        let mut s = base_spring();
        s.services[0].name = "api-gateway".into();
        assert!(spring(&s).is_err(), "nombre reservado");
        let mut s = base_spring();
        s.services[0].name = "new".into();
        assert!(spring(&s).is_err(), "paquete reservado");
        let mut s = base_spring();
        s.services.push(svc("orders", 8090));
        assert!(spring(&s).is_err(), "nombre duplicado");
        let mut s = base_spring();
        s.services[0].dependencies = vec!["-bad".into()];
        assert!(spring(&s).is_err());
    }

    #[test]
    fn request_location_and_target() {
        let d = tempfile::tempdir().unwrap();
        let mk = |name: &str, loc: &str| ProjectRequest::Empty {
            common: CommonOpts { name: name.into(), location: loc.into(), git: false },
        };
        let loc = d.path().to_string_lossy().into_owned();
        let (l, t) = request(&mk("proj", &loc)).unwrap();
        assert_eq!(t, l.join("proj"));
        assert!(request(&mk("../evil", &loc)).is_err());
        assert!(request(&mk("proj", "/no/existe/seguro")).is_err());
        std::fs::create_dir(d.path().join("full")).unwrap();
        std::fs::write(d.path().join("full/x"), "1").unwrap();
        assert!(matches!(request(&mk("full", &loc)), Err(AppError::AlreadyExists(_))));
        std::fs::create_dir(d.path().join("empty")).unwrap();
        assert!(request(&mk("empty", &loc)).is_ok());
    }

    #[test]
    fn whitelists() {
        let d = tempfile::tempdir().unwrap();
        let common = CommonOpts { name: "v".into(), location: d.path().to_string_lossy().into(), git: false };
        let r = ProjectRequest::Vite { common: common.clone(), template: "react-ts".into() };
        assert!(request(&r).is_ok());
        let r = ProjectRequest::Vite { common: common.clone(), template: "react; rm -rf".into() };
        assert!(request(&r).is_err());
        let r = ProjectRequest::Angular { common, routing: true, style: "stylus".into() };
        assert!(request(&r).is_err());
    }
}

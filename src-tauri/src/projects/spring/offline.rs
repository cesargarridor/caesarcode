//! Plantillas offline de un módulo Spring: `pom.xml` / `build.gradle(.kts)` a partir del catálogo local.
//! No necesita red ni Maven: solo escribe archivos.

use std::path::Path;

use super::catalog::{self, DepDef, Scope};
use super::generate::{Module, Plan};
use crate::error::AppResult;
use crate::projects::template;
use crate::projects::{BuildSystem, Reporter};

/// Resuelve las dependencias del módulo; las que la plantilla offline no soporta se devuelven aparte.
pub fn resolve(m: &Module) -> (Vec<&'static DepDef>, Vec<String>) {
    let mut defs = Vec::new();
    let mut missing = Vec::new();
    for id in &m.deps {
        match catalog::find(id) {
            Some(d) => defs.push(d),
            None => missing.push(id.clone()),
        }
    }
    (defs, missing)
}

pub fn maven_pom(plan: &Plan, m: &Module, defs: &[&DepDef]) -> String {
    let cloud = defs.iter().any(|d| d.cloud);
    let lombok = defs.iter().any(|d| d.coords.iter().any(|c| c.scope == Scope::Lombok));
    let mut s = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>
    <parent>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-starter-parent</artifactId>
        <version>@BOOT@</version>
        <relativePath/>
    </parent>
    <groupId>@GROUP@</groupId>
    <artifactId>@NAME@</artifactId>
    <version>0.0.1-SNAPSHOT</version>
    <name>@NAME@</name>
    <description>@NAME@</description>
    <properties>
        <java.version>@JAVA@</java.version>
"#,
    );
    if cloud {
        s.push_str("        <spring-cloud.version>@CLOUD@</spring-cloud.version>\n");
    }
    s.push_str("    </properties>\n    <dependencies>\n");
    for d in defs {
        for c in d.coords {
            s.push_str(&format!("        <dependency>\n            <groupId>{}</groupId>\n            <artifactId>{}</artifactId>\n", c.group, c.artifact));
            match c.scope {
                Scope::Compile => {}
                Scope::Runtime => s.push_str("            <scope>runtime</scope>\n"),
                Scope::Lombok => s.push_str("            <optional>true</optional>\n"),
                Scope::DevTools => s.push_str("            <scope>runtime</scope>\n            <optional>true</optional>\n"),
            }
            s.push_str("        </dependency>\n");
        }
    }
    s.push_str("        <dependency>\n            <groupId>org.springframework.boot</groupId>\n            <artifactId>spring-boot-starter-test</artifactId>\n            <scope>test</scope>\n        </dependency>\n    </dependencies>\n");
    if cloud {
        s.push_str("    <dependencyManagement>\n        <dependencies>\n            <dependency>\n                <groupId>org.springframework.cloud</groupId>\n                <artifactId>spring-cloud-dependencies</artifactId>\n                <version>${spring-cloud.version}</version>\n                <type>pom</type>\n                <scope>import</scope>\n            </dependency>\n        </dependencies>\n    </dependencyManagement>\n");
    }
    s.push_str("    <build>\n        <plugins>\n");
    if lombok {
        s.push_str("            <plugin>\n                <groupId>org.apache.maven.plugins</groupId>\n                <artifactId>maven-compiler-plugin</artifactId>\n                <configuration>\n                    <annotationProcessorPaths>\n                        <path>\n                            <groupId>org.projectlombok</groupId>\n                            <artifactId>lombok</artifactId>\n                        </path>\n                    </annotationProcessorPaths>\n                </configuration>\n            </plugin>\n");
    }
    s.push_str("            <plugin>\n                <groupId>org.springframework.boot</groupId>\n                <artifactId>spring-boot-maven-plugin</artifactId>\n");
    if lombok {
        s.push_str("                <configuration>\n                    <excludes>\n                        <exclude>\n                            <groupId>org.projectlombok</groupId>\n                            <artifactId>lombok</artifactId>\n                        </exclude>\n                    </excludes>\n                </configuration>\n");
    }
    s.push_str("            </plugin>\n        </plugins>\n    </build>\n</project>\n");
    s.replace("@BOOT@", &plan.boot)
        .replace("@GROUP@", &plan.req.group_id)
        .replace("@NAME@", &m.name)
        .replace("@JAVA@", &plan.req.java_version.to_string())
        .replace("@CLOUD@", &plan.cloud)
}

pub fn gradle_build(plan: &Plan, m: &Module, defs: &[&DepDef], kts: bool) -> String {
    let cloud = defs.iter().any(|d| d.cloud);
    let q = |s: &str| if kts { format!("(\"{s}\")") } else { format!(" '{s}'") };
    let mut s = String::new();
    if kts {
        s.push_str("plugins {\n    java\n    id(\"org.springframework.boot\") version \"@BOOT@\"\n    id(\"io.spring.dependency-management\") version \"1.1.7\"\n}\n\ngroup = \"@GROUP@\"\nversion = \"0.0.1-SNAPSHOT\"\n\njava {\n    toolchain {\n        languageVersion = JavaLanguageVersion.of(@JAVA@)\n    }\n}\n\nrepositories {\n    mavenCentral()\n}\n\n");
        if cloud {
            s.push_str("extra[\"springCloudVersion\"] = \"@CLOUD@\"\n\n");
        }
    } else {
        s.push_str("plugins {\n    id 'java'\n    id 'org.springframework.boot' version '@BOOT@'\n    id 'io.spring.dependency-management' version '1.1.7'\n}\n\ngroup = '@GROUP@'\nversion = '0.0.1-SNAPSHOT'\n\njava {\n    toolchain {\n        languageVersion = JavaLanguageVersion.of(@JAVA@)\n    }\n}\n\nrepositories {\n    mavenCentral()\n}\n\n");
        if cloud {
            s.push_str("ext {\n    set('springCloudVersion', \"@CLOUD@\")\n}\n\n");
        }
    }
    s.push_str("dependencies {\n");
    for d in defs {
        for c in d.coords {
            let gav = format!("{}:{}", c.group, c.artifact);
            let configs: &[&str] = match c.scope {
                Scope::Compile => &["implementation"],
                Scope::Runtime => &["runtimeOnly"],
                Scope::Lombok => &["compileOnly", "annotationProcessor"],
                Scope::DevTools => &["developmentOnly"],
            };
            for cfg in configs {
                s.push_str(&format!("    {cfg}{}\n", q(&gav)));
            }
        }
    }
    s.push_str(&format!("    testImplementation{}\n", q("org.springframework.boot:spring-boot-starter-test")));
    s.push_str(&format!("    testRuntimeOnly{}\n}}\n", q("org.junit.platform:junit-platform-launcher")));
    if cloud {
        if kts {
            s.push_str("\ndependencyManagement {\n    imports {\n        mavenBom(\"org.springframework.cloud:spring-cloud-dependencies:${property(\"springCloudVersion\")}\")\n    }\n}\n");
        } else {
            s.push_str("\ndependencyManagement {\n    imports {\n        mavenBom \"org.springframework.cloud:spring-cloud-dependencies:${springCloudVersion}\"\n    }\n}\n");
        }
    }
    s.push_str(if kts { "\ntasks.withType<Test> {\n    useJUnitPlatform()\n}\n" } else { "\ntasks.named('test') {\n    useJUnitPlatform()\n}\n" });
    s.replace("@BOOT@", &plan.boot)
        .replace("@GROUP@", &plan.req.group_id)
        .replace("@JAVA@", &plan.req.java_version.to_string())
        .replace("@CLOUD@", &plan.cloud)
        .replace("@NAME@", &m.name)
}

/// Escribe los archivos de build del módulo en `dir` (sin código Java: eso lo hace `finish_module`).
pub fn write_build(plan: &Plan, m: &Module, dir: &Path, rep: &Reporter) -> AppResult<()> {
    let (defs, missing) = resolve(m);
    if !missing.is_empty() {
        rep.warn(format!("{}: dependencias no soportadas por la plantilla offline (omitidas): {}", m.name, missing.join(", ")));
    }
    std::fs::create_dir_all(dir)?;
    match plan.req.build {
        BuildSystem::Maven | BuildSystem::Intellij => template::write_text(dir, "pom.xml", &maven_pom(plan, m, &defs))?,
        BuildSystem::GradleKotlin => {
            template::write_text(dir, "build.gradle.kts", &gradle_build(plan, m, &defs, true))?;
            template::write_text(dir, "settings.gradle.kts", &format!("rootProject.name = \"{}\"\n", m.name))?;
        }
        BuildSystem::GradleGroovy => {
            template::write_text(dir, "build.gradle", &gradle_build(plan, m, &defs, false))?;
            template::write_text(dir, "settings.gradle", &format!("rootProject.name = '{}'\n", m.name))?;
        }
    }
    std::fs::create_dir_all(dir.join("src/main/resources"))?;
    Ok(())
}

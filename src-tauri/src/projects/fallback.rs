//! Plantillas Maven mínimas de Quarkus y Micronaut para cuando no hay red (versiones fijas en `spring::catalog`).
//! Siempre Maven: el aviso al usuario lo emite el llamador.

use std::path::Path;

use super::spring::catalog::{MICRONAUT_FALLBACK, QUARKUS_FALLBACK};
use super::template;
use super::{JvmOpts, Reporter};
use crate::error::AppResult;

const HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>
"#;

pub fn quarkus_pom(name: &str, j: &JvmOpts, extensions: &[String], rep: Option<&Reporter>) -> String {
    let mut deps = String::new();
    let mut ids = vec!["rest".to_string(), "arc".to_string()];
    for e in extensions {
        if e.contains(':') {
            if let Some(r) = rep {
                r.warn(format!("extensión '{e}' no soportada offline (omitida)"));
            }
        } else if !ids.contains(e) {
            ids.push(e.clone());
        }
    }
    for id in &ids {
        let id = id.strip_prefix("quarkus-").unwrap_or(id);
        deps.push_str(&format!("        <dependency>\n            <groupId>io.quarkus</groupId>\n            <artifactId>quarkus-{id}</artifactId>\n        </dependency>\n"));
    }
    let mut s = String::from(HEAD);
    s.push_str(&format!(
        r#"    <groupId>{g}</groupId>
    <artifactId>{a}</artifactId>
    <version>1.0.0-SNAPSHOT</version>
    <name>{name}</name>
    <properties>
        <maven.compiler.release>{java}</maven.compiler.release>
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <quarkus.platform.group-id>io.quarkus.platform</quarkus.platform.group-id>
        <quarkus.platform.artifact-id>quarkus-bom</quarkus.platform.artifact-id>
        <quarkus.platform.version>{q}</quarkus.platform.version>
    </properties>
    <dependencyManagement>
        <dependencies>
            <dependency>
                <groupId>${{quarkus.platform.group-id}}</groupId>
                <artifactId>${{quarkus.platform.artifact-id}}</artifactId>
                <version>${{quarkus.platform.version}}</version>
                <type>pom</type>
                <scope>import</scope>
            </dependency>
        </dependencies>
    </dependencyManagement>
    <dependencies>
{deps}        <dependency>
            <groupId>io.quarkus</groupId>
            <artifactId>quarkus-junit5</artifactId>
            <scope>test</scope>
        </dependency>
    </dependencies>
    <build>
        <plugins>
            <plugin>
                <groupId>${{quarkus.platform.group-id}}</groupId>
                <artifactId>quarkus-maven-plugin</artifactId>
                <version>${{quarkus.platform.version}}</version>
                <extensions>true</extensions>
                <executions>
                    <execution>
                        <goals>
                            <goal>build</goal>
                            <goal>generate-code</goal>
                            <goal>generate-code-tests</goal>
                        </goals>
                    </execution>
                </executions>
            </plugin>
            <plugin>
                <artifactId>maven-compiler-plugin</artifactId>
                <version>3.13.0</version>
                <configuration>
                    <parameters>true</parameters>
                </configuration>
            </plugin>
        </plugins>
    </build>
</project>
"#,
        g = j.group_id,
        a = j.artifact_id,
        java = j.java_version,
        q = QUARKUS_FALLBACK,
    ));
    s
}

pub fn micronaut_pom(name: &str, j: &JvmOpts) -> String {
    let mut s = String::from(HEAD);
    s.push_str(&format!(
        r#"    <parent>
        <groupId>io.micronaut.platform</groupId>
        <artifactId>micronaut-parent</artifactId>
        <version>{m}</version>
    </parent>
    <groupId>{g}</groupId>
    <artifactId>{a}</artifactId>
    <version>0.1</version>
    <name>{name}</name>
    <properties>
        <packaging>jar</packaging>
        <jdk.version>{java}</jdk.version>
        <release.version>{java}</release.version>
        <micronaut.version>{m}</micronaut.version>
        <micronaut.runtime>netty</micronaut.runtime>
        <exec.mainClass>{pkg}.Application</exec.mainClass>
    </properties>
    <dependencies>
        <dependency>
            <groupId>io.micronaut</groupId>
            <artifactId>micronaut-http-server-netty</artifactId>
            <scope>compile</scope>
        </dependency>
        <dependency>
            <groupId>io.micronaut.serde</groupId>
            <artifactId>micronaut-serde-jackson</artifactId>
            <scope>compile</scope>
        </dependency>
        <dependency>
            <groupId>ch.qos.logback</groupId>
            <artifactId>logback-classic</artifactId>
            <scope>runtime</scope>
        </dependency>
    </dependencies>
    <build>
        <plugins>
            <plugin>
                <groupId>io.micronaut.maven</groupId>
                <artifactId>micronaut-maven-plugin</artifactId>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-compiler-plugin</artifactId>
                <configuration>
                    <annotationProcessorPaths combine.children="append">
                        <path>
                            <groupId>io.micronaut</groupId>
                            <artifactId>micronaut-http-validation</artifactId>
                            <version>${{micronaut.core.version}}</version>
                        </path>
                        <path>
                            <groupId>io.micronaut.serde</groupId>
                            <artifactId>micronaut-serde-processor</artifactId>
                            <version>${{micronaut.serde.version}}</version>
                            <exclusions>
                                <exclusion>
                                    <groupId>io.micronaut</groupId>
                                    <artifactId>micronaut-inject</artifactId>
                                </exclusion>
                            </exclusions>
                        </path>
                    </annotationProcessorPaths>
                </configuration>
            </plugin>
        </plugins>
    </build>
</project>
"#,
        m = MICRONAUT_FALLBACK,
        g = j.group_id,
        a = j.artifact_id,
        java = j.java_version,
        pkg = j.package_name,
    ));
    s
}

pub fn write_quarkus(target: &Path, name: &str, j: &JvmOpts, extensions: &[String], rep: &Reporter) -> AppResult<()> {
    template::write_text(target, "pom.xml", &quarkus_pom(name, j, extensions, Some(rep)))?;
    let path = j.package_name.replace('.', "/");
    template::write_text(
        target,
        &format!("src/main/java/{path}/GreetingResource.java"),
        &format!("package {};\n\nimport jakarta.ws.rs.GET;\nimport jakarta.ws.rs.Path;\nimport jakarta.ws.rs.Produces;\nimport jakarta.ws.rs.core.MediaType;\n\n@Path(\"/hello\")\npublic class GreetingResource {{\n\n    @GET\n    @Produces(MediaType.TEXT_PLAIN)\n    public String hello() {{\n        return \"Hello from {name}\";\n    }}\n}}\n", j.package_name),
    )?;
    template::write_text(target, "src/main/resources/application.properties", "quarkus.http.port=8080\n")?;
    Ok(())
}

pub fn write_micronaut(target: &Path, name: &str, j: &JvmOpts, features: &[String], rep: &Reporter) -> AppResult<()> {
    if !features.is_empty() {
        rep.warn(format!("features de Micronaut no soportadas offline (omitidas): {}", features.join(", ")));
    }
    template::write_text(target, "pom.xml", &micronaut_pom(name, j))?;
    let path = j.package_name.replace('.', "/");
    template::write_text(
        target,
        &format!("src/main/java/{path}/Application.java"),
        &format!("package {};\n\nimport io.micronaut.runtime.Micronaut;\n\npublic class Application {{\n\n    public static void main(String[] args) {{\n        Micronaut.run(Application.class, args);\n    }}\n}}\n", j.package_name),
    )?;
    template::write_text(
        target,
        &format!("src/main/java/{path}/HelloController.java"),
        &format!("package {};\n\nimport io.micronaut.http.MediaType;\nimport io.micronaut.http.annotation.Controller;\nimport io.micronaut.http.annotation.Get;\n\n@Controller(\"/hello\")\npublic class HelloController {{\n\n    @Get(produces = MediaType.TEXT_PLAIN)\n    public String hello() {{\n        return \"Hello from {name}\";\n    }}\n}}\n", j.package_name),
    )?;
    template::write_text(
        target,
        "src/main/resources/application.properties",
        &format!("micronaut.application.name={}\n", j.artifact_id),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{BuildSystem, GenEvent};
    use std::sync::Arc;

    fn jvm() -> JvmOpts {
        JvmOpts { build: BuildSystem::Maven, java_version: 21, jdk_home: None, group_id: "com.example".into(), artifact_id: "demo".into(), package_name: "com.example.demo".into(), sample_code: true }
    }

    #[test]
    fn quarkus_and_micronaut_fallback_files() {
        let rep = Reporter::detached(Arc::new(|_: GenEvent| true));
        let d = tempfile::tempdir().unwrap();
        write_quarkus(d.path(), "demo", &jvm(), &["hibernate-orm-panache".into(), "io.other:ext".into()], &rep).unwrap();
        let pom = std::fs::read_to_string(d.path().join("pom.xml")).unwrap();
        assert!(pom.contains("quarkus-rest") && pom.contains("quarkus-hibernate-orm-panache") && !pom.contains("io.other"));
        assert!(pom.contains("${quarkus.platform.version}") && pom.contains(QUARKUS_FALLBACK));
        assert!(d.path().join("src/main/java/com/example/demo/GreetingResource.java").is_file());
        let d = tempfile::tempdir().unwrap();
        write_micronaut(d.path(), "demo", &jvm(), &[], &rep).unwrap();
        let pom = std::fs::read_to_string(d.path().join("pom.xml")).unwrap();
        assert!(pom.contains("micronaut-parent") && pom.contains("com.example.demo.Application") && pom.contains("${micronaut.core.version}"));
        assert!(d.path().join("src/main/java/com/example/demo/Application.java").is_file());
    }
}

plugins {
    java
    application
}

group = "{{group_id}}"
version = "1.0-SNAPSHOT"

repositories {
    mavenCentral()
}

dependencies {
    testImplementation(platform("org.junit:junit-bom:5.11.3"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

tasks.withType<JavaCompile> {
    options.encoding = "UTF-8"
    options.release.set({{java_version}})
}

application {
    mainClass.set("{{package}}.Main")
}

tasks.test {
    useJUnitPlatform()
}

plugins {
    kotlin("jvm") version "2.0.21"
    application
}

group = "{{group_id}}"
version = "1.0-SNAPSHOT"

repositories {
    mavenCentral()
}

dependencies {
    testImplementation(kotlin("test"))
}

application {
    mainClass.set("{{package}}.MainKt")
}

tasks.test {
    useJUnitPlatform()
}

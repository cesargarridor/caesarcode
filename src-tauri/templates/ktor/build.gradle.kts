plugins {
    kotlin("jvm") version "2.0.21"
    id("io.ktor.plugin") version "3.0.2"
}

group = "{{group_id}}"
version = "1.0-SNAPSHOT"

application {
    mainClass.set("{{package}}.ApplicationKt")
}

repositories {
    mavenCentral()
}

dependencies {
    implementation("io.ktor:ktor-server-core-jvm")
    implementation("io.ktor:ktor-server-netty-jvm")
    implementation("ch.qos.logback:logback-classic:1.5.12")
    testImplementation("io.ktor:ktor-server-test-host-jvm")
    testImplementation("org.jetbrains.kotlin:kotlin-test-junit")
}

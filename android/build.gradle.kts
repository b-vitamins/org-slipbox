// Copyright (C) 2026 Ayan Das
// SPDX-License-Identifier: GPL-3.0-or-later

import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.ValueSource
import org.gradle.api.provider.ValueSourceParameters

// Keep AGP and the raised Kotlin plugins in the same classloader.
buildscript {
    dependencyLocking {
        lockAllConfigurations()
        lockMode = LockMode.STRICT
    }

    // Version-catalogue accessors are unavailable while resolving this classpath.
    val catalogue = file("gradle/libs.versions.toml").readText()

    fun pinnedVersion(name: String): String =
        checkNotNull(Regex("""(?m)^$name = "([^"]+)"$""").find(catalogue)) {
            "gradle/libs.versions.toml declares no $name version"
        }.groupValues[1]

    val expectedJdk = pinnedVersion("jdk")
    val runtimeJdk = Runtime.version()
    val actualJdk = "${runtimeJdk.feature()}.${runtimeJdk.interim()}.${runtimeJdk.update()}"
    check(actualJdk == expectedJdk) {
        "Android builds require JDK $expectedJdk; found $actualJdk. Set JAVA_HOME accordingly."
    }

    val agpVersion = pinnedVersion("agp")
    val kotlinVersion = pinnedVersion("kotlin")

    repositories {
        google {
            content {
                includeGroupAndSubgroups("com.android")
                includeGroupAndSubgroups("com.google")
                includeGroupAndSubgroups("androidx")
            }
        }
        mavenCentral()
    }

    dependencies {
        classpath("com.android.tools.build:gradle:$agpVersion")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:$kotlinVersion")
        classpath("org.jetbrains.kotlin:compose-compiler-gradle-plugin:$kotlinVersion")
        classpath("org.jetbrains.kotlin:kotlin-serialization:$kotlinVersion")
    }
}

abstract class RustlsPlatformVerifierVersion :
    ValueSource<String, RustlsPlatformVerifierVersion.Parameters> {

    interface Parameters : ValueSourceParameters {
        val lockFile: RegularFileProperty
    }

    override fun obtain(): String {
        val lines = parameters.lockFile.get().asFile.readLines()
        val packageLine = lines.indexOfFirst {
            it.trim() == "name = \"rustls-platform-verifier-android\""
        }
        val version =
            if (packageLine < 0) {
                null
            } else {
                lines.drop(packageLine + 1)
                    .firstOrNull { it.trimStart().startsWith("version = ") }
                    ?.substringAfter('"', "")
                    ?.substringBefore('"', "")
                    ?.takeIf(String::isNotEmpty)
            }
        return requireNotNull(version) {
            "rustls-platform-verifier-android is absent from Cargo.lock"
        }
    }
}

val rustlsPlatformVerifierVersion =
    providers.of(RustlsPlatformVerifierVersion::class.java) {
        parameters.lockFile.set(rootProject.layout.projectDirectory.file("../Cargo.lock"))
    }

subprojects {
    configurations.configureEach {
        resolutionStrategy.eachDependency {
            if (requested.group == "org.rustls" && requested.name == "rustls-platform-verifier") {
                useVersion(rustlsPlatformVerifierVersion.get())
                because("the JVM and Rust verifier components must have identical versions")
            }
        }
    }
}

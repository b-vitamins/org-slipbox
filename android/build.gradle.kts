// Copyright (C) 2026 Ayan Das
// SPDX-License-Identifier: GPL-3.0-or-later

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
    }
}

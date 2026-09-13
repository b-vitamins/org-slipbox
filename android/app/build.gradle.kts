// Copyright (C) 2026 Ayan Das
// SPDX-License-Identifier: GPL-3.0-or-later

import java.util.Properties

buildscript {
    dependencyLocking {
        lockAllConfigurations()
        lockMode = LockMode.STRICT
    }
}

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

val applicationIdBase = "io.github.b_vitamins.slipbox"
val debugApplicationIdSuffix = ".debug"

val versionProperties =
    Properties().apply {
        val text =
            providers
                .fileContents(rootProject.layout.projectDirectory.file("version.properties"))
                .asText
                .get()
        load(text.reader())
    }

fun versionComponent(name: String, limit: Int): Int {
    val declared =
        requireNotNull(versionProperties.getProperty(name)) {
            "android/version.properties is missing the $name component"
        }
    val value =
        requireNotNull(declared.trim().toIntOrNull()) {
            "android/version.properties $name is not an integer: $declared"
        }
    require(value in 0 until limit) {
        "android/version.properties $name must be between 0 and ${limit - 1}, was $value"
    }
    return value
}

val versionMajor = versionComponent("major", 100)
val versionMinor = versionComponent("minor", 100)
val versionPatch = versionComponent("patch", 100)
val versionCandidate = versionComponent("candidate", 1000)
val versionStage =
    requireNotNull(versionProperties.getProperty("stage")?.trim()) {
        "android/version.properties is missing the stage component"
    }.also {
        require(it.matches(Regex("[a-z]+"))) {
            "android/version.properties stage must be lower-case letters, was $it"
        }
    }

val derivedVersionCode =
    versionMajor * 10_000_000 + versionMinor * 100_000 + versionPatch * 1_000 + versionCandidate

val derivedVersionName =
    if (versionStage == "release") {
        "$versionMajor.$versionMinor.$versionPatch"
    } else {
        "$versionMajor.$versionMinor.$versionPatch-$versionStage.$versionCandidate"
    }

android {
    namespace = applicationIdBase

    compileSdk { version = release(libs.versions.compile.sdk.get().toInt()) }
    buildToolsVersion = libs.versions.build.tools.get()

    defaultConfig {
        applicationId = applicationIdBase

        minSdk { version = release(libs.versions.min.sdk.get().toInt()) }
        targetSdk { version = release(libs.versions.target.sdk.get().toInt()) }

        versionCode = derivedVersionCode
        versionName = derivedVersionName

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        buildConfigField("String", "APPLICATION_ID_BASE", "\"$applicationIdBase\"")
        buildConfigField("String", "DEBUG_APPLICATION_ID_SUFFIX", "\"$debugApplicationIdSuffix\"")
        buildConfigField("int", "VERSION_MAJOR", "$versionMajor")
        buildConfigField("int", "VERSION_MINOR", "$versionMinor")
        buildConfigField("int", "VERSION_PATCH", "$versionPatch")
        buildConfigField("int", "VERSION_CANDIDATE", "$versionCandidate")
        buildConfigField("String", "VERSION_STAGE", "\"$versionStage\"")
    }

    buildTypes {
        debug {
            applicationIdSuffix = debugApplicationIdSuffix
        }
        release {
            isMinifyEnabled = false
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    lint {
        abortOnError = true
        warningsAsErrors = true
        checkDependencies = true
        checkTestSources = true

        // A newer publication does not invalidate this qualified, pinned stack.
        disable += setOf("AndroidGradlePluginVersion", "GradleDependency")
    }
}

dependencyLocking {
    lockAllConfigurations()
    lockMode = LockMode.STRICT
}

dependencies {
    implementation(platform(libs.compose.bom))

    implementation(libs.androidx.activity.compose)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    implementation(libs.compose.ui)
    implementation(libs.androidx.navigation3.runtime)
    implementation(libs.androidx.navigation3.ui)

    testImplementation(libs.junit)

    androidTestImplementation(platform(libs.compose.bom))
    androidTestImplementation(libs.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.test.ext.junit)
    androidTestImplementation(libs.androidx.test.runner)

    // Compose's transitive Espresso 3.5.0 is incompatible with API 37 input injection.
    androidTestImplementation(libs.androidx.test.espresso.core)

    debugImplementation(libs.compose.ui.test.manifest)
}

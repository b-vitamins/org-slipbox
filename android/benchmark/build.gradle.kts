// Copyright (C) 2026 Ayan Das
// SPDX-License-Identifier: GPL-3.0-or-later

buildscript {
    dependencyLocking {
        lockAllConfigurations()
        lockMode = LockMode.STRICT
    }
}

plugins {
    id("com.android.test")
}

android {
    namespace = "io.github.b_vitamins.slipbox.macrobenchmark"

    compileSdk { version = release(libs.versions.compile.sdk.get().toInt()) }
    buildToolsVersion = libs.versions.build.tools.get()

    defaultConfig {
        minSdk { version = release(29) }
        targetSdk { version = release(libs.versions.target.sdk.get().toInt()) }
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    targetProjectPath = ":app"
    experimentalProperties["android.experimental.self-instrumenting"] = true

    buildTypes {
        create("benchmark") {
            isDebuggable = true
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    lint {
        abortOnError = true
        warningsAsErrors = true
        checkDependencies = true
    }
}

dependencyLocking {
    lockAllConfigurations()
    lockMode = LockMode.STRICT
}

dependencies {
    implementation(libs.androidx.benchmark.macro.junit4)
    implementation(libs.androidx.test.ext.junit)
    implementation(libs.androidx.test.runner)
    implementation(libs.androidx.test.uiautomator)
}

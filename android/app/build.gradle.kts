// Copyright (C) 2026 Ayan Das
// SPDX-License-Identifier: GPL-3.0-or-later

import java.util.Properties
import javax.inject.Inject

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

val qualifiedAbis = mapOf("arm64-v8a" to "aarch64-linux-android")

val rustWorkspaceDirectory = rootProject.layout.projectDirectory.dir("..")
val rustCrateDirectory = rustWorkspaceDirectory.dir("crates/slipbox-android")

// The Cargo closure of the packaged library: no other crate can change its bytes.
val rustClosure =
    listOf(
        "slipbox-android",
        "slipbox-core",
        "slipbox-engine",
        "slipbox-index",
        "slipbox-rpc",
        "slipbox-store",
        "slipbox-write",
    )

// Every packaged library must load on a kernel with 16 KB pages.
val nativeLinkerArguments = listOf("-Wl,-z,max-page-size=16384")

// Both variants package the library the release profile builds, so the bytes a
// device exercises are the bytes a release carries.
val nativeCargoProfile = "release"

android {
    namespace = applicationIdBase

    compileSdk { version = release(libs.versions.compile.sdk.get().toInt()) }
    buildToolsVersion = libs.versions.build.tools.get()
    ndkVersion = libs.versions.ndk.get()

    defaultConfig {
        applicationId = applicationIdBase

        minSdk { version = release(libs.versions.min.sdk.get().toInt()) }
        targetSdk { version = release(libs.versions.target.sdk.get().toInt()) }

        ndk { abiFilters += qualifiedAbis.keys }

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
        buildConfigField("String", "QUALIFIED_ABIS", "\"${qualifiedAbis.keys.joinToString(",")}\"")
    }

    packaging {
        jniLibs {
            // Uncompressed, unextracted libraries load straight from the APK, so
            // the mapped bytes are the packaged bytes.
            useLegacyPackaging = false
        }
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

        // An unqualified ABI is worse than an absent one: an x86_64 library no
        // run has loaded would install and fail at the first native call.
        disable += "ChromeOsAbiSupport"
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

/**
 * Cross-compiles the native library for every qualified ABI.
 *
 * The NDK supplies the compiler, archiver and linker of the ABI and minimum API
 * level, so the Rust code and the bundled SQLite amalgamation observe one
 * sysroot. The version script keeps every symbol but the JNI entry point local.
 */
abstract class CargoAndroidLibrary
    @Inject
    constructor(
        private val exec: ExecOperations,
        private val files: FileSystemOperations,
    ) : DefaultTask() {
        @get:InputFiles
        @get:PathSensitive(PathSensitivity.RELATIVE)
        abstract val cargoSources: ConfigurableFileCollection

        @get:InputFile
        @get:PathSensitive(PathSensitivity.NAME_ONLY)
        abstract val versionScript: RegularFileProperty

        @get:Input
        abstract val cargoExecutable: Property<String>

        @get:Input
        abstract val cargoPackage: Property<String>

        @get:Input
        abstract val cargoProfile: Property<String>

        @get:Input
        abstract val rustToolchain: Property<String>

        @get:Input
        abstract val libraryName: Property<String>

        @get:Input
        abstract val minimumApi: Property<Int>

        @get:Input
        abstract val ndkVersion: Property<String>

        @get:Input
        abstract val targetsByAbi: MapProperty<String, String>

        @get:Input
        abstract val linkerArguments: ListProperty<String>

        // The install locations are not inputs: the pinned tool versions are.
        @get:Internal
        abstract val workspaceDirectory: DirectoryProperty

        @get:Internal
        abstract val ndkDirectory: DirectoryProperty

        @get:Internal
        abstract val cargoTargetDirectory: DirectoryProperty

        @get:OutputDirectory
        abstract val outputDirectory: DirectoryProperty

        @TaskAction
        fun cross() {
            val toolchain = prebuiltToolchain()
            val output = outputDirectory.get().asFile
            // A dropped ABI must not survive in the packaged output.
            files.delete { delete(output) }

            for ((abi, target) in targetsByAbi.get()) {
                val library = compile(target, toolchain)
                files.copy {
                    from(library)
                    into(File(output, abi))
                }
            }
        }

        private fun prebuiltToolchain(): File {
            val prebuilt = File(ndkDirectory.get().asFile, "toolchains/llvm/prebuilt")
            val hosts = (prebuilt.listFiles()?.filter { it.isDirectory } ?: emptyList()).sorted()
            check(hosts.size == 1) {
                "NDK ${ndkVersion.get()} ships ${hosts.size} prebuilt toolchains in $prebuilt"
            }
            return hosts.single()
        }

        private fun compile(target: String, toolchain: File): File {
            val compiler = File(toolchain, "bin/$target${minimumApi.get()}-clang")
            val archiver = File(toolchain, "bin/llvm-ar")
            for (tool in listOf(compiler, archiver)) {
                check(tool.canExecute()) {
                    "NDK ${ndkVersion.get()} has no executable ${tool.name} for $target: $tool"
                }
            }

            val script = versionScript.get().asFile
            val scoped = target.uppercase().replace('-', '_')
            val prefixed = target.replace('-', '_')
            exec.exec {
                executable = cargoExecutable.get()
                args(
                    "build",
                    "--locked",
                    "--package",
                    cargoPackage.get(),
                    "--profile",
                    cargoProfile.get(),
                    "--target",
                    target,
                )
                workingDir = workspaceDirectory.get().asFile
                environment("RUSTUP_TOOLCHAIN", rustToolchain.get())
                environment("CARGO_TARGET_DIR", cargoTargetDirectory.get().asFile.absolutePath)
                environment("CARGO_TARGET_${scoped}_LINKER", compiler.absolutePath)
                // Cargo splits every other rustflags variable on whitespace, so
                // only this 0x1f-separated encoding carries a build directory
                // whose path holds any. It applies to the cross-compiled units
                // alone because the build passes an explicit --target.
                environment(
                    "CARGO_ENCODED_RUSTFLAGS",
                    (linkerArguments.get() + "-Wl,--version-script=${script.absolutePath}")
                        .flatMap { listOf("-C", "link-arg=$it") }
                        .joinToString("\u001F"),
                )
                // The bundled amalgamation compiles against the same sysroot as
                // the Rust code that links it.
                environment("CC_$prefixed", compiler.absolutePath)
                environment("AR_$prefixed", archiver.absolutePath)
            }

            val profile = cargoProfile.get()
            val artifact =
                File(
                    cargoTargetDirectory.get().asFile,
                    "$target/${if (profile == "dev") "debug" else profile}/lib${libraryName.get()}.so",
                )
            check(artifact.isFile) { "cargo built no shared library at $artifact" }
            return artifact
        }
    }

val ndkDirectoryProvider = androidComponents.sdkComponents.ndkDirectory

androidComponents {
    onVariants { variant ->
        val cross =
            tasks.register<CargoAndroidLibrary>(
                "cargoAndroidLibrary${variant.name.replaceFirstChar(Char::uppercaseChar)}",
            ) {
                description = "Cross-compiles the native library the ${variant.name} variant packages."
                cargoSources.from(
                    rustWorkspaceDirectory.file("Cargo.toml"),
                    rustWorkspaceDirectory.file("Cargo.lock"),
                    rustCrateDirectory.file("Cargo.toml"),
                )
                for (crate in rustClosure) {
                    cargoSources.from(
                        rustWorkspaceDirectory.file("crates/$crate/Cargo.toml"),
                        rustWorkspaceDirectory.dir("crates/$crate/src"),
                    )
                }
                versionScript.set(rustCrateDirectory.file("jni-exports.map"))
                cargoExecutable.set(providers.gradleProperty("slipbox.cargo").orElse("cargo"))
                cargoPackage.set("slipbox-android")
                cargoProfile.set(nativeCargoProfile)
                rustToolchain.set(libs.versions.rust)
                libraryName.set("slipbox_android")
                minimumApi.set(libs.versions.min.sdk.get().toInt())
                ndkVersion.set(libs.versions.ndk)
                targetsByAbi.set(qualifiedAbis)
                linkerArguments.set(nativeLinkerArguments)
                workspaceDirectory.set(rustWorkspaceDirectory)
                ndkDirectory.set(ndkDirectoryProvider)
                cargoTargetDirectory.set(layout.buildDirectory.dir("rust-target"))
                outputDirectory.set(layout.buildDirectory.dir("generated/rust/${variant.name}/jniLibs"))
            }

        // A variant that packages no native library directory would silently
        // drop the engine.
        requireNotNull(variant.sources.jniLibs) {
            "the ${variant.name} variant has no jniLibs source directory"
        }.addGeneratedSourceDirectory(cross, CargoAndroidLibrary::outputDirectory)
    }
}

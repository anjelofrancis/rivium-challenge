plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.linkjobs"
    compileSdk = 35
    // Must be installed via Android Studio > SDK Manager > SDK Tools > NDK (Side by side).
    ndkVersion = "27.2.12479018"

    defaultConfig {
        applicationId = "dev.linkjobs"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        // arm64 for real phones, x86_64 for the emulator on Intel hosts.
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
}

// Compile crates/linkjobs-jni with cargo-ndk into src/main/jniLibs before every build,
// so pressing Run in Android Studio also rebuilds the Rust core.
// Set LINKJOBS_SKIP_RUST=1 to skip (e.g. when you built the .so files yourself).
val buildRust by tasks.registering(Exec::class) {
    group = "rust"
    description = "Build liblinkjobs_jni.so with cargo-ndk"
    workingDir = rootProject.projectDir.parentFile
    commandLine("bash", "scripts/build-rust-android.sh")
    environment("ANDROID_NDK_HOME", android.ndkDirectory.absolutePath)
    onlyIf { System.getenv("LINKJOBS_SKIP_RUST") == null }
}
tasks.named("preBuild") { dependsOn(buildRust) }

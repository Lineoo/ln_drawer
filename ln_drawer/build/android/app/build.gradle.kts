plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val repoRoot = rootProject.projectDir.parentFile.parentFile

android {
    namespace = "dev.linn.lndrawer"
    compileSdk = 34

    defaultConfig {
        applicationId = "dev.linn.lndrawer"
        minSdk = 26
        targetSdk = 34
        versionCode = 16778496
        versionName = "0.5.0-dev"

        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    packaging {
        jniLibs {
            // NativeActivity dlopen()s the .so by lib_name; it must be extracted.
            useLegacyPackaging = true
        }
    }
}

val cargoProfile = providers.gradleProperty("cargoProfile").orElse("dev")

val cargoNdk = tasks.register<Exec>("cargoNdk") {
    workingDir = repoRoot

    val args = mutableListOf(
        "cargo", "ndk",
        "-t", "arm64-v8a",
        "-P", "26",
        "-o", "build/android/app/src/main/jniLibs",
        "build",
    )
    if (cargoProfile.get() == "release") {
        args += "--release"
    }
    args += listOf("--package=ln_drawer", "--lib")
    commandLine(args)
}

tasks.named("preBuild") { dependsOn(cargoNdk) }

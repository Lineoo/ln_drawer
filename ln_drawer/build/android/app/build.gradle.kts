plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.linn.lndrawer"
    compileSdk = 34

    defaultConfig {
        applicationId = "dev.linn.lndrawer"
        minSdk = 21
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
    // set cargo workspace root
    workingDir = rootProject.projectDir.parentFile.parentFile

    val args = mutableListOf(
        "cargo", "ndk",
        "--target=aarch64-linux-android",
        "--platform=21",
        "--output-dir=build/android/app/src/main/jniLibs",
        "build",
        "--package=ln_drawer",
        "--lib"
    )
    if (cargoProfile.get() == "release") {
        args += "--release"
    }
    commandLine(args)
}

tasks.named("preBuild") { dependsOn(cargoNdk) }

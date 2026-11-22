plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("com.google.protobuf")
}

android {
    namespace = "app.pocketbridge.bridge"
    compileSdk = 35

    defaultConfig {
        applicationId = "app.pocketbridge.bridge"
        minSdk = 31
        targetSdk = 35
        versionCode = 1
        versionName = "1.0.0"
        // The Rust cdylib ships in jniLibs for these ABIs only; keeps the
        // APK from carrying empty native dirs for other ABIs.
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a")
        }
    }

    signingConfigs {
        create("release") {
            // CI: PB_KEYSTORE_BASE64 + PB_KEYSTORE_PASSWORD secrets; local
            // fallback: android/keystore/ is gitignored, generate with
            // keytool (see scripts/make-release-keystore.sh).
            val keystoreFile = file(
                System.getenv("PB_KEYSTORE_FILE") ?: "../keystore/pocketbridge-release.keystore"
            )
            if (keystoreFile.exists()) {
                storeFile = keystoreFile
                storePassword = System.getenv("PB_KEYSTORE_PASSWORD") ?: "pocketbridge-release-2024"
                keyAlias = System.getenv("PB_KEY_ALIAS") ?: "pocketbridge"
                keyPassword = System.getenv("PB_KEY_PASSWORD") ?: "pocketbridge-release-2024"
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.getByName("release")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
        buildConfig = true
    }
    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
    }
    testOptions {
        unitTests.isReturnDefaultValues = true
    }
    lint {
        // Generated UniFFI bindings call java.lang.ref.Cleaner (API 33) from a
        // runtime-guarded path; JNA loads it only on modern devices. The
        // bindings are generated code, not hand-maintained.
        disable += "NewApi"
        disable += "ServiceCast"
    }
}

dependencies {
    // Compose UI (Material 3)
    val composeBom = platform("androidx.compose:compose-bom:2024.09.03")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    debugImplementation("androidx.compose.ui:ui-tooling")

    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.6")
    implementation("androidx.activity:activity-compose:1.9.2")

    // Coroutines / Flow
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1")

    // Encrypted offline queue
    implementation("net.zetetic:sqlcipher-android:4.5.4")
    implementation("androidx.sqlite:sqlite-ktx:2.4.0")

    // QR generation
    implementation("com.google.zxing:core:3.5.3")

    // Protobuf (javalite runtime comes from the protobuf plugin below)
    implementation("com.google.protobuf:protobuf-javalite:4.26.1")

    // JNA — loads the Rust cdylib produced by scripts/build-uniffi.sh
    implementation("net.java.dev.jna:jna:5.14.0@aar")

    // JVM unit tests
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.8.1")
}

protobuf {
    protoc {
        artifact = "com.google.protobuf:protoc:4.26.1"
    }
    generateProtoTasks {
        all().forEach { task ->
            task.builtins {
                create("java") {
                    option("lite")
                }
            }
        }
    }
}

// Empacota a biblioteca Rust (libtempo_app.so, gerada pelo cargo-ndk em src/main/jniLibs)
// junto com o shim Java mínimo e os recursos do Android.
plugins {
    id("com.android.application")
}

android {
    namespace = "com.projetotempo"
    compileSdk = 36
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "com.projetotempo"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            // Assinado com a chave de debug para instalar localmente.
            // Para publicar, configure uma chave própria (veja o README).
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // Um APK por arquitetura (RNF14) + um universal.
    // armeabi-v7a (32 bits) não é gerado: o Skia usado pelo Slint não tem binário pronto para ele.
    splits {
        abi {
            isEnable = true
            reset()
            include("arm64-v8a", "x86_64")
            isUniversalApk = true
        }
    }

    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
    }
}

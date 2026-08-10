plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.kotlin.serialization)
    alias(libs.plugins.android.kmp.library)
}

// Test-only helpers (contract fixture loading, fake transports, deterministic
// randomness). Production modules must never depend on this one; the boundary
// check in scripts/check-mobile-architecture.mjs enforces that.
kotlin {
    jvmToolchain(17)

    jvm()

    android {
        namespace = "com.bitfun.mobile.core.testing"
        compileSdk = libs.versions.androidCompileSdk.get().toInt()
        minSdk = libs.versions.androidMinSdk.get().toInt()
    }

    iosArm64()
    iosSimulatorArm64()

    sourceSets {
        commonMain.dependencies {
            api(kotlin("test"))
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.kotlinx.serialization.json)
        }
    }
}

import org.jetbrains.kotlin.gradle.plugin.mpp.apple.XCFramework

plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.kotlin.serialization)
    alias(libs.plugins.android.kmp.library)
    alias(libs.plugins.skie)
}

// The platform seam. Everything exported to Android and iOS UI code is a
// UiState or an Intent declared here; no module above this one is visible to
// the apps.
kotlin {
    jvmToolchain(17)

    // This is the module Swift will see, so every public declaration must be
    // deliberate. Explicit API mode makes the compiler demand the visibility
    // keyword and an explicit return type on each one — which is also what lets
    // scripts/check-mobile-architecture.mjs trust a `public` keyword scan when
    // it enforces design doc section 4.1.
    explicitApi()

    jvm()

    android {
        namespace = "com.openbitfun.mobile.core.feature"
        compileSdk = libs.versions.androidCompileSdk.get().toInt()
        minSdk = libs.versions.androidMinSdk.get().toInt()
        withHostTest {}
    }

    val xcframework = XCFramework("OpenBitFunMobileCore")
    val appleTargets = listOf(iosArm64(), iosSimulatorArm64())
    appleTargets.forEach { target ->
        target.binaries.framework {
            baseName = "OpenBitFunMobileCore"
            isStatic = true
            transitiveExport = true
            export(project(":core-domain"))
            xcframework.add(this)
        }
    }

    sourceSets {
        commonMain.dependencies {
            api(project(":core-domain"))
            // Composition root: this is where the ports declared in
            // :core-domain get their real adapters.
            implementation(project(":core-transport"))
            implementation(project(":core-persistence"))
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.kotlinx.serialization.json)
            // Calendar arithmetic for `SessionTimePresentation`: the reader's
            // zone decides what date an old timestamp falls on. `implementation`
            // because no public type here exposes one — the results are Ints.
            implementation(libs.kotlinx.datetime)
        }
        commonTest.dependencies {
            implementation(kotlin("test"))
            implementation(project(":core-testing"))
            implementation(libs.kotlinx.coroutines.test)
            implementation(libs.turbine)
            // The pairing tests run the real cipher against a mock relay, so a
            // broken state mapping fails here rather than on a device.
            implementation(libs.ktor.client.mock)
            implementation(libs.kotlinx.serialization.json)
        }
    }
}

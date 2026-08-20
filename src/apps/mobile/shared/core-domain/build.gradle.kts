plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.android.kmp.library)
}

kotlin {
    jvmToolchain(17)

    // :core-feature re-exports this module with api(), so these public types
    // land on the Swift-facing surface too — the same reason core-feature turns
    // this on.
    explicitApi()

    jvm()

    android {
        namespace = "com.bitfun.mobile.core.domain"
        compileSdk = libs.versions.androidCompileSdk.get().toInt()
        minSdk = libs.versions.androidMinSdk.get().toInt()
        withHostTest {}
    }

    iosArm64()
    iosSimulatorArm64()

    sourceSets {
        commonMain.dependencies {
            // Pure logic: the timeline state machine, projectors and session
            // policy. Transport and persistence are adapters behind ports
            // declared here and wired by :core-feature, so depending on them
            // from this module is a boundary violation.
            api(project(":core-protocol"))
            // The relay leaves a few fields as raw JSON (`tool_input`,
            // `tool_output`), and reading them is policy, so it happens here.
            // `implementation`, not `api`: no public type in this module is a
            // JSON type, so this stays off the Swift-facing surface.
            implementation(libs.kotlinx.serialization.json)
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.kotlinx.datetime)
        }
        commonTest.dependencies {
            implementation(kotlin("test"))
            implementation(project(":core-testing"))
            implementation(libs.kotlinx.coroutines.test)
            implementation(libs.turbine)
        }
    }
}

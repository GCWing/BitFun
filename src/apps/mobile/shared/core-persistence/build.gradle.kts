plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.android.kmp.library)
    alias(libs.plugins.sqldelight)
    alias(libs.plugins.kotlin.serialization)
}

sqldelight {
    databases {
        create("MobileDatabase") {
            packageName.set("com.openbitfun.mobile.core.persistence.db")
        }
    }
}

kotlin {
    jvmToolchain(17)

    jvm()

    android {
        namespace = "com.openbitfun.mobile.core.persistence"
        compileSdk = libs.versions.androidCompileSdk.get().toInt()
        minSdk = libs.versions.androidMinSdk.get().toInt()
        withHostTest {}
        withDeviceTest {
            instrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        }
    }

    iosArm64()
    iosSimulatorArm64()

    sourceSets {
        commonMain.dependencies {
            api(project(":core-protocol"))
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.kotlinx.serialization.json)
            implementation(libs.multiplatform.settings)
            implementation(libs.sqldelight.runtime)
        }
        androidMain.dependencies {
            implementation(libs.sqldelight.android.driver)
        }
        getByName("androidDeviceTest") {
            dependencies {
                implementation(kotlin("test"))
                implementation(libs.androidx.test.runner)
            }
        }
        iosMain.dependencies {
            implementation(libs.sqldelight.native.driver)
        }
        jvmMain.dependencies {
            implementation(libs.sqldelight.sqlite.driver)
        }
        commonTest.dependencies {
            implementation(kotlin("test"))
            implementation(libs.kotlinx.coroutines.test)
        }
        jvmTest.dependencies {
            implementation(libs.sqldelight.sqlite.driver)
        }
    }
}

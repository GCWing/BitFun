pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
    // One catalog for the whole mobile stack. The app cannot drift onto a
    // different Kotlin or AGP than the modules it links against, because there
    // is no second place to write a version down.
    versionCatalogs {
        create("libs") {
            from(files("../shared/gradle/libs.versions.toml"))
        }
    }
}

rootProject.name = "bitfun-android"

// The dependency points this way and only this way: the app includes the
// shared build, the shared build knows nothing about the app.
includeBuild("../shared")

include(":app")

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

// Every module pins jvmToolchain(17). Without a resolver, a machine whose
// Gradle JVM is 21 or 25 silently produces different bytecode than CI; with it,
// Gradle fetches JDK 17 itself and the output is identical everywhere.
plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "bitfun-mobile-shared"

// Phase 1 fills protocol / crypto / transport / testing.
// The remaining modules exist as skeletons so the dependency graph and the
// architecture boundary check are in place from the start.
include(":core-protocol")
include(":core-crypto")
include(":core-transport")
include(":core-persistence")
include(":core-domain")
include(":core-feature")
include(":core-testing")

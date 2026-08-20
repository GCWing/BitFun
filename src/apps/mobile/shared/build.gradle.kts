// Plugins are resolved here once and applied in the modules that need them, so
// every module agrees on a single version without repeating the classpath.
// The platform apps consume this build as an included build, and Gradle
// substitutes module coordinates for its projects — so the projects need
// coordinates. Nothing is published from here; the version only distinguishes
// one local build from another.
allprojects {
    group = "com.bitfun.mobile"
    version = "0.1.0"
}

plugins {
    alias(libs.plugins.kotlin.multiplatform) apply false
    alias(libs.plugins.kotlin.serialization) apply false
    alias(libs.plugins.android.kmp.library) apply false
    alias(libs.plugins.skie) apply false
}

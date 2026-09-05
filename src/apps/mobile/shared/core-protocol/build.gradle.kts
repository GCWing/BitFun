plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.kotlin.serialization)
    alias(libs.plugins.android.kmp.library)
}

// The contract fixtures live outside this build, at
// src/shared/relay-protocol-contract-fixtures, so the desktop peer and the other
// mobile clients can assert against the same bytes. commonTest has no portable
// way to read a file, so the JSON is embedded into a generated Kotlin source
// instead of loaded at runtime.
val contractFixturesDir = rootProject.layout.projectDirectory
    .dir("../../../shared/relay-protocol-contract-fixtures").asFile
val generatedFixturesDir = layout.buildDirectory.dir("generated/relayFixtures/kotlin")

val generateRelayContractFixtures = tasks.register("generateRelayContractFixtures") {
    val inputDir = contractFixturesDir
    val outputDir = generatedFixturesDir
    inputs.dir(inputDir).withPropertyName("contractFixtures")
    outputs.dir(outputDir).withPropertyName("generatedSources")
    doLast {
        val target = outputDir.get().asFile
            .resolve("com/openbitfun/mobile/core/protocol/RelayContractFixtures.kt")
        target.parentFile.mkdirs()
        val fixtures = inputDir.listFiles { file -> file.extension == "json" }
            .orEmpty()
            .sortedBy { it.name }
        val source = buildString {
            appendLine("package com.openbitfun.mobile.core.protocol")
            appendLine()
            appendLine("// Generated from src/shared/relay-protocol-contract-fixtures by the")
            appendLine("// generateRelayContractFixtures Gradle task. Do not edit by hand.")
            appendLine("internal val RelayContractFixtures: Map<String, String> = mapOf(")
            for (fixture in fixtures) {
                // Raw strings interpolate, and a captured payload may legitimately
                // contain a dollar sign.
                val escaped = fixture.readText().replace("$", "\${'$'}")
                appendLine("    \"${fixture.nameWithoutExtension}\" to \"\"\"$escaped\"\"\",")
            }
            appendLine(")")
        }
        target.writeText(source)
    }
}

kotlin {
    // jvm() exists so the shared logic can be tested on the host and in CI
    // without a device or simulator. It is not a shipped target.
    jvmToolchain(17)

    jvm()

    android {
        namespace = "com.openbitfun.mobile.core.protocol"
        compileSdk = libs.versions.androidCompileSdk.get().toInt()
        minSdk = libs.versions.androidMinSdk.get().toInt()
        withHostTest {}
    }

    iosArm64()
    iosSimulatorArm64()

    sourceSets {
        commonMain.dependencies {
            implementation(libs.kotlinx.serialization.json)
        }
        commonTest {
            kotlin.srcDir(generateRelayContractFixtures)
            dependencies {
                implementation(kotlin("test"))
                implementation(libs.kotlinx.serialization.json)
            }
        }
    }
}

plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.kotlin.serialization)
    alias(libs.plugins.android.kmp.library)
}

val sharedArgon2Dir = rootProject.layout.projectDirectory.dir("../../../shared/argon2")
val argon2InteropDir = layout.projectDirectory.dir("src/nativeInterop/cinterop")

fun registerArgon2Archive(
    targetName: String,
    targetTriple: String,
): TaskProvider<Exec> = tasks.register<Exec>("buildOpenBitFunArgon2${targetName.replaceFirstChar(Char::uppercase)}") {
    val outputDir = layout.buildDirectory.dir("native/argon2/$targetName")
    val sources = listOf(
        "argon2.c",
        "core.c",
        "encoding.c",
        "ref.c",
        "blake2/blake2b.c",
    )
    inputs.dir(sharedArgon2Dir)
    inputs.file(argon2InteropDir.file("openbitfun_argon2_wrapper.c"))
    outputs.file(outputDir.map { it.file("libopenbitfun_argon2.a") })
    doFirst {
        val directory = outputDir.get().asFile
        directory.mkdirs()
        val include = sharedArgon2Dir.asFile.absolutePath
        val objects = sources.mapIndexed { index, source -> directory.resolve("argon2-$index.o") }
        val wrapper = directory.resolve("openbitfun-argon2-wrapper.o")
        // The target triple is the single source of platform and deployment
        // metadata, avoiding conflicting minimum-version flags.
        val compile = listOf(
            "clang", "-target", targetTriple,
            "-DARGON2_NO_THREADS", "-I$include", "-I${sharedArgon2Dir.dir("blake2").asFile.absolutePath}",
            "-O2", "-fvisibility=hidden", "-c",
        )
        fun run(command: List<String>) {
            val process = ProcessBuilder(command).inheritIO().start()
            check(process.waitFor() == 0) { "Argon2 native command failed: ${command.first()}" }
        }
        sources.zip(objects).forEach { (source, objectFile) ->
            run(compile + listOf(sharedArgon2Dir.file(source).asFile.absolutePath, "-o", objectFile.absolutePath))
        }
        run(compile + listOf(argon2InteropDir.file("openbitfun_argon2_wrapper.c").asFile.absolutePath, "-o", wrapper.absolutePath))
        run(listOf("ar", "rcs", directory.resolve("libopenbitfun_argon2.a").absolutePath) + objects.map(File::getAbsolutePath) + wrapper.absolutePath)
    }
    commandLine("true")
}

val iosArgon2Archive = registerArgon2Archive("iosArm64", "arm64-apple-ios16.0")
val iosSimulatorArgon2Archive = registerArgon2Archive("iosSimulatorArm64", "arm64-apple-ios15.0-simulator")

kotlin {
    jvmToolchain(17)
    compilerOptions {
        freeCompilerArgs.add("-Xexpect-actual-classes")
    }

    jvm()

    android {
        namespace = "com.openbitfun.mobile.core.crypto"
        compileSdk = libs.versions.androidCompileSdk.get().toInt()
        minSdk = libs.versions.androidMinSdk.get().toInt()
        withHostTest {}
        // The host test compiles androidMain — so it does exercise BouncyCastle —
        // but it runs on the desktop JVM, not on ART. This module is the one place
        // where that difference can bite, so the same suite also runs on a device.
        withDeviceTest {
            instrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        }
    }

    iosArm64 {
        compilations.getByName("main").cinterops.create("openbitfunArgon2") {
            defFile(argon2InteropDir.file("openbitfun_argon2.def"))
            includeDirs(sharedArgon2Dir, argon2InteropDir)
            extraOpts("-libraryPath", layout.buildDirectory.dir("native/argon2/iosArm64").get().asFile.absolutePath)
        }
    }
    iosSimulatorArm64 {
        compilations.getByName("main").cinterops.create("openbitfunArgon2") {
            defFile(argon2InteropDir.file("openbitfun_argon2_simulator.def"))
            includeDirs(sharedArgon2Dir, argon2InteropDir)
            extraOpts("-libraryPath", layout.buildDirectory.dir("native/argon2/iosSimulatorArm64").get().asFile.absolutePath)
        }
        // A non-standard Xcode selected explicitly through DEVELOPER_DIR needs its
        // Swift compatibility archives when linking the simulator DEBUG test.
        // Without that environment variable, leave toolchain discovery to Kotlin.
        System.getenv("DEVELOPER_DIR")?.takeIf(String::isNotBlank)?.let { developerDir ->
            val swiftSimulatorLibraries = file(developerDir)
                .resolve("Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/iphonesimulator")
            binaries.getTest("DEBUG").linkerOpts("-L${swiftSimulatorLibraries.absolutePath}")
        }
    }

    sourceSets {
        commonMain.dependencies {
            api(project(":core-protocol"))
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.kotlinx.serialization.json)
            implementation(libs.cryptography.core)
            implementation(libs.cryptography.random)
        }
        commonTest.dependencies {
            implementation(kotlin("test"))
            implementation(libs.kotlinx.coroutines.test)
        }
        // The device suite *is* the common suite, recompiled for ART. Sharing the
        // directory rather than copying a smoke subset keeps the two from drifting.
        getByName("androidDeviceTest") {
            kotlin.srcDir("src/commonTest/kotlin")
            dependencies {
                implementation(kotlin("test"))
                implementation(libs.kotlinx.coroutines.test)
                implementation(libs.androidx.test.runner)
            }
        }
        // The JDK provider has no XDH on Android, so BouncyCastle supplies it.
        androidMain.dependencies {
            implementation(libs.cryptography.provider.jdk)
            implementation(libs.bouncycastle.prov)
        }
        jvmMain.dependencies {
            implementation(libs.cryptography.provider.jdk)
            implementation(libs.bouncycastle.prov)
        }
        iosMain.dependencies {
            implementation(libs.cryptography.provider.cryptokit)
        }
    }
}

tasks.matching { it.name == "cinteropOpenBitFunArgon2IosArm64" }.configureEach { dependsOn(iosArgon2Archive) }
tasks.matching { it.name == "cinteropOpenBitFunArgon2IosSimulatorArm64" }.configureEach { dependsOn(iosSimulatorArgon2Archive) }

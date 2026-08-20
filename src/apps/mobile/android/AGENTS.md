# Android App Guide

This directory owns the native Android application and is an independent
Gradle build root over `../shared`.

## Java Runtime

Use the JetBrains Runtime bundled with Android Studio. The system `java`
launcher may exist without a configured macOS Java runtime, so do not rely on
it for Gradle commands in this directory.

```bash
export JAVA_HOME='/Applications/Android Studio.app/Contents/jbr/Contents/Home'
"$JAVA_HOME/bin/java" -version
```

Android Studio uses the standard macOS SDK location `~/Library/Android/sdk`.
Export that location once so commands do not bake a machine-specific absolute
home directory into repository documentation:

```bash
export ANDROID_SDK_ROOT="$HOME/Library/Android/sdk"
"$ANDROID_SDK_ROOT/platform-tools/adb" devices
```

For a one-off command, keep the runtime selection local to that process:

```bash
JAVA_HOME='/Applications/Android Studio.app/Contents/jbr/Contents/Home' ./gradlew :app:assembleDebug
```

## Focused Verification

Run commands from `src/apps/mobile/android` unless a command says otherwise.

```bash
# Compile the Android app.
JAVA_HOME='/Applications/Android Studio.app/Contents/jbr/Contents/Home' ./gradlew :app:assembleDebug

# Run Android host tests.
JAVA_HOME='/Applications/Android Studio.app/Contents/jbr/Contents/Home' ./gradlew :app:testDebugUnitTest

# Run instrumentation tests on an attached emulator or device.
JAVA_HOME='/Applications/Android Studio.app/Contents/jbr/Contents/Home' ./gradlew :app:connectedDebugAndroidTest

# Run shared JVM tests after core-feature changes.
cd ../shared
JAVA_HOME='/Applications/Android Studio.app/Contents/jbr/Contents/Home' ./gradlew jvmTest
```

Use the narrowest command covering the changed behavior. Keep credentials,
keystores, signing values, device identifiers, and local SDK paths out of the
repository.

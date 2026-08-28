# AGENTS.md

Native mobile applications are product entrypoints under `src/apps/mobile`.

## Boundaries

- Keep Android, iOS, and HarmonyOS lifecycle and platform API usage inside the
  corresponding platform directory.
- Keep reusable product logic platform-agnostic and expose it through stable
  contracts or adapters.
- Do not import implementation details from `src/apps/desktop` or
  `src/web-ui`.
- Treat remote workspace support as part of feature design. Gate unsupported
  behavior with a clear user-facing state.
- Keep credentials, signing files, provisioning profiles, device identifiers,
  and local SDK paths out of the repository.
- Add platform-specific build and verification commands here when a native
  toolchain is selected.

## Directory Ownership

| Directory | Ownership |
|---|---|
| `android/` | Android app, resources, lifecycle, and adapters |
| `ios/` | iOS app, resources, lifecycle, and adapters |
| `harmonyos/` | HarmonyOS app, resources, lifecycle, and adapters |
| `shared/` | Kotlin Multiplatform core: protocol, crypto, transport, persistence, domain, feature stores |
| `design-system/` | HarmonyOS-derived mobile tokens, component contracts, deterministic preview scenarios, and the desktop comparison surface |

## Native UI Contract

HarmonyOS is the visual reference implementation. Stable colors, typography,
geometry, breakpoints, motion durations, component anatomy, and comparison
scenarios are recorded under `design-system/`; Android and iOS consume generated
native constants but continue to render with Compose and SwiftUI respectively.
Do not introduce a shared cross-platform renderer or make generated files the
source of truth.

- Change the HarmonyOS implementation and the source contract together when a
  stable visual fact changes.
- Run `pnpm run mobile:ui:generate` after contract changes and commit the
  generated native files.
- Run `pnpm run mobile:ui:check` before pushing to reject generated drift.
- Use `pnpm run mobile:ui:preview` for the local three-column HarmonyOS / Android
  / iOS comparison surface. Native captures belong under the documented
  `design-system/preview/snapshots/` convention and are local evidence unless a
  fixture is intentionally reviewed into the repository.
- Keep safe areas, keyboard behavior, accessibility, navigation gestures, and
  platform presentation primitives in each native app.

## Shared Core

`shared/` holds the Kotlin Multiplatform modules that `android/` and `ios/`
build on. `core-feature` is the platform seam: everything the apps see is a
UiState or an Intent declared there, and no module above it is visible to them.

- `android/` and `ios/` may depend on `shared/`. `shared/` must not depend on
  any platform directory, and `harmonyos/` and `shared/` do not depend on each
  other.
- `android/` is its own build root and pulls `shared/` in with
  `includeBuild("../shared")`, so the arrow points one way only: the app knows
  the shared build, never the reverse. That makes app dependencies read as
  coordinates — `implementation("com.bitfun.mobile:core-feature")` — rather than
  `project(":core-feature")`. Both spellings are subject to the seam rule below.
- `core-feature` is the only shared module the apps may reference. Everything
  crossing that seam is a UiState or an Intent, never a repository or a DTO.
- `core-domain` is pure logic. Transport and persistence are adapters behind
  ports it declares, wired by `core-feature`.
- Shared code returns typed states, never localized display strings; the apps
  own all user-facing text.
- Run `pnpm run mobile:architecture` before pushing. It enforces the rules
  above, mirroring `pnpm run harmony:architecture`.
- Build and test from `shared/`: `./gradlew jvmTest` for host logic,
  `./gradlew assembleAndroidMain testAndroidHostTest` for Android, and
  `./gradlew compileKotlinIosSimulatorArm64` for iOS. Run iOS pure Swift
  infrastructure checks from the repository root with
  `(cd src/apps/mobile/ios && ./Testing/run-pure-swift-tests.sh)`.
  `local.properties` holds
  the local SDK path and is not committed.
- The Android app builds from `android/`: `./gradlew :app:assembleDebug` and
  `:app:installDebug`; release verification is `./gradlew :app:assembleRelease`.
  Release signing is enabled only when all four `BITFUN_ANDROID_KEYSTORE`,
  `BITFUN_ANDROID_KEYSTORE_PASSWORD`, `BITFUN_ANDROID_KEY_ALIAS`, and
  `BITFUN_ANDROID_KEY_PASSWORD` environment variables are present. Signing
  files and values must never be committed. AGP 9 compiles Kotlin itself — applying
  `org.jetbrains.kotlin.android` is an error, not a no-op, and
  `kotlin { jvmToolchain(...) }` is no longer available in an app module.
- Crypto and transport also run on a device: `./gradlew
  :core-crypto:connectedAndroidDeviceTest :core-transport:connectedAndroidDeviceTest`
  from `shared/`, with an emulator or handset attached. Those suites compile the
  same `commonTest` sources onto ART; they are not in CI, so run them by hand
  when touching either module.

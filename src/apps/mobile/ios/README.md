# OpenBitFun iOS

Native SwiftUI client for the mobile conversation surface. The initial iOS
entrypoint is intentionally small, but it is a real Xcode application and uses
the same geometry as the HarmonyOS reference: 76pt conversation header, 44pt
circle controls, 16pt content margins, the 48pt connection strip, and the
floating composer with 52pt collapsed height.

## Project layout

- `OpenBitFun/App/`: lifecycle, launch configuration, and composition root.
- `OpenBitFun/Features/Chat/`: conversation home, header, timeline bubbles, and composer.
- `OpenBitFun/Features/Remote/`: remote conversation home surfaces.
- `OpenBitFun/Features/Settings/`: app settings composition and reusable settings cards.
- `OpenBitFun/Features/Pairing/`: pairing sheet flow.
- `OpenBitFun/Features/Account/`: account settings and device rows.
- `OpenBitFun/Features/Shell/`: theme tokens, drawer, shell layout, and remote supporting surfaces.
- `OpenBitFun/Infrastructure/`: observable state, failure copy, and platform adapters.
- `OpenBitFun/Presentation/Models/`: SwiftUI-facing presentation DTOs.
- `OpenBitFun/Resources.xcassets/`: app icon and future native assets.

## Build and run

The repository does not change the machine-wide developer directory. Use the
Xcode copy in `~/Downloads` when it is the compatible version:

```bash
export DEVELOPER_DIR="$HOME/Downloads/Xcode.app/Contents/Developer"
"$DEVELOPER_DIR/usr/bin/xcodebuild" \
  -project OpenBitFun.xcodeproj -scheme OpenBitFun \
  -destination 'platform=iOS Simulator,id=1D7E5AA6-1AE9-4CAB-966B-A83B5F113B4A' \
  -derivedDataPath /tmp/OpenBitFun-iOS-Derived \
  CODE_SIGNING_ALLOWED=YES CODE_SIGNING_IDENTITY=- build
```

`MobileAppModel` is kept in `Infrastructure` so the SwiftUI views do not know
about transport or persistence. Local chat, pairing, and remote session state
are supplied by the generated `OpenBitFunMobileCore` framework from
`src/apps/mobile/shared/core-feature`; SwiftUI only maps the typed state to its
presentation model. Pairing accepts the desktop connection URL through the
connection sheet (the camera scanner remains a native adapter concern).

Run the platform-independent Swift infrastructure checks through the registered
focused entry point. It compiles production helpers together with their local
test executables; test mains are not part of the app target:

```bash
export DEVELOPER_DIR="$HOME/Downloads/Xcode.app/Contents/Developer"
./Testing/run-pure-swift-tests.sh
```

When the framework has not been built yet, generate it with the same compatible
toolchain before opening the Xcode project:

```bash
export JAVA_HOME="/Applications/DevEco-Studio.app/Contents/jbr/Contents/Home"
export PATH="$JAVA_HOME/bin:$PATH"
export DEVELOPER_DIR="$HOME/Downloads/Xcode.app/Contents/Developer"
../../../../gradlew :core-feature:assembleOpenBitFunMobileCoreDebugXCFramework
```

The shell includes both product surfaces already: Local opens the HarmonyOS
welcome prompts, Remote has the disconnected desktop state and connection
action, and the unified drawer mirrors the HarmonyOS recent-chat, device,
workspace, chat, and settings sections. A connected preview exposes the
remote empty home through the same conversation chrome.

For repeatable simulator captures, pass `--remote`, `--connected`, `--drawer`,
`--settings`, `--remote-settings`, `--remote-view-settings`, `--remote-view-density`, `--model-settings`, `--composer-model-picker`, `--pairing`, `--pairing-manual`, `--pairing-account`, `--remote-create`, `--remote-create-workspace-picker`, `--remote-chat-section`, `--project-create-menu`, `--file-preview`, `--session-actions`, `--sidebar-actions`, `--local-actions`, and/or
`--account-login` or `--account-profile` after the bundle identifier in `simctl launch`. The local
actions flag can be combined with the session-actions flag; the account-login
flag opens a deterministic signed-out surface without storing credentials. These launch flags
select deterministic inspection states; normal launches use the live KMP
pairing/session stores.

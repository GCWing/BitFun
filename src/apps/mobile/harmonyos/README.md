# OpenBitFun HarmonyOS

Native HarmonyOS controller for OpenBitFun desktop and CLI hosts on phones,
foldables, and tablets. Sign in with GitHub, select an account device, then send
tasks and view results from that host. The phone does not run an Agent Runtime or
store model-provider configuration. Model selection applies to the selected host.

Device QR codes identify a target; the authenticated account directory authorizes
access. Old room records and local conversation data remain on disk during an
upgrade, but do not automatically reconnect or start a local runtime.

GitHub usernames and avatars are presentation metadata loaded from GitHub's public
user-by-ID API without forwarding account credentials. The phone caches them for
24 hours in its encrypted account store, scoped to the authenticated GitHub ID.
Existing signed-in installs are enriched on startup. Offline or rate-limited
profile requests retain the session and cached display; device authorization
continues to use the immutable ID issued by Relay.

## Project Layout

- `AppScope/`: application metadata and shared resources.
- `entry/src/main/ets/`: ArkTS application code.
- `entry/src/main/resources/`: entry-module resources.
- `entry/src/test/`: local unit tests.
- `entry/src/ohosTest/`: device tests.
- `tools/fake-relay.mjs`: local relay simulator for UI and protocol testing.

## Development

Open this directory as a project in DevEco Studio. Install dependencies through
OHPM before building the `entry` module.

On macOS with DevEco Studio installed in its default location:

```bash
source scripts/ohos-env.sh
"$OHPM" install
"$HVIGORW" --mode module -p module=entry assembleHap --no-daemon
```

Signing configuration is intentionally not stored in the repository. Configure
a local signing identity in DevEco Studio when installing the app on a device.

The current project targets HarmonyOS `6.1.1(24)` and supports
`6.0.1(21)` or newer on phone and tablet devices.

## Built-in MiniApps

Choose **MiniApps** from the sidebar or home to open Gomoku, Regex Playground,
or Daily Divination. These bundled pages run locally in ArkWeb without signing
in, pairing, a desktop connection, or internet access. Storage and clipboard
writes use the phone. Data is isolated by app and retained across app upgrades;
it is not synchronized with desktop MiniApps. Closing a page discards its
unsaved page state. Changing desktop targets has no effect on these local tools.

Every IDE/CLI HAP build runs `miniapps/generate.cjs` through
`entry/hvigorfile.ts`. It packages the existing shared MiniApp sources and
canonical appearance tokens into generated raw resources; do not edit those
outputs. `miniapps/mobile.css` owns phone-only layout adaptation. The pages
follow the app language and system light/dark appearance. They resize in place
for compact, wide, and folded/unfolded layouts.

The sandboxed iframe has no network access or local-file access. The native
bridge permits only per-app storage keys and clipboard writes; it has no generic
invoke, Worker, shell, workspace, AI, or Agent API. Local records are replaced
atomically, and malformed records are retained and return an error. These local
tools do not read any remote workspace or peer content, and do not execute jobs
on CLI/Detached Dispatch hosts. Existing Remote Connect protocols are unchanged.
Desktop catalogs, page transfer, and remote MiniApp execution are deferred.

Run the local test and HAP build commands in `AGENTS.md`, plus:

```bash
node --test miniapps/*.test.cjs
```

Before claiming device verification, exercise local/offline launches, saved data
after relaunch, clipboard, keyboard input, compact/wide layouts, light/dark
appearance, and a live resize/fold transition on supported hardware.

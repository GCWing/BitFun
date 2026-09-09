# OpenBitFun HarmonyOS

Native HarmonyOS client for OpenBitFun. The application provides general chat and
remote control of OpenBitFun desktop sessions on phone and tablet devices.

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

## MiniApp H5 preview

Connect to an updated desktop through account-device connection or QR pairing,
then choose **MiniApps** in the sidebar or remote home. Select an installed app.
Signing into an account alone is not a desktop connection: open a remote
conversation on the desired desktop first if no control target is connected.
Its compiled HTML/CSS/JavaScript runs in ArkWeb on the phone; storage and Worker
calls use the existing encrypted connection to the desktop. No market upload,
public hosting port, or mobile-web page is involved. Try the built-in Gomoku app
first: board interactions run locally and its saved statistics live on desktop.

This first version supports `app.call`, `app.storage.get/set`, and the existing
permission-checked host primitives for apps with Node disabled. The phone never
grants filesystem permissions or inherits the desktop's current workspace;
workspace-dependent calls may report missing access. AI/Agent, desktop dialogs,
clipboard, notifications, deck export and chat integration are unsupported and
return errors. MiniApp pages use the default light appearance and must adapt
their own content to narrow screens. The compiled page transfer limit is 2 MiB.

The desktop advertises `miniapp_h5_v1` only when its adapter is registered.
Each catalog load refreshes the advertisement so a desktop restart or upgrade
does not leave the phone using capability information from an older handshake.
Older hosts keep their existing chat behavior and show an unsupported state for
MiniApps. SSH workspaces and Peer Device Mode are explicitly rejected; CLI and
Detached Dispatch hosts have no mobile MiniApp adapter. Switching connections
invalidates an open page's ability to issue calls; reopen the app after reconnect.
App version changes also require reopening. Calls are not automatically retried,
and a timed-out Worker operation may still finish on desktop. Temporary page
state is separate from any desktop window and is discarded when the page closes.

The ArkWeb wrapper isolates H5 in a sandboxed iframe. The native bridge treats
all frame input as untrusted and binds calls to the selected app/version and
connection generation; it does not expose a generic desktop invoke API.

Focused coverage lives in `entry/src/test/MiniAppUnit.test.ets` (device switching,
late responses after close, unsupported calls and markup isolation). Run the
local test and HAP build commands in `AGENTS.md`; for desktop protocol changes,
also run:

```bash
cargo test --locked -p openbitfun-services-integrations --no-default-features --features remote-connect --lib remote_connect::miniapp::tests::
cargo check -p openbitfun-desktop --lib --no-default-features
```

Before treating this preview as device-verified, exercise QR and account-device
connections with a real desktop, compact and wide layouts, light/dark app chrome,
keyboard input, and a live folded/unfolded transition on supported hardware.

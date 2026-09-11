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

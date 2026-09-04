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

# Native preview captures

The desktop comparison tool looks for optional PNG captures at:

```text
snapshots/<scenario-id>/<platform>.png
```

`platform` is `harmony`, `android`, or `ios`. For example:

```text
snapshots/connected-conversation/harmony.png
snapshots/connected-conversation/android.png
snapshots/connected-conversation/ios.png
```

Use the matching generated native preview gallery and the same scenario id.
Captures can also be selected directly from each column in the browser. Local
captures are visual evidence and should not be committed unless they are being
reviewed as deliberate regression fixtures.

Android capture evidence is produced by on-device instrumentation tests, not
committed PNGs. HarmonyOS captures remain unproven until a device and toolchain
are available.

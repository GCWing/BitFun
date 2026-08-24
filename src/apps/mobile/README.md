# BitFun Native Mobile Apps

This directory contains the native mobile product surfaces for BitFun:

- `android/`: Android application code and resources.
- `ios/`: iOS application code and resources.
- `harmonyos/`: HarmonyOS application code and resources.

Each platform directory owns its native UI, lifecycle, permissions, packaging,
and platform adapters. Product logic and stable contracts should remain in the
platform-agnostic Rust layers and be exposed to these apps through explicit
interfaces.

## Shared visual contract

HarmonyOS is the current visual baseline. The source contract in
[`design-system/`](design-system/README.md) records the stable HarmonyOS colors,
type scale, geometry, breakpoints, motion, component anatomy, and deterministic
preview scenarios. A generator emits native constants for ArkUI, Compose, and
SwiftUI; each platform still owns its native component implementation.

```bash
pnpm run mobile:ui:generate
pnpm run mobile:ui:check
pnpm run mobile:ui:preview
```

The preview command opens a local three-column desktop surface for HarmonyOS,
Android, and iOS. It renders the same scenario from the contract and can overlay
native simulator or IDE-preview captures for pixel-level comparison.

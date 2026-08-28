# BitFun Mobile Design System

This directory is the source-neutral visual contract for the native HarmonyOS,
Android, and iOS applications. It owns stable visual facts and deterministic
preview scenarios; it does not implement a cross-platform renderer.

## Ownership

- `tokens/mobile-tokens.json`: the single source for semantic colors,
  typography, shared geometry, breakpoints, and motion durations.
- `components/mobile-components.json`: component anatomy, states, and the token
  roles each native implementation must consume.
- `scenarios/mobile-preview-scenarios.json`: deterministic states rendered by
  native preview galleries and the desktop comparison tool.
- `preview/`: the local three-column inspection surface. It can show the
  contract fallback immediately and accepts native screenshots for overlay or
  side-by-side inspection.

Generated platform files are checked in so IDE previews and native builds do
not require Node.js. Change the contract, then run:

```bash
pnpm run mobile:ui:generate
pnpm run mobile:ui:check
pnpm run mobile:ui:preview
```

Do not edit generated files by hand. Native components remain responsible for
safe areas, keyboard behavior, accessibility bridges, navigation gestures, and
platform presentation primitives.

## Typography

- Product text consumes the semantic roles in `tokens/mobile-tokens.json`;
  native components must not introduce literal text sizes.
- Keep the hierarchy shallow: display and page titles use the display/headline
  roles, row titles use title roles, reading text uses body roles, and compact
  metadata uses label roles. The smallest product-text role is `label_small`
  at 12 units; icon glyph sizing is independent of text sizing.
- Native hosts follow the system font-size preference with a documented maximum
  scale. Validate the standard size and at least one enlarged accessibility size
  without changing display zoom, and prefer wrapping or ellipsis over clipping.

## Simulator captures

The native galleries can be launched without changing the normal app path:

```bash
# Android (after installing the debug APK)
adb shell am force-stop com.bitfun.mobile.debug
adb shell am start \
  -n com.bitfun.mobile.debug/com.bitfun.mobile.app.MainActivity \
  --ez bitfun.design_preview true \
  --es bitfun.design_scenario connected-conversation

# iOS Simulator (after installing the simulator app)
xcrun simctl launch booted com.bitfun.mobile.ios \
  --design-preview connected-conversation

# HarmonyOS emulator (after installing a locally signed debug HAP)
hdc -t <emulator-tcp-target> shell aa force-stop com.bitfun.app
hdc -t <emulator-tcp-target> shell aa start \
  -a EntryAbility -b com.bitfun.app \
  --ps bitfunDesignPreview connected-conversation
```

Valid scenario ids come from `scenarios/mobile-preview-scenarios.json`. Save
captures using the convention documented in `preview/snapshots/README.md`, then
open the desktop comparison surface to inspect them beside the HarmonyOS
baseline.

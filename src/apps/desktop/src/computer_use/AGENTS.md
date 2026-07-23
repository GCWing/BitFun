# Computer Use (desktop host)

## Scope

Platform-specific automation for the unified `computer_use` tool lives under
`src/apps/desktop/src/computer_use/`. Shared contracts and tool orchestration
are in `src/crates/assembly/core` and `src/crates/execution/tool-contracts`.

## Platform maturity

| Platform | Tier | Capabilities |
|---|---|---|
| **macOS** | AX-first | Accessibility tree, background input, Skylight/window capture, menu shortcuts, interactive/visual views |
| **Windows** | AX-first | UI Automation tree, `PrintWindow` + WGC + BitBlt capture, MSAA for legacy VCL, background input |
| **Linux** | **Legacy only** | Full-screen / region screenshot, enigo pointer/keyboard (X11), AT-SPI locate + OCR. No AX-first APIs |

### Linux legacy layer

Linux is intentionally a **compatibility layer**, not parity with macOS/Windows.

**Available:** `screenshot`, `click` / `move` / `scroll` / `type` / `key_chord`,
`locate` (AT-SPI + OCR fallback).

**Unavailable (return `LINUX_LEGACY_AX_UNAVAILABLE`):** `get_app_state`,
`get_app_shortcuts`, all `app_*` actions, `interactive_*`, `visual_*`,
`list_apps` (returns empty), background-input flags.

Requires an interactive X11 session for input; Wayland-only setups may fail
with permission or coordinate errors surfaced to the agent.

## Module map

- `desktop_host/` — `ComputerUseHost` trait impl; entry for all actions
- `macos_*` / `windows_*` — platform AX, capture, list-apps, shortcuts
- `windows_capture.rs` — tiered capture: PrintWindow → WGC → BitBlt
- `windows_wgc_capture.rs` — Windows.Graphics.Capture (Direct3D11)
- `linux_ax_ui.rs` — AT-SPI locate (legacy)
- `screen_ocr.rs`, `ui_locate_common.rs` — shared OCR/locate helpers

## Windows capture fallback chain

When `PrintWindow` returns a mostly-black bitmap (DirectComposition / UWP):

1. Try **WGC** via `screenshot_window_via_wgc` (occlusion-immune)
2. Fall back to **screen-region BitBlt** (on-screen, non-occluded targets)

## Verification

```bash
cargo check -p bitfun-desktop
cargo test -p bitfun-desktop
```

Windows-only paths (`windows_wgc_capture`, UIA) compile on CI (`windows-latest`).

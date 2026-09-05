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

For observation payloads, OCR projection, AX filtering/digests, and browser
snapshot context, use the isolated compiler harness first. It compiles the
production Rust files by path, with the actual DTO source and no replacement
algorithms or native-host mocks. Cargo dependencies must already be cached
(`cargo fetch --locked` on a fresh machine).

```bash
node scripts/test-computer-use-context.mjs
cargo test -p openbitfun-desktop --lib context_integrity_tests
```

Native black-box fixtures use only a dedicated test window / rendered image:

```bash
# Node 22/24 and installed workspace dependencies; installed Chrome required.
# CHROME_PATH can select another Chromium executable.
node scripts/test-browser-snapshot.mjs --native-ocr
# macOS + Accessibility permission; creates and closes its own AppKit window.
node scripts/test-native-ax-context.mjs
```

The browser fixture runs production snapshot/resolver JavaScript in Chromium,
then passes the actual DOM payload to compiled Rust presentation tests. The
optional OCR step compiles the Desktop test target and invokes macOS Vision on
the rendered JPEG. The AX fixture compiles the Desktop test target and checks
native AX nodes, text, states, parent indices, cache entries, filtering, and DTO
round trips. These are local macOS checks, not Windows/Linux or remote evidence.

Observation invariants:

- Rectangle containment does not prove two controls have the same action.
- Projection uses content padding and authoritative global bounds; invalid
  geometry and OCR matches outside the content are not actionable coordinates.
- AX snapshot digests cover state and geometry as well as labels. A state-only
  change must not be mistaken for a failed action and trigger duplicate input.
- Stale interactive/visual indices must be returned to the caller for a fresh
  choice, never silently reused after rebuilding a different view.
- Element-budget omissions are reported in `omitted_element_count`, including
  when text rendering is disabled; focused controls survive budget selection.

```bash
cargo check -p openbitfun-desktop
cargo test -p openbitfun-desktop
```

Windows-only paths (`windows_wgc_capture`, UIA) compile on CI (`windows-latest`).

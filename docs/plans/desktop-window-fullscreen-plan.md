# Desktop Window Fullscreen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add OS-level fullscreen support for the BitFun Desktop main window without changing maximize, panel fullscreen, CLI/TUI rendering, or product runtime logic.

**Architecture:** Keep fullscreen as a Desktop shell capability owned by the Tauri/Web UI adapter layer. Extend the existing `useWindowControls` hook so maximize and fullscreen share focus restoration and state-sync helpers while keeping separate state, handlers, permissions, and comments.

**Tech Stack:** Tauri v2 window APIs, React hooks, BitFun `ShortcutManager` where appropriate, Vitest for pure shortcut helper coverage, existing locale JSON files.

---

## Product Scope

This feature means OS window fullscreen:

- Windows/Linux: pressing `F11` asks the operating system to put the whole BitFun Desktop window into fullscreen.
- macOS: pressing `Control+Command+F` uses the platform fullscreen convention.
- The BitFun internal layout remains the same: NavBar, SceneBar, panels, chat, editor, terminal, browser, and diff surfaces continue to render.

This feature is not:

- `maximize()` / `unmaximize()`.
- Internal panel fullscreen, editor Zen Mode, or diff preview fullscreen.
- CLI/TUI alternate-screen rendering.
- A persisted workspace/session state.

## Competitor Notes

- Claude Code Desktop treats desktop as a workbench with panes, terminal, file editor, preview, and computer use. This supports adding OS window fullscreen as shell chrome behavior, not as model/runtime logic.
- Claude Code CLI fullscreen rendering is an alternate terminal renderer using the terminal's drawing surface; its docs explicitly say it is unrelated to maximizing the terminal window.
- Codex CLI launches into a full-screen terminal UI, while the Codex app handles app/window workflows separately.
- OpenCode TUI uses TUI keybinds, mouse capture, and terminal-aware layout settings. That reinforces keeping CLI fullscreen separate from Desktop OS fullscreen.

## State Model

| State | Meaning | Owner |
|---|---|---|
| Normal | Ordinary Desktop window | OS/Tauri |
| Maximized | Desktop window fills available work area but remains a normal window | existing maximize logic |
| Fullscreen | OS-level fullscreen window state | new fullscreen logic |
| Minimized | Hidden/minimized app window | existing minimize logic |

`isMaximized` and `isFullscreen` must remain independent. Callers must not use maximize as a proxy for fullscreen.

## Remote Compatibility

OS window fullscreen is a local Desktop shell capability. It must not be modeled as a remote workspace, SSH session, agent runtime, or transport command.

- Remote SSH workspaces continue to render inside the same local Desktop window; toggling fullscreen changes only that local shell window.
- The shortcut path is gated by native window-control support and does not add remote network, SSH, file-tree, terminal, or agent-loop round trips.
- If a future remote-control product surface needs to control a client window's fullscreen state, it should be exposed as an explicit client shell capability with capability negotiation, not by reusing workspace/session APIs.

## Milestone 1: Desktop Shell Fullscreen

Risk: Medium. The code path is narrow, but platform fullscreen behavior differs across Windows, Linux window managers, and macOS Spaces.

- [x] Add Tauri permission for frontend `is_fullscreen` state sync. `set_fullscreen` stays in the desktop host command and does not need a frontend window permission. Risk: Low.
- [x] Extend `useWindowControls` with `isFullscreen` and `handleToggleFullscreen`. Risk: Medium.
- [x] Keep fullscreen comments explicit: OS fullscreen is not maximize and not panel fullscreen. Risk: Low.
- [x] Reuse shared focus restoration and state-sync helpers instead of copying maximize logic. Risk: Medium.
- [x] Register a Desktop-only fullscreen shortcut. Risk: Medium.
  - Windows/Linux: `F11`.
  - macOS: `Control+Command+F`.
- [x] Add locale error copy for fullscreen failure. Risk: Low.
- [x] Add pure tests for shortcut detection. Risk: Low.
- [x] Verify. Risk: Low.
  - `pnpm run lint:web`
  - `pnpm run type-check:web`
  - `pnpm --dir src/web-ui run test:run`
  - `cargo check -p bitfun-desktop`
  - `cargo test -p bitfun-desktop`

M1 implementation notes:

- `useWindowControls` now owns both `isMaximized` and `isFullscreen`, but the states remain independent.
- Maximize and fullscreen share focus restoration, titlebar restoration, and window-state refresh helpers.
- Fullscreen uses desktop-host `set_fullscreen(...)` through `toggle_main_window_fullscreen`; ordinary maximize continues to use the existing frontend `maximize()` / `unmaximize()` path.
- Entering fullscreen from a maximized window must not call `unmaximize()`, `hide()`, or `show()` as part of the enter path; those create visible restore/focus artifacts on Windows. The desktop command instead records whether the window was maximized. On Windows, where direct fullscreen from an undecorated maximized window can remain stuck at work-area size, it enters fullscreen while preserving maximize state and then applies the current monitor's full bounds as a post-fullscreen geometry correction.
- The native fullscreen transition now lives behind the desktop command `toggle_main_window_fullscreen`. The web UI calls a single `systemAPI.toggleMainWindowFullscreen()` method instead of stitching together multiple frontend window calls.
- The Desktop shortcut listener is raw `keydown` handling by design because the macOS fullscreen chord is exact `Control+Command+F`, not the app-level `mod+F` shortcut abstraction.
- Successful fullscreen toggles show a short top-center mode hint so accidental `F11` presses explain both the current mode and the platform exit shortcut.
- Browser mode and toolbar mode do not register the OS fullscreen shortcut.

## Milestone 2: Product Polish And Cross-Platform Hardening

Risk: Medium to Low. The main work is QA and discoverability, not new behavior.

- [ ] Add a menu/command entry such as `View > Toggle Full Screen`. Risk: Low.
- [ ] Show the platform shortcut in a read-only shortcuts/settings surface. Risk: Low.
- [ ] Confirm terminal/editor focus behavior and document whether focused terminal receives or yields `F11`. Risk: Medium.
- [ ] Manually verify Windows, macOS, Linux, browser mode, multiple monitors, and 125%/150% DPI. Risk: Medium.
- [ ] Confirm fullscreen does not persist into workspace/session state. Risk: Low.
- [ ] Confirm existing maximize button, double-click titlebar maximize, diff fullscreen, and editor/image fullscreen still behave independently. Risk: Medium.

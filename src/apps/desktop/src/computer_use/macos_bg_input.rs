//! Codex-style background input injection for macOS.
//!
//! Wraps `CGEventCreate*` + `CGEventSourceStateID::Private` +
//! `CGEventPostToPid` so we can drive a *specific* application without
//!   * moving the user's mouse cursor,
//!   * stealing the user's keyboard focus,
//!   * or polluting the global HID event stream with our synthesized
//!     modifier presses (the `Private` source is decoupled from the user's
//!     real keyboard latch state).
//!
//! ## SkyLight SPI dual-post (ported from cua-driver-rs v0.6.8)
//!
//! When the SkyLight private framework is available, mouse/keyboard events
//! are **dual-posted**: first via `SLEventPostToPid` (which triggers
//! `CGSTickleActivityMonitor` — required for Chromium/Catalyst/Electron
//! background delivery), then via the public `CGEvent::post_to_pid` (which
//! lands on native AppKit targets where SkyLight mouse delivery drops).
//!
//! For keyboard events, the SkyLight path attaches an
//! `SLSEventAuthenticationMessage` envelope so Chromium-class targets accept
//! synthetic keystrokes as trusted live input (macOS 14+).
//!
//! Used by the AX-first dispatch path in ControlHub: when an `app_*` action
//! cannot be satisfied by `AXUIElementPerformAction` alone (e.g. scroll,
//! free-form typing, complex chords) we fall back to PID-targeted events
//! from this module instead of the global foreground click path.

#![allow(dead_code)]

use bitfun_core::util::errors::{BitFunError, BitFunResult};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventType, CGMouseButton, ScrollEventUnit};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use foreign_types::ForeignType;
use log::{debug, info, warn};
use std::ffi::c_void;
use std::thread;
use std::time::{Duration, Instant};

/// Logical mouse button for `bg_click`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgMouseButton {
    Left,
    Right,
    Middle,
}

impl BgMouseButton {
    fn cg(self) -> CGMouseButton {
        match self {
            Self::Left => CGMouseButton::Left,
            Self::Right => CGMouseButton::Right,
            Self::Middle => CGMouseButton::Center,
        }
    }
    fn down(self) -> CGEventType {
        match self {
            Self::Left => CGEventType::LeftMouseDown,
            Self::Right => CGEventType::RightMouseDown,
            Self::Middle => CGEventType::OtherMouseDown,
        }
    }
    fn up(self) -> CGEventType {
        match self {
            Self::Left => CGEventType::LeftMouseUp,
            Self::Right => CGEventType::RightMouseUp,
            Self::Middle => CGEventType::OtherMouseUp,
        }
    }
}

/// Modifier keys understood by `bg_key_chord` / mouse modifiers.
///
/// Maps to the standard macOS modifier flag bits. We deliberately do not
/// touch `CapsLock` here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgModifier {
    Command,
    Shift,
    Option, // alias: alt
    Control,
    Fn,
}

impl BgModifier {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "meta" | "super" => Some(Self::Command),
            "shift" => Some(Self::Shift),
            "alt" | "option" | "opt" => Some(Self::Option),
            "ctrl" | "control" => Some(Self::Control),
            "fn" => Some(Self::Fn),
            _ => None,
        }
    }
    fn flag(self) -> CGEventFlags {
        match self {
            Self::Command => CGEventFlags::CGEventFlagCommand,
            Self::Shift => CGEventFlags::CGEventFlagShift,
            Self::Option => CGEventFlags::CGEventFlagAlternate,
            Self::Control => CGEventFlags::CGEventFlagControl,
            Self::Fn => CGEventFlags::CGEventFlagSecondaryFn,
        }
    }
    fn keycode(self) -> u16 {
        match self {
            Self::Command => 55,
            Self::Shift => 56,
            Self::Option => 58,
            Self::Control => 59,
            Self::Fn => 63,
        }
    }
}

/// Whether this host can deliver background input to arbitrary pids.
///
/// Both `CGEventSourceStateID::Private` and `CGEventPostToPid` require the
/// macOS Accessibility privilege to be granted to the *host* process; if it
/// is not, the calls are silently dropped by the kernel. Callers should
/// surface `BACKGROUND_INPUT_UNAVAILABLE` upstream when this returns
/// `false`.
///
/// Result is cached after the first successful probe so we don't pay the
/// `CGEventSource` create + `CGEventPostToPid` round-trip on every call.
/// A `false` result is NOT cached so callers can re-probe after the user
/// grants Accessibility permission without restarting the host.
pub fn supports_background_input() -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    static CACHED_OK: AtomicBool = AtomicBool::new(false);
    if CACHED_OK.load(Ordering::Relaxed) {
        return true;
    }
    if !accessibility_is_trusted() {
        return false;
    }
    // Real Codex-style probe: build a private source and post a no-op scroll
    // to *our own* pid. Posting to self never disturbs the user's foreground
    // app or real cursor, but it round-trips through the same kernel path
    // that would deliver to a third-party pid.
    let probe_ok = (|| -> bool {
        let src = match CGEventSource::new(CGEventSourceStateID::Private) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let ev = match CGEvent::new_scroll_event(src, ScrollEventUnit::PIXEL, 2, 0, 0, 0) {
            Ok(e) => e,
            Err(_) => return false,
        };
        let me = std::process::id() as i32;
        // Dual-post probe: if SkyLight is available, it takes the SkyLight
        // path; the public path always fires as belt+suspenders.
        post_both_mouse(me, &ev);
        true
    })();
    if probe_ok {
        CACHED_OK.store(true, Ordering::Relaxed);
    }
    probe_ok
}

/// Whether the SkyLight SPI bridge is available for dual-post delivery.
/// When `true`, Chromium/Catalyst/Electron background targets are reachable.
pub fn supports_skylight_post() -> bool {
    super::macos_skylight::is_available()
}

/// Whether the focus-without-raise SPI is available.
/// When `true`, we can activate a window without raising it or stealing
/// focus/Space.
pub fn supports_focus_without_raise() -> bool {
    super::macos_skylight::is_focus_without_raise_available()
}

/// Best-effort check for "host has been granted Accessibility access".
/// We re-implement it locally rather than depending on the
/// `permissions::accessibility` module so this file stays unit-testable
/// outside the broader desktop app.
fn accessibility_is_trusted() -> bool {
    // Re-declared with the same loosely-typed signature used elsewhere in
    // this crate (`desktop_host.rs`) to avoid a clashing-extern warning.
    unsafe extern "C" {
        fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
    }
    // We pass NULL options so we never auto-prompt the user — explicit
    // permission-prompting lives in the existing `permissions` module.
    unsafe { AXIsProcessTrustedWithOptions(std::ptr::null()) }
}

fn private_source(label: &str) -> BitFunResult<CGEventSource> {
    CGEventSource::new(CGEventSourceStateID::Private)
        .map_err(|_| BitFunError::tool(format!("CGEventSource::Private failed ({})", label)))
}

/// Compose modifier flags for a chord.
fn flags_from(mods: &[BgModifier]) -> CGEventFlags {
    mods.iter()
        .fold(CGEventFlags::CGEventFlagNull, |acc, m| acc | m.flag())
}

// ── SkyLight dual-post helpers ─────────────────────────────────────────────
//
// When the SkyLight private framework is available, events are posted via
// BOTH `SLEventPostToPid` (SkyLight path) AND `CGEvent::post_to_pid` (public
// path). The SkyLight path triggers `CGSTickleActivityMonitor` which is
// required for Chromium/Catalyst/Electron background delivery. The public
// path lands on native AppKit targets where SkyLight mouse delivery drops.
//
// For keyboard events, the SkyLight path attaches an
// `SLSEventAuthenticationMessage` envelope (auth=true) so Chromium-class
// targets accept synthetic keystrokes as trusted live input (macOS 14+).
// For NSMenu key equivalents, auth must be false because the envelope routes
// events through a direct-Mach path that bypasses `IOHIDPostEvent`, so
// `NSApplication.sendEvent:` never dispatches NSMenu key equivalents.

/// Dual-post a mouse event to `pid`: SkyLight (no auth) + public API.
fn post_both_mouse(pid: i32, event: &CGEvent) {
    let event_ptr = event.as_ptr() as *mut c_void;
    // Mouse events skip the auth-message envelope (Chromium's window handler
    // subscribes to cgAnnotatedSessionEventTap which the envelope bypasses).
    if !super::macos_skylight::post_to_pid(pid, event_ptr, false) {
        // SkyLight unavailable — fall back to public API only.
        event.post_to_pid(pid);
    } else {
        // Belt+suspenders: also fire public API for AppKit targets where
        // SkyLight mouse delivery drops.
        event.post_to_pid(pid);
    }
}

/// Dual-post a keyboard event to `pid` with auth-message envelope (Chromium).
fn post_both_keyboard(pid: i32, event: &CGEvent) {
    let event_ptr = event.as_ptr() as *mut c_void;
    if !super::macos_skylight::post_to_pid(pid, event_ptr, true) {
        event.post_to_pid(pid);
    }
    // When SkyLight succeeds, we do NOT also fire the public API for keyboard
    // events — the auth envelope routes through a different Mach path, and
    // double-posting causes duplicate keystrokes in some apps.
}

/// Dual-post a keyboard event to `pid` WITHOUT the auth-message envelope.
///
/// Required for NSMenu key equivalents: with the envelope, SLEventPostToPid
/// forks onto a direct-Mach path that bypasses IOHIDPostEvent — NSMenu never
/// sees those events. Without the envelope the path goes through
/// IOHIDPostEvent so `NSApplication.sendEvent:` dispatches NSMenu key
/// equivalents.
fn post_both_keyboard_no_auth(pid: i32, event: &CGEvent) {
    let event_ptr = event.as_ptr() as *mut c_void;
    if !super::macos_skylight::post_to_pid(pid, event_ptr, false) {
        event.post_to_pid(pid);
    }
}

/// Stamp Chromium routing fields onto a mouse event for better backgrounded-
/// target delivery. Called when a `window_id` is known.
fn stamp_chromium_fields(
    event: &CGEvent,
    pid: i32,
    window_id: Option<u32>,
    click_group_id: Option<i64>,
    click_state: i64,
    window_local: Option<(f64, f64)>,
) {
    let event_ptr = event.as_ptr() as *mut c_void;
    let set = |f: u32, v: i64| {
        super::macos_skylight::set_integer_field(event_ptr, f, v);
    };

    // f40 = target pid (Chromium synthetic-event filter) — always stamped.
    set(40, pid as i64);

    if let (Some(wid), Some(cgid)) = (window_id, click_group_id) {
        let wid_i = wid as i64;
        set(1, click_state); // kCGMouseEventClickState
        set(3, 0); // kCGMouseEventButtonNumber (left)
        set(7, 3); // kCGMouseEventSubtype (NSEventSubtypeTouch)
        set(51, wid_i); // windowNumber
        set(58, cgid); // click-group ID (gesture coalescing)
        set(91, wid_i); // kCGMouseEventWindowUnderMousePointer
        set(92, wid_i); // kCGMouseEventWindowUnderMousePointerThatCanHandleThisEvent
    }

    if let Some((wx, wy)) = window_local {
        super::macos_skylight::set_window_location(event_ptr, wx, wy);
    }
}

/// Send a click (down + up, possibly multi-click) at the given **global**
/// pointer position to the target pid. The user's real cursor is NOT moved
/// because we never call `CGWarpMouseCursorPosition` and the synthesized
/// event's `MouseMoved` predecessor is also pid-scoped.
///
/// `point` is in Quartz global pointer coordinates (origin top-left of main
/// display, same space as the existing screenshot pipeline).
pub fn bg_click(
    pid: i32,
    point: (f64, f64),
    button: BgMouseButton,
    click_count: u32,
    modifiers: &[BgModifier],
) -> BitFunResult<()> {
    if click_count == 0 {
        return Ok(());
    }
    let pt = CGPoint {
        x: point.0,
        y: point.1,
    };
    let flags = flags_from(modifiers);
    let self_pid = std::process::id() as i32;
    let frontmost = frontmost_pid_macos();
    let started = Instant::now();
    info!(
        target: "computer_use::bg_input",
        "bg_click.enter pid={} self_pid={} same_process={} frontmost_pid={:?} is_frontmost={} x={:.2} y={:.2} button={:?} click_count={} modifiers={:?}",
        pid,
        self_pid,
        pid == self_pid,
        frontmost,
        Some(pid) == frontmost,
        point.0,
        point.1,
        button,
        click_count,
        modifiers
    );
    // Codex parity: a *single* `CGEventSource` is shared across the whole
    // gesture so the kernel-side modifier latch state stays consistent
    // between MouseMoved / Down / Up. Allocating a fresh source per event
    // (the previous shape) caused some Cocoa apps (notably Chromium-based
    // webviews and SwiftUI text fields) to drop modifier flags between the
    // down and up events and either select text or miss the chord entirely.
    let src = match private_source("click") {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "computer_use::bg_input", "bg_click.private_source_failed pid={} error={}", pid, e);
            return Err(e);
        }
    };

    // Pre-position the synthetic pointer inside the app's event queue so AX
    // hit-testing in the target app sees the right coordinates. Does NOT
    // move the user's real cursor because we post pid-scoped, not global.
    let mv = CGEvent::new_mouse_event(src.clone(), CGEventType::MouseMoved, pt, button.cg())
        .map_err(|_| BitFunError::tool("CGEvent MouseMoved failed".to_string()))?;
    if !flags.is_empty() {
        mv.set_flags(flags);
    }
    post_both_mouse(pid, &mv);

    for i in 1..=click_count {
        let down = CGEvent::new_mouse_event(src.clone(), button.down(), pt, button.cg())
            .map_err(|_| BitFunError::tool("CGEvent MouseDown failed".to_string()))?;
        // Click count field lets the target app recognise double / triple
        // clicks within its own quench-time window.
        down.set_integer_value_field(
            core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
            i as i64,
        );
        if !flags.is_empty() {
            down.set_flags(flags);
        }
        post_both_mouse(pid, &down);

        let up = CGEvent::new_mouse_event(src.clone(), button.up(), pt, button.cg())
            .map_err(|_| BitFunError::tool("CGEvent MouseUp failed".to_string()))?;
        up.set_integer_value_field(
            core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
            i as i64,
        );
        if !flags.is_empty() {
            up.set_flags(flags);
        }
        post_both_mouse(pid, &up);
    }
    info!(
        target: "computer_use::bg_input",
        "bg_click.posted pid={} elapsed_ms={}",
        pid,
        started.elapsed().as_millis() as u64
    );
    Ok(())
}

/// Best-effort lookup of the macOS frontmost-application pid via NSWorkspace.
/// Returns `None` when the AppKit lookup is not available (e.g. headless tests
/// or non-main-thread contexts where we don't want to assert).
pub fn frontmost_pid_macos() -> Option<i32> {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    unsafe {
        let cls = objc2::runtime::AnyClass::get(c"NSWorkspace")?;
        let ws: *mut AnyObject = msg_send![cls, sharedWorkspace];
        if ws.is_null() {
            return None;
        }
        let app: *mut AnyObject = msg_send![ws, frontmostApplication];
        if app.is_null() {
            return None;
        }
        let pid: i32 = msg_send![app, processIdentifier];
        if pid <= 0 {
            None
        } else {
            Some(pid)
        }
    }
}

/// Best-effort: bring `pid`'s app to the foreground so that GUI hit-testing
/// (especially WKWebView event delivery) reliably routes synthetic clicks
/// to the right window.
///
/// When the SkyLight focus-without-raise SPI is available, uses
/// `SLPSPostEventRecordTo` to change WindowServer focus state **without
/// raising any windows or triggering Space-follow** (ported from yabai).
/// This is the preferred path for background automation because it doesn't
/// disrupt the user's visible window layout.
///
/// Falls back to the public `NSRunningApplication.activateWithOptions` API
/// which **does** raise the window and steal focus — used when the SkyLight
/// SPI is unavailable or when a window_id is not known.
///
/// Returns `Ok(true)` when activation succeeded, `Ok(false)` when the app
/// could not be found, and `Err(_)` on AppKit FFI failures.
pub fn activate_pid_macos(pid: i32) -> BitFunResult<bool> {
    // Without a window_id we can't use the focus-without-raise SPI.
    // Fall through to the public API.
    activate_pid_macos_with_window(pid, None)
}

/// Like `activate_pid_macos` but uses the focus-without-raise SPI when a
/// `window_id` is provided and the SkyLight SPI is available.
pub fn activate_pid_macos_with_window(pid: i32, window_id: Option<u32>) -> BitFunResult<bool> {
    // Try focus-without-raise first when we have a window id.
    if let Some(wid) = window_id {
        if super::macos_skylight::is_focus_without_raise_available() {
            let ok = super::macos_skylight::activate_without_raise(pid, wid);
            if ok {
                info!(
                    target: "computer_use::bg_input",
                    "activate_without_raise.done pid={} wid={}",
                    pid, wid
                );
                return Ok(true);
            }
            // SPI call failed — fall through to public API.
            warn!(
                target: "computer_use::bg_input",
                "activate_without_raise.failed pid={} wid={} — falling back to NSRunningApplication",
                pid, wid
            );
        }
    }

    // Public API fallback (raises window, steals focus).
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    let started = Instant::now();
    let result: bool = unsafe {
        let cls = match objc2::runtime::AnyClass::get(c"NSRunningApplication") {
            Some(c) => c,
            None => {
                debug!(target: "computer_use::bg_input", "activate.class_missing pid={}", pid);
                return Ok(false);
            }
        };
        let app: *mut AnyObject = msg_send![cls, runningApplicationWithProcessIdentifier: pid];
        if app.is_null() {
            debug!(target: "computer_use::bg_input", "activate.app_not_found pid={}", pid);
            return Ok(false);
        }
        // 1<<1 == NSApplicationActivateIgnoringOtherApps
        let ok: bool = msg_send![app, activateWithOptions: 1u64 << 1];
        ok
    };
    info!(
        target: "computer_use::bg_input",
        "activate.done pid={} ok={} elapsed_ms={}",
        pid,
        result,
        started.elapsed().as_millis() as u64
    );
    Ok(result)
}

/// Pixel-delta scroll inside the focused scroll container of the target
/// pid's frontmost window. Positive `dy` scrolls content down (matches
/// trackpad / `wheel1>0` direction).
pub fn bg_scroll(pid: i32, dx: i32, dy: i32) -> BitFunResult<()> {
    info!(
        target: "computer_use::bg_input",
        "bg_scroll.enter pid={} dx={} dy={}",
        pid, dx, dy
    );
    let src = private_source("scroll")?;
    // Two-axis pixel scroll (`wheelCount = 2`): wheel1 = dy, wheel2 = dx.
    // Sign convention matches the system trackpad (positive dy = content
    // moves down on screen, i.e. user is looking further into the document).
    let ev = CGEvent::new_scroll_event(src, ScrollEventUnit::PIXEL, 2, dy, dx, 0)
        .map_err(|_| BitFunError::tool("CGEventCreateScrollWheelEvent2 failed".to_string()))?;
    post_both_mouse(pid, &ev);
    Ok(())
}

/// Type a UTF-8 string into the focused control of the target pid using the
/// `kCGEventKeyboardEventUnicodeString` field. This bypasses keymap
/// translation entirely, so it correctly handles emoji, CJK and other
/// non-Latin input without touching the system IME.
pub fn bg_type_text(pid: i32, text: &str) -> BitFunResult<()> {
    if text.is_empty() {
        return Ok(());
    }
    info!(
        target: "computer_use::bg_input",
        "bg_type_text.enter pid={} char_count={} byte_count={}",
        pid,
        text.chars().count(),
        text.len()
    );
    // Single source for the whole string (Codex parity): keeps the kernel
    // keyboard state coherent and avoids the per-char allocation cost.
    let src = private_source("type_text")?;
    // We send one event per Unicode scalar to keep individual events small
    // and let the target app receive a sane stream of `keyDown` callbacks.
    // (`set_string` itself will accept a longer buffer, but some Cocoa text
    // controls truncate at ~20 UTF-16 units per event.)
    for ch in text.chars() {
        // Keycode 0 is irrelevant when the unicode string field is set.
        let ev = CGEvent::new_keyboard_event(src.clone(), 0, true)
            .map_err(|_| BitFunError::tool("CGEventCreateKeyboardEvent failed".to_string()))?;
        let buf: Vec<u16> = ch.encode_utf16(&mut [0u16; 2]).to_vec();
        ev.set_string_from_utf16_unchecked(&buf);
        post_both_keyboard(pid, &ev);
        // Match keyup so the target app sees a complete keystroke.
        let ev2 = CGEvent::new_keyboard_event(src.clone(), 0, false)
            .map_err(|_| BitFunError::tool("CGEventCreateKeyboardEvent (up) failed".to_string()))?;
        ev2.set_string_from_utf16_unchecked(&buf);
        post_both_keyboard(pid, &ev2);
        // 8ms inter-key gap matches Codex / native typing rates and avoids
        // dropped chars in Chromium webviews and SwiftUI multi-line fields
        // that throttle their keystroke handler. 1ms (the previous value)
        // was reliably losing ~5–10% of CJK glyphs in informal smoke tests.
        thread::sleep(Duration::from_millis(8));
    }
    Ok(())
}

/// Send a key chord (modifier+key combo) to the target pid using the
/// private event source. `key` is the AX / Carbon virtual keycode; callers
/// can use `keycode_for_char` for ASCII letters or pass a literal keycode.
pub fn bg_key_chord(pid: i32, modifiers: &[BgModifier], key: u16) -> BitFunResult<()> {
    info!(
        target: "computer_use::bg_input",
        "bg_key_chord.enter pid={} keycode={} modifiers={:?}",
        pid, key, modifiers
    );
    let flags = flags_from(modifiers);
    // Single source across the whole chord — required for the modifier
    // latch state to survive between mod_down → key_down → key_up → mod_up.
    let src = private_source("key_chord")?;

    // Press modifiers.
    for m in modifiers {
        let ev = CGEvent::new_keyboard_event(src.clone(), m.keycode(), true)
            .map_err(|_| BitFunError::tool("CGEvent ModDown failed".to_string()))?;
        ev.set_flags(flags);
        post_both_keyboard(pid, &ev);
    }
    // Press main key.
    {
        let ev = CGEvent::new_keyboard_event(src.clone(), key, true)
            .map_err(|_| BitFunError::tool("CGEvent KeyDown failed".to_string()))?;
        ev.set_flags(flags);
        post_both_keyboard(pid, &ev);
    }
    {
        let ev = CGEvent::new_keyboard_event(src.clone(), key, false)
            .map_err(|_| BitFunError::tool("CGEvent KeyUp failed".to_string()))?;
        ev.set_flags(flags);
        post_both_keyboard(pid, &ev);
    }
    // Release modifiers in reverse press order.
    for m in modifiers.iter().rev() {
        let ev = CGEvent::new_keyboard_event(src.clone(), m.keycode(), false)
            .map_err(|_| BitFunError::tool("CGEvent ModUp failed".to_string()))?;
        // Drop this modifier from the flag set as we release it.
        let remaining = modifiers
            .iter()
            .copied()
            .filter(|x| x != m)
            .collect::<Vec<_>>();
        ev.set_flags(flags_from(&remaining));
        post_both_keyboard(pid, &ev);
    }
    Ok(())
}

/// Full Chromium-compatible left-click recipe matching cua-driver-rs's
/// `click_at_xy_chromium`.
///
/// Sequence:
///  1. Stamped `mouseMoved` at target coords (phase=2, cursor-state primer).
///  2. Off-screen primer down/up at (-1, -1) (phase=1/2) — satisfies
///     Chromium's user-activation gate without hitting any DOM element.
///  3. Target down/up pair(s) at real coordinates (phase=3), clickState 1→N.
///
/// All events carry Chromium routing fields (f0 phase, f1 clickState, f3
/// button, f7 NSEventSubtypeTouch, f40 pid, f51/f91/f92 windowID, f58
/// click-group) and `CGEventSetWindowLocation` for window-local point.
///
/// Uses both SkyLight `SLEventPostToPid` AND `CGEvent::post_to_pid`
/// (belt+suspenders) for AppKit/Catalyst target coverage.
pub fn bg_click_chromium(
    pid: i32,
    screen_x: f64,
    screen_y: f64,
    win_local_x: f64,
    win_local_y: f64,
    wid: u32,
    click_count: u32,
    modifiers: &[BgModifier],
) -> BitFunResult<()> {
    use std::time::{SystemTime, UNIX_EPOCH};
    if click_count == 0 {
        return Ok(());
    }
    let src = private_source("click_chromium")?;
    let target = CGPoint {
        x: screen_x,
        y: screen_y,
    };
    let off_screen = CGPoint { x: -1.0, y: -1.0 };
    let win_local = (win_local_x, win_local_y);
    let off_local = (-1.0_f64, -1.0_f64);
    let flags = flags_from(modifiers);
    let click_pairs = click_count.min(2) as usize;
    let window_id = wid as i64;

    // All events share the same click-group ID so WindowServer/Chromium
    // treat the sequence as one gesture.
    let click_group_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as i64;

    let stamp = |event: &CGEvent, local: (f64, f64), click_state: i64, phase: i64| {
        let ptr = event.as_ptr() as *mut c_void;
        let set = |f: u32, v: i64| {
            super::macos_skylight::set_integer_field(ptr, f, v);
        };
        set(0, phase); // gesture phase
        set(1, click_state); // kCGMouseEventClickState
        set(3, 0); // button (left)
        set(7, 3); // NSEventSubtypeTouch
        set(40, pid as i64); // Chromium synthetic-event filter
        if window_id != 0 {
            set(51, window_id); // windowNumber
            set(91, window_id); // WindowUnderMousePointer
            set(92, window_id); // WindowUnderMousePointerThatCanHandleThisEvent
        }
        set(58, click_group_id); // click-group ID
        super::macos_skylight::set_window_location(ptr, local.0, local.1);
        if flags != CGEventFlags::CGEventFlagNull {
            event.set_flags(flags);
        }
    };

    let post = |event: &CGEvent| {
        post_both_mouse(pid, event);
    };

    // Step 1: mouseMoved at target (phase=2, clickState=0).
    let move_ev = CGEvent::new_mouse_event(
        src.clone(),
        CGEventType::MouseMoved,
        target,
        CGMouseButton::Left,
    )
    .map_err(|_| BitFunError::tool("Chromium click: mouseMoved creation failed".to_string()))?;
    stamp(&move_ev, win_local, 0, 2);
    post(&move_ev);
    thread::sleep(Duration::from_millis(15));

    // Step 2: off-screen primer click — opens Chromium user-activation gate.
    let primer_down = CGEvent::new_mouse_event(
        src.clone(),
        CGEventType::LeftMouseDown,
        off_screen,
        CGMouseButton::Left,
    )
    .map_err(|_| BitFunError::tool("Chromium click: primer down failed".to_string()))?;
    stamp(&primer_down, off_local, 1, 1);
    post(&primer_down);
    thread::sleep(Duration::from_millis(1));

    let primer_up = CGEvent::new_mouse_event(
        src.clone(),
        CGEventType::LeftMouseUp,
        off_screen,
        CGMouseButton::Left,
    )
    .map_err(|_| BitFunError::tool("Chromium click: primer up failed".to_string()))?;
    stamp(&primer_up, off_local, 1, 2);
    post(&primer_up);
    // ≥1 frame so Chromium sees primer + target as separate gestures.
    thread::sleep(Duration::from_millis(100));

    // Step 3: target click pair(s) with clickState stepped 1→N.
    for pair_index in 1..=click_pairs {
        let click_state = pair_index as i64;
        let down = CGEvent::new_mouse_event(
            src.clone(),
            CGEventType::LeftMouseDown,
            target,
            CGMouseButton::Left,
        )
        .map_err(|_| BitFunError::tool("Chromium click: target down failed".to_string()))?;
        stamp(&down, win_local, click_state, 3);
        post(&down);
        thread::sleep(Duration::from_millis(1));

        let up = CGEvent::new_mouse_event(
            src.clone(),
            CGEventType::LeftMouseUp,
            target,
            CGMouseButton::Left,
        )
        .map_err(|_| BitFunError::tool("Chromium click: target up failed".to_string()))?;
        stamp(&up, win_local, click_state, 3);
        post(&up);

        if pair_index < click_pairs {
            thread::sleep(Duration::from_millis(80));
        }
    }

    info!(
        target: "computer_use::bg_input",
        "bg_click_chromium.posted pid={} wid={} x={:.2} y={:.2} pairs={}",
        pid, wid, screen_x, screen_y, click_pairs
    );
    Ok(())
}

/// Mouse button for drag gestures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgDragButton {
    Left,
    Right,
    Middle,
}

impl BgDragButton {
    fn cg(self) -> CGMouseButton {
        match self {
            Self::Left => CGMouseButton::Left,
            Self::Right => CGMouseButton::Right,
            Self::Middle => CGMouseButton::Center,
        }
    }
    fn down(self) -> CGEventType {
        match self {
            Self::Left => CGEventType::LeftMouseDown,
            Self::Right => CGEventType::RightMouseDown,
            Self::Middle => CGEventType::OtherMouseDown,
        }
    }
    fn dragged(self) -> CGEventType {
        match self {
            Self::Left => CGEventType::LeftMouseDragged,
            Self::Right => CGEventType::RightMouseDragged,
            Self::Middle => CGEventType::OtherMouseDragged,
        }
    }
    fn up(self) -> CGEventType {
        match self {
            Self::Left => CGEventType::LeftMouseUp,
            Self::Right => CGEventType::RightMouseUp,
            Self::Middle => CGEventType::OtherMouseUp,
        }
    }
}

/// Press-drag-release gesture from `(from_x, from_y)` to `(to_x, to_y)` in
/// screen coordinates, posted to `pid`.
///
/// `duration_ms` is the wall-clock budget; `steps` is the number of
/// intermediate `leftMouseDragged` events linearly interpolated along the
/// path. Modifiers are held across the entire gesture.
pub fn bg_drag(
    pid: i32,
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
    from_local: Option<(f64, f64)>,
    to_local: Option<(f64, f64)>,
    wid: Option<u32>,
    duration_ms: u64,
    steps: usize,
    modifiers: &[BgModifier],
    button: BgDragButton,
) -> BitFunResult<()> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let src = private_source("drag")?;
    let flags = flags_from(modifiers);
    let cg_button = button.cg();

    let click_group_id: Option<i64> = wid.map(|_| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as i64
    });

    let steps = steps.max(1);
    let step_delay_ms = if steps > 1 {
        duration_ms / steps as u64
    } else {
        duration_ms
    };

    // MouseDown at start.
    let from_pt = CGPoint {
        x: from_x,
        y: from_y,
    };
    let down = CGEvent::new_mouse_event(src.clone(), button.down(), from_pt, cg_button)
        .map_err(|_| BitFunError::tool("drag: mouseDown failed".to_string()))?;
    if flags != CGEventFlags::CGEventFlagNull {
        down.set_flags(flags);
    }
    stamp_chromium_fields(&down, pid, wid, click_group_id, 1, from_local);
    post_both_mouse(pid, &down);
    thread::sleep(Duration::from_millis(16));

    // Interpolated drag steps.
    for i in 1..=steps {
        let t = i as f64 / steps as f64;
        let ix = from_x + (to_x - from_x) * t;
        let iy = from_y + (to_y - from_y) * t;
        let il = from_local
            .zip(to_local)
            .map(|((fx, fy), (tx, ty))| (fx + (tx - fx) * t, fy + (ty - fy) * t));
        let drag_pt = CGPoint { x: ix, y: iy };
        let drag = CGEvent::new_mouse_event(src.clone(), button.dragged(), drag_pt, cg_button)
            .map_err(|_| BitFunError::tool("drag: mouseDragged failed".to_string()))?;
        if flags != CGEventFlags::CGEventFlagNull {
            drag.set_flags(flags);
        }
        stamp_chromium_fields(&drag, pid, wid, click_group_id, 1, il);
        post_both_mouse(pid, &drag);
        if step_delay_ms > 0 {
            thread::sleep(Duration::from_millis(step_delay_ms));
        }
    }

    // MouseUp at end.
    let to_pt = CGPoint { x: to_x, y: to_y };
    let up = CGEvent::new_mouse_event(src.clone(), button.up(), to_pt, cg_button)
        .map_err(|_| BitFunError::tool("drag: mouseUp failed".to_string()))?;
    if flags != CGEventFlags::CGEventFlagNull {
        up.set_flags(flags);
    }
    stamp_chromium_fields(&up, pid, wid, click_group_id, 1, to_local);
    post_both_mouse(pid, &up);

    info!(
        target: "computer_use::bg_input",
        "bg_drag.posted pid={} from=({:.0},{:.0}) to=({:.0},{:.0}) steps={} button={:?}",
        pid, from_x, from_y, to_x, to_y, steps, button
    );
    Ok(())
}

/// Send a key chord to `pid` WITHOUT the auth-message envelope.
///
/// Required for NSMenu key equivalents: with the envelope, SLEventPostToPid
/// forks onto a direct-Mach path that bypasses IOHIDPostEvent — NSMenu never
/// sees those events. Without the envelope the path goes through
/// IOHIDPostEvent so `NSApplication.sendEvent:` dispatches NSMenu key
/// equivalents.
pub fn bg_key_chord_no_auth(pid: i32, modifiers: &[BgModifier], key: u16) -> BitFunResult<()> {
    info!(
        target: "computer_use::bg_input",
        "bg_key_chord_no_auth.enter pid={} keycode={} modifiers={:?}",
        pid, key, modifiers
    );
    let flags = flags_from(modifiers);
    let src = private_source("key_chord_no_auth")?;

    for m in modifiers {
        let ev = CGEvent::new_keyboard_event(src.clone(), m.keycode(), true)
            .map_err(|_| BitFunError::tool("CGEvent ModDown (no_auth) failed".to_string()))?;
        ev.set_flags(flags);
        post_both_keyboard_no_auth(pid, &ev);
    }
    {
        let ev = CGEvent::new_keyboard_event(src.clone(), key, true)
            .map_err(|_| BitFunError::tool("CGEvent KeyDown (no_auth) failed".to_string()))?;
        ev.set_flags(flags);
        post_both_keyboard_no_auth(pid, &ev);
    }
    {
        let ev = CGEvent::new_keyboard_event(src.clone(), key, false)
            .map_err(|_| BitFunError::tool("CGEvent KeyUp (no_auth) failed".to_string()))?;
        ev.set_flags(flags);
        post_both_keyboard_no_auth(pid, &ev);
    }
    for m in modifiers.iter().rev() {
        let ev = CGEvent::new_keyboard_event(src.clone(), m.keycode(), false)
            .map_err(|_| BitFunError::tool("CGEvent ModUp (no_auth) failed".to_string()))?;
        let remaining = modifiers
            .iter()
            .copied()
            .filter(|x| x != m)
            .collect::<Vec<_>>();
        ev.set_flags(flags_from(&remaining));
        post_both_keyboard_no_auth(pid, &ev);
    }
    Ok(())
}

/// Right-click at `(x, y)` screen coordinates, posted to `pid` via dual-post.
pub fn bg_right_click(pid: i32, point: (f64, f64), modifiers: &[BgModifier]) -> BitFunResult<()> {
    let src = private_source("right_click")?;
    let pt = CGPoint {
        x: point.0,
        y: point.1,
    };
    let flags = flags_from(modifiers);

    let down = CGEvent::new_mouse_event(
        src.clone(),
        CGEventType::RightMouseDown,
        pt,
        CGMouseButton::Right,
    )
    .map_err(|_| BitFunError::tool("CGEvent RightMouseDown failed".to_string()))?;
    if flags != CGEventFlags::CGEventFlagNull {
        down.set_flags(flags);
    }
    stamp_chromium_fields(&down, pid, None, None, 1, None);
    post_both_mouse(pid, &down);
    thread::sleep(Duration::from_millis(16));

    let up = CGEvent::new_mouse_event(
        src.clone(),
        CGEventType::RightMouseUp,
        pt,
        CGMouseButton::Right,
    )
    .map_err(|_| BitFunError::tool("CGEvent RightMouseUp failed".to_string()))?;
    if flags != CGEventFlags::CGEventFlagNull {
        up.set_flags(flags);
    }
    stamp_chromium_fields(&up, pid, None, None, 1, None);
    post_both_mouse(pid, &up);
    Ok(())
}

/// Middle-click at `(x, y)` screen coordinates, posted to `pid` via dual-post.
pub fn bg_middle_click(pid: i32, point: (f64, f64), modifiers: &[BgModifier]) -> BitFunResult<()> {
    let src = private_source("middle_click")?;
    let pt = CGPoint {
        x: point.0,
        y: point.1,
    };
    let flags = flags_from(modifiers);

    let down = CGEvent::new_mouse_event(
        src.clone(),
        CGEventType::OtherMouseDown,
        pt,
        CGMouseButton::Center,
    )
    .map_err(|_| BitFunError::tool("CGEvent OtherMouseDown failed".to_string()))?;
    if flags != CGEventFlags::CGEventFlagNull {
        down.set_flags(flags);
    }
    stamp_chromium_fields(&down, pid, None, None, 1, None);
    post_both_mouse(pid, &down);
    thread::sleep(Duration::from_millis(16));

    let up = CGEvent::new_mouse_event(
        src.clone(),
        CGEventType::OtherMouseUp,
        pt,
        CGMouseButton::Center,
    )
    .map_err(|_| BitFunError::tool("CGEvent OtherMouseUp failed".to_string()))?;
    if flags != CGEventFlags::CGEventFlagNull {
        up.set_flags(flags);
    }
    stamp_chromium_fields(&up, pid, None, None, 1, None);
    post_both_mouse(pid, &up);
    Ok(())
}

/// Parse a key spec the dispatch layer might pass us, of the form
/// `"command+shift+p"` / `"return"` / `"escape"` / `"a"`. Returns the
/// modifier list and the resolved keycode.
pub fn parse_key_spec(spec: &str) -> BitFunResult<(Vec<BgModifier>, u16)> {
    let mut mods = Vec::new();
    let parts: Vec<&str> = spec.split('+').map(str::trim).collect();
    if parts.is_empty() {
        return Err(BitFunError::tool("empty key spec".to_string()));
    }
    let (last, head) = parts.split_last().unwrap();
    for p in head {
        let m = BgModifier::from_str(p)
            .ok_or_else(|| BitFunError::tool(format!("unknown modifier in key spec: {}", p)))?;
        mods.push(m);
    }
    let kc = keycode_for_named(last)
        .or_else(|| {
            // Single-char ASCII fallback.
            let mut chars = last.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            keycode_for_char(c)
        })
        .ok_or_else(|| BitFunError::tool(format!("unknown key in key spec: {}", last)))?;
    Ok((mods, kc))
}

/// Parse the ControlHub/Codex chord shape: `["command", "shift", "p"]`,
/// `["command+shift+p"]`, or `["return"]`.
pub fn parse_key_sequence(keys: &[String]) -> BitFunResult<(Vec<BgModifier>, u16)> {
    if keys.is_empty() {
        return Err(BitFunError::tool("empty key sequence".to_string()));
    }
    if keys.len() == 1 {
        return parse_key_spec(&keys[0]);
    }

    let (last, head) = keys.split_last().unwrap();
    let mut mods = Vec::with_capacity(head.len());
    for p in head {
        let m = BgModifier::from_str(p)
            .ok_or_else(|| BitFunError::tool(format!("unknown modifier in key sequence: {}", p)))?;
        mods.push(m);
    }
    let kc = keycode_for_named(last)
        .or_else(|| {
            let mut chars = last.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            keycode_for_char(c)
        })
        .ok_or_else(|| BitFunError::tool(format!("unknown key in key sequence: {}", last)))?;
    Ok((mods, kc))
}

/// Map common named keys (Codex parity) to AX / Carbon keycodes.
pub fn keycode_for_named(name: &str) -> Option<u16> {
    Some(match name.to_ascii_lowercase().as_str() {
        "return" | "enter" => 36,
        "tab" => 48,
        "space" => 49,
        "delete" | "backspace" => 51,
        "escape" | "esc" => 53,
        "left" => 123,
        "right" => 124,
        "down" => 125,
        "up" => 126,
        "home" => 115,
        "end" => 119,
        "pageup" | "page_up" => 116,
        "pagedown" | "page_down" => 121,
        "f1" => 122,
        "f2" => 120,
        "f3" => 99,
        "f4" => 118,
        "f5" => 96,
        "f6" => 97,
        "f7" => 98,
        "f8" => 100,
        "f9" => 101,
        "f10" => 109,
        "f11" => 103,
        "f12" => 111,
        _ => return None,
    })
}

/// Map a single ASCII character to the **US-keyboard** keycode. This is the
/// same table Codex / enigo use; the user's actual keymap is irrelevant for
/// our chord injection because we set explicit modifier flags ourselves.
pub fn keycode_for_char(c: char) -> Option<u16> {
    let upper = c.to_ascii_uppercase();
    Some(match upper {
        'A' => 0,
        'S' => 1,
        'D' => 2,
        'F' => 3,
        'H' => 4,
        'G' => 5,
        'Z' => 6,
        'X' => 7,
        'C' => 8,
        'V' => 9,
        'B' => 11,
        'Q' => 12,
        'W' => 13,
        'E' => 14,
        'R' => 15,
        'Y' => 16,
        'T' => 17,
        '1' => 18,
        '2' => 19,
        '3' => 20,
        '4' => 21,
        '6' => 22,
        '5' => 23,
        '=' => 24,
        '9' => 25,
        '7' => 26,
        '-' => 27,
        '8' => 28,
        '0' => 29,
        ']' => 30,
        'O' => 31,
        'U' => 32,
        '[' => 33,
        'I' => 34,
        'P' => 35,
        'L' => 37,
        'J' => 38,
        '\'' => 39,
        'K' => 40,
        ';' => 41,
        '\\' => 42,
        ',' => 43,
        '/' => 44,
        'N' => 45,
        'M' => 46,
        '.' => 47,
        '`' => 50,
        _ => return None,
    })
}

// ── Terminal-safe typing detection ─────────────────────────────────────────
//
// Terminal emulators (Ghostty, iTerm2, Terminal.app, etc.) often silently
// drop Unicode string keyboard events (`kCGEventKeyboardEventUnicodeString`).
// When the target is a terminal, the dispatch layer should route `type_text`
// through individual key events instead of the Unicode string field.
// Ported from cua-driver-rs terminal detection (per-platform).

/// Known macOS terminal emulator bundle identifiers.
const TERMINAL_BUNDLE_IDS: &[&str] = &[
    "com.mitchellh.ghostty",
    "com.googlecode.iterm2",
    "com.apple.Terminal",
    "com.todesktop.230313mzl4w4u92", // Warp
    "com.neovide.neovide",
    "org.alacritty",
    "io.wez.wezterm",
    "com.kitty",
    "com.github.wez.wezterm",
];

/// Known macOS terminal app names (lowercase, for substring matching).
const TERMINAL_NAME_HINTS: &[&str] = &[
    "ghostty",
    "iterm",
    "terminal",
    "warp",
    "neovide",
    "alacritty",
    "wezterm",
    "kitty",
    "hyper",
    "tabby",
];

/// Check if the target pid is a terminal emulator by looking up its
/// bundle id via `NSRunningApplication`. Returns `true` when the app is
/// a known terminal emulator that may silently drop Unicode string events.
pub fn is_terminal_emulator(pid: i32) -> bool {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    let bundle_id = unsafe {
        let cls = match objc2::runtime::AnyClass::get(c"NSRunningApplication") {
            Some(c) => c,
            None => return false,
        };
        let app: *mut AnyObject = msg_send![cls, runningApplicationWithProcessIdentifier: pid];
        if app.is_null() {
            return false;
        }
        let bundle: *mut AnyObject = msg_send![app, bundleIdentifier];
        if bundle.is_null() {
            // Fallback: check localized name.
            let name: *mut AnyObject = msg_send![app, localizedName];
            if name.is_null() {
                return false;
            }
            let utf8: *const std::os::raw::c_char = msg_send![name, UTF8String];
            if utf8.is_null() {
                return false;
            }
            let name_str = std::ffi::CStr::from_ptr(utf8)
                .to_string_lossy()
                .to_ascii_lowercase();
            return TERMINAL_NAME_HINTS.iter().any(|&h| name_str.contains(h));
        }
        let utf8: *const std::os::raw::c_char = msg_send![bundle, UTF8String];
        if utf8.is_null() {
            return false;
        }
        std::ffi::CStr::from_ptr(utf8)
            .to_string_lossy()
            .to_ascii_lowercase()
    };
    if TERMINAL_BUNDLE_IDS.iter().any(|&b| bundle_id == b) {
        return true;
    }
    if TERMINAL_NAME_HINTS.iter().any(|&h| bundle_id.contains(h)) {
        return true;
    }
    false
}

/// Type text into a terminal emulator using individual key events instead of
/// Unicode string injection. This bypasses the silent-drop problem in
/// Ghostty/iTerm2/Terminal.app by sending actual key-down/up pairs.
///
/// Only works for ASCII characters that have direct keycodes. Non-ASCII text
/// (CJK, emoji) should use `bg_type_text` (Unicode string) or `paste` instead.
pub fn bg_type_text_terminal_safe(pid: i32, text: &str) -> BitFunResult<()> {
    if text.is_empty() {
        return Ok(());
    }
    info!(
        target: "computer_use::bg_input",
        "bg_type_text_terminal_safe.enter pid={} char_count={}",
        pid,
        text.chars().count()
    );
    let src = private_source("type_text_terminal")?;
    for ch in text.chars() {
        let kc = keycode_for_char(ch);
        let needs_shift = ch.is_ascii_uppercase();
        let flags = if needs_shift {
            flags_from(&[BgModifier::Shift])
        } else {
            CGEventFlags::CGEventFlagNull
        };

        if let Some(kc) = kc {
            // Use key events for mappable ASCII characters.
            let down = CGEvent::new_keyboard_event(src.clone(), kc, true)
                .map_err(|_| BitFunError::tool("terminal type: keydown failed".to_string()))?;
            if flags != CGEventFlags::CGEventFlagNull {
                down.set_flags(flags);
            }
            post_both_keyboard(pid, &down);
            thread::sleep(Duration::from_millis(8));

            let up = CGEvent::new_keyboard_event(src.clone(), kc, false)
                .map_err(|_| BitFunError::tool("terminal type: keyup failed".to_string()))?;
            if flags != CGEventFlags::CGEventFlagNull {
                up.set_flags(flags);
            }
            post_both_keyboard(pid, &up);
            thread::sleep(Duration::from_millis(8));
        } else {
            // Fallback to Unicode string for non-ASCII characters.
            let buf: Vec<u16> = ch.encode_utf16(&mut [0u16; 2]).to_vec();
            let down = CGEvent::new_keyboard_event(src.clone(), 0, true)
                .map_err(|_| BitFunError::tool("terminal type: unicode down failed".to_string()))?;
            down.set_string_from_utf16_unchecked(&buf);
            post_both_keyboard(pid, &down);
            thread::sleep(Duration::from_millis(8));

            let up = CGEvent::new_keyboard_event(src.clone(), 0, false)
                .map_err(|_| BitFunError::tool("terminal type: unicode up failed".to_string()))?;
            up.set_string_from_utf16_unchecked(&buf);
            post_both_keyboard(pid, &up);
            thread::sleep(Duration::from_millis(8));
        }
    }
    Ok(())
}

/// Type text with automatic terminal detection: routes to
/// `bg_type_text_terminal_safe` when the target is a terminal emulator,
/// otherwise uses the standard `bg_type_text` (Unicode string injection).
pub fn bg_type_text_auto(pid: i32, text: &str) -> BitFunResult<()> {
    if is_terminal_emulator(pid) {
        debug!(
            target: "computer_use::bg_input",
            "bg_type_text_auto: pid={} detected as terminal, using key-event typing",
            pid
        );
        bg_type_text_terminal_safe(pid, text)
    } else {
        bg_type_text(pid, text)
    }
}

// ── Window-id resolution + Chromium/Electron detection ───────────────────────

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(
        option: u32,
        relative_to_window: u32,
    ) -> core_foundation::array::CFArrayRef;
}

#[allow(non_upper_case_globals)]
const kCGWindowListOptionOnScreenOnly: u32 = 1;
#[allow(non_upper_case_globals)]
const kCGWindowListExcludeDesktopElements: u32 = 16;
#[allow(non_upper_case_globals)]
const kCGNullWindowID: u32 = 0;

/// Returns the CGWindowID (window number) of the first on-screen, layer-0
/// window owned by `pid`. Uses `CGWindowListCopyWindowInfo` — the same API
/// `screencapture -l <wid>` consumes. Returns `None` when no matching
/// window is found.
pub fn frontmost_window_id_for_pid(pid: i32) -> Option<u32> {
    use core_foundation::array::CFArray;
    use core_foundation::base::{CFGetTypeID, CFTypeRef, TCFType};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use std::os::raw::c_void;

    let raw_ref = unsafe {
        CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        )
    };
    if raw_ref.is_null() {
        return None;
    }
    let array: CFArray<CFTypeRef> = unsafe { CFArray::wrap_under_create_rule(raw_ref as _) };
    let dict_type_id = CFDictionary::<*const c_void, *const c_void>::type_id();

    for item in array.iter() {
        let item = *item;
        if unsafe { CFGetTypeID(item) } != dict_type_id {
            continue;
        }
        let dict: CFDictionary<*const c_void, *const c_void> =
            unsafe { CFDictionary::wrap_under_get_rule(item as _) };

        let get_num = |key: &str| -> i64 {
            let k = CFString::new(key);
            dict.find(k.as_concrete_TypeRef() as *const c_void)
                .and_then(|v| unsafe {
                    let v = *v;
                    if CFGetTypeID(v) == CFNumber::type_id() {
                        CFNumber::wrap_under_get_rule(v as _).to_i64()
                    } else {
                        None
                    }
                })
                .unwrap_or(0)
        };

        let owner_pid = get_num("kCGWindowOwnerPID") as i32;
        if owner_pid != pid {
            continue;
        }
        let layer = get_num("kCGWindowLayer") as i32;
        if layer != 0 {
            continue;
        }
        let wid = get_num("kCGWindowNumber") as u32;
        if wid != 0 {
            return Some(wid);
        }
    }
    None
}

/// Bundle-id keywords for Chromium-based / Electron-based applications.
/// Matched via `contains` against the lowercased bundle id.
const CHROMIUM_BUNDLE_KEYWORDS: &[&str] = &[
    "chrome",
    "chromium",
    "electron",
    "brave",
    "microsoft-edge",
    "arc.", // Arc browser
    "vivaldi",
    "operamini", // Opera
    "com.operasoftware.operaprofiles",
];

/// Returns `true` when the bundle_id indicates a Chromium-based or
/// Electron-based application. These apps need the `bg_click_chromium`
/// 5-event recipe for reliable background clicks.
pub fn is_chromium_electron(bundle_id: Option<&str>) -> bool {
    if let Some(bid) = bundle_id {
        let lc = bid.to_ascii_lowercase();
        CHROMIUM_BUNDLE_KEYWORDS.iter().any(|&kw| lc.contains(kw))
    } else {
        false
    }
}

/// Convenience: look up the bundle_id for a pid via NSRunningApplication.
pub fn bundle_id_for_pid(pid: i32) -> Option<String> {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    unsafe {
        let cls = objc2::runtime::AnyClass::get(c"NSRunningApplication")?;
        let app: *mut AnyObject = msg_send![cls, runningApplicationWithProcessIdentifier: pid];
        if app.is_null() {
            return None;
        }
        let bundle: *mut AnyObject = msg_send![app, bundleIdentifier];
        if bundle.is_null() {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![bundle, UTF8String];
        if utf8.is_null() {
            return None;
        }
        std::ffi::CStr::from_ptr(utf8)
            .to_str()
            .ok()
            .map(|s| s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_key_spec_command_shift_p() {
        let (mods, key) = parse_key_spec("command+shift+p").unwrap();
        assert_eq!(mods, vec![BgModifier::Command, BgModifier::Shift]);
        assert_eq!(key, 35);
    }

    #[test]
    fn parse_key_spec_named_return() {
        let (mods, key) = parse_key_spec("return").unwrap();
        assert!(mods.is_empty());
        assert_eq!(key, 36);
    }

    #[test]
    fn parse_key_spec_aliases() {
        let (mods, _) = parse_key_spec("cmd+opt+a").unwrap();
        assert_eq!(mods, vec![BgModifier::Command, BgModifier::Option]);
    }

    #[test]
    fn parse_key_sequence_array_chord() {
        let keys = vec!["command".to_string(), "shift".to_string(), "p".to_string()];
        let (mods, key) = parse_key_sequence(&keys).unwrap();
        assert_eq!(mods, vec![BgModifier::Command, BgModifier::Shift]);
        assert_eq!(key, 35);
    }

    #[test]
    fn parse_key_sequence_single_plus_spec() {
        let keys = vec!["command+f".to_string()];
        let (mods, key) = parse_key_sequence(&keys).unwrap();
        assert_eq!(mods, vec![BgModifier::Command]);
        assert_eq!(key, 3);
    }

    #[test]
    fn modifier_from_str_aliases() {
        assert_eq!(BgModifier::from_str("CMD"), Some(BgModifier::Command));
        assert_eq!(BgModifier::from_str("control"), Some(BgModifier::Control));
        assert_eq!(BgModifier::from_str("alt"), Some(BgModifier::Option));
        assert_eq!(BgModifier::from_str("fn"), Some(BgModifier::Fn));
        assert_eq!(BgModifier::from_str("zzz"), None);
    }

    #[test]
    fn flags_from_combines() {
        let f = flags_from(&[BgModifier::Command, BgModifier::Shift]);
        assert!(f.contains(CGEventFlags::CGEventFlagCommand));
        assert!(f.contains(CGEventFlags::CGEventFlagShift));
        assert!(!f.contains(CGEventFlags::CGEventFlagControl));
    }

    #[test]
    fn fn_modifier_flag_and_keycode() {
        assert_eq!(BgModifier::Fn.flag(), CGEventFlags::CGEventFlagSecondaryFn);
        assert_eq!(BgModifier::Fn.keycode(), 63);
    }
}

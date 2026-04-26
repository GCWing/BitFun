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
//! Used by the AX-first dispatch path in ControlHub: when an `app_*` action
//! cannot be satisfied by `AXUIElementPerformAction` alone (e.g. scroll,
//! free-form typing, complex chords) we fall back to PID-targeted events
//! from this module instead of the global foreground click path.
//!
//! Wired up by the next todos (`macos-ax-write` + `controlhub-actions`);
//! kept as standalone helpers here so it can be unit-tested and audited
//! independently of the dispatch glue.

#![allow(dead_code)]

use bitfun_core::util::errors::{BitFunError, BitFunResult};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventType, CGMouseButton, ScrollEventUnit};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use log::{debug, info, warn};
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
/// Maps to the 4 standard macOS modifier flag bits. We deliberately do not
/// touch `CapsLock` here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgModifier {
    Command,
    Shift,
    Option, // alias: alt
    Control,
}

impl BgModifier {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "meta" | "super" => Some(Self::Command),
            "shift" => Some(Self::Shift),
            "alt" | "option" | "opt" => Some(Self::Option),
            "ctrl" | "control" => Some(Self::Control),
            _ => None,
        }
    }
    fn flag(self) -> CGEventFlags {
        match self {
            Self::Command => CGEventFlags::CGEventFlagCommand,
            Self::Shift => CGEventFlags::CGEventFlagShift,
            Self::Option => CGEventFlags::CGEventFlagAlternate,
            Self::Control => CGEventFlags::CGEventFlagControl,
        }
    }
    fn keycode(self) -> u16 {
        match self {
            Self::Command => 55,
            Self::Shift => 56,
            Self::Option => 58,
            Self::Control => 59,
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
        ev.post_to_pid(me);
        true
    })();
    if probe_ok {
        CACHED_OK.store(true, Ordering::Relaxed);
    }
    probe_ok
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
    mv.post_to_pid(pid);

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
        down.post_to_pid(pid);

        let up = CGEvent::new_mouse_event(src.clone(), button.up(), pt, button.cg())
            .map_err(|_| BitFunError::tool("CGEvent MouseUp failed".to_string()))?;
        up.set_integer_value_field(
            core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
            i as i64,
        );
        if !flags.is_empty() {
            up.set_flags(flags);
        }
        up.post_to_pid(pid);
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
fn frontmost_pid_macos() -> Option<i32> {
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
/// to the right window. Uses the public NSRunningApplication API.
///
/// Returns `Ok(true)` when the activation call returned success, `Ok(false)`
/// when the app could not be found, and `Err(_)` on AppKit FFI failures.
pub fn activate_pid_macos(pid: i32) -> BitFunResult<bool> {
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
    ev.post_to_pid(pid);
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
        ev.post_to_pid(pid);
        // Match keyup so the target app sees a complete keystroke.
        let ev2 = CGEvent::new_keyboard_event(src.clone(), 0, false)
            .map_err(|_| BitFunError::tool("CGEventCreateKeyboardEvent (up) failed".to_string()))?;
        ev2.set_string_from_utf16_unchecked(&buf);
        ev2.post_to_pid(pid);
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
        ev.post_to_pid(pid);
    }
    // Press main key.
    {
        let ev = CGEvent::new_keyboard_event(src.clone(), key, true)
            .map_err(|_| BitFunError::tool("CGEvent KeyDown failed".to_string()))?;
        ev.set_flags(flags);
        ev.post_to_pid(pid);
    }
    {
        let ev = CGEvent::new_keyboard_event(src.clone(), key, false)
            .map_err(|_| BitFunError::tool("CGEvent KeyUp failed".to_string()))?;
        ev.set_flags(flags);
        ev.post_to_pid(pid);
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
        ev.post_to_pid(pid);
    }
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
        assert_eq!(BgModifier::from_str("zzz"), None);
    }

    #[test]
    fn flags_from_combines() {
        let f = flags_from(&[BgModifier::Command, BgModifier::Shift]);
        assert!(f.contains(CGEventFlags::CGEventFlagCommand));
        assert!(f.contains(CGEventFlags::CGEventFlagShift));
        assert!(!f.contains(CGEventFlags::CGEventFlagControl));
    }
}

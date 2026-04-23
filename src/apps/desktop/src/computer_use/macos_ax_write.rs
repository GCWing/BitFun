//! AX-first writers: prefer `AXUIElementPerformAction` /
//! `AXUIElementSetAttributeValue` over synthetic `CGEvent` injection.
//!
//! The dispatch layer's contract:
//!   1. Resolve `(pid, idx)` to a live `AxRef` via `macos_ax_dump::cached_ref`.
//!   2. Try the AX path here. On success: zero foreground impact, no event
//!      taps fired, accessibility services see a real semantic action.
//!   3. On failure (`Err(AxWriteUnavailable)`): the dispatch layer falls back
//!      to `macos_bg_input` (background `CGEvent` injection to the pid).
//!
//! This mirrors Codex: AX-first for correctness + speed, event-fallback for
//! pathological apps that refuse `AXPress` / `AXSetValue`.

#![allow(dead_code)]

use crate::computer_use::macos_ax_dump::AxRef;
use core_foundation::base::{CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};

type AXUIElementRef = *const std::ffi::c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> i32;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> i32;
}

/// Result of an AX-first attempt.
#[derive(Debug)]
pub enum AxWriteOutcome {
    /// The AX call succeeded — no fallback needed.
    Ok,
    /// AX rejected the call (status non-zero or unsupported). Caller should
    /// fall through to event injection.
    Unavailable(i32),
}

/// Try to "click" via AXPress. Most controls (NSButton, links, menu items)
/// implement this; many text fields and webviews do not.
pub fn try_ax_press(target: AxRef) -> AxWriteOutcome {
    if target.0.is_null() {
        return AxWriteOutcome::Unavailable(-1);
    }
    let action = CFString::new("AXPress");
    let st = unsafe { AXUIElementPerformAction(target.0, action.as_concrete_TypeRef()) };
    if st == 0 {
        AxWriteOutcome::Ok
    } else {
        AxWriteOutcome::Unavailable(st)
    }
}

/// Try to set the AXValue of a text field. `value` is sent as a CFString.
/// Caller is responsible for any subsequent focus / commit (Tab, Return).
pub fn try_ax_set_value(target: AxRef, value: &str) -> AxWriteOutcome {
    if target.0.is_null() {
        return AxWriteOutcome::Unavailable(-1);
    }
    let attr = CFString::new("AXValue");
    let v = CFString::new(value);
    let st = unsafe {
        AXUIElementSetAttributeValue(
            target.0,
            attr.as_concrete_TypeRef(),
            v.as_concrete_TypeRef() as CFTypeRef,
        )
    };
    if st == 0 {
        AxWriteOutcome::Ok
    } else {
        AxWriteOutcome::Unavailable(st)
    }
}

/// Try a generic AX action by name (e.g. `"AXShowMenu"`, `"AXIncrement"`).
pub fn try_ax_action(target: AxRef, action_name: &str) -> AxWriteOutcome {
    if target.0.is_null() {
        return AxWriteOutcome::Unavailable(-1);
    }
    let a = CFString::new(action_name);
    let st = unsafe { AXUIElementPerformAction(target.0, a.as_concrete_TypeRef()) };
    if st == 0 {
        AxWriteOutcome::Ok
    } else {
        AxWriteOutcome::Unavailable(st)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Null AX refs must short-circuit to `Unavailable(-1)` so the dispatch
    /// layer falls back to event injection instead of dereferencing a null
    /// pointer in the AX framework.
    #[test]
    fn null_ref_press_returns_unavailable() {
        let r = AxRef(std::ptr::null());
        match try_ax_press(r) {
            AxWriteOutcome::Unavailable(-1) => {}
            other => panic!("expected Unavailable(-1), got {:?}", other),
        }
    }

    #[test]
    fn null_ref_set_value_returns_unavailable() {
        let r = AxRef(std::ptr::null());
        match try_ax_set_value(r, "hello") {
            AxWriteOutcome::Unavailable(-1) => {}
            other => panic!("expected Unavailable(-1), got {:?}", other),
        }
    }

    #[test]
    fn null_ref_action_returns_unavailable() {
        let r = AxRef(std::ptr::null());
        match try_ax_action(r, "AXShowMenu") {
            AxWriteOutcome::Unavailable(-1) => {}
            other => panic!("expected Unavailable(-1), got {:?}", other),
        }
    }
}

//! macOS-only low-level FFI helpers for the desktop Computer Use host:
//! Accessibility trust / prompt, screen-capture permission preflight, Quartz
//! pointer location, and the main-thread / Objective-C exception dispatch
//! wrappers required by AppKit / HIToolbox / Accessibility calls.
//!
//! Extracted verbatim from `desktop_host.rs` (no behavior change) so the
//! permission and thread-dispatch primitives have a single, independently
//! reviewable home instead of living inline inside the multi-thousand-line
//! host file.

use super::{BitFunError, BitFunResult};
use core_foundation::base::{CFRelease, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use dispatch::Queue;
use std::ffi::c_void;

#[repr(C)]
#[derive(Copy, Clone)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "System", kind = "dylib")]
unsafe extern "C" {
    fn pthread_main_np() -> i32;
}

/// Run work that may call TSM / HIToolbox (enigo keyboard & text) on the main dispatch queue.
///
/// The closure is wrapped in `objc2::exception::catch` so that any
/// Objective-C `NSException` thrown by TSM / HIToolbox / AppKit (which
/// historically appears as `__rust_foreign_exception` and aborts the
/// process when it crosses back into the Rust runtime) is converted into
/// a `BitFunError` we can return to the caller. The closure must itself
/// return a `BitFunResult<T>` so we can flatten the two error sources
/// (ObjC exception + Rust-side error) into one.
pub(super) fn run_on_main_for_enigo<F, T>(f: F) -> BitFunResult<T>
where
    F: FnOnce() -> BitFunResult<T> + Send,
    T: Send,
{
    let work = move || catch_only(f);
    unsafe {
        if pthread_main_np() != 0 {
            work()
        } else {
            Queue::main().exec_sync(work)
        }
    }
}

/// Run a closure on the main dispatch queue under an Objective-C
/// `@try/@catch`. This is the correct wrapper for calls that may reach
/// AppKit / HIToolbox / Accessibility code paths from a background
/// (`tokio::spawn_blocking`) worker thread.
///
/// Two failure modes are defended against simultaneously:
///
///   1. `NSException` thrown by the framework (caught and converted into
///      `BitFunError`).
///   2. AppKit's `__assert_rtn` "Must only be used from the main thread"
///      `SIGTRAP` which fires when AX cross-process callbacks (e.g.
///      `AXUIElementCopyActionNames` → `_NSThemeWidgetCell.accessibility…`
///      → `_WMWindow performUpdatesUsingBlock:`) are evaluated off the
///      main thread. `objc2::exception::catch` cannot intercept this
///      trap; the only fix is to actually run the closure on the main
///      thread, which is what this helper does.
///
/// If we're already on the main thread we run inline (avoids
/// `dispatch_sync(main)` deadlock).
pub(super) fn catch_objc<F, T>(f: F) -> BitFunResult<T>
where
    F: FnOnce() -> BitFunResult<T> + Send,
    T: Send,
{
    unsafe {
        let on_main = pthread_main_np() != 0;
        if on_main {
            catch_only(f)
        } else {
            Queue::main().exec_sync(move || catch_only(f))
        }
    }
}

/// Run a closure under an Objective-C `@try/@catch` **on the current
/// thread** (no main-queue dispatch). Use this for closures that borrow
/// non-`Send` data and that are guaranteed not to reach AppKit's
/// main-thread-only AX callbacks (e.g. Vision OCR on an in-memory
/// screenshot buffer).
pub(super) fn catch_objc_local<F, T>(f: F) -> BitFunResult<T>
where
    F: FnOnce() -> BitFunResult<T>,
{
    catch_only(f)
}

fn catch_only<F, T>(f: F) -> BitFunResult<T>
where
    F: FnOnce() -> BitFunResult<T>,
{
    use std::panic::AssertUnwindSafe;
    match objc2::exception::catch(AssertUnwindSafe(f)) {
        Ok(inner) => inner,
        Err(Some(exc)) => Err(BitFunError::tool(format!("Objective-C exception: {}", exc))),
        Err(None) => Err(BitFunError::tool("Objective-C exception (nil)".to_string())),
    }
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGEventCreate(source: *const c_void) -> *const c_void;
    fn CGEventGetLocation(event: *const c_void) -> CGPoint;
}

/// Mouse location in Quartz global coordinates (same space as `CGEvent` / `CGWarpMouseCursorPosition`).
pub(super) fn quartz_mouse_location() -> BitFunResult<(f64, f64)> {
    unsafe {
        let ev = CGEventCreate(std::ptr::null());
        if ev.is_null() {
            return Err(BitFunError::tool(
                "CGEventCreate returned null (pointer overlay).".to_string(),
            ));
        }
        let pt = CGEventGetLocation(ev);
        CFRelease(ev as *const _);
        Ok((pt.x, pt.y))
    }
}

pub(super) fn ax_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

pub(super) fn screen_capture_preflight() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

pub(super) fn request_ax_prompt() {
    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let val = CFBoolean::true_value();
    let dict = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), val.as_CFType())]);
    unsafe {
        AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef() as *const _);
    }
}

pub(super) fn request_screen_capture() -> bool {
    unsafe { CGRequestScreenCaptureAccess() }
}

/// Shared AX-first desktop guard: `get_app_state_inner` and
/// `get_app_shortcuts_inner` both silently degrade (truncated tree / empty
/// menu bar) instead of erroring when Accessibility trust is missing, which
/// looks like "nothing here" rather than "BitFun lacks permission". Fail
/// fast with a structured `[PERMISSION_DENIED]` error and re-trigger the
/// system prompt so the user has a way back to the dialog without digging
/// through System Settings manually.
///
/// `retry_hint` is the caller-specific closing sentence (e.g. `"After
/// granting, retry \`desktop.get_app_state\` and the AX tree will include
/// all WebView subtree nodes."`), so each call site keeps its own precise
/// guidance while the shared boilerplate (check + prompt + explanation)
/// cannot drift between call sites.
pub(super) fn require_ax_trust_for(retry_hint: &str) -> BitFunResult<()> {
    if ax_trusted() {
        return Ok(());
    }
    request_ax_prompt();
    Err(BitFunError::tool(format!(
        "[PERMISSION_DENIED] macOS Accessibility permission not granted to BitFun. \
         The system has been asked to surface the permission dialog (System Settings → \
         Privacy & Security → Accessibility → enable BitFun). {}",
        retry_hint
    )))
}

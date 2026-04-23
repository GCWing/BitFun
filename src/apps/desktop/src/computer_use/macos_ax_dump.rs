//! Codex-style macOS Accessibility (AX) tree dump.
//!
//! Walks an application's full AX tree (BFS) starting from a `pid`, emits:
//!   * a human-readable indented `tree_text` (Codex parity),
//!   * a structured `Vec<AxNode>` with stable, monotonic `idx` values,
//!   * a sha1 `digest` over the structural fingerprint so callers can detect
//!     "did anything change?" cheaply,
//!   * a per-pid cache mapping `idx → AXUIElementRef` so subsequent
//!     `app_click` / `app_type_text` / ... actions can resolve a numeric idx
//!     back to a live AX element without re-walking.
//!
//! All AX refs returned in the cache are `CFRetain`-ed and released when
//! the snapshot for that pid is replaced.

// Symbols here are wired up by the ControlHub `desktop.*` dispatch layer in a
// follow-up step (`controlhub-actions`). Until then, suppress dead-code lints
// without weakening real warnings elsewhere.
#![allow(dead_code)]

use bitfun_core::agentic::tools::computer_use_host::{AppStateSnapshot, AxNode};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFGetTypeID, CFTypeRef, TCFType};
use core_foundation::boolean::{CFBooleanGetTypeID, CFBooleanRef};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::geometry::{CGPoint, CGSize};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

type CFNumberRef = *const c_void;
type CFTypeID = usize;
const K_CF_NUMBER_DOUBLE_TYPE: i32 = 13;
const K_CF_NUMBER_LONG_LONG_TYPE: i32 = 11;

type AXUIElementRef = *const c_void;
type AXValueRef = *const c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementCopyActionNames(element: AXUIElementRef, names: *mut CFArrayRef) -> i32;
    fn AXValueGetType(value: AXValueRef) -> u32;
    fn AXValueGetValue(value: AXValueRef, the_type: u32, ptr: *mut c_void) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    fn CFBooleanGetValue(boolean: CFBooleanRef) -> u8;
    fn CFStringGetTypeID() -> CFTypeID;
    fn CFNumberGetTypeID() -> CFTypeID;
    fn CFNumberIsFloatType(number: CFNumberRef) -> u8;
    fn CFNumberGetValue(number: CFNumberRef, the_type: i32, value_ptr: *mut c_void) -> u8;
}

const K_AX_VALUE_CGPOINT: u32 = 1;
const K_AX_VALUE_CGSIZE: u32 = 2;

// ── Wrappers around raw pointers so we can stash them in `Send`-able caches ─

/// Newtype wrapping `AXUIElementRef`. Manually implements `Send + Sync` —
/// AX refs are CF objects, safe to share across threads as long as we only
/// drop them with `CFRelease`. The cache is internally locked.
#[derive(Copy, Clone)]
pub(crate) struct AxRef(pub AXUIElementRef);
unsafe impl Send for AxRef {}
unsafe impl Sync for AxRef {}

impl AxRef {
    fn release(self) {
        if !self.0.is_null() {
            unsafe { core_foundation::base::CFRelease(self.0 as CFTypeRef) };
        }
    }
}

// ── Per-pid cache: snapshot id → idx → retained AXUIElementRef ─────────────
//
// We keep the most recent snapshot per pid only; resolving a stale `idx`
// against an old snapshot returns `None`, which the dispatch layer maps to
// `AX_NODE_STALE`.

struct CachedSnapshot {
    digest: String,
    refs: Vec<AxRef>,
}

impl Drop for CachedSnapshot {
    fn drop(&mut self) {
        for r in self.refs.drain(..) {
            r.release();
        }
    }
}

static SNAPSHOT_CACHE: OnceLock<Mutex<HashMap<i32, CachedSnapshot>>> = OnceLock::new();

fn snapshot_cache() -> &'static Mutex<HashMap<i32, CachedSnapshot>> {
    SNAPSHOT_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Resolve `(pid, idx)` to a live AX ref. Caller must NOT release it; the
/// cache owns the retain. Returns `None` if the snapshot has been replaced
/// (i.e. the digest no longer matches) or the idx is out of range.
pub(crate) fn cached_ref(pid: i32, expected_digest: Option<&str>, idx: u32) -> Option<AxRef> {
    let cache = snapshot_cache().lock().ok()?;
    let snap = cache.get(&pid)?;
    if let Some(want) = expected_digest {
        if snap.digest != want {
            return None;
        }
    }
    snap.refs.get(idx as usize).copied()
}

/// Like `cached_ref` but does not require a digest match. Used for
/// best-effort follow-up actions where the caller did not have a chance to
/// re-snapshot (e.g. `app_wait_for` polling).
pub(crate) fn cached_ref_loose(pid: i32, idx: u32) -> Option<AxRef> {
    cached_ref(pid, None, idx)
}

// ── Low-level CF / AX helpers (intentionally separate from macos_ax_ui.rs
//    to keep the older locate path self-contained and untouched) ──────────

unsafe fn ax_release(v: CFTypeRef) {
    if !v.is_null() {
        core_foundation::base::CFRelease(v);
    }
}

unsafe fn ax_copy_attr(elem: AXUIElementRef, key: &str) -> Option<CFTypeRef> {
    let mut val: CFTypeRef = std::ptr::null();
    let k = CFString::new(key);
    let st = AXUIElementCopyAttributeValue(elem, k.as_concrete_TypeRef(), &mut val);
    if st != 0 || val.is_null() {
        if !val.is_null() {
            ax_release(val);
        }
        return None;
    }
    Some(val)
}

/// Safely convert a CF object to a Rust `String`. **MUST type-check first**:
/// blindly wrapping a non-CFString as `CFStringRef` and calling `.to_string()`
/// dispatches `_fastCStringContents:` to whatever class the object actually
/// is, raising an Objective-C `NSException` (`unrecognized selector …`) that
/// unwinds across the FFI boundary and either aborts the process or, if
/// caught, simply blanks out the entire AX snapshot.
///
/// This is the canonical foot-gun on Tauri / Electron / WebKit-hosted apps,
/// where `AXValue` on tabs is the selected child *element*, on toggles is a
/// `CFNumber`, on bool attributes is a `CFBoolean`, and on geometric
/// attributes is an opaque `AXValueRef` — none of which are strings.
unsafe fn cfstring_to_string(cf: CFTypeRef) -> Option<String> {
    if cf.is_null() {
        return None;
    }
    if CFGetTypeID(cf) != CFStringGetTypeID() {
        return None;
    }
    let s = CFString::wrap_under_get_rule(cf as CFStringRef);
    Some(s.to_string())
}

/// Best-effort: read an attribute and coerce *whatever* CF type comes back
/// into a printable string — strings stay verbatim, booleans become
/// `"true"`/`"false"`, numbers become decimal, AX value refs (CGPoint /
/// CGSize / CGRect) become `(x, y)` / `(w x h)` / `(x, y, w, h)`. Anything
/// else (e.g. an AXUIElementRef returned for `AXValue` on a tab group)
/// becomes `None` rather than blowing up.
unsafe fn cf_to_display_string(cf: CFTypeRef) -> Option<String> {
    if cf.is_null() {
        return None;
    }
    let tid = CFGetTypeID(cf);
    if tid == CFStringGetTypeID() {
        let s = CFString::wrap_under_get_rule(cf as CFStringRef);
        return Some(s.to_string());
    }
    if tid == CFBooleanGetTypeID() {
        return Some(if CFBooleanGetValue(cf as CFBooleanRef) != 0 {
            "true".to_string()
        } else {
            "false".to_string()
        });
    }
    if tid == CFNumberGetTypeID() {
        let nref = cf as CFNumberRef;
        if CFNumberIsFloatType(nref) != 0 {
            let mut d: f64 = 0.0;
            if CFNumberGetValue(
                nref,
                K_CF_NUMBER_DOUBLE_TYPE,
                &mut d as *mut _ as *mut c_void,
            ) != 0
            {
                // Trim trailing zeros for cleaner display (1.0 → "1").
                let s = format!("{}", d);
                return Some(s);
            }
            return None;
        } else {
            let mut i: i64 = 0;
            if CFNumberGetValue(
                nref,
                K_CF_NUMBER_LONG_LONG_TYPE,
                &mut i as *mut _ as *mut c_void,
            ) != 0
            {
                return Some(i.to_string());
            }
            return None;
        }
    }
    // CGPoint / CGSize / CGRect / CFRange via AXValueRef.
    if let Some(p) = ax_value_to_point(cf) {
        return Some(format!("({}, {})", p.x, p.y));
    }
    if let Some(s) = ax_value_to_size(cf) {
        return Some(format!("({} x {})", s.width, s.height));
    }
    None
}

unsafe fn read_cf_string_attr(elem: AXUIElementRef, key: &str) -> Option<String> {
    let v = ax_copy_attr(elem, key)?;
    let s = cfstring_to_string(v);
    ax_release(v);
    s
}

/// Like `read_cf_string_attr` but accepts numbers / booleans / AXValues too
/// (used for `AXValue`, which on macOS can be almost anything depending on
/// the role).
unsafe fn read_cf_value_attr(elem: AXUIElementRef, key: &str) -> Option<String> {
    let v = ax_copy_attr(elem, key)?;
    let s = cf_to_display_string(v);
    ax_release(v);
    s
}

unsafe fn read_cf_bool_attr(elem: AXUIElementRef, key: &str) -> Option<bool> {
    let v = ax_copy_attr(elem, key)?;
    let mut out = None;
    if CFGetTypeID(v) == CFBooleanGetTypeID() {
        out = Some(CFBooleanGetValue(v as CFBooleanRef) != 0);
    }
    ax_release(v);
    out
}

/// Returns `Some(point)` only if `v` is a non-null AXValueRef encoding a
/// CGPoint. Safe to call on any CFTypeRef — non-AXValue inputs return `None`.
unsafe fn ax_value_to_point(v: CFTypeRef) -> Option<CGPoint> {
    if v.is_null() {
        return None;
    }
    let av = v as AXValueRef;
    if AXValueGetType(av) != K_AX_VALUE_CGPOINT {
        return None;
    }
    let mut pt = CGPoint { x: 0.0, y: 0.0 };
    if !AXValueGetValue(av, K_AX_VALUE_CGPOINT, &mut pt as *mut _ as *mut c_void) {
        return None;
    }
    Some(pt)
}

unsafe fn ax_value_to_size(v: CFTypeRef) -> Option<CGSize> {
    if v.is_null() {
        return None;
    }
    let av = v as AXValueRef;
    if AXValueGetType(av) != K_AX_VALUE_CGSIZE {
        return None;
    }
    let mut sz = CGSize {
        width: 0.0,
        height: 0.0,
    };
    if !AXValueGetValue(av, K_AX_VALUE_CGSIZE, &mut sz as *mut _ as *mut c_void) {
        return None;
    }
    Some(sz)
}

unsafe fn read_global_frame(elem: AXUIElementRef) -> Option<(f64, f64, f64, f64)> {
    let pos = ax_copy_attr(elem, "AXPosition")?;
    let size = ax_copy_attr(elem, "AXSize")?;
    let pt = ax_value_to_point(pos);
    let sz = ax_value_to_size(size);
    ax_release(pos);
    ax_release(size);
    let pt = pt?;
    let sz = sz?;
    Some((pt.x, pt.y, sz.width, sz.height))
}

unsafe fn read_action_names(elem: AXUIElementRef) -> Vec<String> {
    let mut names: CFArrayRef = std::ptr::null();
    let st = AXUIElementCopyActionNames(elem, &mut names);
    if st != 0 || names.is_null() {
        return vec![];
    }
    let arr = CFArray::<*const c_void>::wrap_under_create_rule(names);
    let mut out = Vec::with_capacity(arr.len() as usize);
    for i in 0..arr.len() {
        if let Some(s) = arr.get(i) {
            let p = *s;
            if !p.is_null() {
                out.push(CFString::wrap_under_get_rule(p as CFStringRef).to_string());
            }
        }
    }
    out
}

// ── BFS walker ────────────────────────────────────────────────────────────

struct Queued {
    elem: AXUIElementRef,
    parent_idx: Option<u32>,
    depth: u32,
}

/// Configurable knobs for the dump. Defaults mirror what the dispatch layer
/// will call with: depth 32, focus_window_only false, capped at 4000 nodes.
pub struct DumpOpts {
    pub max_depth: u32,
    pub max_nodes: usize,
    pub focus_window_only: bool,
}

impl Default for DumpOpts {
    fn default() -> Self {
        Self {
            max_depth: 32,
            max_nodes: 4_000,
            focus_window_only: false,
        }
    }
}

pub fn dump_app_ax(pid: i32, opts: DumpOpts) -> BitFunResult<AppStateSnapshot> {
    let app = unsafe { AXUIElementCreateApplication(pid) };
    if app.is_null() {
        return Err(BitFunError::tool(format!(
            "AXUIElementCreateApplication returned null for pid={}",
            pid
        )));
    }

    // Pick the root we'll walk.
    let root = if opts.focus_window_only {
        unsafe {
            try_focused_window(app).unwrap_or_else(|| {
                // Retain the app element so we can drop both consistently.
                CFRetain(app as CFTypeRef) as AXUIElementRef
            })
        }
    } else {
        unsafe { CFRetain(app as CFTypeRef) as AXUIElementRef }
    };

    let window_title = unsafe { try_focused_window(app) }.and_then(|w| {
        let t = unsafe { read_cf_string_attr(w, "AXTitle") };
        unsafe { ax_release(w as CFTypeRef) };
        t
    });

    // We're done with the app handle for now (root is independently retained).
    unsafe { ax_release(app as CFTypeRef) };

    let mut nodes: Vec<AxNode> = Vec::new();
    let mut refs: Vec<AxRef> = Vec::new();
    let mut queue: VecDeque<Queued> = VecDeque::new();
    queue.push_back(Queued {
        elem: root,
        parent_idx: None,
        depth: 0,
    });
    let mut visited: usize = 0;

    while let Some(cur) = queue.pop_front() {
        if cur.depth > opts.max_depth || visited >= opts.max_nodes {
            unsafe { ax_release(cur.elem as CFTypeRef) };
            continue;
        }
        visited += 1;

        let idx = nodes.len() as u32;
        let role = unsafe { read_cf_string_attr(cur.elem, "AXRole") };
        let role_description = unsafe { read_cf_string_attr(cur.elem, "AXRoleDescription") };
        let subrole = unsafe { read_cf_string_attr(cur.elem, "AXSubrole") };
        let title = unsafe { read_cf_string_attr(cur.elem, "AXTitle") };
        // AXValue is the canonical foot-gun: on a slider it's a CFNumber, on
        // a toggle it's a CFBoolean, on a tab group it's an AXUIElementRef
        // pointing at the selected child. Use the type-tolerant reader.
        let value = unsafe { read_cf_value_attr(cur.elem, "AXValue") };
        let description = unsafe { read_cf_string_attr(cur.elem, "AXDescription") };
        let help = unsafe { read_cf_string_attr(cur.elem, "AXHelp") };
        let identifier = unsafe { read_cf_string_attr(cur.elem, "AXIdentifier") };
        let url = unsafe { read_cf_string_attr(cur.elem, "AXURL") };
        let enabled = unsafe { read_cf_bool_attr(cur.elem, "AXEnabled") };
        let focused = unsafe { read_cf_bool_attr(cur.elem, "AXFocused") };
        let selected = unsafe { read_cf_bool_attr(cur.elem, "AXSelected") };
        let expanded = unsafe { read_cf_bool_attr(cur.elem, "AXExpanded") };
        let frame = unsafe { read_global_frame(cur.elem) };
        let actions = unsafe { read_action_names(cur.elem) };

        nodes.push(AxNode {
            idx,
            parent_idx: cur.parent_idx,
            role: role.unwrap_or_default(),
            title,
            value,
            description,
            identifier,
            enabled: enabled.unwrap_or(true),
            focused: focused.unwrap_or(false),
            selected,
            frame_global: frame,
            actions,
            role_description,
            subrole,
            help,
            url,
            expanded,
        });
        // Cache the retained ref so future actions can look it up.
        refs.push(AxRef(cur.elem));

        // Enqueue children — but DO NOT release `cur.elem`; the cache owns it.
        let children_ref = unsafe { ax_copy_attr(cur.elem, "AXChildren") };
        let next_depth = cur.depth + 1;
        let Some(ch) = children_ref else { continue };
        unsafe {
            let arr = CFArray::<*const c_void>::wrap_under_create_rule(ch as CFArrayRef);
            for i in 0..arr.len() {
                let Some(slot) = arr.get(i) else { continue };
                let child = *slot;
                if child.is_null() {
                    continue;
                }
                let retained = CFRetain(child as CFTypeRef) as AXUIElementRef;
                if !retained.is_null() {
                    queue.push_back(Queued {
                        elem: retained,
                        parent_idx: Some(idx),
                        depth: next_depth,
                    });
                }
            }
        }
    }
    // Drain anything we didn't walk (depth-cap or node-cap overflow).
    while let Some(q) = queue.pop_front() {
        unsafe { ax_release(q.elem as CFTypeRef) };
    }

    let tree_text = render_tree_text(&nodes);
    let digest = compute_digest(&nodes);
    let captured_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // Install in cache, replacing any previous snapshot for this pid.
    {
        let mut cache = snapshot_cache()
            .lock()
            .map_err(|_| BitFunError::tool("AX snapshot cache poisoned".to_string()))?;
        cache.insert(
            pid,
            CachedSnapshot {
                digest: digest.clone(),
                refs,
            },
        );
    }

    Ok(AppStateSnapshot {
        app: bitfun_core::agentic::tools::computer_use_host::AppInfo {
            name: window_title.clone().unwrap_or_default(),
            bundle_id: None,
            pid: Some(pid),
            running: true,
            last_used_ms: None,
            launch_count: 0,
        },
        window_title,
        tree_text,
        nodes,
        digest,
        captured_at_ms,
        screenshot: None,
        loop_warning: None,
    })
}

/// Best-effort: prefer `AXFocusedWindow`, then `AXMainWindow`. Returns a
/// retained ref the caller must release (or hand to the cache).
unsafe fn try_focused_window(app: AXUIElementRef) -> Option<AXUIElementRef> {
    for key in ["AXFocusedWindow", "AXMainWindow"] {
        if let Some(v) = ax_copy_attr(app, key) {
            let elem = v as AXUIElementRef;
            if !elem.is_null() {
                return Some(elem);
            }
            ax_release(v);
        }
    }
    None
}

/// Render a Codex-style indented tree.
///
/// Layout per node (one line):
///
/// ```text
/// {indent}[{idx}] {label} title="…" value="…" id="…" desc="…" help="…" \
///         url="…" frame=(x,y,wxh) {flags…} actions=[AXPress,AXShowMenu]
/// ```
///
/// `{label}` prefers `role_description` (humanised) over `role`+`subrole`
/// because that's what a sighted user calls the element. Numeric `idx` is
/// always shown so the model can address nodes deterministically.
fn render_tree_text(nodes: &[AxNode]) -> String {
    let mut children: Vec<Vec<u32>> = vec![Vec::new(); nodes.len()];
    let mut roots: Vec<u32> = Vec::new();
    for n in nodes {
        match n.parent_idx {
            Some(p) => {
                if let Some(slot) = children.get_mut(p as usize) {
                    slot.push(n.idx);
                }
            }
            None => roots.push(n.idx),
        }
    }
    let mut out = String::new();
    let mut stack: Vec<(u32, u32)> = roots.iter().rev().map(|&r| (r, 0u32)).collect();
    while let Some((idx, depth)) = stack.pop() {
        let n = &nodes[idx as usize];
        for _ in 0..depth {
            out.push_str("  ");
        }
        out.push_str(&format!("[{}] {}", n.idx, format_label(n)));
        if let Some(t) = &n.title {
            if !t.is_empty() {
                out.push_str(&format!(" title={}", quote_clip(t, 120)));
            }
        }
        if let Some(v) = &n.value {
            if !v.is_empty() {
                out.push_str(&format!(" value={}", quote_clip(v, 120)));
            }
        }
        if let Some(id) = &n.identifier {
            if !id.is_empty() {
                out.push_str(&format!(" id={}", quote_clip(id, 80)));
            }
        }
        if let Some(d) = &n.description {
            if !d.is_empty() {
                out.push_str(&format!(" desc={}", quote_clip(d, 120)));
            }
        }
        if let Some(h) = &n.help {
            if !h.is_empty() {
                out.push_str(&format!(" help={}", quote_clip(h, 120)));
            }
        }
        if let Some(u) = &n.url {
            if !u.is_empty() {
                out.push_str(&format!(" url={}", quote_clip(u, 200)));
            }
        }
        if let Some((x, y, w, h)) = n.frame_global {
            out.push_str(&format!(" frame=({:.0},{:.0},{:.0}x{:.0})", x, y, w, h));
        }
        if !n.enabled {
            out.push_str(" [disabled]");
        }
        if n.focused {
            out.push_str(" [focused]");
        }
        if let Some(true) = n.selected {
            out.push_str(" [selected]");
        }
        match n.expanded {
            Some(true) => out.push_str(" [expanded]"),
            Some(false) => out.push_str(" [collapsed]"),
            None => {}
        }
        // Surface non-trivial AX actions inline so the model can pick
        // AXShowMenu / AXIncrement / AXDecrement etc. without re-querying.
        let extra: Vec<&str> = n
            .actions
            .iter()
            .map(String::as_str)
            .filter(|a| !matches!(*a, "AXPress" | "AXShowAlternateUI" | "AXShowDefaultUI"))
            .collect();
        if !extra.is_empty() {
            out.push_str(&format!(" actions=[{}]", extra.join(",")));
        }
        out.push('\n');
        if let Some(kids) = children.get(idx as usize) {
            for &c in kids.iter().rev() {
                stack.push((c, depth + 1));
            }
        }
    }
    out
}

/// Compose a Codex-style label: prefer humanised role description, fall
/// back to `role + (subrole)`.
fn format_label(n: &AxNode) -> String {
    if let Some(rd) = &n.role_description {
        if !rd.is_empty() {
            return rd.clone();
        }
    }
    match &n.subrole {
        Some(s) if !s.is_empty() => format!("{}({})", n.role, s),
        _ => n.role.clone(),
    }
}

/// Quote a value, clipping at `max` chars (counted in bytes for safety on
/// arbitrary UTF-8 — we cut on a char boundary so we never split a code
/// point).
fn quote_clip(s: &str, max: usize) -> String {
    let trimmed: String = s.chars().take(max).collect();
    let escaped = trimmed.replace('\\', "\\\\").replace('"', "\\\"");
    if s.chars().count() > max {
        format!("\"{}…\"", escaped)
    } else {
        format!("\"{}\"", escaped)
    }
}

fn compute_digest(nodes: &[AxNode]) -> String {
    let mut h = Sha1::new();
    for n in nodes {
        h.update(n.idx.to_le_bytes());
        h.update(n.parent_idx.unwrap_or(u32::MAX).to_le_bytes());
        h.update(n.role.as_bytes());
        h.update(b"\x1f");
        h.update(n.subrole.as_deref().unwrap_or("").as_bytes());
        h.update(b"\x1f");
        h.update(n.title.as_deref().unwrap_or("").as_bytes());
        h.update(b"\x1f");
        h.update(n.identifier.as_deref().unwrap_or("").as_bytes());
        h.update(b"\x1f");
        h.update(n.description.as_deref().unwrap_or("").as_bytes());
        h.update(b"\x1f");
        h.update(n.help.as_deref().unwrap_or("").as_bytes());
        h.update(b"\x1f");
        h.update(n.value.as_deref().unwrap_or("").as_bytes());
        h.update(b"\x1f");
        h.update(n.url.as_deref().unwrap_or("").as_bytes());
        h.update(b"\x1f");
        h.update(match n.expanded {
            Some(true) => b"E"[..].to_vec(),
            Some(false) => b"C"[..].to_vec(),
            None => Vec::new(),
        });
        h.update(b"\x1f");
        for a in &n.actions {
            h.update(a.as_bytes());
            h.update(b",");
        }
        h.update(b"\x1e");
    }
    let bytes = h.finalize();
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_core::agentic::tools::computer_use_host::AxNode;

    fn n(idx: u32, parent: Option<u32>, role: &str, title: Option<&str>) -> AxNode {
        AxNode {
            idx,
            parent_idx: parent,
            role: role.to_string(),
            title: title.map(str::to_string),
            value: None,
            description: None,
            identifier: None,
            enabled: true,
            focused: false,
            selected: None,
            frame_global: None,
            actions: vec![],
            role_description: None,
            subrole: None,
            help: None,
            url: None,
            expanded: None,
        }
    }

    #[test]
    fn render_tree_text_indents_by_depth_and_orders_siblings() {
        let nodes = vec![
            n(0, None, "AXApplication", Some("Cursor")),
            n(1, Some(0), "AXWindow", Some("main")),
            n(2, Some(1), "AXButton", Some("Save")),
            n(3, Some(1), "AXButton", Some("Close")),
        ];
        let out = render_tree_text(&nodes);
        let expected =
            "[0] AXApplication title=\"Cursor\"\n  [1] AXWindow title=\"main\"\n    [2] AXButton title=\"Save\"\n    [3] AXButton title=\"Close\"\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn render_tree_text_uses_role_description_and_inline_flags() {
        let mut a = n(0, None, "AXButton", Some("Close"));
        a.role_description = Some("close button".to_string());
        a.help = Some("Close window".to_string());
        a.subrole = Some("AXCloseButton".to_string());
        a.frame_global = Some((10.0, 20.0, 30.0, 30.0));
        a.actions = vec!["AXPress".into(), "AXShowMenu".into()];
        a.focused = true;
        let out = render_tree_text(&[a]);
        // role_description wins over role/subrole; AXPress is filtered out
        // but AXShowMenu shows up as a secondary action.
        assert!(out.contains("[0] close button"));
        assert!(out.contains("title=\"Close\""));
        assert!(out.contains("help=\"Close window\""));
        assert!(out.contains("frame=(10,20,30x30)"));
        assert!(out.contains("[focused]"));
        assert!(out.contains("actions=[AXShowMenu]"));
    }

    #[test]
    fn quote_clip_truncates_on_char_boundary() {
        let s = "中文字符测试abcdef";
        let q = quote_clip(s, 4);
        assert_eq!(q, "\"中文字符…\"");
    }

    #[test]
    fn digest_changes_when_a_title_changes() {
        let mut a = vec![n(0, None, "AXButton", Some("Save"))];
        let d1 = compute_digest(&a);
        a[0].title = Some("Saved".to_string());
        let d2 = compute_digest(&a);
        assert_ne!(d1, d2);
    }

    /// Smoke test: dump the AX tree of *this* test process. The test process
    /// usually has no AX windows of its own, so we only assert the call
    /// returns *something* (possibly an empty tree) without panicking and
    /// produces a stable digest. Marked `#[ignore]` because it requires
    /// Accessibility permission for `cargo test` on macOS.
    #[test]
    #[ignore]
    fn dump_self_pid_returns_snapshot() {
        let pid = std::process::id() as i32;
        let snap = dump_app_ax(pid, DumpOpts::default()).expect("dump_app_ax should succeed");
        assert!(!snap.digest.is_empty(), "digest must be non-empty");
        assert_eq!(snap.app.pid, Some(pid));
    }

    #[test]
    fn digest_is_stable_for_same_input() {
        let nodes = vec![
            n(0, None, "AXWindow", Some("X")),
            n(1, Some(0), "AXButton", Some("Y")),
        ];
        assert_eq!(compute_digest(&nodes), compute_digest(&nodes));
    }
}

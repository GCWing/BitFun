//! macOS Accessibility (AX) tree search for stable UI centers (native “DOM”).
//!
//! Coordinates match CoreGraphics global space used by [`crate::computer_use::DesktopComputerUseHost`].

use crate::computer_use::ui_locate_common;
use bitfun_core::agentic::tools::computer_use_host::{SomElement, UiElementLocateQuery, UiElementLocateResult};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::geometry::{CGPoint, CGSize};
use std::collections::VecDeque;
use std::ffi::c_void;

type AXUIElementRef = *const c_void;
type AXValueRef = *const c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXValueGetType(value: AXValueRef) -> u32;
    fn AXValueGetValue(value: AXValueRef, the_type: u32, ptr: *mut c_void) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
}

const K_AX_VALUE_CGPOINT: u32 = 1;
const K_AX_VALUE_CGSIZE: u32 = 2;

fn frontmost_pid() -> BitFunResult<i32> {
    let out = std::process::Command::new("/usr/bin/osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get unix id of first process whose frontmost is true",
        ])
        .output()
        .map_err(|e| BitFunError::tool(format!("osascript spawn: {}", e)))?;
    if !out.status.success() {
        return Err(BitFunError::tool(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim()
        .parse::<i32>()
        .map_err(|_| BitFunError::tool("Could not parse frontmost process id.".to_string()))
}

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

unsafe fn cfstring_to_string(cf: CFTypeRef) -> Option<String> {
    if cf.is_null() {
        return None;
    }
    let s = CFString::wrap_under_get_rule(cf as CFStringRef);
    Some(s.to_string())
}

unsafe fn ax_value_to_point(v: CFTypeRef) -> Option<CGPoint> {
    let v = v as AXValueRef;
    let t = AXValueGetType(v);
    if t != K_AX_VALUE_CGPOINT {
        return None;
    }
    let mut pt = CGPoint { x: 0.0, y: 0.0 };
    if !AXValueGetValue(v, K_AX_VALUE_CGPOINT, &mut pt as *mut _ as *mut c_void) {
        return None;
    }
    Some(pt)
}

unsafe fn ax_value_to_size(v: CFTypeRef) -> Option<CGSize> {
    let v = v as AXValueRef;
    let t = AXValueGetType(v);
    if t != K_AX_VALUE_CGSIZE {
        return None;
    }
    let mut sz = CGSize {
        width: 0.0,
        height: 0.0,
    };
    if !AXValueGetValue(v, K_AX_VALUE_CGSIZE, &mut sz as *mut _ as *mut c_void) {
        return None;
    }
    Some(sz)
}

unsafe fn read_role_title_id(elem: AXUIElementRef) -> (Option<String>, Option<String>, Option<String>) {
    let role = ax_copy_attr(elem, "AXRole").and_then(|v| {
        let s = cfstring_to_string(v);
        ax_release(v);
        s
    });
    let title = ax_copy_attr(elem, "AXTitle").and_then(|v| {
        let s = cfstring_to_string(v);
        ax_release(v);
        s
    });
    let ident = ax_copy_attr(elem, "AXIdentifier").and_then(|v| {
        let s = cfstring_to_string(v);
        ax_release(v);
        s
    });
    (role, title, ident)
}

/// Global center and axis-aligned bounds from `AXPosition` + `AXSize`.
unsafe fn element_frame_global(elem: AXUIElementRef) -> Option<(f64, f64, f64, f64, f64, f64)> {
    let pos = ax_copy_attr(elem, "AXPosition")?;
    let size = ax_copy_attr(elem, "AXSize")?;
    let pt = ax_value_to_point(pos)?;
    let sz = ax_value_to_size(size)?;
    ax_release(pos);
    ax_release(size);
    if sz.width <= 0.0 || sz.height <= 0.0 {
        return None;
    }
    let left = pt.x;
    let top = pt.y;
    let w = sz.width;
    let h = sz.height;
    Some((left + w / 2.0, top + h / 2.0, left, top, w, h))
}

struct Queued {
    ax: AXUIElementRef,
    depth: u32,
    /// Parent's role + title for context (e.g. "AXWindow: Settings").
    parent_desc: Option<String>,
}

/// A candidate match found during BFS, before ranking.
struct CandidateMatch {
    gx: f64,
    gy: f64,
    bounds_left: f64,
    bounds_top: f64,
    bounds_width: f64,
    bounds_height: f64,
    role: String,
    title: Option<String>,
    identifier: Option<String>,
    parent_desc: Option<String>,
    depth: u32,
    /// Whether AXHidden is explicitly false / absent (visible).
    is_visible: bool,
}

impl CandidateMatch {
    /// Higher = better. Prefer visible, reasonably-sized, shallower, on-screen elements.
    fn rank_score(&self) -> i64 {
        let mut score: i64 = 0;

        // Visibility is critical
        if !self.is_visible {
            score -= 10000;
        }

        // Off-screen penalty
        if !ui_locate_common::is_element_on_screen(
            self.gx, self.gy, self.bounds_width, self.bounds_height,
        ) {
            score -= 5000;
        }

        // Prefer reasonably-sized elements (buttons, text fields) over huge containers
        let area = self.bounds_width * self.bounds_height;
        if area > 0.0 && area < 50000.0 {
            score += 100; // Small interactive element
        } else if area >= 50000.0 && area < 200000.0 {
            score += 50; // Medium element
        }
        // Very large elements (>200000 area) get no bonus -- likely containers

        // Prefer shallower elements (closer to the top of the tree = more likely
        // to be the "primary" instance vs a deeply nested duplicate)
        score -= self.depth as i64;

        // Bonus for elements in focused/active contexts
        if let Some(ref pd) = self.parent_desc {
            let pd_lower = pd.to_lowercase();
            if pd_lower.contains("sheet") || pd_lower.contains("dialog") || pd_lower.contains("popover") {
                score += 200; // Prefer elements in modal dialogs / sheets
            }
        }

        // Prefer elements with a non-empty title (more likely to be interactive)
        if self.title.as_ref().map_or(false, |t| !t.is_empty()) {
            score += 20;
        }

        score
    }

    fn short_description(&self) -> String {
        let title_str = self.title.as_deref().unwrap_or("");
        let parent_str = self.parent_desc.as_deref().unwrap_or("?");
        format!(
            "role={} title={:?} at ({:.0},{:.0}) size={:.0}x{:.0} parent=[{}]",
            self.role, title_str, self.gx, self.gy,
            self.bounds_width, self.bounds_height, parent_str
        )
    }
}

/// Check if an AX element has `AXHidden` set to true.
unsafe fn is_ax_hidden(elem: AXUIElementRef) -> bool {
    let Some(val) = ax_copy_attr(elem, "AXHidden") else {
        return false; // No AXHidden attribute = not hidden
    };
    // AXHidden is a CFBoolean
    let hidden = val as *const c_void == core_foundation::boolean::kCFBooleanTrue as *const c_void;
    ax_release(val);
    hidden
}

/// Build a short description string for an element (for use as parent context).
fn element_short_desc(role: Option<&str>, title: Option<&str>) -> String {
    let r = role.unwrap_or("?");
    match title {
        Some(t) if !t.is_empty() => format!("{}: {}", r, t),
        _ => r.to_string(),
    }
}

const MAX_CANDIDATES: usize = 10;

/// Search the **frontmost** app's accessibility tree (BFS) for elements matching filters.
/// Collects all matches, filters invisible/off-screen ones, ranks by relevance, returns the best.
pub fn locate_ui_element_center(query: &UiElementLocateQuery) -> BitFunResult<UiElementLocateResult> {
    ui_locate_common::validate_query(query)?;
    let max_depth = query.max_depth.unwrap_or(48).clamp(1, 200);
    let pid = frontmost_pid()?;
    let root = unsafe { AXUIElementCreateApplication(pid) };
    if root.is_null() {
        return Err(BitFunError::tool("AXUIElementCreateApplication returned null.".to_string()));
    }
    let mut bfs_queue = VecDeque::new();
    bfs_queue.push_back(Queued { ax: root, depth: 0, parent_desc: None });
    let mut visited = 0usize;
    let max_nodes = 12_000usize;
    let mut candidates: Vec<CandidateMatch> = Vec::new();

    while let Some(cur) = bfs_queue.pop_front() {
        if cur.depth > max_depth {
            unsafe { ax_release(cur.ax as CFTypeRef); }
            continue;
        }
        visited += 1;
        if visited > max_nodes {
            unsafe { ax_release(cur.ax as CFTypeRef); }
            // Drain remaining queue
            while let Some(c) = bfs_queue.pop_front() {
                unsafe { ax_release(c.ax as CFTypeRef); }
            }
            break;
        }

        let (role_s, title_s, id_s) = unsafe { read_role_title_id(cur.ax) };
        let role_ref = role_s.as_deref();
        let title_ref = title_s.as_deref();
        let id_ref = id_s.as_deref();

        let matched = ui_locate_common::matches_filters(query, role_ref, title_ref, id_ref);
        if matched {
            if let Some((gx, gy, bl, bt, bw, bh)) = unsafe { element_frame_global(cur.ax) } {
                let is_visible = !unsafe { is_ax_hidden(cur.ax) };
                candidates.push(CandidateMatch {
                    gx, gy,
                    bounds_left: bl, bounds_top: bt, bounds_width: bw, bounds_height: bh,
                    role: role_s.clone().unwrap_or_default(),
                    title: title_s.clone(),
                    identifier: id_s.clone(),
                    parent_desc: cur.parent_desc.clone(),
                    depth: cur.depth,
                    is_visible,
                });
                // Stop collecting after MAX_CANDIDATES to avoid excessive work
                if candidates.len() >= MAX_CANDIDATES {
                    unsafe { ax_release(cur.ax as CFTypeRef); }
                    while let Some(c) = bfs_queue.pop_front() {
                        unsafe { ax_release(c.ax as CFTypeRef); }
                    }
                    break;
                }
            }
        }

        // Build description for this node to pass as parent context to children
        let this_desc = element_short_desc(role_ref, title_ref);

        let children_ref = unsafe { ax_copy_attr(cur.ax, "AXChildren") };
        let next_depth = cur.depth + 1;
        unsafe { ax_release(cur.ax as CFTypeRef); }

        let Some(ch) = children_ref else {
            continue;
        };
        unsafe {
            let arr = CFArray::<*const c_void>::wrap_under_create_rule(ch as CFArrayRef);
            let n = arr.len();
            for i in 0..n {
                let Some(child_ref) = arr.get(i) else { continue; };
                let child = *child_ref;
                if child.is_null() { continue; }
                let retained = CFRetain(child as CFTypeRef) as AXUIElementRef;
                if !retained.is_null() {
                    bfs_queue.push_back(Queued {
                        ax: retained,
                        depth: next_depth,
                        parent_desc: Some(this_desc.clone()),
                    });
                }
            }
        }
    }

    if candidates.is_empty() {
        return Err(BitFunError::tool(
            "No accessibility element matched in the frontmost app. Tips: use `filter_combine: \"any\"` for OR matching; use only `role_substring` or only `title_contains`; match UI language; ensure the target app is focused. Or fall back to `screenshot` + vision path."
                .to_string(),
        ));
    }

    // Sort by rank score (descending)
    candidates.sort_by(|a, b| b.rank_score().cmp(&a.rank_score()));

    let total = candidates.len() as u32;
    let best = &candidates[0];

    // Build "other matches" summaries for the model to see alternatives
    let other_matches: Vec<String> = candidates.iter()
        .skip(1)
        .take(4)
        .map(|c| c.short_description())
        .collect();

    ui_locate_common::ok_result_with_context(
        best.gx, best.gy,
        best.bounds_left, best.bounds_top, best.bounds_width, best.bounds_height,
        best.role.clone(),
        best.title.clone(),
        best.identifier.clone(),
        best.parent_desc.clone(),
        total,
        other_matches,
    )
}


/// Roles considered "interactive" for Set-of-Mark labeling.
const SOM_INTERACTIVE_ROLES: &[&str] = &[
    "AXButton", "AXTextField", "AXTextArea", "AXCheckBox",
    "AXRadioButton", "AXPopUpButton", "AXComboBox", "AXSlider",
    "AXLink", "AXMenuItem", "AXMenuBarItem", "AXTab",
    "AXDisclosureTriangle", "AXIncrementor", "AXColorWell",
    "AXToolbarButton", "AXToggle", "AXSwitch", "AXSegmentedControl",
    "AXCell", "AXImage", "AXStaticText",
];

fn is_interactive_role(role: &str) -> bool {
    SOM_INTERACTIVE_ROLES.iter().any(|r| role.contains(r) || r.contains(role))
}

/// Enumerate all visible interactive elements in the frontmost app's AX tree.
/// Returns up to `max_elements` SomElement entries with 1-based label numbers.
pub fn enumerate_interactive_elements(max_elements: usize) -> Vec<SomElement> {
    let pid = match frontmost_pid() {
        Ok(p) => p,
        Err(_) => return vec![],
    };
    let root = unsafe { AXUIElementCreateApplication(pid) };
    if root.is_null() {
        return vec![];
    }

    struct BfsItem {
        ax: AXUIElementRef,
        depth: u32,
    }

    let mut queue = VecDeque::new();
    queue.push_back(BfsItem { ax: root, depth: 0 });
    let max_depth: u32 = 30;
    let max_nodes: usize = 8_000;
    let mut visited: usize = 0;
    let mut results: Vec<SomElement> = Vec::new();

    while let Some(cur) = queue.pop_front() {
        if cur.depth > max_depth || results.len() >= max_elements {
            unsafe { ax_release(cur.ax as CFTypeRef); }
            continue;
        }
        visited += 1;
        if visited > max_nodes {
            unsafe { ax_release(cur.ax as CFTypeRef); }
            while let Some(c) = queue.pop_front() {
                unsafe { ax_release(c.ax as CFTypeRef); }
            }
            break;
        }

        let (role_s, title_s, id_s) = unsafe { read_role_title_id(cur.ax) };
        let role = role_s.as_deref().unwrap_or("");

        // Check if this element is interactive and visible
        if is_interactive_role(role) {
            let hidden = unsafe { is_ax_hidden(cur.ax) };
            if !hidden {
                if let Some((gx, gy, bl, bt, bw, bh)) = unsafe { element_frame_global(cur.ax) } {
                    // Filter: reasonable size (not a giant container, not tiny)
                    if bw >= 4.0 && bh >= 4.0 && bw <= 2000.0 && bh <= 1000.0 {
                        // Filter: on-screen (center must be non-negative)
                        if gx >= 0.0 && gy >= 0.0 {
                            let label = results.len() as u32 + 1;
                            results.push(SomElement {
                                label,
                                role: role.to_string(),
                                title: title_s.clone().filter(|s| !s.is_empty()),
                                identifier: id_s.clone().filter(|s| !s.is_empty()),
                                global_center_x: gx,
                                global_center_y: gy,
                                bounds_left: bl,
                                bounds_top: bt,
                                bounds_width: bw,
                                bounds_height: bh,
                            });
                            if results.len() >= max_elements {
                                unsafe { ax_release(cur.ax as CFTypeRef); }
                                while let Some(c) = queue.pop_front() {
                                    unsafe { ax_release(c.ax as CFTypeRef); }
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Enqueue children
        let children_ref = unsafe { ax_copy_attr(cur.ax, "AXChildren") };
        let next_depth = cur.depth + 1;
        unsafe { ax_release(cur.ax as CFTypeRef); }

        let Some(ch) = children_ref else { continue; };
        unsafe {
            let arr = CFArray::<*const c_void>::wrap_under_create_rule(ch as CFArrayRef);
            let n = arr.len();
            for i in 0..n {
                let Some(child_ref) = arr.get(i) else { continue; };
                let child = *child_ref;
                if child.is_null() { continue; }
                let retained = CFRetain(child as CFTypeRef) as AXUIElementRef;
                if !retained.is_null() {
                    queue.push_back(BfsItem { ax: retained, depth: next_depth });
                }
            }
        }
    }

    results
}

// ── Raw OCR: frontmost window bounds (separate from agent screenshot pipeline) ─────────────────

/// Bounds of the foreground app's focused or main window in global screen coordinates (same space as pointer / screen capture).
/// Used to crop **raw** pixels for Vision OCR without pointer/SoM overlays from the agent screenshot path.
pub fn frontmost_window_bounds_global() -> BitFunResult<(i32, i32, u32, u32)> {
    let pid = frontmost_pid()?;
    let app = unsafe { AXUIElementCreateApplication(pid) };
    if app.is_null() {
        return Err(BitFunError::tool(
            "AXUIElementCreateApplication returned null for OCR window bounds.".to_string(),
        ));
    }
    unsafe {
        let win = try_frontmost_window_element(app);
        ax_release(app as CFTypeRef);
        let Some(win) = win else {
            return Err(BitFunError::tool(
                "No AX window for foreground app (try AXFocusedWindow / AXMainWindow / AXWindows).".to_string(),
            ));
        };
        let frame = element_frame_global(win).ok_or_else(|| {
            ax_release(win as CFTypeRef);
            BitFunError::tool("Could not read AXPosition/AXSize for foreground window.".to_string())
        })?;
        ax_release(win as CFTypeRef);
        let (_, _, bl, bt, bw, bh) = frame;
        if bw < 1.0 || bh < 1.0 {
            return Err(BitFunError::tool(
                "Foreground window has invalid size for OCR.".to_string(),
            ));
        }
        let x0 = bl.floor() as i32;
        let y0 = bt.floor() as i32;
        let w = bw.ceil().max(1.0) as u32;
        let h = bh.ceil().max(1.0) as u32;
        Ok((x0, y0, w, h))
    }
}

unsafe fn try_frontmost_window_element(app: AXUIElementRef) -> Option<AXUIElementRef> {
    for key in ["AXFocusedWindow", "AXMainWindow"] {
        if let Some(w) = ax_copy_attr(app, key) {
            let elem = w as AXUIElementRef;
            if !elem.is_null() && element_frame_global(elem).is_some() {
                return Some(elem);
            }
            ax_release(w);
        }
    }
    first_ax_window_from_ax_windows(app)
}

fn is_text_editing_ax_role(role: &str) -> bool {
    matches!(
        role,
        "AXTextField" | "AXTextArea" | "AXComboBox" | "AXSearchField" | "AXSecureTextField"
    )
}

unsafe fn ax_focused_element_from_system_wide() -> Option<AXUIElementRef> {
    let sys = AXUIElementCreateSystemWide();
    if sys.is_null() {
        return None;
    }
    let mut focused: CFTypeRef = std::ptr::null();
    let k = CFString::new("AXFocusedUIElement");
    let st = AXUIElementCopyAttributeValue(sys, k.as_concrete_TypeRef(), &mut focused);
    if st != 0 || focused.is_null() {
        if !focused.is_null() {
            ax_release(focused);
        }
        return None;
    }
    Some(focused as AXUIElementRef)
}

/// Best-effort global (x, y) for a 500×500 screenshot centered near the focused text field (AX element center).
/// Returns `None` if no suitable focused text UI; caller should fall back to the mouse position.
pub fn global_point_for_text_caret_screenshot(mx: f64, my: f64) -> (f64, f64) {
    unsafe {
        let Some(el) = ax_focused_element_from_system_wide() else {
            return (mx, my);
        };
        let (role, _, _) = read_role_title_id(el);
        let Some(role) = role.as_deref() else {
            ax_release(el as CFTypeRef);
            return (mx, my);
        };
        if !is_text_editing_ax_role(role) {
            ax_release(el as CFTypeRef);
            return (mx, my);
        }
        let Some((gx, gy, _, _, _, _)) = element_frame_global(el) else {
            ax_release(el as CFTypeRef);
            return (mx, my);
        };
        ax_release(el as CFTypeRef);
        (gx, gy)
    }
}

unsafe fn first_ax_window_from_ax_windows(app: AXUIElementRef) -> Option<AXUIElementRef> {
    let arr_ref = ax_copy_attr(app, "AXWindows")?;
    let arr = CFArray::<*const c_void>::wrap_under_create_rule(arr_ref as CFArrayRef);
    for i in 0..arr.len() {
        let Some(w) = arr.get(i) else {
            continue;
        };
        let child = *w as AXUIElementRef;
        if child.is_null() {
            continue;
        }
        let retained = CFRetain(child as CFTypeRef) as AXUIElementRef;
        if retained.is_null() {
            continue;
        }
        let (role, _, _) = read_role_title_id(retained);
        if role.as_deref() == Some("AXWindow") && element_frame_global(retained).is_some() {
            return Some(retained);
        }
        ax_release(retained as CFTypeRef);
    }
    None
}

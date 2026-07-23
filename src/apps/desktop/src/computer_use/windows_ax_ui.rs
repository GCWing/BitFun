//! Windows UI Automation (UIA) tree walk for stable screen coordinates.
//!
//! Ported from cua-driver-rs v0.6.8 (`platform-windows/src/uia/mod.rs`):
//!   * `IUIAutomationCacheRequest` batches every property + pattern fetch into
//!     a single cross-process RPC (one `BuildUpdatedCache` instead of N
//!     per-property `CurrentXxx()` calls — Chrome's ~5000-node tree drops from
//!     >4s to a few hundred ms).
//!   * `ControlViewCondition()` filter skips decorative / raw-view nodes.
//!   * Full indexed tree (`Vec<UiaNode>`) with COM element-pointer retention
//!     (`element_ptr`) for later pattern dispatch.
//!   * `detect_cached_actions` probes cached patterns (Invoke / Toggle /
//!     SelectionItem / ExpandCollapse / Value / RangeValue / Text / Scroll).
//!   * Transient `E_FAIL` provider errors retried (3 attempts, 40ms backoff).
//!
//! Unlike the cua daemon, BitFun is a Tauri GUI app, so COM is initialized with
//! `COINIT_APARTMENTTHREADED` (correct for the main thread). VARIANT-based
//! property reads are deliberately avoided: they require the
//! `Win32_System_Ole` + `Win32_System_Variant` features which the desktop
//! crate does not enable. The typed cached accessors (`CachedName`,
//! `CachedControlType`, `CachedIsEnabled`, ...) and `GetCachedPatternAs`
//! cover the same data without VARIANT and without extra Cargo features.

// Symbols here are wired up by the desktop host / ControlHub dispatch layer in a
// follow-up step. Until then, suppress dead-code lints without weakening real
// warnings elsewhere.
#![allow(dead_code)]

use crate::computer_use::ui_locate_common;
use bitfun_core::agentic::tools::computer_use_host::{
    AppInfo, AppStateSnapshot, AxNode, OcrAccessibilityHit, UiElementLocateQuery,
    UiElementLocateResult,
};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use windows::core::Interface;
use windows::Win32::Foundation::POINT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationCacheRequest, IUIAutomationElement,
    IUIAutomationValuePattern, TreeScope_Subtree, UIA_AutomationIdPropertyId,
    UIA_BoundingRectanglePropertyId, UIA_ControlTypePropertyId, UIA_ExpandCollapsePatternId,
    UIA_HelpTextPropertyId, UIA_InvokePatternId, UIA_IsEnabledPropertyId,
    UIA_IsOffscreenPropertyId, UIA_NamePropertyId, UIA_RangeValuePatternId, UIA_ScrollPatternId,
    UIA_SelectionItemPatternId, UIA_TextPatternId, UIA_TogglePatternId, UIA_ValuePatternId,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

/// Transient-provider retry count for `BuildUpdatedCache`.
const BUILD_CACHE_MAX_ATTEMPTS: u32 = 3;
/// Backoff between `BuildUpdatedCache` retries (milliseconds).
const BUILD_CACHE_BACKOFF_MS: u64 = 40;

/// A single node in the UIA accessibility tree.
///
/// Mirrors cua-driver-rs `UiaNode`. The `element_ptr` field retains the raw
/// `IUIAutomationElement` COM pointer (AddRef'd via clone + `mem::forget`) so a
/// follow-up click / pattern-dispatch step can reuse it without re-walking.
/// Lifetime release of those retained pointers is wired by a future
/// `ElementCache` (cua parity); until then the pointers simply outlive the
/// snapshot, which is acceptable for a not-yet-wired code path.
#[derive(Clone)]
pub(super) struct UiaNode {
    /// Dense index assigned only to actionable elements (`[N]` in the tree
    /// text). `None` for non-actionable content-only nodes.
    pub element_index: Option<usize>,
    pub control_type: String,
    pub name: Option<String>,
    pub value: Option<String>,
    pub automation_id: Option<String>,
    pub help_text: Option<String>,
    pub actions: Vec<String>,
    /// Raw `IUIAutomationElement` COM pointer as `usize`.
    pub element_ptr: usize,
    /// Screen-coordinate center, captured at walk time to avoid later COM calls.
    pub center_x: i32,
    pub center_y: i32,
    /// Full screen-coord rect `(left, top, right, bottom)`.
    pub rect: Option<(i32, i32, i32, i32)>,
    /// MSAA role code; `None` on the UIA primary path.
    pub msaa_role: Option<i32>,
    /// Depth in the rendered tree (matches indent level).
    pub depth: usize,
    /// `element_index` of the nearest actionable ancestor, if any.
    pub parent_element_index: Option<usize>,
    /// Cached `UIA_IsEnabled`. Feeds [`AxNode::enabled`] on conversion.
    pub enabled: bool,
}

impl UiaNode {
    /// Convert to BitFun's [`AxNode`] for `get_app_state` integration.
    ///
    /// `idx` / `parent_idx` are supplied by the caller because `AxNode` uses a
    /// dense `u32` index over the *rendered* tree (including content-only
    /// nodes), whereas [`UiaNode::element_index`] only numbers actionable
    /// elements. The integration wiring is responsible for the dense
    /// re-indexing when `get_app_state` is connected on Windows.
    fn to_ax_node(&self, idx: u32, parent_idx: Option<u32>) -> AxNode {
        let frame_global = self
            .rect
            .map(|(l, t, r, b)| (l as f64, t as f64, (r - l) as f64, (b - t) as f64));
        AxNode {
            idx,
            parent_idx,
            role: self.control_type.clone(),
            title: self.name.clone(),
            value: self.value.clone(),
            description: None,
            identifier: self.automation_id.clone(),
            enabled: self.enabled,
            focused: false,
            selected: None,
            frame_global,
            actions: self.actions.clone(),
            role_description: None,
            subrole: None,
            help: self.help_text.clone(),
            url: None,
            expanded: None,
        }
    }
}

fn bstr_to_string(b: windows::core::BSTR) -> String {
    b.to_string()
}

fn localized_control_type_string(elem: &IUIAutomationElement) -> String {
    unsafe {
        elem.CurrentLocalizedControlType()
            .map(bstr_to_string)
            .unwrap_or_default()
    }
}

// ── Cache build ────────────────────────────────────────────────────────────

/// Build a cache request that pre-fetches every property + pattern we later
/// read, so the walk itself issues zero cross-process RPCs.
unsafe fn build_cache_request(
    automation: &IUIAutomation,
) -> BitFunResult<IUIAutomationCacheRequest> {
    // SAFETY: `automation` is a live UI Automation COM interface and all ids
    // supplied below are documented properties, patterns, scopes, or filters.
    let cache_req = unsafe { automation.CreateCacheRequest() }
        .map_err(|e| BitFunError::tool(format!("UI Automation CreateCacheRequest: {}.", e)))?;

    // Properties to pre-fetch (typed cached accessors read these).
    for prop in [
        UIA_ControlTypePropertyId,
        UIA_NamePropertyId,
        UIA_AutomationIdPropertyId,
        UIA_HelpTextPropertyId,
        UIA_IsEnabledPropertyId,
        UIA_IsOffscreenPropertyId,
        UIA_BoundingRectanglePropertyId,
    ] {
        let _ = unsafe { cache_req.AddProperty(prop) };
    }

    // Patterns to pre-fetch (for action detection + Value read).
    for pat in [
        UIA_InvokePatternId,
        UIA_TogglePatternId,
        UIA_SelectionItemPatternId,
        UIA_ExpandCollapsePatternId,
        UIA_ValuePatternId,
        UIA_RangeValuePatternId,
        UIA_TextPatternId,
        UIA_ScrollPatternId,
    ] {
        let _ = unsafe { cache_req.AddPattern(pat) };
    }

    // Fetch the entire subtree in one bulk RPC.
    let _ = unsafe { cache_req.SetTreeScope(TreeScope_Subtree) };

    // Control-view filter (same set ControlViewWalker would walk) — drops
    // decorative / raw-view nodes that only add noise.
    if let Ok(ctrl_cond) = unsafe { automation.ControlViewCondition() } {
        let _ = unsafe { cache_req.SetTreeFilter(&ctrl_cond) };
    }

    Ok(cache_req)
}

/// `BuildUpdatedCache` with a short retry loop. A single transient provider
/// error (commonly `E_FAIL` / `0x80004005` from a control rebuilding its
/// automation subtree mid-walk) must not take down the whole snapshot — the
/// same call usually succeeds a beat later. See cua #1881.
pub(crate) unsafe fn build_updated_cache_with_retry(
    uncached: &IUIAutomationElement,
    cache_req: &IUIAutomationCacheRequest,
) -> BitFunResult<IUIAutomationElement> {
    let mut attempt = 0u32;
    loop {
        // SAFETY: both COM interfaces are live for the call and `cache_req`
        // was constructed by the same UI Automation instance.
        match unsafe { uncached.BuildUpdatedCache(cache_req) } {
            Ok(e) => return Ok(e),
            Err(e) => {
                attempt += 1;
                if attempt >= BUILD_CACHE_MAX_ATTEMPTS {
                    return Err(BitFunError::tool(format!(
                        "UI Automation BuildUpdatedCache failed after {} attempts: {}.",
                        attempt, e
                    )));
                }
                log::debug!(
                    "UIA BuildUpdatedCache transient error (attempt {}): {}; retrying in {}ms",
                    attempt,
                    e,
                    BUILD_CACHE_BACKOFF_MS
                );
                std::thread::sleep(std::time::Duration::from_millis(BUILD_CACHE_BACKOFF_MS));
            }
        }
    }
}

// ── Cached property readers ─────────────────────────────────────────────────
//
// Every reader calls a `CachedXxx` accessor (or `GetCachedPatternAs`) which
// reads from the element's local cache populated by `BuildUpdatedCache`. No
// cross-process RPC is issued during the walk.

fn read_cached_control_type(element: &IUIAutomationElement) -> String {
    unsafe {
        element
            .CachedControlType()
            .ok()
            .map(|ct| control_type_name(ct.0))
            .unwrap_or_else(|| "Unknown".to_string())
    }
}

fn read_cached_name(element: &IUIAutomationElement) -> Option<String> {
    unsafe {
        let bstr = element.CachedName().ok()?;
        let s = bstr.to_string();
        if s.trim().is_empty() {
            None
        } else {
            Some(s)
        }
    }
}

fn read_cached_automation_id(element: &IUIAutomationElement) -> Option<String> {
    unsafe {
        let bstr = element.CachedAutomationId().ok()?;
        let s = bstr.to_string();
        if s.trim().is_empty() {
            None
        } else {
            Some(s)
        }
    }
}

fn read_cached_help_text(element: &IUIAutomationElement) -> Option<String> {
    unsafe {
        let bstr = element.CachedHelpText().ok()?;
        let s = bstr.to_string();
        if s.trim().is_empty() {
            None
        } else {
            Some(s)
        }
    }
}

/// Read `ValuePattern.Value` via the cached pattern (no VARIANT needed).
fn read_cached_value(element: &IUIAutomationElement) -> Option<String> {
    unsafe {
        let vp = element
            .GetCachedPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
            .ok()?;
        let bstr = vp.CachedValue().ok()?;
        let s = bstr.to_string();
        if s.trim().is_empty() {
            None
        } else {
            Some(s)
        }
    }
}

fn read_cached_is_enabled(element: &IUIAutomationElement) -> bool {
    unsafe {
        element
            .CachedIsEnabled()
            .ok()
            .map(|b| b.0 != 0)
            .unwrap_or(true)
    }
}

fn read_cached_is_offscreen(element: &IUIAutomationElement) -> bool {
    unsafe {
        element
            .CachedIsOffscreen()
            .ok()
            .map(|b| b.0 != 0)
            .unwrap_or(false)
    }
}

/// Read bounding rect as `(center_x, center_y, Some((l, t, r, b)))`. Returns
/// `rect=None` when the element has no meaningful `BoundingRectangle`.
type CachedBoundingRect = (i32, i32, Option<(i32, i32, i32, i32)>);

fn read_cached_bounding_rect_full(element: &IUIAutomationElement) -> CachedBoundingRect {
    unsafe {
        match element.CachedBoundingRectangle() {
            Ok(r) if r.right > r.left && r.bottom > r.top => (
                (r.left + r.right) / 2,
                (r.top + r.bottom) / 2,
                Some((r.left, r.top, r.right, r.bottom)),
            ),
            _ => (0, 0, None),
        }
    }
}

/// Probe cached patterns to enumerate the actions an element supports. Each
/// `GetCachedPattern` is an in-process vtable read from the element's cache
/// (no cross-process RPC), so calling it 8 times per element is cheap.
fn detect_cached_actions(element: &IUIAutomationElement, is_enabled: bool) -> Vec<String> {
    if !is_enabled {
        return vec![];
    }
    let mut actions = Vec::new();
    unsafe {
        if element.GetCachedPattern(UIA_InvokePatternId).is_ok() {
            actions.push("invoke".to_string());
        }
        if element.GetCachedPattern(UIA_TogglePatternId).is_ok() {
            actions.push("toggle".to_string());
        }
        if element.GetCachedPattern(UIA_SelectionItemPatternId).is_ok() {
            actions.push("select".to_string());
        }
        if element
            .GetCachedPattern(UIA_ExpandCollapsePatternId)
            .is_ok()
        {
            actions.push("expand".to_string());
        }
        if element.GetCachedPattern(UIA_ValuePatternId).is_ok() {
            actions.push("set_value".to_string());
        }
        // RangeValuePattern is exposed by Sliders / ProgressBars. Without this
        // entry the slider parent gets actions=[] → no `[N]` index, making it
        // unaddressable by AutomationId.
        if element.GetCachedPattern(UIA_RangeValuePatternId).is_ok() {
            actions.push("set_value".to_string());
        }
        if element.GetCachedPattern(UIA_TextPatternId).is_ok() {
            actions.push("text".to_string());
        }
        if element.GetCachedPattern(UIA_ScrollPatternId).is_ok() {
            actions.push("scroll".to_string());
        }
    }
    actions
}

/// Map a UIA control-type id to a stable name. Matches the table in
/// cua-driver-rs (literal numeric ids kept for parity with the proven port).
fn control_type_name(id: i32) -> String {
    match id {
        50000 => "Button",
        50001 => "Calendar",
        50002 => "CheckBox",
        50003 => "ComboBox",
        50004 => "Edit",
        50005 => "Hyperlink",
        50006 => "Image",
        50007 => "ListItem",
        50008 => "List",
        50009 => "Menu",
        50010 => "MenuBar",
        50011 => "MenuItem",
        50012 => "ProgressBar",
        50013 => "RadioButton",
        50014 => "ScrollBar",
        50015 => "Slider",
        50016 => "Spinner",
        50017 => "StatusBar",
        50018 => "Tab",
        50019 => "TabItem",
        50020 => "Text",
        50021 => "ToolBar",
        50022 => "ToolTip",
        50023 => "Tree",
        50024 => "TreeItem",
        50025 => "Custom",
        50026 => "Group",
        50027 => "Thumb",
        50028 => "DataGrid",
        50029 => "DataItem",
        50030 => "Document",
        50031 => "SplitButton",
        50032 => "Window",
        50033 => "Pane",
        50034 => "Header",
        50035 => "HeaderItem",
        50036 => "Table",
        50037 => "TitleBar",
        50038 => "Separator",
        50039 => "SemanticZoom",
        50040 => "AppBar",
        _ => "Unknown",
    }
    .to_string()
}

// ── Tree walk ───────────────────────────────────────────────────────────────

/// Core walk: COM init → cache request → `ElementFromHandle` →
/// `BuildUpdatedCache` (retried) → recursive cached traversal → render.
unsafe fn walk_tree_full(
    hwnd: windows::Win32::Foundation::HWND,
    max_elements: usize,
    max_depth: usize,
) -> BitFunResult<(String, Vec<UiaNode>)> {
    // SAFETY: initializes COM for the current thread and creates the documented
    // in-process UI Automation class; failures are handled below.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.map_err(|e| {
            BitFunError::tool(format!(
                "UI Automation (CoCreateInstance CUIAutomation): {}.",
                e
            ))
        })?;

    let cache_req = unsafe { build_cache_request(&automation) }?;

    // SAFETY: `hwnd` is the caller-provided target handle. UIA reports an
    // error for an invalid or stale handle, which is propagated here.
    let uncached = unsafe { automation.ElementFromHandle(hwnd) }.map_err(|e| {
        BitFunError::tool(format!("UI Automation ElementFromHandle failed: {}.", e))
    })?;

    let root_elem = unsafe { build_updated_cache_with_retry(&uncached, &cache_req) }?;

    let mut nodes: Vec<UiaNode> = Vec::new();
    let mut lines: Vec<(usize, String)> = Vec::new();
    let mut counter = 0usize;
    let mut total = 0usize;
    unsafe {
        walk_cached_bounded(
            &root_elem,
            0,
            None,
            &mut nodes,
            &mut lines,
            &mut counter,
            &mut total,
            max_elements,
            max_depth,
        )
    };

    let tree_text = render_lines(&lines);
    Ok((tree_text, nodes))
}

#[allow(clippy::too_many_arguments)]
unsafe fn walk_cached_bounded(
    element: &IUIAutomationElement,
    depth: usize,
    parent_index: Option<usize>,
    nodes: &mut Vec<UiaNode>,
    lines: &mut Vec<(usize, String)>,
    counter: &mut usize,
    total: &mut usize,
    max_elements: usize,
    max_depth: usize,
) {
    if depth > max_depth || *total >= max_elements {
        return;
    }
    *total += 1;

    let control_type = read_cached_control_type(element);
    let name = read_cached_name(element);
    let value = read_cached_value(element);
    let automation_id = read_cached_automation_id(element);
    let help_text = read_cached_help_text(element);
    let enabled = read_cached_is_enabled(element);
    let offscreen = read_cached_is_offscreen(element);

    let actions = detect_cached_actions(element, enabled);
    let is_actionable = !actions.is_empty() && enabled && !offscreen;
    let has_content = name
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
        || value
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);

    let mut emitted_parent = parent_index;
    if is_actionable || has_content {
        // Retain the COM element pointer for later pattern dispatch. The clone
        // AddRef's; `mem::forget` prevents the local Drop from releasing it.
        let retained: IUIAutomationElement = element.clone();
        let ptr = retained.as_raw() as usize;
        std::mem::forget(retained);

        // Read the bounding rect for content-only nodes too, so text/role
        // locate-by-filter can still resolve a click center (cua only reads it
        // for actionable nodes; BitFun's `locate_ui_element_center` needs it).
        let (center_x, center_y, rect) = read_cached_bounding_rect_full(element);

        let node = if is_actionable {
            let idx = *counter;
            *counter += 1;
            emitted_parent = Some(idx);
            UiaNode {
                element_index: Some(idx),
                control_type: control_type.clone(),
                name: name.clone(),
                value: value.clone(),
                automation_id: automation_id.clone(),
                help_text: help_text.clone(),
                actions: actions.clone(),
                element_ptr: ptr,
                center_x,
                center_y,
                rect,
                msaa_role: None,
                depth,
                parent_element_index: parent_index,
                enabled,
            }
        } else {
            UiaNode {
                element_index: None,
                control_type: control_type.clone(),
                name: name.clone(),
                value: value.clone(),
                automation_id: automation_id.clone(),
                help_text: help_text.clone(),
                actions: vec![],
                element_ptr: ptr,
                center_x,
                center_y,
                rect,
                msaa_role: None,
                depth,
                parent_element_index: parent_index,
                enabled,
            }
        };

        lines.push((depth, format_node_line(&node)));
        nodes.push(node);
    }

    // Recurse using cached children — zero additional cross-process RPCs.
    // SAFETY: `element` is a live cached UIA element, and child indices are
    // bounded by the array length returned by UI Automation.
    if let Ok(children) = unsafe { element.GetCachedChildren() } {
        let len = unsafe { children.Length() }.unwrap_or(0);
        for i in 0..len {
            if let Ok(child) = unsafe { children.GetElement(i) } {
                unsafe {
                    walk_cached_bounded(
                        &child,
                        depth + 1,
                        emitted_parent,
                        nodes,
                        lines,
                        counter,
                        total,
                        max_elements,
                        max_depth,
                    )
                };
            }
        }
    }
}

// ── Rendering ──────────────────────────────────────────────────────────────

/// Format one node as a cua-style tree line:
///   `- [N] ControlType "Name" [value="…" id=… help="…" actions=[…]]`
///   `- ControlType "Name" = "Value"` (non-indexed read-only elements)
pub(crate) fn format_node_line(node: &UiaNode) -> String {
    let mut s = String::new();
    if let Some(idx) = node.element_index {
        s.push_str(&format!("- [{}] {}", idx, node.control_type));
        if let Some(n) = &node.name {
            s.push_str(&format!(" \"{}\"", n));
        }
        let mut attrs = Vec::new();
        if let Some(v) = &node.value {
            attrs.push(format!("value=\"{}\"", v));
        }
        if let Some(id) = &node.automation_id {
            attrs.push(format!("id={}", id));
        }
        if let Some(h) = &node.help_text {
            attrs.push(format!("help=\"{}\"", h));
        }
        if !node.actions.is_empty() {
            attrs.push(format!("actions=[{}]", node.actions.join(",")));
        }
        if !attrs.is_empty() {
            s.push_str(&format!(" [{}]", attrs.join(" ")));
        }
    } else {
        s.push_str(&format!("- {}", node.control_type));
        if let Some(n) = &node.name {
            s.push_str(&format!(" \"{}\"", n));
        }
        if let Some(v) = &node.value {
            s.push_str(&format!(" = \"{}\"", v));
        }
    }
    s
}

fn render_lines(lines: &[(usize, String)]) -> String {
    let mut out = String::new();
    for (depth, line) in lines {
        for _ in 0..*depth {
            out.push_str("  ");
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Render tree text directly from a `UiaNode` vector (used by the MSAA
/// fallback, which returns nodes without a pre-rendered line list). Indents by
/// each node's `depth` and reuses [`format_node_line`] for parity with the UIA
/// primary path.
pub(crate) fn render_nodes_text(nodes: &[UiaNode]) -> String {
    let lines: Vec<(usize, String)> = nodes
        .iter()
        .map(|n| (n.depth, format_node_line(n)))
        .collect();
    render_lines(&lines)
}

// ── Locate (cached approach) ────────────────────────────────────────────────

/// Build a locate result from a walked node's retained rect + metadata.
fn center_result_from_node(
    node: &UiaNode,
    matched_node_idx: Option<u32>,
    matched_via: &str,
) -> BitFunResult<UiElementLocateResult> {
    let (l, t, r, b) = node.rect.ok_or_else(|| {
        BitFunError::tool(format!(
            "Matched UI element \"{}\" has no usable bounding rectangle.",
            node.name.as_deref().unwrap_or(node.control_type.as_str())
        ))
    })?;
    let gx = (l + r) as f64 / 2.0;
    let gy = (t + b) as f64 / 2.0;
    let bl = l as f64;
    let bt = t as f64;
    let bw = (r - l) as f64;
    let bh = (b - t) as f64;
    ui_locate_common::ok_result_with_context_full(
        gx,
        gy,
        bl,
        bt,
        bw,
        bh,
        node.control_type.clone(),
        node.name.clone(),
        node.automation_id.clone(),
        None,
        1,
        vec![],
        matched_node_idx,
        Some(matched_via.to_string()),
    )
}

/// Foreground window root, then a cached control-view UIA tree walk.
///
/// Uses the batched cache path internally (one `BuildUpdatedCache` RPC for the
/// whole subtree, then in-process cached reads). `node_idx` is now supported
/// because the cached walk produces a real indexed tree (previously
/// Windows-only-`text_contains`/`title_contains`+`role_substring`).
pub(super) fn locate_ui_element_center(
    query: &UiElementLocateQuery,
) -> BitFunResult<UiElementLocateResult> {
    ui_locate_common::validate_query(query)?;

    let max_depth = query.max_depth.unwrap_or(48).clamp(1, 200) as usize;
    let max_elements = 12_000usize;

    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return Err(BitFunError::tool(
            "No foreground window (GetForegroundWindow returned null).".to_string(),
        ));
    }

    let (_tree_text, nodes) = unsafe { walk_tree_full(hwnd, max_elements, max_depth) }?;

    // node_idx fast-path: address an actionable element by its `[N]` index.
    if let Some(idx) = query.node_idx {
        if let Some(node) = nodes.iter().find(|n| n.element_index == Some(idx as usize)) {
            return center_result_from_node(node, Some(idx), "node_idx");
        }
        return Err(BitFunError::tool(format!(
            "[AX_IDX_NOT_FOUND] No UI element with node_idx={} in the foreground window tree \
             ({} nodes walked).",
            idx,
            nodes.len()
        )));
    }

    // Filter path: first node whose attrs match the query and that has a
    // usable bounding rect.
    let mut total_matches = 0u32;
    let mut other_matches: Vec<String> = Vec::new();
    for node in &nodes {
        let attrs = ui_locate_common::NodeAttrs {
            role: Some(node.control_type.as_str()),
            subrole: None,
            title: node.name.as_deref(),
            value: node.value.as_deref(),
            description: None,
            identifier: node.automation_id.as_deref(),
            help: node.help_text.as_deref(),
        };
        if !ui_locate_common::matches_filters_attrs(query, &attrs) {
            continue;
        }
        total_matches += 1;
        if node.rect.is_some() {
            let idx = node.element_index.map(|i| i as u32);
            return center_result_from_node(node, idx, "filters");
        }
        // Matched but no usable rect — record for diagnostics, keep scanning.
        if other_matches.len() < 5 {
            other_matches.push(format_node_line(node));
        }
    }

    if total_matches == 0 {
        Err(BitFunError::tool(
            "No UI element matched in the foreground window for this query. Refine filters or \
             use ComputerUse screenshot. Locate uses the same UI Automation permission as \
             mouse/keyboard automation."
                .to_string(),
        ))
    } else {
        Err(BitFunError::tool(format!(
            "UI element matched filters but had no usable bounding rectangle ({} match(es): {}).",
            total_matches,
            other_matches.join(" | ")
        )))
    }
}

// ── Hit-test (single element, unchanged signature) ──────────────────────────

/// Hit-test UIA at global screen coordinates (OCR `move_to_text` disambiguation).
///
/// Single-element hit-test: only a handful of COM calls, so it stays on the
/// `CurrentXxx` accessors (caching does not help one element). Signature is
/// intentionally unchanged.
pub(super) fn accessibility_hit_at_global_point(
    gx: f64,
    gy: f64,
) -> BitFunResult<Option<OcrAccessibilityHit>> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    let automation: IUIAutomation = unsafe {
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| BitFunError::tool(format!("UI Automation (CoCreateInstance): {}.", e)))?
    };
    let pt = POINT {
        x: gx.round() as i32,
        y: gy.round() as i32,
    };
    let elem = unsafe { automation.ElementFromPoint(pt) };
    let elem = match elem {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };
    let name = unsafe {
        elem.CurrentName()
            .ok()
            .map(bstr_to_string)
            .unwrap_or_default()
    };
    let ident = unsafe {
        elem.CurrentAutomationId()
            .ok()
            .map(bstr_to_string)
            .unwrap_or_default()
    };
    let role = localized_control_type_string(&elem);
    let parent_context = if let Ok(walker) = unsafe { automation.ControlViewWalker() } {
        unsafe { walker.GetParentElement(&elem) }
            .ok()
            .and_then(|parent| {
                let pn = unsafe {
                    parent
                        .CurrentName()
                        .ok()
                        .map(bstr_to_string)
                        .unwrap_or_default()
                };
                let pr = localized_control_type_string(&parent);
                let s = format!("{}: {}", pr, pn);
                if s == ": " || s.trim().is_empty() {
                    None
                } else {
                    Some(s)
                }
            })
    } else {
        None
    };
    let desc = format!(
        "role={} name={:?} id={:?} parent={:?}",
        role, name, ident, parent_context
    );
    Ok(Some(OcrAccessibilityHit {
        role: if role.is_empty() { None } else { Some(role) },
        title: if name.is_empty() { None } else { Some(name) },
        identifier: if ident.is_empty() { None } else { Some(ident) },
        parent_context,
        description: desc,
    }))
}

// ── AppStateSnapshot builder ────────────────────────────────────────────────

/// Build a full [`AppStateSnapshot`] for an explicit top-level HWND selected by
/// the caller.
pub(super) fn get_app_state_snapshot_for_window(
    hwnd: windows::Win32::Foundation::HWND,
    max_depth: u32,
    focus_window_only: bool,
) -> BitFunResult<AppStateSnapshot> {
    if hwnd.is_invalid() {
        return Err(BitFunError::tool(
            "No target window (invalid HWND).".to_string(),
        ));
    }
    let _ = focus_window_only; // Windows UIA walk is always rooted at the given HWND.

    let hwnd_raw = hwnd.0 as isize;

    // Primary: UIA control-view walk. Fallback: MSAA for SAL/VCL windows
    // (LibreOffice / OpenOffice) whose UIA provider hangs on
    // `BuildUpdatedCache(Subtree)` or returns an empty tree, OR whenever the
    // UIA walk errors / yields nothing on a SAL/VCL class.
    let (tree_text, uia_nodes) = match unsafe { walk_tree_full(hwnd, 500, max_depth as usize) } {
        Ok((text, nodes)) if !nodes.is_empty() => (text, nodes),
        primary => {
            if crate::computer_use::windows_msaa::is_sal_vcl_window(hwnd_raw) {
                match crate::computer_use::windows_msaa::walk_msaa_tree(hwnd_raw) {
                    Ok(msaa_nodes) if !msaa_nodes.is_empty() => {
                        let text = render_nodes_text(&msaa_nodes);
                        (text, msaa_nodes)
                    }
                    _ => primary?,
                }
            } else {
                primary?
            }
        }
    };

    // Dense re-index: assign idx to every node (including content-only),
    // remap parent_element_index to the dense space.
    let mut nodes: Vec<AxNode> = Vec::with_capacity(uia_nodes.len());
    let mut uia_idx_to_dense: std::collections::HashMap<usize, u32> =
        std::collections::HashMap::new();
    for (dense_idx, n) in uia_nodes.iter().enumerate() {
        if let Some(ei) = n.element_index {
            uia_idx_to_dense.insert(ei, dense_idx as u32);
        }
    }
    for (dense_idx, n) in uia_nodes.iter().enumerate() {
        let parent_dense = n
            .parent_element_index
            .and_then(|p| uia_idx_to_dense.get(&p).copied());
        nodes.push(n.to_ax_node(dense_idx as u32, parent_dense));
    }

    // Compute digest — same algorithm as macOS `compute_digest`.
    let digest = compute_digest(&nodes);

    let window_title = window_title_for(hwnd);
    let pid = window_pid_for(hwnd).map(|p| p as i32);
    let app = AppInfo {
        name: window_title
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        bundle_id: None,
        pid,
        running: true,
        last_used_ms: None,
        launch_count: 0,
    };

    Ok(AppStateSnapshot {
        app,
        window_title,
        tree_text,
        nodes,
        digest,
        captured_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        screenshot: None,
        loop_warning: None,
    })
}

fn compute_digest(nodes: &[AxNode]) -> String {
    use sha1::{Digest, Sha1};
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
        h.update(n.enabled.to_string().as_bytes());
        h.update(b"\x1f");
        for a in &n.actions {
            h.update(a.as_bytes());
            h.update(b",");
        }
        h.update(b"\n");
    }
    let hash = h.finalize();
    let mut hex = String::with_capacity(hash.len() * 2);
    for b in hash.iter() {
        hex.push_str(&format!("{:02x}", b));
    }
    hex
}

fn foreground_app_name() -> Option<String> {
    let hwnd = unsafe { GetForegroundWindow() };
    window_title_for(hwnd)
}

fn window_title_for(hwnd: windows::Win32::Foundation::HWND) -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    if hwnd.is_invalid() {
        return None;
    }
    unsafe {
        let mut buf = [0u16; 256];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

fn window_pid_for(hwnd: windows::Win32::Foundation::HWND) -> Option<u32> {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
    if hwnd.is_invalid() {
        return None;
    }
    unsafe {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            None
        } else {
            Some(pid)
        }
    }
}

/// Raw handle of the current foreground window as `isize` (0 when none). Used
/// by the desktop host to capture a screenshot of the same window the AX
/// snapshot was taken from.
pub(super) fn foreground_window_handle() -> isize {
    let hwnd = unsafe { GetForegroundWindow() };
    hwnd.0 as isize
}

/// Owning process id of the current foreground window, if any.
pub(super) fn foreground_window_pid() -> Option<u32> {
    let hwnd = unsafe { GetForegroundWindow() };
    window_pid_for(hwnd)
}

//! Windows UI Automation (UIA) tree walk for stable screen coordinates.

use crate::computer_use::ui_locate_common;
use bitfun_core::agentic::tools::computer_use_host::{
    OcrAccessibilityHit, UiElementLocateQuery, UiElementLocateResult,
};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use std::collections::VecDeque;
use windows::Win32::Foundation::POINT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

fn bstr_to_string(b: windows_core::BSTR) -> String {
    b.to_string()
}

fn walker_children(
    walker: &IUIAutomationTreeWalker,
    parent: &IUIAutomationElement,
) -> BitFunResult<Vec<IUIAutomationElement>> {
    let mut out = Vec::new();
    let first = unsafe { walker.GetFirstChildElement(parent) };
    let Ok(mut cur) = first else {
        return Ok(out);
    };
    loop {
        out.push(cur.clone());
        let next = unsafe { walker.GetNextSiblingElement(&cur) };
        match next {
            Ok(n) => cur = n,
            Err(_) => break,
        }
    }
    Ok(out)
}

fn localized_control_type_string(elem: &IUIAutomationElement) -> String {
    unsafe {
        elem.CurrentLocalizedControlType()
            .map(bstr_to_string)
            .unwrap_or_default()
    }
}

/// Foreground window root, then UIA RawViewWalker BFS.
pub fn locate_ui_element_center(
    query: &UiElementLocateQuery,
) -> BitFunResult<UiElementLocateResult> {
    ui_locate_common::validate_query(query)?;

    if query.node_idx.is_some() {
        return Err(BitFunError::tool(
            "[AX_IDX_NOT_SUPPORTED] node_idx lookup is only implemented on macOS. \
             Fall back to `text_contains` / `title_contains` + `role_substring` on this host."
                .to_string(),
        ));
    }

    let max_depth = query.max_depth.unwrap_or(48).clamp(1, 200);
    let max_nodes = 12_000usize;

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let automation: IUIAutomation = unsafe {
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).map_err(|e| {
            BitFunError::tool(format!(
                "UI Automation (CoCreateInstance CUIAutomation): {}.",
                e
            ))
        })?
    };

    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return Err(BitFunError::tool(
            "No foreground window (GetForegroundWindow returned null).".to_string(),
        ));
    }

    let root = unsafe {
        automation.ElementFromHandle(hwnd).map_err(|e| {
            BitFunError::tool(format!("UI Automation ElementFromHandle failed: {}.", e))
        })?
    };

    let walker = unsafe {
        automation
            .RawViewWalker()
            .map_err(|e| BitFunError::tool(format!("UI Automation RawViewWalker: {}.", e)))?
    };

    struct Queued {
        el: IUIAutomationElement,
        depth: u32,
    }

    let mut q = VecDeque::new();
    q.push_back(Queued { el: root, depth: 0 });
    let mut visited = 0usize;

    loop {
        let Some(cur) = q.pop_front() else {
            return Err(BitFunError::tool(
                "No UI element matched in the foreground window for this query. Refine filters or use ComputerUse screenshot. Locate uses the same UI Automation permission as mouse/keyboard automation."
                    .to_string(),
            ));
        };
        if cur.depth > max_depth {
            continue;
        }
        visited += 1;
        if visited > max_nodes {
            return Err(BitFunError::tool(
                "UI Automation search limit reached; narrow title/role/identifier filters."
                    .to_string(),
            ));
        }

        let name = unsafe {
            cur.el
                .CurrentName()
                .ok()
                .map(bstr_to_string)
                .unwrap_or_default()
        };
        let ident = unsafe {
            cur.el
                .CurrentAutomationId()
                .ok()
                .map(bstr_to_string)
                .unwrap_or_default()
        };
        let role = localized_control_type_string(&cur.el);
        let help = unsafe {
            cur.el
                .CurrentHelpText()
                .ok()
                .map(bstr_to_string)
                .unwrap_or_default()
        };

        let attrs = ui_locate_common::NodeAttrs {
            role: Some(role.as_str()),
            subrole: None,
            title: Some(name.as_str()),
            value: None,
            description: None,
            identifier: Some(ident.as_str()),
            help: if help.is_empty() {
                None
            } else {
                Some(help.as_str())
            },
        };
        let matched = ui_locate_common::matches_filters_attrs(query, &attrs);
        if matched {
            let rect = unsafe { cur.el.CurrentBoundingRectangle() };
            if let Ok(r) = rect {
                if r.right > r.left && r.bottom > r.top {
                    let gx = (r.left + r.right) as f64 / 2.0;
                    let gy = (r.top + r.bottom) as f64 / 2.0;
                    let bl = r.left as f64;
                    let bt = r.top as f64;
                    let bw = (r.right - r.left) as f64;
                    let bh = (r.bottom - r.top) as f64;
                    return ui_locate_common::ok_result(
                        gx,
                        gy,
                        bl,
                        bt,
                        bw,
                        bh,
                        role,
                        if name.is_empty() { None } else { Some(name) },
                        if ident.is_empty() { None } else { Some(ident) },
                    );
                }
            }
        }

        let children = walker_children(&walker, &cur.el)?;
        let next_depth = cur.depth + 1;
        for ch in children {
            q.push_back(Queued {
                el: ch,
                depth: next_depth,
            });
        }
    }
}

/// Hit-test UIA at global screen coordinates (OCR `move_to_text` disambiguation).
pub fn accessibility_hit_at_global_point(
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

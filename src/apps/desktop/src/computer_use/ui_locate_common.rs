//! Shared validation, filter matching, and global→native pixel mapping for UI locate tools.

use bitfun_core::agentic::tools::computer_use_host::{UiElementLocateQuery, UiElementLocateResult};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use screenshots::display_info::DisplayInfo;

pub fn validate_query(q: &UiElementLocateQuery) -> BitFunResult<()> {
    let t = q.title_contains.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false);
    let r = q.role_substring.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false);
    let i = q
        .identifier_contains
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !t && !r && !i {
        return Err(BitFunError::tool(
            "Provide at least one of: title_contains, role_substring, identifier_contains (non-empty)."
                .to_string(),
        ));
    }
    Ok(())
}

fn global_xy_to_native_with_display(d: &DisplayInfo, gx: f64, gy: f64) -> BitFunResult<(u32, u32)> {
    let disp_ox = d.x as f64;
    let disp_oy = d.y as f64;
    let disp_w = d.width as f64;
    let disp_h = d.height as f64;
    if disp_w <= 0.0 || disp_h <= 0.0 || d.width == 0 || d.height == 0 {
        return Err(BitFunError::tool(
            "Invalid display geometry for UI locate mapping.".to_string(),
        ));
    }
    let px_w = d.width as f64;
    let px_h = d.height as f64;
    let cx = ((gx - disp_ox) / disp_w) * px_w;
    let cy = ((gy - disp_oy) / disp_h) * px_h;
    let nx = cx.round().clamp(0.0, px_w - 1.0) as u32;
    let ny = cy.round().clamp(0.0, px_h - 1.0) as u32;
    Ok((nx, ny))
}

pub fn global_to_native_center(gx: f64, gy: f64) -> BitFunResult<(u32, u32)> {
    let d = DisplayInfo::from_point(gx.round() as i32, gy.round() as i32)
        .map_err(|e| BitFunError::tool(format!("DisplayInfo::from_point: {}", e)))?;
    global_xy_to_native_with_display(&d, gx, gy)
}

fn global_bounds_to_native_minmax(
    center_gx: f64,
    center_gy: f64,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
) -> BitFunResult<(u32, u32, u32, u32)> {
    let d = DisplayInfo::from_point(center_gx.round() as i32, center_gy.round() as i32)
        .map_err(|e| BitFunError::tool(format!("DisplayInfo::from_point: {}", e)))?;
    let corners = [
        (left, top),
        (left + width, top),
        (left, top + height),
        (left + width, top + height),
    ];
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    for (gx, gy) in corners {
        let (nx, ny) = global_xy_to_native_with_display(&d, gx, gy)?;
        min_x = min_x.min(nx);
        min_y = min_y.min(ny);
        max_x = max_x.max(nx);
        max_y = max_y.max(ny);
    }
    Ok((min_x, min_y, max_x, max_y))
}

fn contains_ci(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    hay.to_lowercase().contains(&needle.to_lowercase())
}

fn combine_is_any(query: &UiElementLocateQuery) -> bool {
    matches!(
        query.filter_combine.as_deref(),
        Some("any") | Some("or")
    )
}

/// OR semantics: element matches if **at least one** non-empty filter matches.
pub fn matches_filters_any(
    query: &UiElementLocateQuery,
    role: Option<&str>,
    title: Option<&str>,
    ident: Option<&str>,
) -> bool {
    let mut has_filter = false;
    let mut matched = false;
    if let Some(ref want) = query.role_substring {
        if !want.trim().is_empty() {
            has_filter = true;
            if contains_ci(role.unwrap_or(""), want.trim()) {
                matched = true;
            }
        }
    }
    if let Some(ref want) = query.title_contains {
        if !want.trim().is_empty() {
            has_filter = true;
            if contains_ci(title.unwrap_or(""), want.trim()) {
                matched = true;
            }
        }
    }
    if let Some(ref want) = query.identifier_contains {
        if !want.trim().is_empty() {
            has_filter = true;
            if contains_ci(ident.unwrap_or(""), want.trim()) {
                matched = true;
            }
        }
    }
    has_filter && matched
}

/// AND semantics (default): **every** non-empty filter must match the same element.
pub fn matches_filters_all(
    query: &UiElementLocateQuery,
    role: Option<&str>,
    title: Option<&str>,
    ident: Option<&str>,
) -> bool {
    if let Some(ref want) = query.role_substring {
        if !want.trim().is_empty() {
            let r = role.unwrap_or("");
            if !contains_ci(r, want.trim()) {
                return false;
            }
        }
    }
    if let Some(ref want) = query.title_contains {
        if !want.trim().is_empty() {
            let t = title.unwrap_or("");
            if !contains_ci(t, want.trim()) {
                return false;
            }
        }
    }
    if let Some(ref want) = query.identifier_contains {
        if !want.trim().is_empty() {
            let i = ident.unwrap_or("");
            if !contains_ci(i, want.trim()) {
                return false;
            }
        }
    }
    true
}

pub fn matches_filters(
    query: &UiElementLocateQuery,
    role: Option<&str>,
    title: Option<&str>,
    ident: Option<&str>,
) -> bool {
    if combine_is_any(query) {
        matches_filters_any(query, role, title, ident)
    } else {
        matches_filters_all(query, role, title, ident)
    }
}

#[allow(dead_code)] // Used by windows_ax_ui / linux_ax_ui (not compiled on macOS)
pub fn ok_result(
    gx: f64,
    gy: f64,
    bounds_left: f64,
    bounds_top: f64,
    bounds_width: f64,
    bounds_height: f64,
    matched_role: String,
    matched_title: Option<String>,
    matched_identifier: Option<String>,
) -> BitFunResult<UiElementLocateResult> {
    ok_result_with_context(
        gx, gy, bounds_left, bounds_top, bounds_width, bounds_height,
        matched_role, matched_title, matched_identifier,
        None, 1, vec![],
    )
}

pub fn ok_result_with_context(
    gx: f64,
    gy: f64,
    bounds_left: f64,
    bounds_top: f64,
    bounds_width: f64,
    bounds_height: f64,
    matched_role: String,
    matched_title: Option<String>,
    matched_identifier: Option<String>,
    parent_context: Option<String>,
    total_matches: u32,
    other_matches: Vec<String>,
) -> BitFunResult<UiElementLocateResult> {
    let (nx, ny) = global_to_native_center(gx, gy)?;
    let (nminx, nminy, nmaxx, nmaxy) = if bounds_width > 0.0 && bounds_height > 0.0 {
        global_bounds_to_native_minmax(gx, gy, bounds_left, bounds_top, bounds_width, bounds_height)?
    } else {
        (nx, ny, nx, ny)
    };
    Ok(UiElementLocateResult {
        global_center_x: gx,
        global_center_y: gy,
        native_center_x: nx,
        native_center_y: ny,
        global_bounds_left: bounds_left,
        global_bounds_top: bounds_top,
        global_bounds_width: bounds_width,
        global_bounds_height: bounds_height,
        native_bounds_min_x: nminx,
        native_bounds_min_y: nminy,
        native_bounds_max_x: nmaxx,
        native_bounds_max_y: nmaxy,
        matched_role,
        matched_title,
        matched_identifier,
        parent_context,
        total_matches,
        other_matches,
    })
}

/// Whether an element's global bounds fall within any visible display.
pub fn is_element_on_screen(gx: f64, gy: f64, width: f64, height: f64) -> bool {
    // Element must have reasonable size (not a giant container)
    if width > 3000.0 || height > 2000.0 {
        return false;
    }
    // Center must be resolvable to a display
    DisplayInfo::from_point(gx.round() as i32, gy.round() as i32).is_ok()
}

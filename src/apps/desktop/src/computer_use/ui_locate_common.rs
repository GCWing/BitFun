//! Shared validation, filter matching, and global→native pixel mapping for UI locate tools.

use bitfun_core::agentic::tools::computer_use_host::{UiElementLocateQuery, UiElementLocateResult};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use screenshots::display_info::DisplayInfo;

pub fn validate_query(q: &UiElementLocateQuery) -> BitFunResult<()> {
    let t = q
        .title_contains
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let r = q
        .role_substring
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
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
    // Phase 1 fix: `DisplayInfo.width / height` are **logical** points, and
    // `scale_factor` is the device pixel ratio (2.0 on Retina, 1.5/1.75 on
    // Windows mixed-DPI, etc.). The screenshot we hand to the model is
    // captured in **native** pixels, so to translate a global logical point
    // into the same coordinate space we must scale by `scale_factor`.
    //
    // Previously this used `d.width` for the native pixel count, which
    // collapsed to a no-op transform: clicks landed at the logical
    // coordinate inside a 2x-resolution image, missing the target by half
    // the screen on Retina displays. This was the root cause of `locate +
    // click` falling on the wrong element on multi-display / mixed-DPI Macs.
    let disp_ox = d.x as f64;
    let disp_oy = d.y as f64;
    let disp_w = d.width as f64;
    let disp_h = d.height as f64;
    if disp_w <= 0.0 || disp_h <= 0.0 || d.width == 0 || d.height == 0 {
        return Err(BitFunError::tool(
            "Invalid display geometry for UI locate mapping.".to_string(),
        ));
    }
    let scale = if d.scale_factor > 0.0 {
        d.scale_factor as f64
    } else {
        1.0
    };
    let px_w = disp_w * scale;
    let px_h = disp_h * scale;
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

/// `role_substring` match with macOS AX aliases: chat apps often expose compose as **`AXTextField`**
/// while models ask for `TextArea`; treat those as overlapping for locate/click_element.
pub fn role_substring_matches_ax_role(ax_role: &str, want: &str) -> bool {
    let w = want.trim();
    if w.is_empty() {
        return true;
    }
    if contains_ci(ax_role, w) {
        return true;
    }
    let wl = w.to_lowercase();
    match wl.as_str() {
        "textarea" | "text area" | "text_area" | "axtextarea" => {
            contains_ci(ax_role, "TextArea") || contains_ci(ax_role, "TextField")
        }
        "textfield" | "text field" | "text_field" | "axtextfield" => {
            contains_ci(ax_role, "TextField") || contains_ci(ax_role, "TextArea")
        }
        _ => false,
    }
}

fn combine_is_any(query: &UiElementLocateQuery) -> bool {
    matches!(query.filter_combine.as_deref(), Some("any") | Some("or"))
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
            if role_substring_matches_ax_role(role.unwrap_or(""), want.trim()) {
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
            if !role_substring_matches_ax_role(r, want.trim()) {
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
#[allow(clippy::too_many_arguments)]
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
        gx,
        gy,
        bounds_left,
        bounds_top,
        bounds_width,
        bounds_height,
        matched_role,
        matched_title,
        matched_identifier,
        None,
        1,
        vec![],
    )
}

#[allow(clippy::too_many_arguments)]
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
        global_bounds_to_native_minmax(
            gx,
            gy,
            bounds_left,
            bounds_top,
            bounds_width,
            bounds_height,
        )?
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_textarea_alias_matches_axtextfield() {
        assert!(role_substring_matches_ax_role("AXTextField", "TextArea"));
        assert!(role_substring_matches_ax_role("AXTextField", "textarea"));
        assert!(!role_substring_matches_ax_role("AXButton", "TextArea"));
    }

    #[test]
    fn role_textfield_alias_matches_axtextarea() {
        assert!(role_substring_matches_ax_role("AXTextArea", "TextField"));
    }

    /// Build a synthetic `DisplayInfo` for unit tests without going through
    /// the platform-specific constructors. We only need the fields the
    /// mapping function reads.
    fn fake_display(x: i32, y: i32, w: u32, h: u32, scale: f32) -> DisplayInfo {
        let mut d: DisplayInfo = unsafe { std::mem::zeroed() };
        d.x = x;
        d.y = y;
        d.width = w;
        d.height = h;
        d.scale_factor = scale;
        d
    }

    #[test]
    fn maps_global_to_native_on_retina_display() {
        // 1440×900 logical, 2.0 scale ⇒ 2880×1800 native.
        let d = fake_display(0, 0, 1440, 900, 2.0);
        // Center: logical (720, 450) ⇒ native (1440, 900)
        let (nx, ny) = global_xy_to_native_with_display(&d, 720.0, 450.0).unwrap();
        assert_eq!((nx, ny), (1440, 900));
        // Bottom-right corner clamped to last native pixel.
        let (nx, ny) = global_xy_to_native_with_display(&d, 1440.0, 900.0).unwrap();
        assert_eq!((nx, ny), (2879, 1799));
    }

    #[test]
    fn maps_global_to_native_on_secondary_offset_display_with_fractional_scale() {
        // Secondary monitor placed to the right of a primary, 1920×1080
        // logical with 1.5 scale (common Windows config) ⇒ 2880×1620 native.
        let d = fake_display(1440, 0, 1920, 1080, 1.5);
        // A point at logical (1440 + 960, 540) = display center.
        let (nx, ny) = global_xy_to_native_with_display(&d, 2400.0, 540.0).unwrap();
        assert_eq!((nx, ny), (1440, 810));
    }

    #[test]
    fn maps_global_to_native_with_unit_scale_is_identity() {
        let d = fake_display(0, 0, 800, 600, 1.0);
        let (nx, ny) = global_xy_to_native_with_display(&d, 100.0, 200.0).unwrap();
        assert_eq!((nx, ny), (100, 200));
    }
}

/// Whether an element's global bounds fall within any visible display.
#[allow(dead_code)]
pub fn is_element_on_screen(gx: f64, gy: f64, width: f64, height: f64) -> bool {
    // Element must have reasonable size (not a giant container)
    if width > 3000.0 || height > 2000.0 {
        return false;
    }
    // Center must be resolvable to a display
    DisplayInfo::from_point(gx.round() as i32, gy.round() as i32).is_ok()
}

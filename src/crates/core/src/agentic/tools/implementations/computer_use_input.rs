use crate::agentic::tools::computer_use_host::{
    ComputerUseImplicitScreenshotCenter, ComputerUseNavigateQuadrant, ComputerUseScreenshotParams,
    ScreenshotCropCenter,
};
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json::Value;

pub fn use_screen_coordinates(input: &Value) -> bool {
    input
        .get("use_screen_coordinates")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Rejects JPEG/normalized coordinates for pointer moves — vision-derived positions are unreliable.
/// Use `use_screen_coordinates: true` with globals from OCR/AX tools, or non-coordinate actions.
pub fn ensure_pointer_move_uses_screen_coordinates_only(input: &Value) -> BitFunResult<()> {
    if use_screen_coordinates(input) {
        return Ok(());
    }
    Err(BitFunError::tool(
        "Positioning from screenshot pixels (coordinate_mode image/normalized) is disabled: do not guess coordinates from vision. Set use_screen_coordinates: true with global display coordinates from move_to_text (global_center_x/y), locate, click_element, or pointer_image_x/y from the last screenshot JSON; or use move_to_text, click_element, click_label, pointer_move_rel, ComputerUseMouseStep. Screenshots are for confirmation only.".to_string(),
    ))
}

pub fn coordinate_mode(input: &Value) -> &str {
    input
        .get("coordinate_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("image")
}

pub fn parse_screenshot_crop_center(input: &Value) -> BitFunResult<Option<ScreenshotCropCenter>> {
    let xv = input.get("screenshot_crop_center_x");
    let yv = input.get("screenshot_crop_center_y");
    let x_none = xv.is_none() || xv.is_some_and(|v| v.is_null());
    let y_none = yv.is_none() || yv.is_some_and(|v| v.is_null());

    match (x_none, y_none) {
        (true, true) => Ok(None),
        (false, false) => {
            let x = xv
                .and_then(|v| v.as_u64())
                .ok_or_else(|| BitFunError::tool("screenshot_crop_center_x must be a non-negative integer (full-display native pixels).".to_string()))?;
            let y = yv
                .and_then(|v| v.as_u64())
                .ok_or_else(|| BitFunError::tool("screenshot_crop_center_y must be a non-negative integer (full-display native pixels).".to_string()))?;
            Ok(Some(ScreenshotCropCenter {
                x: u32::try_from(x)
                    .map_err(|_| BitFunError::tool("screenshot_crop_center_x is too large.".to_string()))?,
                y: u32::try_from(y)
                    .map_err(|_| BitFunError::tool("screenshot_crop_center_y is too large.".to_string()))?,
            }))
        }
        _ => Err(BitFunError::tool(
            "screenshot_crop_center_x and screenshot_crop_center_y must both be set or both omitted for action screenshot.".to_string(),
        )),
    }
}

pub fn parse_screenshot_crop_half_extent_native(input: &Value) -> BitFunResult<Option<u32>> {
    match input.get("screenshot_crop_half_extent_native") {
        None => Ok(None),
        Some(v) if v.is_null() => Ok(None),
        Some(v) => {
            let n = v.as_u64().ok_or_else(|| {
                BitFunError::tool(
                    "screenshot_crop_half_extent_native must be a non-negative integer."
                        .to_string(),
                )
            })?;
            Ok(Some(u32::try_from(n).map_err(|_| {
                BitFunError::tool("screenshot_crop_half_extent_native is too large.".to_string())
            })?))
        }
    }
}

pub fn input_has_screenshot_crop_fields(input: &Value) -> bool {
    let x = input.get("screenshot_crop_center_x");
    let y = input.get("screenshot_crop_center_y");
    x.is_some_and(|v| !v.is_null()) || y.is_some_and(|v| !v.is_null())
}

pub fn parse_screenshot_implicit_center(
    input: &Value,
) -> BitFunResult<Option<ComputerUseImplicitScreenshotCenter>> {
    match input
        .get("screenshot_implicit_center")
        .and_then(|v| v.as_str())
        .map(str::trim)
    {
        None | Some("") => Ok(None),
        Some("mouse") => Ok(Some(ComputerUseImplicitScreenshotCenter::Mouse)),
        Some("text_caret") => Ok(Some(ComputerUseImplicitScreenshotCenter::TextCaret)),
        Some(other) => Err(BitFunError::tool(format!(
            "screenshot_implicit_center must be \"mouse\" or \"text_caret\", got {:?}",
            other
        ))),
    }
}

pub fn parse_screenshot_navigate_quadrant(
    input: &Value,
) -> BitFunResult<Option<ComputerUseNavigateQuadrant>> {
    let value = input
        .get("screenshot_navigate_quadrant")
        .filter(|x| !x.is_null())
        .and_then(|x| x.as_str());
    let Some(s) = value else {
        return Ok(None);
    };

    let n = s.trim().to_ascii_lowercase().replace('-', "_");
    Ok(Some(match n.as_str() {
        "top_left" | "topleft" | "upper_left" => ComputerUseNavigateQuadrant::TopLeft,
        "top_right" | "topright" | "upper_right" => ComputerUseNavigateQuadrant::TopRight,
        "bottom_left" | "bottomleft" | "lower_left" => ComputerUseNavigateQuadrant::BottomLeft,
        "bottom_right" | "bottomright" | "lower_right" => ComputerUseNavigateQuadrant::BottomRight,
        _ => {
            return Err(BitFunError::tool(
                "screenshot_navigate_quadrant must be one of: top_left, top_right, bottom_left, bottom_right.".to_string(),
            ));
        }
    }))
}

pub fn parse_screenshot_params(input: &Value) -> BitFunResult<(ComputerUseScreenshotParams, bool)> {
    let navigate = parse_screenshot_navigate_quadrant(input)?;
    let reset_navigation = input
        .get("screenshot_reset_navigation")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let implicit_center = parse_screenshot_implicit_center(input)?;

    if navigate.is_some() {
        let ignored_crop = input_has_screenshot_crop_fields(input);
        return Ok((
            ComputerUseScreenshotParams {
                crop_center: None,
                navigate_quadrant: navigate,
                reset_navigation,
                point_crop_half_extent_native: None,
                implicit_confirmation_center: implicit_center,
                crop_to_focused_window: false,
            },
            ignored_crop,
        ));
    }

    let crop = parse_screenshot_crop_center(input)?;
    let half = if crop.is_some() {
        parse_screenshot_crop_half_extent_native(input)?
    } else {
        None
    };

    Ok((
        ComputerUseScreenshotParams {
            crop_center: crop,
            navigate_quadrant: None,
            reset_navigation,
            point_crop_half_extent_native: half,
            implicit_confirmation_center: implicit_center,
            crop_to_focused_window: false,
        },
        false,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn screenshot_params_prefer_quadrant_over_crop_fields() {
        let input = json!({
            "screenshot_navigate_quadrant": "top_left",
            "screenshot_crop_center_x": 120,
            "screenshot_crop_center_y": 340,
            "screenshot_reset_navigation": true,
        });

        let (params, ignored_crop) =
            parse_screenshot_params(&input).expect("parse screenshot params");

        assert_eq!(
            params.navigate_quadrant,
            Some(ComputerUseNavigateQuadrant::TopLeft)
        );
        assert_eq!(params.crop_center, None);
        assert!(params.reset_navigation);
        assert!(ignored_crop);
    }

    #[test]
    fn screenshot_params_parse_crop_half_extent_only_with_crop() {
        let input = json!({
            "screenshot_crop_center_x": 33,
            "screenshot_crop_center_y": 44,
            "screenshot_crop_half_extent_native": 180
        });

        let (params, ignored_crop) =
            parse_screenshot_params(&input).expect("parse screenshot params");

        let crop = params.crop_center.expect("crop center");
        assert_eq!(crop.x, 33);
        assert_eq!(crop.y, 44);
        assert_eq!(params.point_crop_half_extent_native, Some(180));
        assert!(!ignored_crop);
    }

    #[test]
    fn screenshot_params_parse_implicit_center() {
        let input = json!({
            "screenshot_implicit_center": "text_caret"
        });
        let (params, _) = parse_screenshot_params(&input).expect("parse");
        assert_eq!(
            params.implicit_confirmation_center,
            Some(ComputerUseImplicitScreenshotCenter::TextCaret)
        );
    }
}

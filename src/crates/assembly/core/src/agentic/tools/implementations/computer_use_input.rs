//! Compatibility shims for the former core-owned Computer Use input helpers.
//!
//! The provider-neutral implementation now lives in `bitfun-agent-tools`.

use crate::agentic::tools::computer_use_host::{
    ComputerUseImplicitScreenshotCenter, ComputerUseNavigateQuadrant, ComputerUseScreenshotParams,
    ScreenshotCropCenter,
};
use crate::util::errors::BitFunResult;
use serde_json::Value;

pub use bitfun_agent_tools::computer_use::{
    coordinate_mode, input_has_screenshot_crop_fields, parse_screenshot_window_flag,
    use_screen_coordinates,
};

pub fn ensure_pointer_move_uses_screen_coordinates_only(input: &Value) -> BitFunResult<()> {
    bitfun_agent_tools::computer_use::ensure_pointer_move_uses_screen_coordinates_only(input)
        .map_err(Into::into)
}

pub fn parse_screenshot_crop_center(input: &Value) -> BitFunResult<Option<ScreenshotCropCenter>> {
    bitfun_agent_tools::computer_use::parse_screenshot_crop_center(input).map_err(Into::into)
}

pub fn parse_screenshot_crop_half_extent_native(input: &Value) -> BitFunResult<Option<u32>> {
    bitfun_agent_tools::computer_use::parse_screenshot_crop_half_extent_native(input)
        .map_err(Into::into)
}

pub fn parse_screenshot_implicit_center(
    input: &Value,
) -> BitFunResult<Option<ComputerUseImplicitScreenshotCenter>> {
    bitfun_agent_tools::computer_use::parse_screenshot_implicit_center(input).map_err(Into::into)
}

pub fn parse_screenshot_navigate_quadrant(
    input: &Value,
) -> BitFunResult<Option<ComputerUseNavigateQuadrant>> {
    bitfun_agent_tools::computer_use::parse_screenshot_navigate_quadrant(input).map_err(Into::into)
}

pub fn parse_screenshot_params(input: &Value) -> BitFunResult<(ComputerUseScreenshotParams, bool)> {
    bitfun_agent_tools::computer_use::parse_screenshot_params(input).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compatibility_parser_keeps_old_result_type_and_behavior() {
        let input = json!({
            "screenshot_navigate_quadrant": "top_left",
            "screenshot_crop_center_x": 120,
            "screenshot_crop_center_y": 340,
            "screenshot_reset_navigation": true,
        });

        let (params, ignored_crop) =
            parse_screenshot_params(&input).expect("parse screenshot params");

        assert_eq!(params.navigate_quadrant, None);
        assert_eq!(params.crop_center, None);
        assert!(!params.reset_navigation);
        assert!(!ignored_crop);
    }
}

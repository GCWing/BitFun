use crate::agentic::tools::computer_use_host::{ComputerScreenshot, ComputerUseInteractionState};
use serde_json::{json, Value};

pub fn append_interaction_state(body: &mut Value, interaction: &ComputerUseInteractionState) {
    if let Value::Object(map) = body {
        map.insert("interaction_state".to_string(), json!(interaction));
    }
}

pub fn build_screenshot_body(
    shot: &ComputerScreenshot,
    debug_rel: Option<String>,
    interaction: &ComputerUseInteractionState,
) -> Value {
    let mut data = json!({
        "success": true,
        "mime_type": shot.mime_type,
        "image_width": shot.image_width,
        "image_height": shot.image_height,
        "display_width_px": shot.image_width,
        "display_height_px": shot.image_height,
        "native_width": shot.native_width,
        "native_height": shot.native_height,
        "display_origin_x": shot.display_origin_x,
        "display_origin_y": shot.display_origin_y,
        "vision_scale": shot.vision_scale,
        "pointer_image_x": shot.pointer_image_x,
        "pointer_image_y": shot.pointer_image_y,
        "screenshot_crop_center": shot.screenshot_crop_center,
        "point_crop_half_extent_native": shot.point_crop_half_extent_native,
        "navigation_native_rect": shot.navigation_native_rect,
        "quadrant_navigation_click_ready": shot.quadrant_navigation_click_ready,
        "implicit_confirmation_crop_applied": shot.implicit_confirmation_crop_applied,
        "debug_screenshot_path": debug_rel,
    });
    append_interaction_state(&mut data, interaction);
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::computer_use_host::{
        ComputerUseImageContentRect, ComputerUseInteractionScreenshotKind,
    };

    #[test]
    fn append_interaction_state_includes_structured_block() {
        let mut body = json!({ "success": true });
        let interaction = ComputerUseInteractionState {
            click_ready: false,
            enter_ready: true,
            requires_fresh_screenshot_before_click: true,
            requires_fresh_screenshot_before_enter: false,
            recommend_screenshot_to_verify_last_action: true,
            last_screenshot_kind: Some(ComputerUseInteractionScreenshotKind::FullDisplay),
            last_mutation: None,
            recommended_next_action: Some("screenshot_navigate_quadrant".to_string()),
        };

        append_interaction_state(&mut body, &interaction);

        assert_eq!(body["interaction_state"]["click_ready"], json!(false));
        assert_eq!(body["interaction_state"]["enter_ready"], json!(true));
        assert_eq!(
            body["interaction_state"]["recommended_next_action"],
            json!("screenshot_navigate_quadrant")
        );
        assert_eq!(
            body["interaction_state"]["recommend_screenshot_to_verify_last_action"],
            json!(true)
        );
    }

    #[test]
    fn screenshot_body_keeps_existing_fields_and_adds_interaction_state() {
        let shot = ComputerScreenshot {
            bytes: vec![1, 2, 3],
            mime_type: "image/jpeg".to_string(),
            image_width: 100,
            image_height: 80,
            native_width: 100,
            native_height: 80,
            display_origin_x: 0,
            display_origin_y: 0,
            vision_scale: 1.0,
            pointer_image_x: Some(10),
            pointer_image_y: Some(11),
            screenshot_crop_center: None,
            point_crop_half_extent_native: None,
            navigation_native_rect: None,
            quadrant_navigation_click_ready: false,
            image_content_rect: Some(ComputerUseImageContentRect {
                left: 1,
                top: 2,
                width: 98,
                height: 76,
            }),
            som_labels: vec![],
            implicit_confirmation_crop_applied: false,
        };
        let interaction = ComputerUseInteractionState {
            click_ready: false,
            enter_ready: true,
            requires_fresh_screenshot_before_click: true,
            requires_fresh_screenshot_before_enter: false,
            recommend_screenshot_to_verify_last_action: false,
            last_screenshot_kind: Some(ComputerUseInteractionScreenshotKind::FullDisplay),
            last_mutation: None,
            recommended_next_action: Some("screenshot_navigate_quadrant".to_string()),
        };

        let body = build_screenshot_body(&shot, None, &interaction);

        assert_eq!(body["success"], json!(true));
        assert_eq!(body["mime_type"], json!("image/jpeg"));
        assert_eq!(
            body["interaction_state"]["last_screenshot_kind"],
            json!("full_display")
        );
    }
}

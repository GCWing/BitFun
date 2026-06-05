use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;

use super::types::ElementScreenshotMetadata;
use crate::server::response::WebDriverErrorResponse;

pub fn crop_screenshot(
    screenshot_base64: String,
    metadata: ElementScreenshotMetadata,
) -> Result<String, WebDriverErrorResponse> {
    let png_bytes = BASE64_STANDARD.decode(screenshot_base64).map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Invalid PNG payload: {error}"))
    })?;
    let image = image::load_from_memory(&png_bytes).map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to decode screenshot PNG: {error}"))
    })?;

    let scale = if metadata.device_pixel_ratio.is_finite() && metadata.device_pixel_ratio > 0.0 {
        metadata.device_pixel_ratio
    } else {
        1.0
    };

    let x = (metadata.x * scale).floor().max(0.0) as u32;
    let y = (metadata.y * scale).floor().max(0.0) as u32;
    let width = (metadata.width * scale).ceil().max(1.0) as u32;
    let height = (metadata.height * scale).ceil().max(1.0) as u32;

    let image_width = image.width();
    let image_height = image.height();
    if x >= image_width || y >= image_height {
        return Err(WebDriverErrorResponse::unknown_error(
            "Element screenshot rectangle is outside the viewport",
        ));
    }

    let clamped_width = width.min(image_width.saturating_sub(x)).max(1);
    let clamped_height = height.min(image_height.saturating_sub(y)).max(1);
    let cropped = image.crop_imm(x, y, clamped_width, clamped_height);

    let mut png = std::io::Cursor::new(Vec::new());
    cropped
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to encode cropped PNG: {error}"))
        })?;

    Ok(BASE64_STANDARD.encode(png.into_inner()))
}

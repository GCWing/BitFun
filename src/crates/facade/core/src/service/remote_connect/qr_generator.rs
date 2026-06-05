//! QR code generation for Remote Connect pairing.

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use qrcode::QrCode;

use super::pairing::QrPayload;

pub struct QrGenerator;

impl QrGenerator {
    /// Build the URL that the QR code points to.
    /// `web_app_url` = where the mobile web app is hosted.
    /// `payload.url` = the relay server that the mobile WebSocket should connect to.
    pub fn build_url(payload: &QrPayload, web_app_url: &str, language: &str) -> String {
        let relay_ws = payload
            .url
            .replace("https://", "wss://")
            .replace("http://", "ws://");
        format!(
            "{web_app}/#/pair?room={room}&did={did}&pk={pk}&dn={dn}&relay={relay}&v={v}&lang={lang}",
            web_app = web_app_url.trim_end_matches('/'),
            room = urlencoding::encode(&payload.room_id),
            did = urlencoding::encode(&payload.device_id),
            pk = urlencoding::encode(&payload.public_key),
            dn = urlencoding::encode(&payload.device_name),
            relay = urlencoding::encode(&relay_ws),
            v = payload.version,
            lang = urlencoding::encode(language),
        )
    }

    /// Generate a QR code as a base64-encoded PNG from a pre-built URL.
    pub fn generate_png_base64_from_url(url: &str) -> Result<String> {
        let code =
            QrCode::new(url.as_bytes()).map_err(|e| anyhow!("QR code generation failed: {e}"))?;
        let img = code.render::<image::Luma<u8>>().quiet_zone(true).build();
        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(
            encoder,
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::L8,
        )
        .map_err(|e| anyhow!("PNG encoding failed: {e}"))?;
        Ok(BASE64.encode(&buf))
    }

    /// Generate the QR code as an SVG string from a pre-built URL.
    pub fn generate_svg_from_url(url: &str) -> Result<String> {
        let code =
            QrCode::new(url.as_bytes()).map_err(|e| anyhow!("QR code generation failed: {e}"))?;
        let svg = code
            .render::<qrcode::render::svg::Color>()
            .quiet_zone(true)
            .build();
        Ok(svg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::remote_connect::pairing::QrPayload;

    #[test]
    fn build_url_includes_language_parameter() {
        let payload = QrPayload {
            room_id: "room_123".to_string(),
            url: "https://relay.example.com".to_string(),
            device_id: "device_123".to_string(),
            device_name: "BitFun Desktop".to_string(),
            public_key: "public_key_value".to_string(),
            version: 1,
        };

        let url = QrGenerator::build_url(&payload, "https://mobile.example.com", "en-US");
        assert!(url.contains("lang=en-US"));
    }
}

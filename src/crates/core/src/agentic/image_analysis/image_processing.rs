//! Shared image processing utilities used by both API-side image analysis and tool-driven image analysis.

use super::types::{ImageContextData, ImageLimits};
use crate::service::config::get_global_config_service;
use crate::service::config::types::{
    AIConfig as ServiceAIConfig, AIModelConfig, ModelCapability, ModelCategory,
};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::Message;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::ColorType;
use image::DynamicImage;
use image::ImageEncoder;
use image::ImageFormat;
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone)]
pub struct ProcessedImage {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
}

pub fn resolve_vision_model_from_ai_config(
    ai_config: &ServiceAIConfig,
) -> BitFunResult<AIModelConfig> {
    let target_model_id = ai_config
        .default_models
        .image_understanding
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());

    let Some(id) = target_model_id else {
        return Err(BitFunError::service(
            "Image understanding model is not configured.\nPlease select a model in Settings."
                .to_string(),
        ));
    };

    let model = ai_config
        .models
        .iter()
        .find(|m| m.id == id)
        .cloned()
        .ok_or_else(|| BitFunError::service(format!("Model not found: {}", id)))?;

    if !model.enabled {
        return Err(BitFunError::service(format!("Model is disabled: {}", id)));
    }

    let supports_image_understanding = model
        .capabilities
        .iter()
        .any(|cap| matches!(cap, ModelCapability::ImageUnderstanding))
        || matches!(model.category, ModelCategory::Multimodal);
    if !supports_image_understanding {
        return Err(BitFunError::service(format!(
            "Model does not support image understanding: {}",
            id
        )));
    }

    Ok(model)
}

pub async fn resolve_vision_model_from_global_config() -> BitFunResult<AIModelConfig> {
    let config_service = get_global_config_service().await?;
    let ai_config: ServiceAIConfig = config_service
        .get_config(Some("ai"))
        .await
        .map_err(|e| BitFunError::service(format!("Failed to get AI config: {}", e)))?;

    resolve_vision_model_from_ai_config(&ai_config)
}

pub fn resolve_image_path(path: &str, workspace_path: Option<&Path>) -> BitFunResult<PathBuf> {
    let path_buf = PathBuf::from(path);

    if path_buf.is_absolute() {
        Ok(path_buf)
    } else if let Some(workspace) = workspace_path {
        Ok(workspace.join(path_buf))
    } else {
        Ok(path_buf)
    }
}

pub async fn load_image_from_path(
    path: &Path,
    _workspace_path: Option<&Path>,
) -> BitFunResult<Vec<u8>> {
    fs::read(path)
        .await
        .map_err(|e| BitFunError::io(format!("Failed to read image: {}", e)))
}

pub fn decode_data_url(data_url: &str) -> BitFunResult<(Vec<u8>, Option<String>)> {
    if !data_url.starts_with("data:") {
        return Err(BitFunError::validation("Invalid data URL format"));
    }

    let parts: Vec<&str> = data_url.splitn(2, ',').collect();
    if parts.len() != 2 {
        return Err(BitFunError::validation("Data URL format error"));
    }

    let header = parts[0];
    let mime_type = header
        .strip_prefix("data:")
        .and_then(|s| s.split(';').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    let base64_data = parts[1];
    let image_data = BASE64
        .decode(base64_data)
        .map_err(|e| BitFunError::parse(format!("Base64 decode failed: {}", e)))?;

    Ok((image_data, mime_type))
}

pub fn detect_mime_type_from_bytes(
    image_data: &[u8],
    fallback_mime: Option<&str>,
) -> BitFunResult<String> {
    if let Ok(format) = image::guess_format(image_data) {
        if let Some(mime) = image_format_to_mime(format) {
            return Ok(mime.to_string());
        }
    }

    if let Some(fallback) = fallback_mime {
        if fallback.starts_with("image/") {
            return Ok(fallback.to_string());
        }
    }

    Err(BitFunError::validation(
        "Unsupported or unrecognized image format",
    ))
}

pub fn optimize_image_for_provider(
    image_data: Vec<u8>,
    provider: &str,
    fallback_mime: Option<&str>,
) -> BitFunResult<ProcessedImage> {
    optimize_image_with_size_limit(image_data, provider, fallback_mime, None)
}

/// Like `optimize_image_for_provider` but allows an explicit size cap.
/// When `max_output_size` is `Some(n)`, the effective limit is
/// `min(provider_limit, n)`.
pub fn optimize_image_with_size_limit(
    image_data: Vec<u8>,
    provider: &str,
    fallback_mime: Option<&str>,
    max_output_size: Option<usize>,
) -> BitFunResult<ProcessedImage> {
    let limits = ImageLimits::for_provider(provider);
    let effective_max = match max_output_size {
        Some(cap) => cap.min(limits.max_size),
        None => limits.max_size,
    };

    let guessed_format = image::guess_format(&image_data).ok();
    let dynamic = image::load_from_memory(&image_data)
        .map_err(|e| BitFunError::validation(format!("Failed to decode image data: {}", e)))?;

    let (orig_width, orig_height) = (dynamic.width(), dynamic.height());
    let needs_resize = orig_width > limits.max_width || orig_height > limits.max_height;

    if !needs_resize && image_data.len() <= effective_max {
        let mime_type = detect_mime_type_from_bytes(&image_data, fallback_mime)?;
        return Ok(ProcessedImage {
            data: image_data,
            mime_type,
            width: orig_width,
            height: orig_height,
        });
    }

    let mut working = if needs_resize {
        dynamic.resize(limits.max_width, limits.max_height, FilterType::Triangle)
    } else {
        dynamic
    };

    let preferred_format = match guessed_format {
        Some(ImageFormat::Jpeg) => ImageFormat::Jpeg,
        _ => ImageFormat::Png,
    };

    let mut encoded = encode_dynamic_image(&working, preferred_format, 85)?;

    if encoded.0.len() > effective_max {
        for quality in [80u8, 65, 50, 35] {
            encoded = encode_dynamic_image(&working, ImageFormat::Jpeg, quality)?;
            if encoded.0.len() <= effective_max {
                break;
            }
        }
    }

    if encoded.0.len() > effective_max {
        for _ in 0..5 {
            let next_w = ((working.width() as f32) * 0.75).round().max(64.0) as u32;
            let next_h = ((working.height() as f32) * 0.75).round().max(64.0) as u32;
            if next_w == working.width() && next_h == working.height() {
                break;
            }

            working = working.resize(next_w, next_h, FilterType::Triangle);

            for quality in [70u8, 55, 40, 25] {
                encoded = encode_dynamic_image(&working, ImageFormat::Jpeg, quality)?;
                if encoded.0.len() <= effective_max {
                    break;
                }
            }

            if encoded.0.len() <= effective_max {
                break;
            }
        }
    }

    Ok(ProcessedImage {
        data: encoded.0,
        mime_type: encoded.1,
        width: working.width(),
        height: working.height(),
    })
}

pub fn build_multimodal_message(
    prompt: &str,
    image_data: &[u8],
    mime_type: &str,
    provider: &str,
) -> BitFunResult<Vec<Message>> {
    let base64_data = BASE64.encode(image_data);
    let provider_lower = provider.to_lowercase();

    let message = if provider_lower.contains("anthropic") {
        Message {
            role: "user".to_string(),
            content: Some(serde_json::to_string(&json!([
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": mime_type,
                        "data": base64_data
                    }
                },
                {
                    "type": "text",
                    "text": prompt
                }
            ]))?),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }
    } else if provider_lower.contains("gemini") || provider_lower.contains("google") {
        Message {
            role: "user".to_string(),
            content: Some(serde_json::to_string(&json!([
                {
                    "inline_data": {
                        "mime_type": mime_type,
                        "data": base64_data
                    }
                },
                {
                    "text": prompt
                }
            ]))?),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }
    } else {
        // Default to OpenAI-compatible payload shape for OpenAI and most OpenAI-compatible providers.
        Message {
            role: "user".to_string(),
            content: Some(serde_json::to_string(&json!([
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}", mime_type, base64_data)
                    }
                },
                {
                    "type": "text",
                    "text": prompt
                }
            ]))?),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }
    };

    Ok(vec![message])
}

pub async fn process_image_contexts_for_provider(
    image_contexts: &[ImageContextData],
    provider: &str,
    workspace_path: Option<&Path>,
) -> BitFunResult<Vec<ProcessedImage>> {
    let limits = ImageLimits::for_provider(provider);

    if image_contexts.len() > limits.max_images_per_request {
        return Err(BitFunError::validation(format!(
            "Too many images in one request: {} > {}",
            image_contexts.len(),
            limits.max_images_per_request
        )));
    }

    let mut results = Vec::with_capacity(image_contexts.len());

    for ctx in image_contexts {
        let (image_data, fallback_mime) = if let Some(data_url) = &ctx.data_url {
            let (data, data_url_mime) = decode_data_url(data_url)?;
            (data, data_url_mime.or_else(|| Some(ctx.mime_type.clone())))
        } else if let Some(path_str) = &ctx.image_path {
            let path = resolve_image_path(path_str, workspace_path)?;
            let data = load_image_from_path(&path, workspace_path).await?;
            let detected_mime = detect_mime_type_from_bytes(&data, Some(&ctx.mime_type)).ok();
            (data, detected_mime.or_else(|| Some(ctx.mime_type.clone())))
        } else {
            return Err(BitFunError::validation(format!(
                "Image context missing image_path/data_url: id={}",
                ctx.id
            )));
        };

        let processed =
            optimize_image_for_provider(image_data, provider, fallback_mime.as_deref())?;
        results.push(processed);
    }

    Ok(results)
}

pub fn build_multimodal_message_with_images(
    prompt: &str,
    images: &[ProcessedImage],
    provider: &str,
) -> BitFunResult<Vec<Message>> {
    if images.is_empty() {
        return Ok(vec![Message::user(prompt.to_string())]);
    }

    let provider_lower = provider.to_lowercase();

    let content_json = if provider_lower.contains("anthropic") {
        let mut blocks = Vec::with_capacity(images.len() + 1);
        for img in images {
            let base64_data = BASE64.encode(&img.data);
            blocks.push(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": img.mime_type,
                    "data": base64_data
                }
            }));
        }
        blocks.push(json!({
            "type": "text",
            "text": prompt
        }));
        json!(blocks)
    } else if provider_lower.contains("gemini") || provider_lower.contains("google") {
        let mut parts = Vec::with_capacity(images.len() + 1);
        for img in images {
            let base64_data = BASE64.encode(&img.data);
            parts.push(json!({
                "inline_data": {
                    "mime_type": img.mime_type,
                    "data": base64_data
                }
            }));
        }
        parts.push(json!({ "text": prompt }));
        json!(parts)
    } else {
        let mut blocks = Vec::with_capacity(images.len() + 1);
        for img in images {
            let base64_data = BASE64.encode(&img.data);
            blocks.push(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{};base64,{}", img.mime_type, base64_data)
                }
            }));
        }
        blocks.push(json!({
            "type": "text",
            "text": prompt
        }));
        json!(blocks)
    };

    Ok(vec![Message {
        role: "user".to_string(),
        content: Some(serde_json::to_string(&content_json)?),
        reasoning_content: None,
        thinking_signature: None,
        tool_calls: None,
        tool_call_id: None,
        name: None,
        is_error: None,
        tool_image_attachments: None,
    }])
}

fn image_format_to_mime(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Png => Some("image/png"),
        ImageFormat::Jpeg => Some("image/jpeg"),
        ImageFormat::Gif => Some("image/gif"),
        ImageFormat::WebP => Some("image/webp"),
        ImageFormat::Bmp => Some("image/bmp"),
        _ => None,
    }
}

fn encode_dynamic_image(
    image: &DynamicImage,
    format: ImageFormat,
    jpeg_quality: u8,
) -> BitFunResult<(Vec<u8>, String)> {
    let target_format = match format {
        ImageFormat::Jpeg => ImageFormat::Jpeg,
        _ => ImageFormat::Png,
    };

    let mut buffer = Vec::new();

    match target_format {
        ImageFormat::Png => {
            let rgba = image.to_rgba8();
            let encoder = PngEncoder::new(&mut buffer);
            encoder
                .write_image(
                    rgba.as_raw(),
                    image.width(),
                    image.height(),
                    ColorType::Rgba8.into(),
                )
                .map_err(|e| BitFunError::tool(format!("PNG encode failed: {}", e)))?;
        }
        ImageFormat::Jpeg => {
            let mut encoder = JpegEncoder::new_with_quality(&mut buffer, jpeg_quality);
            encoder
                .encode_image(image)
                .map_err(|e| BitFunError::tool(format!("JPEG encode failed: {}", e)))?;
        }
        _ => unreachable!("unsupported target format"),
    }

    let mime = image_format_to_mime(target_format)
        .unwrap_or("image/png")
        .to_string();

    Ok((buffer, mime))
}

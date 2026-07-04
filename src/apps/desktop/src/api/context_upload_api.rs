//! Temporary Image Storage API

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bitfun_core::agentic::tools::image_context::{
    create_image_context_provider as create_core_image_context_provider, store_image_contexts,
    GlobalImageContextProvider, ImageContextData as CoreImageContextData,
};
use bitfun_core::infrastructure::try_get_path_manager_arc;
use log::warn;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageContextData {
    pub id: String,
    pub image_path: Option<String>,
    pub data_url: Option<String>,
    pub mime_type: String,
    pub image_name: String,
    pub file_size: usize,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub source: String,
}

impl From<ImageContextData> for CoreImageContextData {
    fn from(data: ImageContextData) -> Self {
        CoreImageContextData {
            id: data.id,
            image_path: data.image_path,
            data_url: data.data_url,
            mime_type: data.mime_type,
            image_name: data.image_name,
            file_size: data.file_size,
            width: data.width,
            height: data.height,
            source: data.source,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UploadImageContextRequest {
    pub images: Vec<ImageContextData>,
}

#[derive(Debug, Serialize)]
pub struct UploadedImageContextResponse {
    pub id: String,
    pub image_path: Option<String>,
}

#[tauri::command]
pub async fn upload_image_contexts(
    request: UploadImageContextRequest,
) -> Result<Vec<UploadedImageContextResponse>, String> {
    let mut images = Vec::with_capacity(request.images.len());
    for image in request.images {
        images.push(prepare_image_context_for_storage(image).await?);
    }
    let response = images
        .iter()
        .map(|image| UploadedImageContextResponse {
            id: image.id.clone(),
            image_path: image
                .image_path
                .as_ref()
                .filter(|path| !path.trim().is_empty())
                .cloned(),
        })
        .collect();
    let images: Vec<CoreImageContextData> = images.into_iter().map(Into::into).collect();
    store_image_contexts(images);
    Ok(response)
}

pub fn create_image_context_provider() -> GlobalImageContextProvider {
    create_core_image_context_provider()
}

async fn prepare_image_context_for_storage(
    mut image: ImageContextData,
) -> Result<ImageContextData, String> {
    if has_text(image.image_path.as_deref()) {
        return Ok(image);
    }

    let Some(data_url) = image
        .data_url
        .as_deref()
        .filter(|value| has_text(Some(value)))
    else {
        return Ok(image);
    };

    match persist_uploaded_image(data_url, &image).await {
        Ok(path) => {
            image.image_path = Some(path.to_string_lossy().to_string());
        }
        Err(error) => {
            warn!(
                "Failed to persist uploaded image to temp storage: image_id={}, error={}",
                image.id, error
            );
            return Err(error);
        }
    }

    Ok(image)
}

async fn persist_uploaded_image(
    data_url: &str,
    image: &ImageContextData,
) -> Result<PathBuf, String> {
    let (bytes, mime_type) = decode_data_url(data_url)?;
    let ext = image_extension(
        mime_type.as_deref().or(Some(image.mime_type.as_str())),
        &image.image_name,
    );
    let file_name = format!("{}-{}.{}", safe_file_stem(&image.id), Uuid::new_v4(), ext);
    let dir = uploaded_image_dir()?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|error| format!("Failed to create image attachment directory: {error}"))?;
    let path = dir.join(file_name);
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|error| format!("Failed to write image attachment: {error}"))?;
    Ok(path)
}

fn uploaded_image_dir() -> Result<PathBuf, String> {
    let root = try_get_path_manager_arc()
        .map(|manager| manager.temp_dir())
        .unwrap_or_else(|_| std::env::temp_dir().join("bitfun"));
    Ok(root.join("attachments").join("images"))
}

fn decode_data_url(data_url: &str) -> Result<(Vec<u8>, Option<String>), String> {
    if !data_url.starts_with("data:") {
        return Err("Invalid image data URL".to_string());
    }

    let (header, payload) = data_url
        .split_once(',')
        .ok_or_else(|| "Invalid image data URL format".to_string())?;
    if !header.contains(";base64") {
        return Err("Only base64 image data URLs are supported".to_string());
    }

    let mime_type = header
        .strip_prefix("data:")
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let bytes = BASE64
        .decode(payload)
        .map_err(|error| format!("Failed to decode image data URL: {error}"))?;
    Ok((bytes, mime_type))
}

fn image_extension(mime_type: Option<&str>, image_name: &str) -> String {
    match mime_type.unwrap_or_default().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg".to_string(),
        "image/png" => "png".to_string(),
        "image/gif" => "gif".to_string(),
        "image/webp" => "webp".to_string(),
        "image/bmp" => "bmp".to_string(),
        _ => Path::new(image_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .and_then(|ext| match ext.as_str() {
                "jpg" | "jpeg" => Some("jpg".to_string()),
                "png" => Some("png".to_string()),
                "gif" => Some("gif".to_string()),
                "webp" => Some("webp".to_string()),
                "bmp" => Some("bmp".to_string()),
                _ => None,
            })
            .unwrap_or_else(|| "png".to_string()),
    }
}

fn safe_file_stem(value: &str) -> String {
    let mut out = String::with_capacity(value.len().min(64));
    for ch in value.chars().take(64) {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "image".to_string()
    } else {
        out
    }
}

fn has_text(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

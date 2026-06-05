//! Image context provider and shared in-memory image storage.
//!
//! Through dependency injection mode, tools can access image context without
//! directly depending on specific implementations.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Image context data
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

static IMAGE_STORAGE: LazyLock<DashMap<String, (ImageContextData, u64)>> =
    LazyLock::new(DashMap::new);
const DEFAULT_IMAGE_MAX_AGE_SECS: u64 = 300;

/// Image context provider trait
///
/// Types that implement this trait can provide image data access capabilities to tools
pub trait ImageContextProvider: Send + Sync + std::fmt::Debug {
    /// Get image context data by image_id
    fn get_image(&self, image_id: &str) -> Option<ImageContextData>;

    /// Optional: delete image context (clean up after use)
    fn remove_image(&self, image_id: &str) {
        // Default implementation: do nothing
        let _ = image_id;
    }
}

/// Optional wrapper type, for convenience
pub type ImageContextProviderRef = Arc<dyn ImageContextProvider>;

pub fn store_image_context(image: ImageContextData) {
    let image_id = image.id.clone();
    let timestamp = current_unix_timestamp();
    IMAGE_STORAGE.insert(image_id, (image, timestamp));
    cleanup_expired_images(DEFAULT_IMAGE_MAX_AGE_SECS);
}

pub fn store_image_contexts(images: Vec<ImageContextData>) {
    for image in images {
        store_image_context(image);
    }
}

pub fn get_image_context(image_id: &str) -> Option<ImageContextData> {
    IMAGE_STORAGE
        .get(image_id)
        .map(|entry| entry.value().0.clone())
}

pub fn remove_image_context(image_id: &str) {
    IMAGE_STORAGE.remove(image_id);
}

pub fn format_image_context_reference(image: &ImageContextData) -> String {
    let size_label = if image.file_size > 0 {
        format!(" ({:.1}KB)", image.file_size as f64 / 1024.0)
    } else {
        String::new()
    };

    if let Some(image_path) = &image.image_path {
        format!(
            "[Image: {}{}]\nPath: {}",
            image.image_name, size_label, image_path
        )
    } else {
        format!(
            "[Image: {}{} (from clipboard)]\nImage ID: {}",
            image.image_name, size_label, image.id
        )
    }
}

#[derive(Debug)]
pub struct GlobalImageContextProvider;

impl ImageContextProvider for GlobalImageContextProvider {
    fn get_image(&self, image_id: &str) -> Option<ImageContextData> {
        get_image_context(image_id)
    }

    fn remove_image(&self, image_id: &str) {
        remove_image_context(image_id);
    }
}

pub fn create_image_context_provider() -> GlobalImageContextProvider {
    GlobalImageContextProvider
}

fn cleanup_expired_images(max_age_secs: u64) {
    let now = current_unix_timestamp();
    let expired_keys: Vec<String> = IMAGE_STORAGE
        .iter()
        .filter(|entry| now.saturating_sub(entry.value().1) > max_age_secs)
        .map(|entry| entry.key().clone())
        .collect();

    for key in expired_keys {
        IMAGE_STORAGE.remove(&key);
    }
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

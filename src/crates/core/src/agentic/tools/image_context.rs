//! Image context provider trait
//!
//! Through dependency injection mode, tools can access image context without directly depending on specific implementations

use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

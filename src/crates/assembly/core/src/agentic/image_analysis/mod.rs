//! Image Analysis Module
//!
//! Implements image pre-understanding functionality, converting image content to text descriptions

pub mod enhancer;
pub mod image_processing;
pub mod processor;
pub mod types;

pub use enhancer::MessageEnhancer;
pub use image_processing::{
    build_multimodal_message, build_multimodal_message_with_images, decode_data_url,
    detect_mime_type_from_bytes, load_image_from_path, optimize_image_for_provider,
    optimize_image_with_size_limit, process_image_contexts_for_provider, resolve_image_path,
    resolve_vision_model_from_ai_config, resolve_vision_model_from_global_config, ProcessedImage,
};
pub use processor::ImageAnalyzer;
pub use types::*;

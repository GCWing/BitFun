//! Image Processor
//!
//! Handles image loading, compression, format conversion, and other operations

use super::types::{AnalyzeImagesRequest, ImageAnalysisResult, ImageContextData, ImageLimits};
use crate::infrastructure::ai::AIClient;
use crate::service::config::types::AIModelConfig;
use crate::util::errors::*;
use crate::util::types::Message;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use log::{debug, error, info};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

/// Image Analyzer
pub struct ImageAnalyzer {
    workspace_path: Option<PathBuf>,
    ai_client: Arc<AIClient>,
}

impl ImageAnalyzer {
    pub fn new(workspace_path: Option<PathBuf>, ai_client: Arc<AIClient>) -> Self {
        Self {
            workspace_path,
            ai_client,
        }
    }

    /// Analyze multiple images
    pub async fn analyze_images(
        &self,
        request: AnalyzeImagesRequest,
        model_config: &AIModelConfig,
    ) -> BitFunResult<Vec<ImageAnalysisResult>> {
        info!("Starting analysis of {} images", request.images.len());

        // Process multiple images in parallel
        let mut tasks = vec![];

        for img_ctx in request.images {
            let model = model_config.clone();
            let user_msg = request.user_message.clone();
            let workspace = self.workspace_path.clone();
            let ai_client = self.ai_client.clone();

            let task = tokio::spawn(async move {
                Self::analyze_single_image(
                    img_ctx,
                    &model,
                    user_msg.as_deref(),
                    workspace,
                    ai_client,
                )
                .await
            });

            tasks.push(task);
        }

        // Wait for all analyses to complete
        let mut results = vec![];
        for task in tasks {
            match task.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => {
                    error!("Image analysis failed: {:?}", e);
                    return Err(e);
                }
                Err(e) => {
                    error!("Image analysis task failed: {:?}", e);
                    return Err(BitFunError::service(format!(
                        "Image analysis task failed: {}",
                        e
                    )));
                }
            }
        }

        info!("All image analysis completed");
        Ok(results)
    }

    /// Analyze a single image
    async fn analyze_single_image(
        image_ctx: ImageContextData,
        model: &AIModelConfig,
        user_context: Option<&str>,
        workspace_path: Option<PathBuf>,
        ai_client: Arc<AIClient>,
    ) -> BitFunResult<ImageAnalysisResult> {
        let start = std::time::Instant::now();

        debug!("Analyzing image: {}", image_ctx.id);

        // 1. Load image
        let image_data =
            Self::load_image_from_context(&image_ctx, workspace_path.as_deref()).await?;

        // 2. Image preprocessing (compression, format conversion)
        let (optimized_data, mime_type) =
            Self::optimize_image_for_model(image_data, &image_ctx.mime_type, model)?;

        // 3. Convert to Base64
        let base64_data = BASE64.encode(&optimized_data);

        debug!(
            "Image processing completed: original_type={}, optimized_type={}, size={}KB",
            image_ctx.mime_type,
            mime_type,
            optimized_data.len() / 1024
        );

        // 4. Build analysis prompt
        let analysis_prompt = Self::build_image_analysis_prompt(user_context);

        // 5. Build multimodal message
        let messages = Self::build_multimodal_message(
            &analysis_prompt,
            &base64_data,
            &mime_type,
            &model.provider,
        )?;

        // Save complete multimodal message to AI log
        debug!(target: "ai::image_analysis_request",
            "Complete multimodal message:\n{}",
            serde_json::to_string_pretty(&messages).unwrap_or_else(|_| "Serialization failed".to_string())
        );

        // 6. Call AI model for image analysis
        debug!(
            "Calling vision model: image_id={}, model={}",
            image_ctx.id, model.model_name
        );
        let ai_response = ai_client.send_message(messages, None).await.map_err(|e| {
            error!("AI call failed: {}", e);
            BitFunError::service(format!("Image analysis AI call failed: {}", e))
        })?;

        debug!("AI response content: {}", ai_response.text);

        // 7. Parse response into structured result
        let mut analysis_result = Self::parse_analysis_response(&ai_response.text, &image_ctx.id)?;

        let elapsed = start.elapsed().as_millis() as u64;
        analysis_result.analysis_time_ms = elapsed;

        info!(
            "Image analysis completed: image_id={}, duration={}ms",
            image_ctx.id, elapsed
        );

        Ok(analysis_result)
    }

    /// Load image from context
    async fn load_image_from_context(
        ctx: &ImageContextData,
        workspace_path: Option<&Path>,
    ) -> BitFunResult<Vec<u8>> {
        if let Some(data_url) = &ctx.data_url {
            // Parse from data URL
            Self::decode_data_url(data_url)
        } else if let Some(path_str) = &ctx.image_path {
            // Load from file path
            let path = PathBuf::from(path_str);

            // Security check: ensure path is within workspace
            if let Some(workspace) = workspace_path {
                let canonical_path = tokio::fs::canonicalize(&path)
                    .await
                    .map_err(|e| BitFunError::io(format!("Image file does not exist: {}", e)))?;
                let canonical_workspace = tokio::fs::canonicalize(workspace)
                    .await
                    .map_err(|e| BitFunError::io(format!("Invalid workspace path: {}", e)))?;

                if !canonical_path.starts_with(&canonical_workspace) {
                    return Err(BitFunError::validation(
                        "Image path must be within workspace",
                    ));
                }
            }

            fs::read(&path)
                .await
                .map_err(|e| BitFunError::io(format!("Failed to read image: {}", e)))
        } else {
            Err(BitFunError::validation(
                "Image context missing path or data",
            ))
        }
    }

    /// Decode data URL
    fn decode_data_url(data_url: &str) -> BitFunResult<Vec<u8>> {
        // data:image/png;base64,iVBORw0KG...
        if !data_url.starts_with("data:") {
            return Err(BitFunError::validation("Invalid data URL format"));
        }

        let parts: Vec<&str> = data_url.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(BitFunError::validation("Data URL format error"));
        }

        let base64_data = parts[1];
        BASE64
            .decode(base64_data)
            .map_err(|e| BitFunError::parse(format!("Base64 decoding failed: {}", e)))
    }

    /// Optimize image (compression, format conversion)
    fn optimize_image_for_model(
        image_data: Vec<u8>,
        original_mime: &str,
        model: &AIModelConfig,
    ) -> BitFunResult<(Vec<u8>, String)> {
        // Get model limits
        let limits = ImageLimits::for_provider(&model.provider);

        // If image size is within limit, return directly
        if image_data.len() <= limits.max_size {
            debug!("Image size within limit, no compression needed");
            return Ok((image_data, original_mime.to_string()));
        }

        info!(
            "Image size {}KB exceeds limit {}KB, compression needed",
            image_data.len() / 1024,
            limits.max_size / 1024
        );

        // TODO: Use image crate for actual compression

        // Temporarily return original image, compression logic to be implemented later
        Ok((image_data, original_mime.to_string()))
    }

    /// Build image analysis prompt
    fn build_image_analysis_prompt(user_context: Option<&str>) -> String {
        let mut prompt = String::from(
            "Please analyze the content of this image in detail. Output in the following JSON format:\n\n\
            ```json\n\
            {\n  \
              \"summary\": \"<one-sentence summary of image content>\",\n  \
              \"detailed_description\": \"<detailed description of elements, layout, text, etc.>\",\n  \
              \"detected_elements\": [\"<key element 1>\", \"<key element 2>\", ...],\n  \
              \"confidence\": <number between 0-1, representing analysis confidence>\n\
            }\n\
            ```\n\n\
            Requirements:\n\
            1. summary should be concise and accurate, 1-2 sentences\n\
            2. detailed_description should be comprehensive, including colors, positions, relationships, etc.\n\
            3. detected_elements should extract 5-10 key elements\n\
            4. If the image contains code, architecture diagrams, flowcharts, or other technical content, focus on technical details\n\
            5. Output JSON directly, no additional explanations\n",
        );

        if let Some(context) = user_context {
            prompt.push_str(&format!(
                "\nThe user's question is: \"{}\"\nPlease analyze in conjunction with the user's intent.\n",
                context
            ));
        }

        prompt
    }

    /// Build multimodal message
    fn build_multimodal_message(
        prompt: &str,
        base64_data: &str,
        mime_type: &str,
        provider: &str,
    ) -> BitFunResult<Vec<Message>> {
        let message = match provider.to_lowercase().as_str() {
            "openai" => {
                // OpenAI format (Zhipu AI compatible)
                // Note:
                // 1. Zhipu AI only supports url field, does not support detail parameter
                // 2. Image must come first, text after (consistent with official examples)
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
                }
            }
            "anthropic" => {
                // Anthropic format (content is an array)
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
                }
            }
            _ => {
                return Err(BitFunError::validation(format!(
                    "Unsupported provider: {}",
                    provider
                )));
            }
        };

        Ok(vec![message])
    }

    /// Parse AI response into structured result
    fn parse_analysis_response(
        response: &str,
        image_id: &str,
    ) -> BitFunResult<ImageAnalysisResult> {
        // Extract JSON
        let json_str = Self::extract_json_from_markdown(response).unwrap_or(response);

        // Parse JSON
        let parsed: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
            BitFunError::parse(format!(
                "Failed to parse image analysis result: {}. Original response: {}",
                e, response
            ))
        })?;

        Ok(ImageAnalysisResult {
            image_id: image_id.to_string(),
            summary: parsed["summary"]
                .as_str()
                .unwrap_or("Image analysis completed")
                .to_string(),
            detailed_description: parsed["detailed_description"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            detected_elements: parsed["detected_elements"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
            confidence: parsed["confidence"].as_f64().unwrap_or(0.8) as f32,
            analysis_time_ms: 0, // Will be filled externally
        })
    }

    /// Extract JSON from Markdown code block
    fn extract_json_from_markdown(text: &str) -> Option<&str> {
        // 1. Try to extract Zhipu AI's special marker format <|begin_of_box|>...<|end_of_box|>
        if let Some(start_idx) = text.find("<|begin_of_box|>") {
            let content_start = start_idx + "<|begin_of_box|>".len();
            if let Some(end_idx) = text[content_start..].find("<|end_of_box|>") {
                let json_content = &text[content_start..content_start + end_idx].trim();
                debug!("Extracted Zhipu AI box format JSON");
                return Some(json_content);
            }
        }

        // 2. Try to extract Markdown code block format ```json ... ``` or ``` ... ```
        let start_markers = ["```json\n", "```\n"];

        for marker in &start_markers {
            if let Some(start_idx) = text.find(marker) {
                let content_start = start_idx + marker.len();
                if let Some(end_idx) = text[content_start..].find("```") {
                    return Some(&text[content_start..content_start + end_idx].trim());
                }
            }
        }

        None
    }
}

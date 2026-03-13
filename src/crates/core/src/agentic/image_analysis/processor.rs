//! Image Processor
//!
//! Handles image loading, preprocessing, multimodal message construction, and response parsing.

use super::image_processing::{
    build_multimodal_message, decode_data_url, detect_mime_type_from_bytes, load_image_from_path,
    optimize_image_with_size_limit, resolve_image_path,
};
use super::types::{AnalyzeImagesRequest, ImageAnalysisResult, ImageContextData};
use crate::infrastructure::ai::AIClient;
use crate::service::config::types::AIModelConfig;
use crate::util::errors::*;
use log::{debug, error, info, warn};
use std::path::PathBuf;
use std::sync::Arc;

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

    async fn analyze_single_image(
        image_ctx: ImageContextData,
        model: &AIModelConfig,
        user_context: Option<&str>,
        workspace_path: Option<PathBuf>,
        ai_client: Arc<AIClient>,
    ) -> BitFunResult<ImageAnalysisResult> {
        let start = std::time::Instant::now();

        debug!("Analyzing image: {}", image_ctx.id);

        let (image_data, fallback_mime) =
            Self::load_image_from_context(&image_ctx, workspace_path.as_deref()).await?;

        const IMAGE_ANALYSIS_MAX_BYTES: usize = 1024 * 1024;
        let processed = optimize_image_with_size_limit(
            image_data,
            &model.provider,
            fallback_mime.as_deref(),
            Some(IMAGE_ANALYSIS_MAX_BYTES),
        )?;

        debug!(
            "Image processing completed: mime={}, size={}KB, dimensions={}x{}",
            processed.mime_type,
            processed.data.len() / 1024,
            processed.width,
            processed.height
        );

        let analysis_prompt = Self::build_image_analysis_prompt(user_context);

        let messages = build_multimodal_message(
            &analysis_prompt,
            &processed.data,
            &processed.mime_type,
            &model.provider,
        )?;

        debug!(target: "ai::image_analysis_request",
            "Complete multimodal message:\n{}",
            serde_json::to_string_pretty(&messages)
                .unwrap_or_else(|_| "Serialization failed".to_string())
        );

        debug!(
            "Calling vision model: image_id={}, model={}",
            image_ctx.id, model.model_name
        );
        let ai_response = ai_client.send_message(messages, None).await.map_err(|e| {
            error!("AI call failed: {}", e);
            BitFunError::service(format!("Image analysis AI call failed: {}", e))
        })?;

        debug!("AI response content: {}", ai_response.text);

        let mut analysis_result = Self::parse_analysis_response(&ai_response.text, &image_ctx.id);
        analysis_result.analysis_time_ms = start.elapsed().as_millis() as u64;

        info!(
            "Image analysis completed: image_id={}, duration={}ms",
            image_ctx.id, analysis_result.analysis_time_ms
        );

        Ok(analysis_result)
    }

    async fn load_image_from_context(
        ctx: &ImageContextData,
        workspace_path: Option<&std::path::Path>,
    ) -> BitFunResult<(Vec<u8>, Option<String>)> {
        if let Some(data_url) = &ctx.data_url {
            let (data, mime) = decode_data_url(data_url)?;
            return Ok((data, mime.or_else(|| Some(ctx.mime_type.clone()))));
        }

        if let Some(path_str) = &ctx.image_path {
            let path = resolve_image_path(path_str, workspace_path)?;
            let data = load_image_from_path(&path, workspace_path).await?;
            let detected_mime = detect_mime_type_from_bytes(&data, Some(&ctx.mime_type)).ok();
            return Ok((data, detected_mime.or_else(|| Some(ctx.mime_type.clone()))));
        }

        Err(BitFunError::validation(
            "Image context missing path or data",
        ))
    }

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

    fn parse_analysis_response(response: &str, image_id: &str) -> ImageAnalysisResult {
        let extracted = crate::util::extract_json_from_ai_response(response);
        let json_str = extracted.as_deref().unwrap_or(response);

        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
            return ImageAnalysisResult {
                image_id: image_id.to_string(),
                summary: parsed["summary"]
                    .as_str()
                    .unwrap_or("Image analysis completed")
                    .to_string(),
                detailed_description: parsed["detailed_description"]
                    .as_str()
                    .unwrap_or(response)
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
                analysis_time_ms: 0,
            };
        }

        warn!(
            "Image analysis response is not valid JSON, falling back to plain text: image_id={}",
            image_id
        );

        let cleaned = response.trim();
        let summary = if cleaned.is_empty() {
            "Image analysis completed".to_string()
        } else {
            cleaned
                .lines()
                .next()
                .unwrap_or("Image analysis completed")
                .chars()
                .take(140)
                .collect()
        };

        ImageAnalysisResult {
            image_id: image_id.to_string(),
            summary,
            detailed_description: cleaned.to_string(),
            detected_elements: Vec::new(),
            confidence: 0.5,
            analysis_time_ms: 0,
        }
    }

}

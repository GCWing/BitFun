//! view_image tool - analyzes image content for text-only or multimodal main models.
//!
//! Current default behavior is to convert image content into structured text analysis.
//! This keeps the tool useful for text-only primary models while preserving an interface
//! that can evolve toward direct multimodal attachment in the future.

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::GenericImageView;
use log::{debug, info, trace};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agentic::image_analysis::{
    build_multimodal_message, decode_data_url, detect_mime_type_from_bytes, load_image_from_path,
    optimize_image_for_provider, resolve_image_path, resolve_vision_model_from_global_config,
    ImageContextData as ModelImageContextData,
};
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::infrastructure::ai::get_global_ai_client_factory;
use crate::infrastructure::get_workspace_path;
use crate::util::errors::{BitFunError, BitFunResult};

#[derive(Debug, Deserialize)]
struct ViewImageInput {
    #[serde(default)]
    image_path: Option<String>,
    #[serde(default)]
    data_url: Option<String>,
    #[serde(default)]
    image_id: Option<String>,
    #[serde(default)]
    analysis_prompt: Option<String>,
    #[serde(default)]
    focus_areas: Option<Vec<String>>,
    #[serde(default)]
    detail_level: Option<String>,
}

pub struct ViewImageTool;

impl ViewImageTool {
    pub fn new() -> Self {
        Self
    }

    fn primary_model_supports_images(context: &ToolUseContext) -> bool {
        context
            .options
            .as_ref()
            .and_then(|o| o.custom_data.as_ref())
            .and_then(|m| m.get("primary_model_supports_image_understanding"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    fn primary_model_provider(context: &ToolUseContext) -> Option<&str> {
        context
            .options
            .as_ref()
            .and_then(|o| o.custom_data.as_ref())
            .and_then(|m| m.get("primary_model_provider"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
    }

    fn build_prompt(
        &self,
        analysis_prompt: Option<&str>,
        focus_areas: &Option<Vec<String>>,
        detail_level: &Option<String>,
    ) -> String {
        let mut prompt = String::new();

        prompt.push_str(
            analysis_prompt
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("Please analyze this image and describe the relevant details."),
        );
        prompt.push_str("\n\n");

        if let Some(areas) = focus_areas {
            if !areas.is_empty() {
                prompt.push_str("Please pay special attention to the following aspects:\n");
                for area in areas {
                    prompt.push_str(&format!("- {}\n", area));
                }
                prompt.push('\n');
            }
        }

        let detail_guide = match detail_level.as_deref() {
            Some("brief") => "Please answer concisely in 1-2 sentences.",
            Some("detailed") => {
                "Please provide a detailed analysis including all relevant details."
            }
            _ => "Please provide a moderate level of analysis detail.",
        };
        prompt.push_str(detail_guide);

        prompt
    }

    async fn build_attachment_image_context(
        &self,
        input_data: &ViewImageInput,
        context: &ToolUseContext,
        primary_provider: &str,
    ) -> BitFunResult<(ModelImageContextData, String)> {
        let workspace_path = get_workspace_path();

        if let Some(image_id) = &input_data.image_id {
            let provider = context.image_context_provider.as_ref().ok_or_else(|| {
                BitFunError::tool(
                    "image_id mode requires ImageContextProvider support, but no provider was injected.\n\
                     Please inject image_context_provider when calling the tool, or use image_path/data_url mode."
                        .to_string(),
                )
            })?;

            let ctx = provider.get_image(image_id).ok_or_else(|| {
                BitFunError::tool(format!(
                    "Image context not found: image_id={}. Image may have expired (5-minute validity) or was never uploaded.",
                    image_id
                ))
            })?;

            let crate::agentic::tools::image_context::ImageContextData {
                id: ctx_id,
                image_path: ctx_image_path,
                data_url: ctx_data_url,
                mime_type: ctx_mime_type,
                image_name: ctx_image_name,
                file_size: ctx_file_size,
                width: ctx_width,
                height: ctx_height,
                source: ctx_source,
            } = ctx;

            let description = format!("{} (clipboard)", ctx_image_name);

            if let Some(path_str) = ctx_image_path.as_ref().filter(|s| !s.is_empty()) {
                let path = resolve_image_path(path_str, workspace_path.as_deref())?;
                let metadata = json!({
                    "name": ctx_image_name,
                    "width": ctx_width,
                    "height": ctx_height,
                    "file_size": ctx_file_size,
                    "source": ctx_source,
                    "origin": "image_id",
                    "image_id": ctx_id.clone(),
                });

                return Ok((
                    ModelImageContextData {
                        id: ctx_id,
                        image_path: Some(path.display().to_string()),
                        data_url: None,
                        mime_type: ctx_mime_type,
                        metadata: Some(metadata),
                    },
                    description,
                ));
            }

            if let Some(data_url) = ctx_data_url.as_ref().filter(|s| !s.is_empty()) {
                let (data, data_url_mime) = decode_data_url(data_url)?;
                let fallback_mime = data_url_mime
                    .as_deref()
                    .or_else(|| Some(ctx_mime_type.as_str()));
                let processed =
                    optimize_image_for_provider(data, primary_provider, fallback_mime)?;
                let optimized_data_url = format!(
                    "data:{};base64,{}",
                    processed.mime_type,
                    BASE64.encode(&processed.data)
                );

                let metadata = json!({
                    "name": ctx_image_name,
                    "width": processed.width,
                    "height": processed.height,
                    "file_size": processed.data.len(),
                    "source": ctx_source,
                    "origin": "image_id",
                    "image_id": ctx_id.clone(),
                });

                return Ok((
                    ModelImageContextData {
                        id: ctx_id,
                        image_path: None,
                        data_url: Some(optimized_data_url),
                        mime_type: processed.mime_type,
                        metadata: Some(metadata),
                    },
                    description,
                ));
            }

            return Err(BitFunError::tool(format!(
                "Image context {} has neither data_url nor image_path",
                image_id
            )));
        }

        if let Some(data_url) = &input_data.data_url {
            let (data, data_url_mime) = decode_data_url(data_url)?;
            let processed =
                optimize_image_for_provider(data, primary_provider, data_url_mime.as_deref())?;
            let optimized_data_url = format!(
                "data:{};base64,{}",
                processed.mime_type,
                BASE64.encode(&processed.data)
            );
            let metadata = json!({
                "name": "clipboard_image",
                "width": processed.width,
                "height": processed.height,
                "file_size": processed.data.len(),
                "source": "data_url",
                "origin": "data_url"
            });

            return Ok((
                ModelImageContextData {
                    id: format!("img-view-{}", Uuid::new_v4()),
                    image_path: None,
                    data_url: Some(optimized_data_url),
                    mime_type: processed.mime_type,
                    metadata: Some(metadata),
                },
                "clipboard_image".to_string(),
            ));
        }

        if let Some(image_path_str) = &input_data.image_path {
            let abs_path = resolve_image_path(image_path_str, workspace_path.as_deref())?;
            let data = load_image_from_path(&abs_path, workspace_path.as_deref()).await?;

            let mime_type = detect_mime_type_from_bytes(&data, None)?;
            let dynamic = image::load_from_memory(&data).map_err(|e| {
                BitFunError::validation(format!("Failed to decode image data: {}", e))
            })?;
            let (width, height) = dynamic.dimensions();

            let name = abs_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("image")
                .to_string();

            let metadata = json!({
                "name": name,
                "width": width,
                "height": height,
                "file_size": data.len(),
                "source": "local_path",
                "origin": "image_path"
            });

            return Ok((
                ModelImageContextData {
                    id: format!("img-view-{}", Uuid::new_v4()),
                    image_path: Some(abs_path.display().to_string()),
                    data_url: None,
                    mime_type,
                    metadata: Some(metadata),
                },
                abs_path.display().to_string(),
            ));
        }

        Err(BitFunError::validation(
            "Must provide one of image_path, data_url, or image_id",
        ))
    }

    async fn load_source(
        &self,
        input_data: &ViewImageInput,
        context: &ToolUseContext,
    ) -> BitFunResult<(Vec<u8>, Option<String>, String)> {
        let workspace_path = get_workspace_path();

        if let Some(image_id) = &input_data.image_id {
            let provider = context.image_context_provider.as_ref().ok_or_else(|| {
                BitFunError::tool(
                    "image_id mode requires ImageContextProvider support, but no provider was injected.\n\
                     Please inject image_context_provider when calling the tool, or use image_path/data_url mode.".to_string()
                )
            })?;

            let image_context = provider.get_image(image_id).ok_or_else(|| {
                BitFunError::tool(format!(
                    "Image context not found: image_id={}. Image may have expired (5-minute validity) or was never uploaded.",
                    image_id
                ))
            })?;

            if let Some(data_url) = &image_context.data_url {
                let (data, data_url_mime) = decode_data_url(data_url)?;
                let fallback_mime = data_url_mime.or_else(|| Some(image_context.mime_type.clone()));
                return Ok((
                    data,
                    fallback_mime,
                    format!("{} (clipboard)", image_context.image_name),
                ));
            }

            if let Some(image_path_str) = &image_context.image_path {
                let image_path = resolve_image_path(image_path_str, workspace_path.as_deref())?;
                let data = load_image_from_path(&image_path, workspace_path.as_deref()).await?;
                let detected_mime =
                    detect_mime_type_from_bytes(&data, Some(&image_context.mime_type)).ok();
                return Ok((data, detected_mime, image_path.display().to_string()));
            }

            return Err(BitFunError::tool(format!(
                "Image context {} has neither data_url nor image_path",
                image_id
            )));
        }

        if let Some(data_url) = &input_data.data_url {
            let (data, data_url_mime) = decode_data_url(data_url)?;
            return Ok((data, data_url_mime, "clipboard_image".to_string()));
        }

        if let Some(image_path_str) = &input_data.image_path {
            let image_path = resolve_image_path(image_path_str, workspace_path.as_deref())?;
            let data = load_image_from_path(&image_path, workspace_path.as_deref()).await?;
            let detected_mime = detect_mime_type_from_bytes(&data, None).ok();
            return Ok((data, detected_mime, image_path.display().to_string()));
        }

        Err(BitFunError::validation(
            "Must provide one of image_path, data_url, or image_id",
        ))
    }
}

#[async_trait]
impl Tool for ViewImageTool {
    fn name(&self) -> &str {
        "view_image"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Analyzes image content and returns detailed text descriptions.

Use this tool when the user provides an image (file path, data URL, or uploaded clipboard image_id) and asks questions about it.

Current behavior:
- For text-only primary models, this tool converts image content to structured text (uses the configured image understanding model).
- For multimodal primary models, this tool attaches the image for the primary model to analyze directly.

Parameters:
- image_path / data_url / image_id: provide one image source
- analysis_prompt: optional custom analysis goal
- focus_areas: optional analysis focus list
- detail_level: brief / normal / detailed"#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "image_path": {
                    "type": "string",
                    "description": "Path to image file (relative to workspace or absolute path). Example: 'screenshot.png'"
                },
                "data_url": {
                    "type": "string",
                    "description": "Base64-encoded image data URL. Example: 'data:image/png;base64,...'"
                },
                "image_id": {
                    "type": "string",
                    "description": "Temporary image ID from clipboard upload. Example: 'img-clipboard-1234567890-abc123'"
                },
                "analysis_prompt": {
                    "type": "string",
                    "description": "Optional custom prompt describing what to extract from the image"
                },
                "focus_areas": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional list of aspects to emphasize"
                },
                "detail_level": {
                    "type": "string",
                    "enum": ["brief", "normal", "detailed"],
                    "description": "Optional detail level"
                }
            }
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let has_path = input
            .get("image_path")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty());
        let has_data_url = input
            .get("data_url")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty());
        let has_image_id = input
            .get("image_id")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty());

        if !has_path && !has_data_url && !has_image_id {
            return ValidationResult {
                result: false,
                message: Some("Must provide one of image_path, data_url, or image_id".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if let Some(image_path) = input.get("image_path").and_then(|v| v.as_str()) {
            if !image_path.is_empty() {
                let workspace_path = get_workspace_path();
                match resolve_image_path(image_path, workspace_path.as_deref()) {
                    Ok(path) => {
                        if !path.exists() {
                            return ValidationResult {
                                result: false,
                                message: Some(format!("Image file does not exist: {}", image_path)),
                                error_code: Some(404),
                                meta: None,
                            };
                        }

                        if !path.is_file() {
                            return ValidationResult {
                                result: false,
                                message: Some(format!("Path is not a file: {}", image_path)),
                                error_code: Some(400),
                                meta: None,
                            };
                        }
                    }
                    Err(e) => {
                        return ValidationResult {
                            result: false,
                            message: Some(format!("Path parsing failed: {}", e)),
                            error_code: Some(400),
                            meta: None,
                        };
                    }
                }
            }
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        let image_source = if let Some(path) = input.get("image_path").and_then(|v| v.as_str()) {
            if !path.is_empty() {
                path.to_string()
            } else {
                "Clipboard image".to_string()
            }
        } else if input
            .get("image_id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| !id.is_empty())
        {
            "Clipboard image (image_id)".to_string()
        } else if input.get("data_url").is_some() {
            "Clipboard image".to_string()
        } else {
            "unknown".to_string()
        };

        if options.verbose {
            let prompt = input
                .get("analysis_prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("default analysis");
            format!("Viewing image: {} (prompt: {})", image_source, prompt)
        } else {
            format!("Viewing image: {}", image_source)
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let start = std::time::Instant::now();

        let input_data: ViewImageInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::parse(format!("Failed to parse input: {}", e)))?;

        let primary_provider = Self::primary_model_provider(context).unwrap_or("openai");
        if Self::primary_model_supports_images(context) {
            let (image, image_source_description) = self
                .build_attachment_image_context(&input_data, context, primary_provider)
                .await?;

            let result_for_assistant = format!(
                "Image attached for primary model analysis ({})",
                image_source_description
            );

            return Ok(vec![ToolResult::Result {
                data: json!({
                    "success": true,
                    "mode": "attached_to_primary_model",
                    "image_source": image_source_description,
                    "image": image,
                }),
                result_for_assistant: Some(result_for_assistant),
            }]);
        }

        let (image_data, fallback_mime, image_source_description) =
            self.load_source(&input_data, context).await?;

        let vision_model = resolve_vision_model_from_global_config().await?;
        debug!(
            "Using image understanding model: id={}, name={}, provider={}",
            vision_model.id, vision_model.name, vision_model.provider
        );

        let processed = optimize_image_for_provider(
            image_data,
            &vision_model.provider,
            fallback_mime.as_deref(),
        )?;

        let prompt = self.build_prompt(
            input_data.analysis_prompt.as_deref(),
            &input_data.focus_areas,
            &input_data.detail_level,
        );
        trace!("Full view_image prompt: {}", prompt);

        let messages = build_multimodal_message(
            &prompt,
            &processed.data,
            &processed.mime_type,
            &vision_model.provider,
        )?;

        let ai_client_factory = get_global_ai_client_factory()
            .await
            .map_err(|e| BitFunError::service(format!("Failed to get AI client factory: {}", e)))?;
        let ai_client = ai_client_factory
            .get_client_by_id(&vision_model.id)
            .await
            .map_err(|e| {
                BitFunError::service(format!(
                    "Failed to create vision model client for {}: {}",
                    vision_model.id, e
                ))
            })?;

        debug!("Calling vision model for image analysis...");
        let ai_response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::service(format!("AI call failed: {}", e)))?;

        let elapsed = start.elapsed();
        info!("view_image completed: duration={:?}", elapsed);

        let result_for_assistant = format!(
            "Image analysis result ({})\n\n{}",
            image_source_description, ai_response.text
        );

        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "image_source": image_source_description,
                "analysis": ai_response.text,
                "metadata": {
                    "mime_type": processed.mime_type,
                    "file_size": processed.data.len(),
                    "width": processed.width,
                    "height": processed.height,
                    "analysis_time_ms": elapsed.as_millis() as u64,
                    "model_used": vision_model.name,
                    "prompt_used": input_data.analysis_prompt.unwrap_or_else(|| "default".to_string()),
                }
            }),
            result_for_assistant: Some(result_for_assistant),
        }])
    }
}

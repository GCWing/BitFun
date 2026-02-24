//! Image analysis tool - allows Agent to analyze image content on demand
//!
//! Provides flexible image analysis capabilities, Agent can customize analysis prompts and focus areas

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use log::{debug, info, trace};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::infrastructure::ai::AIClient;
use crate::infrastructure::{get_path_manager_arc, get_workspace_path};
use crate::service::config::types::{AIConfig as ServiceAIConfig, AIModelConfig, GlobalConfig};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::{AIConfig as ModelConfig, Message};

/// Image analysis tool input
#[derive(Debug, Deserialize)]
struct AnalyzeImageInput {
    /// Image path (relative to workspace or absolute path)
    #[serde(default)]
    image_path: Option<String>,
    /// Base64-encoded image data (clipboard image)
    #[serde(default)]
    data_url: Option<String>,
    /// Image ID (retrieved from temporary storage, for clipboard images)
    #[serde(default)]
    image_id: Option<String>,
    /// Analysis prompt
    analysis_prompt: String,
    /// Focus areas (optional)
    #[serde(default)]
    focus_areas: Option<Vec<String>>,
    /// Detail level (optional)
    #[serde(default)]
    detail_level: Option<String>,
}

/// Image analysis tool
pub struct AnalyzeImageTool;

impl AnalyzeImageTool {
    pub fn new() -> Self {
        Self
    }

    /// Resolve image path (supports relative and absolute paths)
    fn resolve_image_path(&self, path: &str) -> BitFunResult<PathBuf> {
        let path_buf = PathBuf::from(path);

        if path_buf.is_absolute() {
            Ok(path_buf)
        } else {
            let workspace_path = get_workspace_path()
                .ok_or_else(|| BitFunError::tool("Workspace path not set".to_string()))?;
            Ok(workspace_path.join(path))
        }
    }

    /// Load image file
    async fn load_image(&self, path: &Path) -> BitFunResult<Vec<u8>> {
        // Security check: ensure path is within workspace
        if let Some(workspace_path) = get_workspace_path() {
            let canonical_path = tokio::fs::canonicalize(path)
                .await
                .map_err(|e| BitFunError::io(format!("Image file does not exist: {}", e)))?;
            let canonical_workspace = tokio::fs::canonicalize(&workspace_path)
                .await
                .map_err(|e| BitFunError::io(format!("Invalid workspace path: {}", e)))?;

            if !canonical_path.starts_with(&canonical_workspace) {
                return Err(BitFunError::validation(
                    "Image path must be within workspace",
                ));
            }
        }

        fs::read(path)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read image: {}", e)))
    }

    /// Detect image MIME type
    fn detect_mime_type(&self, path: &Path) -> BitFunResult<String> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| BitFunError::validation("Unable to determine image format"))?
            .to_lowercase();

        let mime_type = match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "bmp" => "image/bmp",
            _ => {
                return Err(BitFunError::validation(format!(
                    "Unsupported image format: {}",
                    extension
                )))
            }
        };

        Ok(mime_type.to_string())
    }

    /// Get image dimensions (simple implementation)
    fn get_image_dimensions(&self, _data: &[u8]) -> (u32, u32) {
        // TODO: Implement real image dimension detection
        (0, 0)
    }

    /// Decode data URL
    fn decode_data_url(&self, data_url: &str) -> BitFunResult<(Vec<u8>, String)> {
        // data:image/png;base64,iVBORw0KG...
        if !data_url.starts_with("data:") {
            return Err(BitFunError::validation("Invalid data URL format"));
        }

        let parts: Vec<&str> = data_url.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(BitFunError::validation("Data URL format error"));
        }

        // Extract MIME type
        let header = parts[0];
        let mime_type = header
            .strip_prefix("data:")
            .and_then(|s| s.split(';').next())
            .unwrap_or("image/png")
            .to_string();

        // Decode base64
        let base64_data = parts[1];
        let image_data = BASE64
            .decode(base64_data)
            .map_err(|e| BitFunError::parse(format!("Base64 decode failed: {}", e)))?;

        debug!(
            "Decoded image from data URL: mime={}, size_kb={}",
            mime_type,
            image_data.len() / 1024
        );

        Ok((image_data, mime_type))
    }

    /// Load AI configuration from config file
    async fn load_ai_config(&self) -> BitFunResult<ServiceAIConfig> {
        let path_manager = get_path_manager_arc();
        let config_file = path_manager.app_config_file();

        if !config_file.exists() {
            return Err(BitFunError::tool("Config file does not exist".to_string()));
        }

        let config_content = tokio::fs::read_to_string(&config_file)
            .await
            .map_err(|e| BitFunError::tool(format!("Failed to read config file: {}", e)))?;

        let global_config: GlobalConfig = serde_json::from_str(&config_content)
            .map_err(|e| BitFunError::tool(format!("Failed to parse config file: {}", e)))?;

        Ok(global_config.ai)
    }

    /// Get vision model configuration
    async fn get_vision_model(&self) -> BitFunResult<AIModelConfig> {
        let ai_config = self.load_ai_config().await?;

        let target_model_id = ai_config
            .default_models
            .image_understanding
            .as_ref()
            .filter(|id| !id.is_empty());

        let model = if let Some(id) = target_model_id {
            ai_config
                .models
                .iter()
                .find(|m| m.id == *id)
                .ok_or_else(|| BitFunError::service(format!("Model not found: {}", id)))?
                .clone()
        } else {
            ai_config
                .models
                .iter()
                .find(|m| {
                    m.enabled
                        && m.capabilities.iter().any(|cap| {
                            matches!(
                                cap,
                                crate::service::config::types::ModelCapability::ImageUnderstanding
                            )
                        })
                })
                .ok_or_else(|| {
                    BitFunError::service(
                        "No image understanding model found.\n\
                     Please configure an image understanding model in settings"
                            .to_string(),
                    )
                })?
                .clone()
        };

        Ok(model)
    }

    /// Build analysis prompt
    fn build_prompt(
        &self,
        analysis_prompt: &str,
        focus_areas: &Option<Vec<String>>,
        detail_level: &Option<String>,
    ) -> String {
        let mut prompt = String::new();

        // 1. User's analysis prompt
        prompt.push_str(analysis_prompt);
        prompt.push_str("\n\n");

        if let Some(areas) = focus_areas {
            if !areas.is_empty() {
                prompt.push_str("Please pay special attention to the following aspects:\n");
                for area in areas {
                    prompt.push_str(&format!("- {}\n", area));
                }
                prompt.push_str("\n");
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

    /// Build multimodal message
    fn build_multimodal_message(
        &self,
        prompt: &str,
        base64_data: &str,
        mime_type: &str,
        provider: &str,
    ) -> BitFunResult<Vec<Message>> {
        let message = match provider.to_lowercase().as_str() {
            "openai" => Message {
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
            },
            "anthropic" => Message {
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
            },
            _ => {
                return Err(BitFunError::validation(format!(
                    "Unsupported provider: {}",
                    provider
                )));
            }
        };

        Ok(vec![message])
    }
}

#[async_trait]
impl Tool for AnalyzeImageTool {
    fn name(&self) -> &str {
        "AnalyzeImage"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Analyzes image content and returns detailed descriptions. Use this tool when the user uploads images and asks related questions.

Core Capabilities:
- Identify objects, text, structures and other content in images
- Understand technical diagrams (architecture diagrams, flowcharts, UML diagrams, etc.)
- Extract code and error messages from code screenshots
- Analyze UI designs and interface layouts
- Recognize data, tables, and charts in images

Usage Scenarios:
1. User uploads architecture diagram and asks architecture questions → Analyze components and relationships
2. User uploads error screenshot → Extract error messages and stack traces
3. User uploads code screenshot → Identify code content
4. User uploads UI design → Analyze design elements and layout
5. User uploads data charts → Interpret data and trends

Important Notes:
- You can customize analysis_prompt to precisely control the analysis angle and focus
- Use focus_areas parameter to specify aspects to emphasize
- Choose detail_level as needed (brief/normal/detailed)
- The same image can be analyzed multiple times for different aspects"#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "image_path": {
                    "type": "string",
                    "description": "Path to the image file (relative to workspace or absolute path).\nExamples: 'screenshot.png' or 'docs/architecture.png'\nNote: Provide ONE of: image_path, data_url, or (image_id + session_id)."
                },
                "data_url": {
                    "type": "string",
                    "description": "Base64-encoded image data.\nFormat: 'data:image/png;base64,iVBORw0KG...'\nNot recommended for large images due to token cost."
                },
                "image_id": {
                    "type": "string",
                    "description": "Image ID for clipboard images stored in temporary cache.\nExample: 'img-clipboard-1234567890-abc123'"
                },
                "analysis_prompt": {
                    "type": "string",
                    "description": "Analysis prompt describing what information you want to extract from the image.\n\
                                   Examples:\n\
                                   - 'What is this architecture diagram? What components and connections does it contain?'\n\
                                   - 'Extract all error messages and stack traces from this screenshot'\n\
                                   - 'Describe the layout structure and interactive elements of this UI'"
                },
                "focus_areas": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional. List of aspects to focus on.\nExamples: ['technical architecture', 'data flow'] or ['UI layout', 'color scheme']"
                },
                "detail_level": {
                    "type": "string",
                    "enum": ["brief", "normal", "detailed"],
                    "description": "Optional. Level of analysis detail.\n- brief: Brief summary (1-2 sentences)\n- normal: Normal detail (default)\n- detailed: Detailed analysis (includes all relevant details)"
                }
            },
            "required": ["analysis_prompt"]
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
        // Check if image_path, data_url, or (image_id + session_id) is provided
        let has_path = input
            .get("image_path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .is_some();
        let has_data_url = input
            .get("data_url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .is_some();
        let has_image_id = input
            .get("image_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .is_some();

        if !has_path && !has_data_url && !has_image_id {
            return ValidationResult {
                result: false,
                message: Some("Must provide one of image_path, data_url, or image_id".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if let Some(prompt) = input.get("analysis_prompt").and_then(|v| v.as_str()) {
            if prompt.is_empty() {
                return ValidationResult {
                    result: false,
                    message: Some("analysis_prompt cannot be empty".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        } else {
            return ValidationResult {
                result: false,
                message: Some("analysis_prompt is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if let Some(image_path) = input.get("image_path").and_then(|v| v.as_str()) {
            if !image_path.is_empty() {
                match self.resolve_image_path(image_path) {
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

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        // Determine image source
        let image_source = if let Some(path) = input.get("image_path").and_then(|v| v.as_str()) {
            if !path.is_empty() {
                path.to_string()
            } else {
                "Clipboard image".to_string()
            }
        } else if input.get("data_url").is_some() {
            "Clipboard image".to_string()
        } else {
            "unknown".to_string()
        };

        if options.verbose {
            let prompt = input
                .get("analysis_prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("...");
            format!(
                "Analyzing image: {} (prompt: {})",
                image_source,
                if prompt.len() > 50 {
                    // Safe truncation: find the maximum character boundary not exceeding 50 bytes
                    let pos = prompt
                        .char_indices()
                        .take_while(|(i, _)| *i < 50)
                        .last()
                        .map(|(i, c)| i + c.len_utf8())
                        .unwrap_or(0);
                    format!("{}...", &prompt[..pos])
                } else {
                    prompt.to_string()
                }
            )
        } else {
            format!("Analyzing image: {}", image_source)
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let start = std::time::Instant::now();

        // Parse input
        let input_data: AnalyzeImageInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::parse(format!("Failed to parse input: {}", e)))?;

        let has_data_url = input_data.data_url.is_some();
        let has_path = input_data.image_path.is_some();
        let has_image_id = input_data.image_id.is_some();

        if !has_data_url && !has_path && !has_image_id {
            return Err(BitFunError::validation(
                "Must provide one of image_path, data_url, or image_id",
            ));
        }

        debug!(
            "Starting image analysis: source={}",
            if has_image_id {
                "temporary_storage(image_id)"
            } else if has_data_url {
                "direct_input(data_url)"
            } else {
                "file_path(image_path)"
            }
        );
        debug!("Analysis prompt: {}", input_data.analysis_prompt);

        let (image_data, mime_type, image_source_description) = if let Some(image_id) =
            &input_data.image_id
        {
            let provider = _context.image_context_provider.as_ref()
                .ok_or_else(|| BitFunError::tool(
                    "image_id mode requires ImageContextProvider support, but no provider was injected.\n\
                     Please inject image_context_provider when calling the tool, or use image_path/data_url mode.".to_string()
                ))?;

            let image_context = provider.get_image(image_id)
                .ok_or_else(|| BitFunError::tool(format!(
                    "Image context not found: image_id={}. Image may have expired (5-minute validity) or was never uploaded.",
                    image_id
                )))?;

            debug!(
                "Retrieved image from context provider: name={}, source={}",
                image_context.image_name, image_context.mime_type
            );

            if let Some(data_url) = &image_context.data_url {
                let (data, mime) = self.decode_data_url(data_url)?;
                (
                    data,
                    mime,
                    format!("{} (clipboard)", image_context.image_name),
                )
            } else if let Some(image_path_str) = &image_context.image_path {
                let image_path = self.resolve_image_path(image_path_str)?;
                let data = self.load_image(&image_path).await?;
                let mime = self.detect_mime_type(&image_path)?;
                (data, mime, image_path.display().to_string())
            } else {
                return Err(BitFunError::tool(format!(
                    "Image context {} has neither data_url nor image_path",
                    image_id
                )));
            }
        } else if let Some(data_url) = &input_data.data_url {
            // Decode from data URL
            let (data, mime) = self.decode_data_url(data_url)?;
            (data, mime, "clipboard_image".to_string())
        } else if let Some(image_path_str) = &input_data.image_path {
            // Load from file path
            let image_path = self.resolve_image_path(image_path_str)?;
            debug!("Parsed image path: {}", image_path.display());

            let data = self.load_image(&image_path).await?;
            let mime = self.detect_mime_type(&image_path)?;

            debug!("Image size: {} KB, mime: {}", data.len() / 1024, mime);

            (data, mime, image_path.display().to_string())
        } else {
            unreachable!("Input already checked above")
        };

        let base64_data = BASE64.encode(&image_data);

        let vision_model = self.get_vision_model().await?;
        debug!(
            "Using vision model: name={}, model={}",
            vision_model.name, vision_model.model_name
        );

        let prompt = self.build_prompt(
            &input_data.analysis_prompt,
            &input_data.focus_areas,
            &input_data.detail_level,
        );
        trace!("Full analysis prompt: {}", prompt);

        let messages = self.build_multimodal_message(
            &prompt,
            &base64_data,
            &mime_type,
            &vision_model.provider,
        )?;

        // Vision models cannot set max_tokens (e.g., glm-4v doesn't support this parameter)
        // and should never use the thinking process.
        let mut model_config = ModelConfig::try_from(vision_model.clone())
            .map_err(|e| BitFunError::parse(format!("Config conversion failed for vision model {}: {}", vision_model.name, e)))?;
        model_config.max_tokens = None;
        model_config.enable_thinking_process = false;
        model_config.support_preserved_thinking = false;

        let ai_client = Arc::new(AIClient::new(model_config));

        debug!("Calling vision model for analysis...");
        let ai_response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::service(format!("AI call failed: {}", e)))?;

        let elapsed = start.elapsed();
        info!("Image analysis completed: duration={:?}", elapsed);

        let (width, height) = self.get_image_dimensions(&image_data);

        let result_for_assistant = format!(
            "Image analysis result ({})\n\n{}",
            image_source_description, ai_response.text
        );

        let result = ToolResult::Result {
            data: json!({
                "success": true,
                "image_source": image_source_description,
                "analysis": ai_response.text,
                "metadata": {
                    "mime_type": mime_type,
                    "file_size": image_data.len(),
                    "width": width,
                    "height": height,
                    "analysis_time_ms": elapsed.as_millis() as u64,
                    "model_used": vision_model.name,
                    "prompt_used": input_data.analysis_prompt,
                }
            }),
            result_for_assistant: Some(result_for_assistant),
        };

        Ok(vec![result])
    }
}

use crate::agentic::image_analysis::{
    build_multimodal_message, detect_mime_type_from_bytes, optimize_image_with_size_limit,
    resolve_vision_model_from_global_config,
};
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::infrastructure::ai::get_global_ai_client_factory;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct AnalyzeImageTool;

#[derive(Debug, Clone)]
enum ResolvedImagePath {
    Local(PathBuf),
    RemoteWorkspace { logical_path: String, path: String },
}

impl ResolvedImagePath {
    fn display_path(&self) -> &str {
        match self {
            Self::Local(path) => path.to_str().unwrap_or("<non-utf8-path>"),
            Self::RemoteWorkspace { logical_path, .. } => logical_path,
        }
    }
}

impl Default for AnalyzeImageTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyzeImageTool {
    pub fn new() -> Self {
        Self
    }

    fn path_from_input(input: &Value) -> BitFunResult<&str> {
        input
            .get("path")
            .and_then(Value::as_str)
            .filter(|path| !path.trim().is_empty())
            .ok_or_else(|| BitFunError::tool("path is required".to_string()))
    }

    fn prompt_from_input(input: &Value) -> String {
        input
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|prompt| !prompt.is_empty())
            .unwrap_or("Analyze this image in detail. Describe visible content, text, layout, and any details relevant to the user's task.")
            .to_string()
    }

    fn resolve_path(
        input_path: &str,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<ResolvedImagePath> {
        let local_path = Path::new(input_path);
        if local_path.is_absolute()
            && !crate::agentic::tools::workspace_paths::is_bitfun_runtime_uri(input_path)
            && (!context.is_some_and(|ctx| ctx.is_remote()) || local_path.is_file())
        {
            return Ok(ResolvedImagePath::Local(local_path.to_path_buf()));
        }

        match context.map(|ctx| ctx.resolve_tool_path(input_path)) {
            Some(Ok(resolved)) => {
                if resolved.uses_remote_workspace_backend() {
                    return Ok(ResolvedImagePath::RemoteWorkspace {
                        logical_path: resolved.logical_path,
                        path: resolved.resolved_path,
                    });
                }
                Ok(ResolvedImagePath::Local(PathBuf::from(
                    resolved.resolved_path,
                )))
            }
            Some(Err(err)) => Err(err),
            None => {
                let path = Path::new(input_path);
                if !path.is_absolute() {
                    return Err(BitFunError::tool(format!(
                        "path must be an absolute path when no tool context is available, got: {}",
                        input_path
                    )));
                }
                Ok(ResolvedImagePath::Local(path.to_path_buf()))
            }
        }
    }

    async fn read_image_bytes(
        resolved: &ResolvedImagePath,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<Vec<u8>> {
        match resolved {
            ResolvedImagePath::Local(path) => {
                let metadata = fs::metadata(path).await.map_err(|err| {
                    BitFunError::tool(format!(
                        "unable to locate image at {}: {}",
                        path.display(),
                        err
                    ))
                })?;
                if !metadata.is_file() {
                    return Err(BitFunError::tool(format!(
                        "image path is not a file: {}",
                        path.display()
                    )));
                }

                fs::read(path).await.map_err(|err| {
                    BitFunError::tool(format!(
                        "unable to read image at {}: {}",
                        path.display(),
                        err
                    ))
                })
            }
            ResolvedImagePath::RemoteWorkspace { path, logical_path } => {
                let fs = context.and_then(|ctx| ctx.ws_fs()).ok_or_else(|| {
                    BitFunError::tool(
                        "analyze_image cannot read remote workspace images because workspace filesystem services are unavailable"
                            .to_string(),
                    )
                })?;
                let is_file = fs.is_file(path).await.map_err(|err| {
                    BitFunError::tool(format!(
                        "unable to inspect remote image at {}: {}",
                        logical_path, err
                    ))
                })?;
                if !is_file {
                    return Err(BitFunError::tool(format!(
                        "image path is not a file: {}",
                        logical_path
                    )));
                }

                fs.read_file(path).await.map_err(|err| {
                    BitFunError::tool(format!(
                        "unable to read remote image at {}: {}",
                        logical_path, err
                    ))
                })
            }
        }
    }
}

#[async_trait]
impl Tool for AnalyzeImageTool {
    fn name(&self) -> &str {
        "analyze_image"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            "Analyze an image file with the configured image understanding model and return a text result. Use this when the primary model cannot inspect images directly, or when an image was pasted into the chat and a path is available in the user context."
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Analyze an image file using the configured vision model.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Expanded
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the image file. Use an absolute local path, a workspace-relative path, or an exact bitfun://runtime URI."
                },
                "prompt": {
                    "type": "string",
                    "description": "Optional question or instruction for the image understanding model."
                }
            },
            "required": ["path"],
            "additionalProperties": false
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
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let input_path = match Self::path_from_input(input) {
            Ok(path) => path,
            Err(err) => {
                return ValidationResult {
                    result: false,
                    message: Some(err.to_string()),
                    error_code: Some(400),
                    meta: None,
                }
            }
        };

        let resolved = match Self::resolve_path(input_path, context) {
            Ok(path) => path,
            Err(err) => {
                return ValidationResult {
                    result: false,
                    message: Some(err.to_string()),
                    error_code: Some(400),
                    meta: None,
                }
            }
        };

        match &resolved {
            ResolvedImagePath::Local(local_path) => match std::fs::metadata(local_path) {
                Ok(metadata) if metadata.is_file() => ValidationResult::default(),
                Ok(_) => ValidationResult {
                    result: false,
                    message: Some(format!(
                        "image path is not a file: {}",
                        resolved.display_path()
                    )),
                    error_code: Some(400),
                    meta: None,
                },
                Err(err) => ValidationResult {
                    result: false,
                    message: Some(format!(
                        "unable to locate image at {}: {}",
                        resolved.display_path(),
                        err
                    )),
                    error_code: Some(404),
                    meta: None,
                },
            },
            ResolvedImagePath::RemoteWorkspace { path, .. } => {
                let Some(fs) = context.and_then(|ctx| ctx.ws_fs()) else {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "Workspace filesystem services are required to validate remote image paths"
                                .to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                };
                match fs.is_file(path).await {
                    Ok(true) => ValidationResult::default(),
                    Ok(false) => ValidationResult {
                        result: false,
                        message: Some(format!(
                            "image path is not a file: {}",
                            resolved.display_path()
                        )),
                        error_code: Some(400),
                        meta: None,
                    },
                    Err(err) => ValidationResult {
                        result: false,
                        message: Some(format!(
                            "unable to locate image at {}: {}",
                            resolved.display_path(),
                            err
                        )),
                        error_code: Some(404),
                        meta: None,
                    },
                }
            }
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let path = input.get("path").and_then(Value::as_str).unwrap_or("");
        if path.is_empty() {
            "Analyzing image".to_string()
        } else {
            format!("Analyzing image: {}", path)
        }
    }

    fn render_tool_result_message(&self, output: &Value) -> String {
        output
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("Image analysis completed")
            .to_string()
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        output
            .get("analysis")
            .and_then(Value::as_str)
            .unwrap_or("Image analysis completed")
            .to_string()
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let input_path = Self::path_from_input(input)?;
        let prompt = Self::prompt_from_input(input);
        let resolved = Self::resolve_path(input_path, Some(context))?;
        let bytes = Self::read_image_bytes(&resolved, Some(context)).await?;
        let original_mime_type = detect_mime_type_from_bytes(&bytes, None)
            .map_err(|err| BitFunError::tool(format!("unsupported image file: {}", err)))?;

        let vision_model = resolve_vision_model_from_global_config().await?;
        let processed = optimize_image_with_size_limit(
            bytes,
            &vision_model.provider,
            Some(&original_mime_type),
            Some(1024 * 1024),
        )
        .map_err(|err| BitFunError::tool(format!("unable to prepare image: {}", err)))?;

        let messages = build_multimodal_message(
            &prompt,
            &processed.data,
            &processed.mime_type,
            &vision_model.provider,
        )?;
        let client = get_global_ai_client_factory()
            .await
            .map_err(|err| BitFunError::service(format!("AI client factory unavailable: {err}")))?
            .get_client_by_id(&vision_model.id)
            .await
            .map_err(|err| {
                BitFunError::service(format!(
                    "Failed to create image understanding model client: {err}"
                ))
            })?;
        let response = client
            .send_message(messages, None)
            .await
            .map_err(|err| BitFunError::service(format!("Image analysis failed: {err}")))?;
        let analysis = response.text.trim().to_string();
        let summary = analysis
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(str::trim)
            .unwrap_or("Image analysis completed")
            .chars()
            .take(180)
            .collect::<String>();

        let data = json!({
            "path": resolved.display_path(),
            "model_id": vision_model.id,
            "model_name": vision_model.model_name,
            "mime_type": processed.mime_type,
            "width": processed.width,
            "height": processed.height,
            "summary": summary,
            "analysis": analysis,
        });

        Ok(vec![ToolResult::ok(
            data,
            Some(format!(
                "Image analysis for {}:\n{}",
                resolved.display_path(),
                analysis
            )),
        )])
    }
}

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use napi_ohos::threadsafe_function::ThreadsafeFunctionCallMode;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use napi_derive_ohos::napi;
use parking_lot::{Condvar, Mutex};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use crate::util::JS_THREADSAFE_FUNCTION;
struct BuildState {
    result: Option<String>,
    notified: bool,
}

static BUILD_STATE: once_cell::sync::Lazy<Arc<(Mutex<BuildState>, Condvar)>> =
    once_cell::sync::Lazy::new(|| {
        Arc::new((
            Mutex::new(BuildState {
                result: None,
                notified: false,
            }),
            Condvar::new(),
        ))
    });

pub struct HarmonyBuildTool {}

impl HarmonyBuildTool {
    pub fn new() -> Self {
        Self {}
    }

    fn validate_project_path(&self, project_path: &str) -> bool {
        let path = Path::new(project_path);
        path.exists() && path.is_dir()
    }

    async fn execute_build(&self, project_path: &str) -> BitFunResult<String> {
        log::info!("HarmonyOS build for project: {}", project_path);

        {
            let (lock, cvar) = &**BUILD_STATE;
            let mut state = lock.lock();
            state.result = None;
            state.notified = false;
            cvar.notify_all();
        }

        match call_harmony_build(project_path.to_string()) {
            Ok(_) => {
                log::info!("call_harmony_build success");
                let timeout = Duration::from_secs(60);
                let (lock, cvar) = &**BUILD_STATE;
                let mut state = lock.lock();

                let wait_result = cvar.wait_for(&mut state, timeout);
                if !wait_result.timed_out() && state.notified {
                    if let Some(msg) = &state.result {
                        log::info!("Build result received: {}", msg);
                        return Ok(msg.clone());
                    }
                }

                log::error!("Build timeout");
                Err(BitFunError::tool(
                    "Build timeout: no result received within 1 minute".to_string(),
                ))
            }
            Err(_) => {
                log::error!("call_harmony_build failed");
                Err(BitFunError::tool(
                    "Build failed: call_harmony_build failed".to_string(),
                ))
            }
        }
    }
}

#[async_trait]
impl Tool for HarmonyBuildTool {
    fn name(&self) -> &str {
        "HarmonyBuild"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"HarmonyOS application build tool. Builds a HarmonyOS project.

        Usage:
        - The project_path parameter must be an absolute path to a HarmonyOS project

        Example:
        - Build project: {"project_path": "path/to/harmony/project"}"#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Build the harmony project in ohos".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "project_path": {
                    "type": "string",
                    "description": "The absolute path to the HarmonyOS project"
                }
            },
            "required": [ "project_path" ],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let project_path = match input.get("project_path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return ValidationResult {
                    result: false,
                    message: Some("project_path is required".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        if project_path.is_empty() {
            return ValidationResult {
                result: false,
                message: Some("project_path cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if !self.validate_project_path(project_path) {
            return ValidationResult {
                result: false,
                message: Some(format!(
                    "Project path does not exist or is not a directory: {}",
                    project_path
                )),
                error_code: Some(404),
                meta: None,
            };
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        let project_path = input
            .get("project_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if options.verbose {
            format!("HarmmonyOS build on project: {}", project_path)
        } else {
            format!("HarmonyOS build: {}", project_path)
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let project_path = input
            .get("project_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("project_path is required".to_string()))?;

        let result = self.execute_build(project_path).await?;

        Ok(vec![ToolResult::Result {
            data: json!({
                "project_path": project_path,
                "success": true
            }),
            result_for_assistant: Some(result),
            image_attachments: None,
        }])
    }
}
#[napi]
pub fn set_build_result(msg: String) {
    log::info!("set_build_result msg: {}", msg);
    let (lock, cvar) = &**BUILD_STATE;
    let mut state = lock.lock();
    state.result = Some(msg);
    state.notified = true;
    cvar.notify_all();
}
pub fn call_harmony_build(args: String) -> Result<String, String> {
    let result = Ok(args);
    let results = Arc::new(Mutex::new(String::default()));
    match JS_THREADSAFE_FUNCTION.write().get("call_harmony_build") {
        None => {
            log::error!("call_harmony_build has not register");
            Err("The Arkts has not register the function".to_owned())
        }
        Some(function) => {
            function.call_with_return_value(
                result,
                ThreadsafeFunctionCallMode::Blocking,
                move |result, _| {
                    match result {
                        Ok(_) => {
                            log::info!("call_harmony_build successfully");
                        }
                        Err(err) => {
                            log::error!("call_harmony_build failed with error: {}", err);
                        }
                    }
                    Ok(())
                },
            );
            let res = results.lock().to_string();
            Ok(res)
        }
    }
}
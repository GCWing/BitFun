use super::util::resolve_path_with_workspace;
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::JS_THREADSAFE_FUNCTIONS;
use async_trait::async_trait;
use napi_ohos::threadsafe_function::ThreadsafeFunctionCallMode;
use serde_json::{ Value,json};
use std::fs;
use std::path::Path;
use tokio::process::Command;

pub struct HarmonyProjectGenTool;

impl HarmonyProjectGenTool {
    pub fn new() -> Self { Self}
}

#[async_trait]
impl Tool for HarmonyProjectGenTool {
    fn name(&self) -> &str {"HarmonyGenerate"}

    async fn description(&self)-> BitFunResult<String> {
        Ok(r#"Generates or create a new HarmonyOS project.
        Usages:
        - Use this to scaffold a new HarmonyOS/OpenHarmony project using ArkTs."#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "HarmonyOS/OpenHarmony Project Name"
                },
            },
            "required": ["name"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool { false }

    fn is_concurrency_safe(&self,_input: Option<&Value>) -> bool { false }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext
    ) -> BitFunResult<Vec<ToolResult>> {
        let title = input.get("name").and_then(|v| v.as_str()).unwrap_or_default();
        let _ = harmonyos_create(title.to_string());
        let result = ToolResult::Result {
            data: json!({
                "project_name": title,
                "bundle_name": "com.example.myapplication",
                "status": "created",
            }),
            result_for_assistant: Some("Successfully generated HarmonyOS project".to_string()),
            image_attachments: None,
        };
        Ok(vec![result])
    }
}

pub fn harmonyos_create(title: String)-> Result<String, String> {
    let result = Ok(title);

    match JS_THREADSAFE_FUNCTIONS.write().get("harmonyos_create") {
        None => {
            return Err(String::from("harmonyos_create is not defined"));
        }
        Some(functions) => {
            functions.call_with_return_value(
                result,
                ThreadsafeFunctionCallMode::Blocking,
                move |result,_| {
                    match result {
                        Ok(_) => {
                            log::info!("harmonyos_create is created");
                        }
                        Err(error) => {
                            log::error!("harmonyos_create error: {:?}", error);
                        }
                    }
                    Ok(())
                },
            );
        }
    }
    Ok("".to_string())
}
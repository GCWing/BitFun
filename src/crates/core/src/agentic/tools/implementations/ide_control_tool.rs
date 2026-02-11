//! IDE control tool - allows Agent to control IDE UI operations
//!
//! Provides IDE control capabilities such as panel opening, navigation, layout adjustment

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::infrastructure::events::event_system::{get_global_event_system, BackendEvent};
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use chrono::Utc;
use log::debug;
use serde_json::{json, Value};

/// IDE control tool
pub struct IdeControlTool;

impl IdeControlTool {
    pub fn new() -> Self {
        Self
    }

    /// Check if operation requires user confirmation
    fn needs_confirmation(&self, action: &str, _target: &Value) -> bool {
        // Some sensitive operations may require confirmation, currently all IDE control operations are safe
        matches!(action, "close_all_tabs" | "reset_layout")
    }

    /// Map action to operation
    fn map_action_to_operation(&self, action: &str) -> String {
        match action {
            "open_panel" => "open_panel",
            "close_panel" => "close_panel",
            "toggle_panel" => "toggle_panel",
            "navigate_to" => "navigate",
            "set_layout" => "layout",
            "manage_tab" => "tab",
            "focus_view" => "focus",
            "expand_section" => "expand",
            _ => "unknown",
        }
        .to_string()
    }

    /// Build target information
    fn build_target_info(&self, target: &Value) -> Value {
        let panel_type = target
            .get("panel_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let panel_config = target.get("panel_config").cloned().unwrap_or(json!({}));

        json!({
            "type": panel_type,
            "id": format!("{}_{}", panel_type, Utc::now().timestamp_millis()),
            "config": panel_config
        })
    }

    /// Validate if panel type is valid
    fn is_valid_panel_type(&self, panel_type: &str) -> bool {
        matches!(
            panel_type,
            "git-settings"
                | "git-diff"
                | "config-center"
                | "planner"
                | "files"
                | "code-editor"
                | "markdown-editor"
                | "ai-session"
                | "mermaid-editor"
        )
    }

    /// Generate user-friendly operation description
    fn generate_action_description(&self, action: &str, target: &Value) -> String {
        let panel_type = target
            .get("panel_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        match action {
            "open_panel" => format!("Opening {} panel", panel_type),
            "close_panel" => format!("Closing {} panel", panel_type),
            "toggle_panel" => format!("Toggling {} panel", panel_type),
            "navigate_to" => "Navigating to location".to_string(),
            "set_layout" => "Adjusting layout".to_string(),
            "manage_tab" => "Managing tab".to_string(),
            "focus_view" => "Focusing view".to_string(),
            _ => format!("Executing {} action", action),
        }
    }
}

#[async_trait]
impl Tool for IdeControlTool {
    fn name(&self) -> &str {
        "IdeControl"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Use the IdeControl tool to interact with the IDE user interface. You can:

1. Open panels to show specific views:
   - Git settings: {"action": "open_panel", "target": {"panel_type": "git-settings"}}
   - Settings (specific section): {"action": "open_panel", "target": {"panel_type": "config-center", "panel_config": {"section": "models"}}}
   - Planner: {"action": "open_panel", "target": {"panel_type": "planner"}}

2. Close or toggle panels when needed

3. Navigate to specific files or code locations

Always use this tool when you need to show the user specific IDE panels or navigate the interface."#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "open_panel",
                        "close_panel",
                        "toggle_panel",
                        "navigate_to",
                        "set_layout",
                        "manage_tab",
                        "focus_view",
                        "expand_section"
                    ],
                    "description": "The IDE control action to perform"
                },
                "target": {
                    "type": "object",
                    "properties": {
                        "panel_type": {
                            "type": "string",
                            "enum": [
                                "git-settings",
                                "git-diff",
                                "config-center",
                                "planner",
                                "files",
                                "code-editor",
                                "markdown-editor",
                                "ai-session",
                                "mermaid-editor"
                            ],
                            "description": "Type of panel to control"
                        },
                        "panel_config": {
                            "type": "object",
                            "properties": {
                                "section": {
                                    "type": "string",
                                    "description": "Specific section to open (e.g., 'models' for config-center)"
                                },
                                "tab_id": {
                                    "type": "string",
                                    "description": "Specific tab identifier"
                                },
                                "session_id": {
                                    "type": "string",
                                    "description": "Session ID"
                                },
                                "file_path": {
                                    "type": "string",
                                    "description": "File path (for file-related panels)"
                                },
                                "data": {
                                    "type": "object",
                                    "description": "Additional custom data"
                                }
                            },
                            "description": "Configuration for the panel"
                        }
                    },
                    "required": ["panel_type"],
                    "description": "Target information for the action"
                },
                "position": {
                    "type": "string",
                    "enum": ["left", "right", "bottom", "center"],
                    "description": "Position where the panel should appear"
                },
                "auto_focus": {
                    "type": "boolean",
                    "description": "Whether to automatically focus the panel",
                    "default": true
                },
                "mode": {
                    "type": "string",
                    "enum": ["agent", "project"],
                    "description": "Operation mode",
                    "default": "agent"
                },
                "options": {
                    "type": "object",
                    "properties": {
                        "replace_existing": {
                            "type": "boolean",
                            "description": "Whether to replace existing tab",
                            "default": false
                        },
                        "check_duplicate": {
                            "type": "boolean",
                            "description": "Whether to check for duplicate tabs",
                            "default": true
                        },
                        "expand_panel": {
                            "type": "boolean",
                            "description": "Whether to expand the panel",
                            "default": true
                        }
                    },
                    "description": "Additional operation options"
                }
            },
            "required": ["action", "target"]
        })
    }

    fn user_facing_name(&self) -> String {
        "IDE Control".to_string()
    }

    async fn is_enabled(&self) -> bool {
        true
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
        // Validate action field
        let action = match input.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => {
                return ValidationResult {
                    result: false,
                    message: Some("Missing required field: action".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        // Validate target field
        let target = match input.get("target") {
            Some(t) => t,
            None => {
                return ValidationResult {
                    result: false,
                    message: Some("Missing required field: target".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        // Validate panel_type
        if let Some(panel_type) = target.get("panel_type").and_then(|v| v.as_str()) {
            if !self.is_valid_panel_type(panel_type) {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid panel_type: {}", panel_type)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        } else if matches!(action, "open_panel" | "close_panel" | "toggle_panel") {
            return ValidationResult {
                result: false,
                message: Some("Missing required field: target.panel_type".to_string()),
                error_code: Some(400),
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

    fn render_result_for_assistant(&self, output: &Value) -> String {
        if let Some(success) = output.get("success").and_then(|v| v.as_bool()) {
            if success {
                if let Some(message) = output.get("message").and_then(|v| v.as_str()) {
                    return message.to_string();
                }
                return "IDE operation completed successfully".to_string();
            }
        }

        if let Some(error) = output.get("error").and_then(|v| v.as_str()) {
            return format!("IDE operation failed: {}", error);
        }

        "IDE operation result unknown".to_string()
    }

    fn render_tool_use_message(
        &self,
        input: &Value,
        _options: &crate::agentic::tools::framework::ToolRenderOptions,
    ) -> String {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let target = input.get("target").cloned().unwrap_or(json!({}));

        self.generate_action_description(action, &target)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing action field"))?;

        let target = input
            .get("target")
            .ok_or_else(|| anyhow::anyhow!("Missing target field"))?;

        // Check if user confirmation is needed
        if self.needs_confirmation(action, target) {
            return Ok(vec![ToolResult::Progress {
                content: json!({
                    "message": "This operation requires user confirmation",
                    "action": action,
                    "target": target
                }),
                normalized_messages: None,
                tools: None,
            }]);
        }

        // Generate unique request ID for tracking execution results
        let request_id = uuid::Uuid::new_v4().to_string();

        // Build standardized event
        let operation = self.map_action_to_operation(action);
        let target_info = self.build_target_info(target);

        let event = BackendEvent::Custom {
            event_name: "ide-control-event".to_string(),
            payload: json!({
                "operation": operation,
                "target": target_info,
                "position": input.get("position").and_then(|v| v.as_str()).unwrap_or("right"),
                "options": {
                    "auto_focus": input.get("auto_focus").and_then(|v| v.as_bool()).unwrap_or(true),
                    "replace_existing": input.get("options")
                        .and_then(|o| o.get("replace_existing"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    "check_duplicate": input.get("options")
                        .and_then(|o| o.get("check_duplicate"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    "expand_panel": input.get("options")
                        .and_then(|o| o.get("expand_panel"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    "mode": input.get("mode").and_then(|v| v.as_str()).unwrap_or("agent")
                },
                "metadata": {
                    "source": "agent_tool",
                    "timestamp": Utc::now().timestamp_millis(),
                    "session_id": context.session_id.clone().unwrap_or_default(),
                    "request_id": request_id.clone()
                }
            }),
        };

        // Send event to frontend
        debug!(
            "IdeControl tool sending IDE control event, operation: {}, target_type: {}",
            operation,
            target_info
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        );

        let event_system = get_global_event_system();
        event_system.emit(event).await?;

        // Generate operation description
        let description = self.generate_action_description(action, target);

        // Return execution result (includes request_id for subsequent tracking)
        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "message": format!("{} - Command sent to IDE", description),
                "request_id": request_id,
                "action": action,
                "target": target_info,
                "details": {
                    "operation_type": operation,
                    "panel_type": target_info.get("type"),
                    "timestamp": Utc::now().to_rfc3339()
                }
            }),
            result_for_assistant: Some(format!(
                "{} successfully. The IDE has been updated and the panel should now be visible to the user.",
                description
            ))
        }])
    }
}

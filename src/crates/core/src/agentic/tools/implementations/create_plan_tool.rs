//! CreatePlan tool implementation
//!
//! Used to create and store plan files during the planning phase

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::fs;

/// YAML frontmatter structure for Plan files
#[derive(Serialize)]
struct PlanFrontmatter {
    name: String,
    overview: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    todos: Vec<TodoItem>,
}

/// Todo item structure
#[derive(Serialize)]
struct TodoItem {
    id: String,
    content: String,
    status: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<String>,
}

/// CreatePlan tool - create plan file
pub struct CreatePlanTool;

impl CreatePlanTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CreatePlanTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CreatePlanTool {
    fn name(&self) -> &str {
        "CreatePlan"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r###"Use this tool to create a concise plan for accomplishing the user's request. This tool should be called at the end of the planning phase to finalize and store the plan for user approval.

The plan should be:
- Properly formatted in markdown, using appropriate sections and headers
- Very concise and actionable, providing the minimum amount of detail for the user to understand and action the plan
- The first line MUST BE A TITLE formatted as a level 1 markdown heading

It may be helpful to identify the most important files you will change and existing code you will leverage.
When mentioning files, use markdown links with the full file path (for example, `[backend/src/foo.ts](backend/src/foo.ts)`).

You should provide a structured list of implementation todos:
- Each todo should be a clear, specific, and actionable task that can be tracked and completed
- If the plan is simple, you should provide just a few high-level todos or none at all
- Each todo needs:
    - A clear, unique ID (e.g., "setup-auth", "implement-ui", "add-tests")
    - A descriptive content explaining what needs to be done

UPDATING THE PLAN:
- This tool creates a NEW plan file each time it is called
- The plan file path returned in the tool result may be an absolute runtime path (local) or a `bitfun://runtime/...` URI (remote)
- To update an existing plan, read and edit the plan file directly using your file editing tools
- Do NOT call CreatePlan again to update an existing plan

Additional guidelines:
- Avoid asking clarifying questions in the plan itself. Ask them before calling this tool. Present these to the user using the AskUserQuestion tool.
- After calling this tool, you should end the conversation turn. Briefly tell the user where the plan file is. Do NOT repeat the plan content again.
- Todos help break down complex plans into manageable, trackable tasks
- Focus on high-level meaningful decisions rather than low-level implementation details
- A good plan is glanceable, not a wall of text."###
        .to_string())
    }

    fn short_description(&self) -> String {
        "Create and store a concise implementation plan.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["name", "overview", "plan"],
            "properties": {
                "name": {
                    "type": "string",
                    "description": "A short 3-4 word name for the plan."
                },
                "overview": {
                    "type": "string",
                    "description": "A 1-2 sentence high-level description of the plan that summarizes what will be accomplished"
                },
                "plan": {
                    "type": "string",
                    "description": "The plan you came up with"
                },
                "todos": {
                    "type": "array",
                    "description": "Array of implementation todos",
                    "items": {
                        "type": "object",
                        "required": ["id", "content"],
                        "properties": {
                            "id": {
                                "type": "string",
                                "description": "Unique identifier for the todo"
                            },
                            "content": {
                                "type": "string",
                                "description": "Description of the todo task"
                            },
                            "dependencies": {
                                "type": "array",
                                "description": "Array of todo IDs that must be completed before this todo can start",
                                "items": {
                                    "type": "string"
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    fn is_readonly(&self) -> bool {
        // Only writes plan file, doesn't modify code
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        // Parse parameters
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or(BitFunError::validation("Missing required field: name"))?;

        let overview = input
            .get("overview")
            .and_then(|v| v.as_str())
            .ok_or(BitFunError::validation("Missing required field: overview"))?;

        let plan = input
            .get("plan")
            .and_then(|v| v.as_str())
            .ok_or(BitFunError::validation("Missing required field: plan"))?;

        let todos = input.get("todos").and_then(|v| v.as_array());

        // Generate filename: {name_lowercase_underscored}_{8-digit uuid}.plan.md
        let name_normalized = name
            .to_lowercase()
            .replace(' ', "_")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>();

        let uuid_short = uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("00000000")
            .to_string();

        let plan_file_name = format!("{}_{}.plan.md", name_normalized, uuid_short);

        let file_content = generate_plan_file_content(name, overview, plan, todos);

        let runtime_context = context.ensure_current_workspace_runtime().await?;
        let plans_dir = runtime_context.plans_dir.clone();
        let plan_file_path = plans_dir.join(&plan_file_name);
        fs::write(&plan_file_path, &file_content)
            .await
            .map_err(|e| BitFunError::tool(format!("Failed to write plan file: {}", e)))?;
        let plan_file_path_str = plan_file_path.to_string_lossy().to_string();

        // Process todos for return result
        let processed_todos: Vec<Value> = if let Some(todos_arr) = todos {
            todos_arr
                .iter()
                .map(|todo| {
                    let mut todo_obj = todo.clone();
                    if let Some(obj) = todo_obj.as_object_mut() {
                        // Add default status
                        if !obj.contains_key("status") {
                            obj.insert("status".to_string(), json!("pending"));
                        }
                    }
                    todo_obj
                })
                .collect()
        } else {
            vec![]
        };

        // Prefer workspace-relative computer:// links, but fall back to an
        // absolute computer:// path when plans live outside the workspace tree.
        let computer_link = context
            .workspace_root()
            .and_then(|root| {
                std::path::Path::new(&plan_file_path_str)
                    .strip_prefix(root)
                    .ok()
                    .map(|rel| format!("computer://{}", rel.to_string_lossy().replace('\\', "/")))
            })
            .unwrap_or_else(|| format!("computer://{}", plan_file_path_str.replace('\\', "/")));

        let plan_reference =
            context.build_runtime_artifact_reference(&format!("plans/{}", plan_file_name))?;

        let result_for_assistant = format!(
            "Plan file created at: {}
Clickable link for user: [{}]({})
Your next reply MUST show the clickable link and then end the conversation turn. Do not continue with more planning details or additional questions.",
            plan_reference,
            plan_file_name,
            computer_link,
        );

        let result = json!({
            "success": true,
            "plan_file_path": plan_reference,
            "computer_link": computer_link.clone(),
            "plan_file_name": plan_file_name,
            "name": name,
            "overview": overview,
            "todos": processed_todos
        });

        Ok(vec![ToolResult::Result {
            data: result,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

/// Generate plan file content
fn generate_plan_file_content(
    name: &str,
    overview: &str,
    plan: &str,
    todos: Option<&Vec<Value>>,
) -> String {
    // Convert todos
    let todos_vec: Vec<TodoItem> = todos
        .map(|arr| {
            arr.iter()
                .filter_map(|todo| {
                    let id = todo.get("id").and_then(|v| v.as_str())?;
                    let content = todo.get("content").and_then(|v| v.as_str())?;
                    let dependencies = todo
                        .get("dependencies")
                        .and_then(|v| v.as_array())
                        .map(|deps| {
                            deps.iter()
                                .filter_map(|d| d.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();

                    Some(TodoItem {
                        id: id.to_string(),
                        content: content.to_string(),
                        status: "pending".to_string(),
                        dependencies,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let frontmatter = PlanFrontmatter {
        name: name.to_string(),
        overview: overview.to_string(),
        todos: todos_vec,
    };

    // Serialize frontmatter using serde_yaml
    let yaml = serde_yaml::to_string(&frontmatter).unwrap_or_default();

    format!("---\n{}---\n\n{}", yaml, plan)
}

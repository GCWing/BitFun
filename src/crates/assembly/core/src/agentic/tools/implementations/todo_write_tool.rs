use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

/// TodoWrite tool - record todo items
pub struct TodoWriteTool;

impl TodoWriteTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TodoWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "TodoWrite"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r###"Create and manage the structured task list for the current session. Use it to keep multi-step work visible, prevent missed follow-ups, and track verification.

Use TodoWrite when:
- The task has multiple meaningful steps, files, phases, or verification actions.
- The user gives a list of tasks or explicitly asks for task tracking.
- You are entering a test/fix loop or a broad investigation that may uncover follow-up work.
- New instructions change the scope and should be reflected in the plan.

Skip TodoWrite when:
- The task is a single obvious action or a short conversational answer.
- Tracking would add noise without improving reliability.

Management rules:
- Keep items specific and actionable.
- Keep exactly one item in_progress while actively working; mark it completed as soon as it is finished.
- Do not mark a task completed if implementation is partial, tests are failing, or a blocker remains.
- Add or remove items as the work changes so the list stays accurate.
- Include verification as a task when the result depends on code changes, tool output, external sources, UI state, or generated files.

Task states:
- pending: not started
- in_progress: currently being worked on
- completed: fully done

Each item must include:
- id: stable unique identifier
- content: imperative description of the work
- status: pending, in_progress, or completed

Each item may include:
- dependencies: optional array of todo item ids this item depends on; cyclic dependencies are rejected
"###.to_string())
    }

    fn short_description(&self) -> String {
        "Create and update the session todo list.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {
                                "type": "string",
                                "description": "Unique identifier for the todo item"
                            },
                            "content": {
                                "type": "string",
                                "minLength": 1,
                                "description": "The imperative form describing what needs to be done"
                            },
                            "status": {
                                "type": "string",
                                "enum": [
                                    "pending",
                                    "in_progress",
                                    "completed"
                                ],
                                "description": "Current status of the todo item"
                            },
                            "dependencies": {
                                "type": "array",
                                "items": {
                                    "type": "string"
                                },
                                "description": "Optional ids of todo items this item depends on. Parents are ordered and rendered before this item. Cyclic dependencies are rejected."
                            }
                        },
                        "required": [
                            "id",
                            "content",
                            "status"
                        ],
                        "additionalProperties": false
                    },
                    "description": "The updated todo list"
                }
            },
            "required": ["todos"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        // TodoWrite replaces the session todo list, so it is a
        // state-mutating call, not a read. Marking it readonly let RBAC treat
        // it as side-effect free and skip Write/Communicate gating.
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        use std::collections::HashSet;

        // Parse todos array
        let todos = input
            .get("todos")
            .and_then(|v| v.as_array())
            .ok_or(BitFunError::validation("Missing required field: todos"))?;

        let mut processed_todos = Vec::new();
        // Reject duplicate ids so every todo id stays a stable,
        // addressable key in the list.
        let mut seen_ids: HashSet<String> = HashSet::new();
        for todo in todos {
            let mut todo_obj = todo.clone();
            // Each todo must be a JSON object; a non-object item was
            // previously passed through unvalidated.
            let Some(obj) = todo_obj.as_object_mut() else {
                return Err(BitFunError::validation("Todo item must be an object"));
            };
            if !obj.contains_key("status") {
                return Err(BitFunError::validation("Todo item missing status field"));
            }
            if !obj.contains_key("content") {
                return Err(BitFunError::validation("Todo item missing content field"));
            }
            // Reject status values outside the documented enum
            // instead of silently ignoring them in the stats counter.
            let status = obj
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            match status {
                "pending" | "in_progress" | "completed" => {}
                other => {
                    return Err(BitFunError::validation(format!(
                        "Todo item has invalid status '{}': expected pending, in_progress, or completed",
                        other
                    )));
                }
            }
            // If no id, generate a new one
            if !obj.contains_key("id") {
                let uuid = uuid::Uuid::new_v4().to_string();
                let short_id = uuid.split('-').next().unwrap_or("todo");
                let new_id = format!("todo_{}", short_id);
                obj.insert("id".to_string(), json!(new_id));
            }
            // An id must be a non-empty string so the dependency
            // topology below and downstream consumers can address it reliably.
            let id = obj
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or_else(|| BitFunError::validation("Todo item id must be a string"))?;
            if id.trim().is_empty() {
                return Err(BitFunError::validation("Todo item id must not be empty"));
            }
            if !seen_ids.insert(id.to_string()) {
                return Err(BitFunError::validation(format!(
                    "Duplicate todo id '{}'",
                    id
                )));
            }
            processed_todos.push(todo_obj);
        }

        // Topology validation: reject self-loops, unknown references, and cycles.
        validate_todo_dependencies(&processed_todos)?;

        let todo_count = processed_todos.len();
        let mut status_counts = [0; 3];
        processed_todos.iter().for_each(|t| {
            let status = t.get("status").and_then(|s| s.as_str()).unwrap_or("");
            match status {
                "pending" => status_counts[0] += 1,
                "in_progress" => status_counts[1] += 1,
                "completed" => status_counts[2] += 1,
                _ => {}
            }
        });

        let summary = format!(
            "Updated todo list with {} tasks (completed: {}, in_progress: {}, pending: {})",
            todo_count, status_counts[2], status_counts[1], status_counts[0]
        );

        let result = json!({
            "success": true,
            "todos": processed_todos,
            "merge": false,
            "count": todo_count,
            "summary": summary,
            "stats": {
                "completed": status_counts[2],
                "in_progress": status_counts[1],
                "pending": status_counts[0]
            }
        });

        Ok(vec![ToolResult::Result {
            data: result,
            result_for_assistant: Some(summary),
            image_attachments: None,
        }])
    }
}

/// Validate the todo dependency topology.
///
/// Rejects self-loops, dependencies referencing unknown todo ids, and cycles.
/// Mirrors the legion topology cycle rejection pattern (Kahn topological sort;
/// when not every node is visited, the graph contains a cycle).
fn validate_todo_dependencies(todos: &[Value]) -> BitFunResult<()> {
    use std::collections::{BTreeSet, HashMap, HashSet};

    let mut ids: HashSet<String> = HashSet::new();
    for todo in todos {
        if let Some(id) = todo.get("id").and_then(|v| v.as_str()) {
            ids.insert(id.to_string());
        }
    }

    // Edge validation: endpoints exist, no self-loops.
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    for id in &ids {
        adjacency.insert(id.clone(), Vec::new());
        in_degree.insert(id.clone(), 0);
    }
    for todo in todos {
        let Some(child) = todo.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(deps) = todo.get("dependencies").and_then(|v| v.as_array()) else {
            continue;
        };
        for dep_value in deps {
            let Some(dep) = dep_value.as_str() else {
                return Err(BitFunError::validation("Todo dependency must be a string"));
            };
            if dep == child {
                return Err(BitFunError::validation(format!(
                    "Todo '{}' cannot depend on itself",
                    child
                )));
            }
            if !ids.contains(dep) {
                return Err(BitFunError::validation(format!(
                    "Todo dependency references unknown todo '{}'",
                    dep
                )));
            }
            let nexts = adjacency.get_mut(dep).ok_or_else(|| {
                BitFunError::validation(format!("Internal error: missing adjacency for '{}'", dep))
            })?;
            nexts.push(child.to_string());
            let degree = in_degree.get_mut(child).ok_or_else(|| {
                BitFunError::validation(format!(
                    "Internal error: missing in-degree for '{}'",
                    child
                ))
            })?;
            *degree += 1;
        }
    }

    // Kahn topological sort with deterministic (lexicographic) order.
    let mut ready: BTreeSet<String> = ids
        .iter()
        .filter(|id| in_degree.get(*id).copied().unwrap_or(usize::MAX) == 0)
        .cloned()
        .collect();

    let mut order: Vec<String> = Vec::with_capacity(ids.len());
    while let Some(id) = ready.iter().next().cloned() {
        ready.remove(&id);
        order.push(id.clone());
        let nexts = adjacency.get(&id).cloned().ok_or_else(|| {
            BitFunError::validation(format!("Internal error: missing adjacency for '{}'", id))
        })?;
        for next in nexts {
            let degree = in_degree.get_mut(&next).ok_or_else(|| {
                BitFunError::validation(format!("Internal error: missing in-degree for '{}'", next))
            })?;
            *degree -= 1;
            if *degree == 0 {
                ready.insert(next);
            }
        }
    }
    if order.len() != ids.len() {
        return Err(BitFunError::validation("Todo dependencies contain a cycle"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::framework::ToolUseContext;
    use std::collections::HashMap;

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    fn todo(id: &str, status: &str) -> Value {
        json!({ "id": id, "content": "do the work", "status": status })
    }

    #[test]
    fn todo_write_is_not_readonly() {
        // TodoWrite mutates the session todo list.
        assert!(!TodoWriteTool::new().is_readonly());
    }

    #[tokio::test]
    async fn rejects_duplicate_ids() {
        // Two items with the same id make the list ambiguous.
        let tool = TodoWriteTool::new();
        let input = json!({
            "todos": [todo("a", "pending"), todo("a", "in_progress")]
        });
        let result = tool.call_impl(&input, &empty_context()).await;
        let err = result.expect_err("duplicate ids must be rejected");
        assert!(
            err.to_string().contains("Duplicate todo id 'a'"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn rejects_non_object_todo() {
        // A non-object item (e.g. a bare string) must not pass
        // through unvalidated.
        let tool = TodoWriteTool::new();
        let input = json!({ "todos": ["not-an-object"] });
        let result = tool.call_impl(&input, &empty_context()).await;
        let err = result.expect_err("non-object todos must be rejected");
        assert!(
            err.to_string().contains("must be an object"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn rejects_invalid_status() {
        // Status values outside the documented enum are rejected.
        let tool = TodoWriteTool::new();
        let input = json!({ "todos": [todo("a", "done")] });
        let result = tool.call_impl(&input, &empty_context()).await;
        let err = result.expect_err("invalid status must be rejected");
        assert!(
            err.to_string().contains("invalid status 'done'"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn rejects_non_string_id() {
        // Ids must be strings so the dependency topology can
        // address them reliably.
        let tool = TodoWriteTool::new();
        let input = json!({
            "todos": [{ "id": 123, "content": "do the work", "status": "pending" }]
        });
        let result = tool.call_impl(&input, &empty_context()).await;
        let err = result.expect_err("non-string ids must be rejected");
        assert!(
            err.to_string().contains("id must be a string"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn accepts_valid_todo_list_and_auto_generates_ids() {
        let tool = TodoWriteTool::new();
        let input = json!({
            "todos": [
                { "content": "first", "status": "pending" },
                { "id": "b", "content": "second", "status": "completed", "dependencies": [] }
            ]
        });
        let result = tool.call_impl(&input, &empty_context()).await;
        let results = result.expect("valid todo list should succeed");
        let data = &results[0].content();
        let todos = data
            .get("todos")
            .and_then(|value| value.as_array())
            .expect("todos array");
        assert_eq!(todos.len(), 2);
        assert!(todos[0]
            .get("id")
            .and_then(|value| value.as_str())
            .is_some());
        assert_eq!(
            todos[1].get("id").and_then(|value| value.as_str()),
            Some("b")
        );
    }
}

use super::*;

impl TaskTool {
    pub(super) fn base_schema_properties() -> Map<String, Value> {
        let mut properties = Map::new();
        properties.insert(
            "description".to_string(),
            json!({
                "type": "string",
                "description": "A short (3-5 word) description of the task"
            }),
        );
        properties.insert(
            "prompt".to_string(),
            json!({
                "type": "string",
                "description": "The prompt to be sent to the agent. Keep it scoped and concise."
            }),
        );
        properties.insert(
            "subagent_type".to_string(),
            json!({
                "type": "string",
                "description": "Top-level agent type for a new subagent."
            }),
        );
        properties.insert(
            "model_id".to_string(),
            json!({
                "type": "string",
                "description": "Optional model ID for action='spawn' and action='send_input'. Can be 'inherit', 'primary', 'fast', or a configured model ID."
            }),
        );
        properties
    }

    pub(super) fn regular_input_schema() -> Value {
        let mut properties = Self::base_schema_properties();
        properties.insert(
            "action".to_string(),
            json!({
                "type": "string",
                "enum": ["spawn", "send_input", "cancel"],
                "description": "The action to perform."
            }),
        );
        if let Some(subagent_type) = properties.get_mut("subagent_type") {
            subagent_type["description"] =
                json!("Optional for action='spawn'. Do not provide with fork_context=true.");
        }
        properties.insert(
            "fork_context".to_string(),
            json!({
                "type": "boolean",
                "default": false,
                "description": "Optional for action='spawn'. Defaults to false. When true, do not provide subagent_type."
            }),
        );
        properties.insert(
            "agent_id".to_string(),
            json!({
                "type": "string",
                "description": "Required for action='send_input' and action='cancel'."
            }),
        );
        properties.insert(
            "run_in_background".to_string(),
            json!({
                "type": "boolean",
                "description": "Optional for action='spawn' and action='send_input'. Defaults to false."
            }),
        );
        json!({
            "type": "object",
            "properties": properties,
            "required": [
                "action"
            ],
            "additionalProperties": false
        })
    }

    pub(super) fn render_description(&self) -> String {
        r#"Run or manage a subagent that handles complex, multi-step tasks autonomously.

When to use:
- Delegate when a specialized subagent or separate context is likely to improve coverage, independence, or parallelism.
- Use direct tools instead for focused lookups, known paths, single symbols, or code that can be inspected with a few reads or searches.

Supported actions:
- `spawn`: create and run a new subagent. The result contains an `agent_id` for future `send_input` or `cancel`.
- `send_input`: continue an existing subagent. Provide `agent_id`, `description`, and `prompt`. Optionally provide `model_id` to switch the subagent model for this and later turns.
- `cancel`: cancel a background subagent. Provide `agent_id`.

Two modes for action='spawn':
The two modes are mutually exclusive: do not provide `subagent_type` when `fork_context=true`.
1. With an explicit `subagent_type` (default)
  - Provide `subagent_type`, `description`, and `prompt`.
  - Available types are listed in the <available_agents> section. Each type has specific capabilities and tools.
  - In this mode, the subagent does not share your context. Include all necessary background information in the prompt.
2. By forking the current context
  - Set `fork_context=true`, and provide `description` and `prompt`. Do not provide `subagent_type`.
  - In this mode, the subagent inherits the full conversation history up to this point — all prior user messages, assistant responses, and tool results. You do not need to repeat information already covered in the conversation.

`prompt` writing guidelines:
- Do not put `action`, `subagent_type`, `agent_id`, `description`, or `model_id` inside the prompt string.
- Keep it under 180 lines / 16KB. For large delegations, split the work into multiple Task calls with clear ownership.
- Pass file paths, symbols, constraints, and exact questions instead of pasting large file contents.
- Clearly tell the agent whether you expect code changes or research only (searches, file reads, web fetches, etc.), because it does not know the user's intent unless you state it.

`run_in_background` usage:
- false: Wait for the agent to finish and return its result to you.
- true: Run the agent in the background without blocking you. The response includes a `bg_task_id`; use AgentWait when you need the results.

`model_id` usage:
- Set it only when the user requests a particular model.
- Omit it to use the subagent's configured model, which may differ from your model.
- Special values: `inherit` explicitly uses the same model as yours; `primary` and `fast` use the user's configured model slots.
- For a configured model, call ListModels first and use its returned `model_id`.

Usage notes:
- Include a short description of what the agent will do for this round (for `spawn` and `send_input`).
- Provide a clear prompt for `spawn` and `send_input` so the agent can work autonomously and return the information you need.
- The subagent inherits your workspace. If the subagent should inspect or operate on a path outside the current workspace, say that target path and scope clearly in the prompt.
- Launch independent agents concurrently when that improves coverage or latency or when the user explicitly requests it. To do this, send parallel Task calls in a single assistant message.
- When launching multiple non-read-only subagents in parallel, assign non-overlapping scopes and outputs so their file edits, commands, or external side effects do not conflict.
- Treat subagent outputs as useful evidence, but verify details yourself before making edits or final claims that depend on exact code.
- If an agent description mentions proactive use, consider it when relevant and use your judgment.

Examples (assume "example-reviewer" is present in the agent listing):
<examples>
- Start a new specialized subagent: `{ "action": "spawn", "description": "Inspect parser flow", "subagent_type": "example-reviewer", "prompt": "Inspect the parser flow in src/parser.rs and report risks, key functions, and any missing tests." }`
- Start by forking the current context: `{ "action": "spawn", "description": "Check migration impact", "fork_context": true, "prompt": "Using the current context, check whether the migration affects config loading. Stay read-only and report the answer with file references." }`
- Continue an existing subagent with a specific model: `{ "action": "send_input", "description": "Continue parser review", "agent_id": "a1", "model_id": "fast", "prompt": "Continue from your prior parser review and focus on the error recovery paths." }`
- Cancel a background subagent: `{ "action": "cancel", "agent_id": "a1" }`
</examples>
"#
            .to_string()
    }
}

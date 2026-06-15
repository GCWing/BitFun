//! Product tool collapsed-unlock state owner.

use crate::agentic::core::{Message, MessageContent};
use bitfun_agent_tools::{collect_loaded_collapsed_tool_names, GetToolSpecLoadObservation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductCollapsedUnlockState {
    unlocked_tools: Vec<String>,
}

impl ProductCollapsedUnlockState {
    pub(crate) fn from_messages(messages: &[Message], collapsed_tools: &[String]) -> Self {
        let observations = messages
            .iter()
            .filter_map(get_tool_spec_load_observation)
            .collect::<Vec<_>>();

        Self {
            unlocked_tools: collect_loaded_collapsed_tool_names(
                &observations,
                collapsed_tools,
                crate::agentic::tools::registry::GET_TOOL_SPEC_TOOL_NAME,
            ),
        }
    }

    #[cfg(test)]
    fn is_unlocked(&self, tool_name: &str) -> bool {
        self.unlocked_tools
            .iter()
            .any(|unlocked_tool| unlocked_tool == tool_name)
    }

    pub(crate) fn into_unlocked_tools(self) -> Vec<String> {
        self.unlocked_tools
    }
}

pub(crate) fn collect_product_unlocked_collapsed_tools(
    messages: &[Message],
    collapsed_tools: &[String],
) -> Vec<String> {
    ProductCollapsedUnlockState::from_messages(messages, collapsed_tools).into_unlocked_tools()
}

fn get_tool_spec_load_observation(message: &Message) -> Option<GetToolSpecLoadObservation<'_>> {
    let MessageContent::ToolResult {
        tool_name,
        result,
        is_error,
        ..
    } = &message.content
    else {
        return None;
    };

    Some(GetToolSpecLoadObservation {
        tool_name,
        loaded_tool_name: result.get("tool_name").and_then(|v| v.as_str()),
        is_error: *is_error,
    })
}

#[cfg(test)]
mod tests {
    use super::{collect_product_unlocked_collapsed_tools, ProductCollapsedUnlockState};
    use crate::agentic::core::{Message, ToolResult};
    use serde_json::json;

    #[test]
    fn product_unlock_state_collects_visible_get_tool_spec_results() {
        let visible_get_tool_spec_result = Message::tool_result(ToolResult {
            tool_id: "tool-1".to_string(),
            tool_name: "GetToolSpec".to_string(),
            result: json!({
                "tool_name": "WebFetch",
            }),
            result_for_assistant: None,
            is_error: false,
            duration_ms: Some(1),
            image_attachments: None,
        });
        let hidden_get_tool_spec_result = Message::tool_result(ToolResult {
            tool_id: "tool-2".to_string(),
            tool_name: "GetToolSpec".to_string(),
            result: json!({
                "tool_name": "Read",
            }),
            result_for_assistant: None,
            is_error: false,
            duration_ms: Some(1),
            image_attachments: None,
        });
        let failed_get_tool_spec_result = Message::tool_result(ToolResult {
            tool_id: "tool-3".to_string(),
            tool_name: "GetToolSpec".to_string(),
            result: json!({
                "tool_name": "GetFileDiff",
            }),
            result_for_assistant: None,
            is_error: true,
            duration_ms: Some(1),
            image_attachments: None,
        });

        let unlocked = collect_product_unlocked_collapsed_tools(
            &[
                visible_get_tool_spec_result,
                hidden_get_tool_spec_result,
                failed_get_tool_spec_result,
            ],
            &["WebFetch".to_string(), "GetFileDiff".to_string()],
        );

        assert_eq!(unlocked, vec!["WebFetch".to_string()]);
    }

    #[test]
    fn product_unlock_state_dedupes_and_filters_runtime_unlocks() {
        let unlocked = collect_product_unlocked_collapsed_tools(
            &[
                Message::tool_result(ToolResult {
                    tool_id: "tool-1".to_string(),
                    tool_name: "GetToolSpec".to_string(),
                    result: json!({
                        "tool_name": "WebFetch",
                    }),
                    result_for_assistant: None,
                    is_error: false,
                    duration_ms: Some(1),
                    image_attachments: None,
                }),
                Message::tool_result(ToolResult {
                    tool_id: "tool-2".to_string(),
                    tool_name: "GetToolSpec".to_string(),
                    result: json!({
                        "tool_name": "WebFetch",
                    }),
                    result_for_assistant: None,
                    is_error: false,
                    duration_ms: Some(1),
                    image_attachments: None,
                }),
                Message::tool_result(ToolResult {
                    tool_id: "tool-3".to_string(),
                    tool_name: "GetToolSpec".to_string(),
                    result: json!({
                        "tool_name": "Git",
                    }),
                    result_for_assistant: None,
                    is_error: false,
                    duration_ms: Some(1),
                    image_attachments: None,
                }),
                Message::tool_result(ToolResult {
                    tool_id: "tool-4".to_string(),
                    tool_name: "GetToolSpec".to_string(),
                    result: json!({
                        "tool_name": "Read",
                    }),
                    result_for_assistant: None,
                    is_error: false,
                    duration_ms: Some(1),
                    image_attachments: None,
                }),
                Message::tool_result(ToolResult {
                    tool_id: "tool-5".to_string(),
                    tool_name: "GetToolSpec".to_string(),
                    result: json!({
                        "tool_name": "GetFileDiff",
                    }),
                    result_for_assistant: None,
                    is_error: true,
                    duration_ms: Some(1),
                    image_attachments: None,
                }),
                Message::tool_result(ToolResult {
                    tool_id: "tool-6".to_string(),
                    tool_name: "GetToolSpec".to_string(),
                    result: json!({
                        "tool_name": 42,
                    }),
                    result_for_assistant: None,
                    is_error: false,
                    duration_ms: Some(1),
                    image_attachments: None,
                }),
                Message::tool_result(ToolResult {
                    tool_id: "tool-7".to_string(),
                    tool_name: "Read".to_string(),
                    result: json!({
                        "tool_name": "GetFileDiff",
                    }),
                    result_for_assistant: None,
                    is_error: false,
                    duration_ms: Some(1),
                    image_attachments: None,
                }),
            ],
            &[
                "WebFetch".to_string(),
                "GetFileDiff".to_string(),
                "Git".to_string(),
            ],
        );

        assert_eq!(unlocked, vec!["Git".to_string(), "WebFetch".to_string()]);
    }

    #[test]
    fn product_collapsed_unlock_state_preserves_message_derived_lifecycle() {
        let state = ProductCollapsedUnlockState::from_messages(
            &[Message::tool_result(ToolResult {
                tool_id: "tool-1".to_string(),
                tool_name: "GetToolSpec".to_string(),
                result: json!({
                    "tool_name": "Git",
                }),
                result_for_assistant: None,
                is_error: false,
                duration_ms: Some(1),
                image_attachments: None,
            })],
            &["Git".to_string(), "WebFetch".to_string()],
        );

        assert!(state.is_unlocked("Git"));
        assert!(!state.is_unlocked("WebFetch"));
        assert_eq!(state.into_unlocked_tools(), vec!["Git".to_string()]);
    }
}

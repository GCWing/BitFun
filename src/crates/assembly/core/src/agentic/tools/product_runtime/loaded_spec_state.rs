//! Product deferred-tool loaded-spec state owner.

use crate::agentic::core::{Message, MessageContent};
use bitfun_agent_tools::{
    collect_loaded_deferred_tool_specs, GetToolSpecLoadObservation, LoadedDeferredToolSpec,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductLoadedDeferredToolSpecs {
    loaded_specs: Vec<LoadedDeferredToolSpec>,
}

impl ProductLoadedDeferredToolSpecs {
    pub(crate) fn from_messages(messages: &[Message], deferred_tools: &[String]) -> Self {
        let observations = messages
            .iter()
            .filter_map(get_tool_spec_load_observation)
            .collect::<Vec<_>>();

        Self {
            loaded_specs: collect_loaded_deferred_tool_specs(
                &observations,
                deferred_tools,
                crate::agentic::tools::registry::GET_TOOL_SPEC_TOOL_NAME,
            ),
        }
    }

    #[cfg(test)]
    fn is_loaded(&self, tool_name: &str) -> bool {
        self.loaded_specs
            .iter()
            .any(|spec| spec.tool_name == tool_name)
    }

    pub(crate) fn into_loaded_specs(self) -> Vec<LoadedDeferredToolSpec> {
        self.loaded_specs
    }
}

pub(crate) fn collect_product_loaded_deferred_tool_specs(
    messages: &[Message],
    deferred_tools: &[String],
) -> Vec<LoadedDeferredToolSpec> {
    ProductLoadedDeferredToolSpecs::from_messages(messages, deferred_tools).into_loaded_specs()
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
        catalog_generation: result.get("catalog_generation").and_then(|v| v.as_u64()),
        is_error: *is_error,
    })
}

#[cfg(test)]
mod tests {
    use super::{collect_product_loaded_deferred_tool_specs, ProductLoadedDeferredToolSpecs};
    use crate::agentic::core::{Message, ToolResult};
    use serde_json::json;

    fn loaded_spec(tool_name: &str) -> bitfun_agent_tools::LoadedDeferredToolSpec {
        bitfun_agent_tools::LoadedDeferredToolSpec {
            tool_name: tool_name.to_string(),
            catalog_generation: 42,
        }
    }

    #[test]
    fn product_loaded_spec_state_collects_visible_get_tool_spec_results() {
        let visible_get_tool_spec_result = Message::tool_result(ToolResult {
            tool_id: "tool-1".to_string(),
            tool_name: "GetToolSpec".to_string(),
            result: json!({
                "tool_name": "WebFetch",
                "catalog_generation": 42,
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
                "catalog_generation": 42,
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
                "catalog_generation": 42,
            }),
            result_for_assistant: None,
            is_error: true,
            duration_ms: Some(1),
            image_attachments: None,
        });

        let loaded_specs = collect_product_loaded_deferred_tool_specs(
            &[
                visible_get_tool_spec_result,
                hidden_get_tool_spec_result,
                failed_get_tool_spec_result,
            ],
            &["WebFetch".to_string(), "GetFileDiff".to_string()],
        );

        assert_eq!(loaded_specs, vec![loaded_spec("WebFetch")]);
    }

    #[test]
    fn product_loaded_spec_state_dedupes_and_filters_results() {
        let loaded_specs = collect_product_loaded_deferred_tool_specs(
            &[
                Message::tool_result(ToolResult {
                    tool_id: "tool-1".to_string(),
                    tool_name: "GetToolSpec".to_string(),
                    result: json!({
                            "tool_name": "WebFetch",
                    "catalog_generation": 42,
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
                    "catalog_generation": 42,
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
                    "catalog_generation": 42,
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
                    "catalog_generation": 42,
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
                    "catalog_generation": 42,
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
                    "catalog_generation": 42,
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

        assert_eq!(
            loaded_specs,
            vec![loaded_spec("Git"), loaded_spec("WebFetch")]
        );
    }

    #[test]
    fn product_deferred_loaded_spec_state_preserves_message_derived_lifecycle() {
        let state = ProductLoadedDeferredToolSpecs::from_messages(
            &[Message::tool_result(ToolResult {
                tool_id: "tool-1".to_string(),
                tool_name: "GetToolSpec".to_string(),
                result: json!({
                    "tool_name": "Git",
                "catalog_generation": 42,
                }),
                result_for_assistant: None,
                is_error: false,
                duration_ms: Some(1),
                image_attachments: None,
            })],
            &["Git".to_string(), "WebFetch".to_string()],
        );

        assert!(state.is_loaded("Git"));
        assert!(!state.is_loaded("WebFetch"));
        assert_eq!(state.into_loaded_specs(), vec![loaded_spec("Git")]);
    }
}

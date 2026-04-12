use log::{error, warn};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallBoundary {
    NewTool,
    FinishReason,
    StreamEnd,
    GracefulShutdown,
    EndOfAggregation,
}

impl ToolCallBoundary {
    fn as_str(self) -> &'static str {
        match self {
            Self::NewTool => "new_tool",
            Self::FinishReason => "finish_reason",
            Self::StreamEnd => "stream_end",
            Self::GracefulShutdown => "graceful_shutdown",
            Self::EndOfAggregation => "end_of_aggregation",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PendingToolCall {
    tool_id: String,
    tool_name: String,
    raw_arguments: String,
}

#[derive(Debug, Clone)]
pub struct FinalizedToolCall {
    pub tool_id: String,
    pub tool_name: String,
    pub raw_arguments: String,
    pub arguments: Value,
    pub is_error: bool,
}

impl FinalizedToolCall {
    pub fn arguments_as_object_map(&self) -> HashMap<String, Value> {
        match &self.arguments {
            Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            _ => HashMap::new(),
        }
    }
}

impl PendingToolCall {
    fn remove_trailing_right_brace_once(raw_arguments: &str) -> Option<String> {
        let last_non_whitespace_idx = raw_arguments
            .char_indices()
            .rev()
            .find(|(_, ch)| !ch.is_whitespace())
            .map(|(idx, _)| idx)?;

        if !raw_arguments[last_non_whitespace_idx..].starts_with('}') {
            return None;
        }

        let mut repaired = raw_arguments.to_string();
        repaired.remove(last_non_whitespace_idx);
        Some(repaired)
    }

    fn parse_arguments(raw_arguments: &str) -> Result<Value, String> {
        match serde_json::from_str::<Value>(raw_arguments) {
            Ok(arguments) => Ok(arguments),
            Err(primary_error) => {
                if let Some(repaired_arguments) =
                    Self::remove_trailing_right_brace_once(raw_arguments)
                {
                    match serde_json::from_str::<Value>(&repaired_arguments) {
                        Ok(arguments) => {
                            warn!(
                                "Tool call arguments repaired by removing one trailing right brace"
                            );
                            Ok(arguments)
                        }
                        Err(_) => Err(primary_error.to_string()),
                    }
                } else {
                    Err(primary_error.to_string())
                }
            }
        }
    }

    pub fn has_pending(&self) -> bool {
        !self.tool_id.is_empty()
    }

    pub fn tool_id(&self) -> &str {
        &self.tool_id
    }

    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    pub fn start_new(&mut self, tool_id: String, tool_name: Option<String>) {
        self.tool_id = tool_id;
        self.tool_name = tool_name.unwrap_or_default();
        self.raw_arguments.clear();
    }

    pub fn update_tool_name_if_missing(&mut self, tool_name: Option<String>) {
        if self.tool_name.is_empty() {
            self.tool_name = tool_name.unwrap_or_default();
        }
    }

    pub fn append_arguments(&mut self, arguments_chunk: &str) {
        self.raw_arguments.push_str(arguments_chunk);
    }

    pub fn replace_arguments(&mut self, arguments_snapshot: &str) {
        self.raw_arguments.clear();
        self.raw_arguments.push_str(arguments_snapshot);
    }

    pub fn finalize(&mut self, boundary: ToolCallBoundary) -> Option<FinalizedToolCall> {
        if !self.has_pending() {
            return None;
        }

        let tool_id = std::mem::take(&mut self.tool_id);
        let tool_name = std::mem::take(&mut self.tool_name);
        let raw_arguments = std::mem::take(&mut self.raw_arguments);
        let parsed_arguments = Self::parse_arguments(&raw_arguments);
        let is_error = parsed_arguments.is_err();

        if let Err(error) = &parsed_arguments {
            error!(
                "Tool call arguments parsing failed at boundary={}: tool_id={}, tool_name={}, error={}, raw_arguments={}",
                boundary.as_str(),
                tool_id,
                tool_name,
                error,
                raw_arguments
            );
        }

        Some(FinalizedToolCall {
            tool_id,
            tool_name,
            raw_arguments,
            arguments: parsed_arguments.unwrap_or_else(|_| json!({})),
            is_error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingToolCall, ToolCallBoundary};
    use serde_json::json;

    #[test]
    fn finalizes_complete_json_only_at_boundary() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("tool_a".to_string()));
        pending.append_arguments("{\"a\":1}");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.tool_id, "call_1");
        assert_eq!(finalized.tool_name, "tool_a");
        assert_eq!(finalized.arguments, json!({"a": 1}));
        assert!(!finalized.is_error);
        assert!(!pending.has_pending());
    }

    #[test]
    fn invalid_json_becomes_error_with_empty_object() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("tool_a".to_string()));
        pending.append_arguments("{\"a\":");

        let finalized = pending
            .finalize(ToolCallBoundary::StreamEnd)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({}));
        assert!(finalized.is_error);
    }

    #[test]
    fn repairs_json_with_one_extra_trailing_right_brace() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("tool_a".to_string()));
        pending.append_arguments("{\"a\":1}}");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.raw_arguments, "{\"a\":1}}");
        assert_eq!(finalized.arguments, json!({"a": 1}));
        assert!(!finalized.is_error);
    }

    #[test]
    fn arguments_as_object_map_returns_hash_map_for_objects() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("tool_a".to_string()));
        pending.append_arguments("{\"a\":1,\"b\":\"x\"}");

        let finalized = pending
            .finalize(ToolCallBoundary::EndOfAggregation)
            .expect("finalized tool");
        let map = finalized.arguments_as_object_map();

        assert_eq!(map.get("a"), Some(&json!(1)));
        assert_eq!(map.get("b"), Some(&json!("x")));
    }

    #[test]
    fn replace_arguments_overwrites_partial_buffer() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("tool_a".to_string()));
        pending.append_arguments("{\"city\":\"Bei");
        pending.replace_arguments("{\"city\":\"Beijing\"}");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({"city": "Beijing"}));
        assert!(!finalized.is_error);
    }
}

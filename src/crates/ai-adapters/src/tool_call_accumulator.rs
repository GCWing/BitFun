use log::error;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolCallStreamKey {
    Indexed(usize),
    Unindexed,
}

impl From<Option<usize>> for ToolCallStreamKey {
    fn from(value: Option<usize>) -> Self {
        match value {
            Some(index) => Self::Indexed(index),
            None => Self::Unindexed,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PendingToolCall {
    tool_id: String,
    tool_name: String,
    raw_arguments: String,
    early_detected_emitted: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarlyDetectedToolCall {
    pub tool_id: String,
    pub tool_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallParamsChunk {
    pub tool_id: String,
    pub tool_name: String,
    pub params_chunk: String,
}

#[derive(Debug, Clone, Default)]
pub struct ToolCallDeltaOutcome {
    pub finalized_previous: Option<FinalizedToolCall>,
    pub early_detected: Option<EarlyDetectedToolCall>,
    pub params_partial: Option<ToolCallParamsChunk>,
}

#[derive(Debug, Clone, Default)]
pub struct PendingToolCalls {
    pending: BTreeMap<ToolCallStreamKey, PendingToolCall>,
}

impl PendingToolCall {
    fn parse_arguments(_tool_name: &str, raw_arguments: &str) -> Result<Value, String> {
        serde_json::from_str::<Value>(raw_arguments).map_err(|error| error.to_string())
    }

    pub fn has_pending(&self) -> bool {
        !self.tool_id.is_empty()
    }

    pub fn has_meaningful_payload(&self) -> bool {
        !self.tool_name.is_empty() || !self.raw_arguments.is_empty()
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
        self.early_detected_emitted = false;
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

        if !self.has_meaningful_payload() {
            self.tool_id.clear();
            self.tool_name.clear();
            self.raw_arguments.clear();
            self.early_detected_emitted = false;
            return None;
        }

        let tool_id = std::mem::take(&mut self.tool_id);
        let tool_name = std::mem::take(&mut self.tool_name);
        let raw_arguments = std::mem::take(&mut self.raw_arguments);
        self.early_detected_emitted = false;
        let parsed_arguments = Self::parse_arguments(&tool_name, &raw_arguments);
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

impl PendingToolCalls {
    pub fn apply_delta(
        &mut self,
        key: ToolCallStreamKey,
        tool_id: Option<String>,
        tool_name: Option<String>,
        arguments: Option<String>,
        arguments_is_snapshot: bool,
    ) -> ToolCallDeltaOutcome {
        let mut outcome = ToolCallDeltaOutcome::default();

        let has_tool_id = tool_id.as_ref().is_some_and(|tool_id| !tool_id.is_empty());
        if !self.pending.contains_key(&key) {
            if has_tool_id {
                self.pending.insert(key.clone(), PendingToolCall::default());
            } else {
                return outcome;
            }
        }

        let Some(pending) = self.pending.get_mut(&key) else {
            return outcome;
        };

        if let Some(tool_id) = tool_id.filter(|tool_id| !tool_id.is_empty()) {
            let is_new_tool = pending.tool_id() != tool_id;
            if is_new_tool {
                outcome.finalized_previous = pending.finalize(ToolCallBoundary::NewTool);
                pending.start_new(tool_id, tool_name.clone());
            } else {
                pending.update_tool_name_if_missing(tool_name.clone());
            }
        } else if tool_name
            .as_ref()
            .is_some_and(|tool_name| !tool_name.is_empty())
        {
            pending.update_tool_name_if_missing(tool_name.clone());
        }

        if pending.has_pending()
            && !pending.tool_name().is_empty()
            && !pending.early_detected_emitted
        {
            pending.early_detected_emitted = true;
            outcome.early_detected = Some(EarlyDetectedToolCall {
                tool_id: pending.tool_id().to_string(),
                tool_name: pending.tool_name().to_string(),
            });
        }

        if let Some(arguments) = arguments.filter(|arguments| !arguments.is_empty()) {
            if pending.has_pending() {
                if arguments_is_snapshot {
                    pending.replace_arguments(&arguments);
                } else {
                    pending.append_arguments(&arguments);
                }
                outcome.params_partial = Some(ToolCallParamsChunk {
                    tool_id: pending.tool_id().to_string(),
                    tool_name: pending.tool_name().to_string(),
                    params_chunk: arguments,
                });
            }
        }

        outcome
    }

    pub fn finalize_key(
        &mut self,
        key: &ToolCallStreamKey,
        boundary: ToolCallBoundary,
    ) -> Option<FinalizedToolCall> {
        let mut pending = self.pending.remove(key)?;
        pending.finalize(boundary)
    }

    pub fn finalize_all(&mut self, boundary: ToolCallBoundary) -> Vec<FinalizedToolCall> {
        let keys: Vec<_> = self.pending.keys().cloned().collect();
        keys.into_iter()
            .filter_map(|key| self.finalize_key(&key, boundary))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EarlyDetectedToolCall, PendingToolCall, PendingToolCalls, ToolCallBoundary,
        ToolCallParamsChunk, ToolCallStreamKey,
    };
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
    fn git_raw_command_arguments_become_invalid_json_diagnostic() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Git".to_string()));
        pending.append_arguments("git status");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.raw_arguments, "git status");
        assert_eq!(finalized.arguments, json!({}));
        assert!(finalized.is_error);
    }

    #[test]
    fn git_json_string_command_arguments_are_not_rewritten() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Git".to_string()));
        pending.append_arguments("\"git diff --staged\"");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!("git diff --staged"));
        assert!(!finalized.is_error);
    }

    #[test]
    fn git_args_only_object_is_left_for_tool_schema_diagnostic() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Git".to_string()));
        pending.append_arguments("{\"args\": \"--since=\\\"2026-05-02\\\" --oneline\"}");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(
            finalized.arguments,
            json!({"args": "--since=\"2026-05-02\" --oneline"})
        );
        assert!(!finalized.is_error);
    }

    #[test]
    fn git_duplicate_subcommand_in_args_is_left_for_tool_schema_diagnostic() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Git".to_string()));
        pending.append_arguments("{\"args\": \"log --oneline -10\"}");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({"args": "log --oneline -10"}));
        assert!(!finalized.is_error);
    }

    #[test]
    fn does_not_infer_git_operation_from_ambiguous_args_only_object() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Git".to_string()));
        pending.append_arguments("{\"args\": \"--stat\"}");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({"args": "--stat"}));
        assert!(!finalized.is_error);
    }

    #[test]
    fn raw_string_arguments_for_single_field_tools_stay_invalid_json() {
        let cases = [
            ("Bash", "pnpm test"),
            ("Skill", "openai-docs"),
            ("Read", "src/main.rs"),
            ("GetFileDiff", "src/lib.rs"),
            ("LS", "src/crates"),
            ("Delete", "tmp/output.log"),
            ("Glob", "**/*.rs"),
            ("Grep", "Arguments are invalid JSON"),
            ("WebSearch", "OpenAI Agents SDK"),
            ("WebFetch", "https://example.com"),
            ("InitMiniApp", "Markdown Viewer"),
        ];

        for (tool_name, raw_arguments) in cases {
            let mut pending = PendingToolCall::default();
            pending.start_new("call_1".to_string(), Some(tool_name.to_string()));
            pending.append_arguments(raw_arguments);

            let finalized = pending
                .finalize(ToolCallBoundary::FinishReason)
                .expect("finalized tool");

            assert_eq!(finalized.arguments, json!({}), "tool={tool_name}");
            assert!(finalized.is_error, "tool={tool_name}");
        }
    }

    #[test]
    fn incomplete_json_object_for_single_field_tools_stays_invalid() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Bash".to_string()));
        pending.append_arguments(
            "{\"command\": \"git log --since=\\\"2026-05-02\\\" --oneline --stat",
        );

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({}));
        assert!(finalized.is_error);
    }

    #[test]
    fn does_not_wrap_incomplete_json_object_as_raw_string_argument() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Bash".to_string()));
        pending.append_arguments("{\"command\": ");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({}));
        assert!(finalized.is_error);
    }

    #[test]
    fn does_not_repair_incomplete_json_object_for_multifield_tools() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Task".to_string()));
        pending.append_arguments(
            "{\"description\":\"Explore BitFun project structure\",\"prompt\":\"read README\\n\\nthoroughness: very",
        );

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({}));
        assert!(finalized.is_error);
    }

    #[test]
    fn does_not_repair_object_without_key_value_payload() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Bash".to_string()));
        pending.append_arguments("{");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({}));
        assert!(finalized.is_error);
    }

    #[test]
    fn does_not_execute_truncated_incomplete_json_object() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Bash".to_string()));
        pending.append_arguments("{\"command\": \"git log --since=\\\"2026-05-02\\\" --on");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({}));
        assert!(finalized.is_error);
    }

    #[test]
    fn json_string_arguments_for_single_field_tools_are_schema_errors_not_rewritten() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Bash".to_string()));
        pending.append_arguments("\"git status\"");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!("git status"));
        assert!(!finalized.is_error);
    }

    #[test]
    fn fenced_raw_arguments_for_single_field_tools_stay_invalid_json() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Bash".to_string()));
        pending.append_arguments("```bash\npnpm run lint:web\n```");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({}));
        assert!(finalized.is_error);
    }

    #[test]
    fn does_not_repair_raw_string_arguments_for_multifield_tools() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Edit".to_string()));
        pending.append_arguments("src/main.rs");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({}));
        assert!(finalized.is_error);
    }

    #[test]
    fn json_with_one_extra_trailing_right_brace_stays_invalid() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("tool_a".to_string()));
        pending.append_arguments("{\"a\":1}}");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.raw_arguments, "{\"a\":1}}");
        assert_eq!(finalized.arguments, json!({}));
        assert!(finalized.is_error);
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

    #[test]
    fn manages_multiple_pending_tool_calls_by_index() {
        let mut pending = PendingToolCalls::default();

        assert_eq!(
            pending
                .apply_delta(
                    ToolCallStreamKey::Indexed(0),
                    Some("call_1".to_string()),
                    Some("tool_a".to_string()),
                    None,
                    false,
                )
                .early_detected,
            Some(EarlyDetectedToolCall {
                tool_id: "call_1".to_string(),
                tool_name: "tool_a".to_string(),
            })
        );
        assert_eq!(
            pending
                .apply_delta(
                    ToolCallStreamKey::Indexed(1),
                    Some("call_2".to_string()),
                    Some("tool_b".to_string()),
                    None,
                    false,
                )
                .early_detected,
            Some(EarlyDetectedToolCall {
                tool_id: "call_2".to_string(),
                tool_name: "tool_b".to_string(),
            })
        );

        pending.apply_delta(
            ToolCallStreamKey::Indexed(0),
            None,
            None,
            Some("{\"a\":1}".to_string()),
            false,
        );
        pending.apply_delta(
            ToolCallStreamKey::Indexed(1),
            None,
            None,
            Some("{\"b\":2}".to_string()),
            false,
        );

        let finalized = pending.finalize_all(ToolCallBoundary::FinishReason);
        assert_eq!(finalized.len(), 2);
        assert_eq!(finalized[0].tool_id, "call_1");
        assert_eq!(finalized[0].arguments, json!({"a": 1}));
        assert_eq!(finalized[1].tool_id, "call_2");
        assert_eq!(finalized[1].arguments, json!({"b": 2}));
    }

    #[test]
    fn id_only_prelude_is_attached_to_following_payload_without_id() {
        let mut pending = PendingToolCalls::default();

        let prelude = pending.apply_delta(
            ToolCallStreamKey::Indexed(0),
            Some("call_1".to_string()),
            None,
            None,
            false,
        );
        assert_eq!(prelude.early_detected, None);
        assert_eq!(prelude.params_partial, None);

        let payload = pending.apply_delta(
            ToolCallStreamKey::Indexed(0),
            None,
            Some("tool_a".to_string()),
            Some("{\"a\":1}".to_string()),
            false,
        );
        assert_eq!(
            payload.early_detected,
            Some(EarlyDetectedToolCall {
                tool_id: "call_1".to_string(),
                tool_name: "tool_a".to_string(),
            })
        );
        assert_eq!(
            payload.params_partial,
            Some(ToolCallParamsChunk {
                tool_id: "call_1".to_string(),
                tool_name: "tool_a".to_string(),
                params_chunk: "{\"a\":1}".to_string(),
            })
        );
    }

    #[test]
    fn id_only_orphan_is_dropped_on_finalize() {
        let mut pending = PendingToolCalls::default();

        let outcome = pending.apply_delta(
            ToolCallStreamKey::Indexed(1),
            Some("call_orphan".to_string()),
            None,
            None,
            false,
        );
        assert!(outcome.finalized_previous.is_none());
        assert!(outcome.early_detected.is_none());
        assert!(outcome.params_partial.is_none());
        assert!(pending
            .finalize_all(ToolCallBoundary::FinishReason)
            .is_empty());
    }

    #[test]
    fn empty_argument_delta_is_ignored() {
        let mut pending = PendingToolCalls::default();

        let header = pending.apply_delta(
            ToolCallStreamKey::Indexed(0),
            Some("call_1".to_string()),
            Some("tool_a".to_string()),
            Some(String::new()),
            false,
        );
        assert_eq!(
            header.early_detected,
            Some(EarlyDetectedToolCall {
                tool_id: "call_1".to_string(),
                tool_name: "tool_a".to_string(),
            })
        );
        assert!(header.params_partial.is_none());

        let empty_delta = pending.apply_delta(
            ToolCallStreamKey::Indexed(0),
            None,
            None,
            Some(String::new()),
            false,
        );
        assert!(empty_delta.finalized_previous.is_none());
        assert!(empty_delta.early_detected.is_none());
        assert!(empty_delta.params_partial.is_none());
    }
}

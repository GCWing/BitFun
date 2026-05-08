use log::{error, warn};
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
    const SINGLE_STRING_ARGUMENT_TOOLS: &[(&str, &str)] = &[
        ("Bash", "command"),
        ("Skill", "command"),
        ("Read", "file_path"),
        ("GetFileDiff", "file_path"),
        ("LS", "path"),
        ("Delete", "path"),
        ("Glob", "pattern"),
        ("Grep", "pattern"),
        ("WebSearch", "query"),
        ("WebFetch", "url"),
        ("InitMiniApp", "name"),
    ];

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

    fn strip_argument_wrapping(raw_arguments: &str) -> &str {
        let trimmed = raw_arguments.trim();
        let Some(stripped) = trimmed
            .strip_prefix("```")
            .and_then(|value| value.strip_suffix("```"))
        else {
            return trimmed.trim_matches('`').trim();
        };

        let stripped = stripped.trim();
        if let Some((first_line, rest)) = stripped.split_once('\n') {
            if first_line
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
            {
                return rest.trim();
            }
        }

        stripped
    }

    fn single_string_argument_field(tool_name: &str) -> Option<&'static str> {
        Self::SINGLE_STRING_ARGUMENT_TOOLS
            .iter()
            .find_map(|(name, field)| (*name == tool_name).then_some(*field))
    }

    fn repair_single_string_arguments(tool_name: &str, raw_arguments: &str) -> Option<Value> {
        let field = Self::single_string_argument_field(tool_name)?;
        let raw = Self::strip_argument_wrapping(raw_arguments);
        if raw.is_empty() {
            return None;
        }
        Some(json!({ field: raw }))
    }

    /// Best-effort repair for a Git tool call whose `arguments` came back as a
    /// raw shell-style command (e.g. `git status`, `"git diff --staged"`).
    ///
    /// We deliberately do NOT enforce a subcommand whitelist here — the Git
    /// tool itself owns the allow-list and produces a clear "operation X is
    /// not allowed" error when it sees something unexpected. Replicating that
    /// list at the accumulator layer used to silently fall through to the raw
    /// JSON parser, which made the model receive a generic "Arguments are
    /// invalid JSON" instead of the actionable Git-level error.
    ///
    /// We still require the subcommand to look like a plain identifier
    /// (alphanumerics + `-` + `_`) so we don't mistake unrelated payloads for
    /// Git commands.
    fn parse_git_command_arguments(raw_arguments: &str) -> Option<Value> {
        let trimmed = Self::strip_argument_wrapping(raw_arguments);
        let command = trimmed
            .strip_prefix("git ")
            .map(str::trim)
            .unwrap_or(trimmed);
        let mut parts = command.splitn(2, char::is_whitespace);
        let operation = parts.next()?.trim();
        if operation.is_empty()
            || !operation
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            return None;
        }

        let args = parts.next().map(str::trim).filter(|args| !args.is_empty());
        let mut value = json!({ "operation": operation });
        if let Some(args) = args {
            value["args"] = json!(args);
        }
        Some(value)
    }

    fn normalize_tool_arguments(tool_name: &str, arguments: Value) -> Value {
        if let Value::String(raw) = &arguments {
            if tool_name == "Git" {
                if let Some(repaired) = Self::parse_git_command_arguments(raw) {
                    warn!("Git tool call arguments repaired from JSON string command");
                    return repaired;
                }
            }
            if let Some(repaired) = Self::repair_single_string_arguments(tool_name, raw) {
                warn!(
                    "{} tool call arguments repaired from JSON string argument",
                    tool_name
                );
                return repaired;
            }
        }
        arguments
    }

    fn parse_arguments(tool_name: &str, raw_arguments: &str) -> Result<Value, String> {
        match serde_json::from_str::<Value>(raw_arguments) {
            Ok(arguments) => Ok(Self::normalize_tool_arguments(tool_name, arguments)),
            Err(primary_error) => {
                if tool_name == "Git" {
                    if let Some(arguments) = Self::parse_git_command_arguments(raw_arguments) {
                        warn!("Git tool call arguments repaired from raw command");
                        return Ok(arguments);
                    }
                }

                if let Some(arguments) =
                    Self::repair_single_string_arguments(tool_name, raw_arguments)
                {
                    warn!(
                        "{} tool call arguments repaired from raw string argument",
                        tool_name
                    );
                    return Ok(arguments);
                }

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
    fn repairs_git_raw_command_arguments() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Git".to_string()));
        pending.append_arguments("git status");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({"operation": "status"}));
        assert!(!finalized.is_error);
    }

    #[test]
    fn repairs_git_json_string_command_arguments() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Git".to_string()));
        pending.append_arguments("\"git diff --staged\"");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(
            finalized.arguments,
            json!({"operation": "diff", "args": "--staged"})
        );
        assert!(!finalized.is_error);
    }

    #[test]
    fn repairs_raw_string_arguments_for_single_field_tools() {
        let cases = [
            ("Bash", "pnpm test", json!({"command": "pnpm test"})),
            ("Skill", "openai-docs", json!({"command": "openai-docs"})),
            ("Read", "src/main.rs", json!({"file_path": "src/main.rs"})),
            (
                "GetFileDiff",
                "src/lib.rs",
                json!({"file_path": "src/lib.rs"}),
            ),
            ("LS", "src/crates", json!({"path": "src/crates"})),
            (
                "Delete",
                "tmp/output.log",
                json!({"path": "tmp/output.log"}),
            ),
            ("Glob", "**/*.rs", json!({"pattern": "**/*.rs"})),
            (
                "Grep",
                "Arguments are invalid JSON",
                json!({"pattern": "Arguments are invalid JSON"}),
            ),
            (
                "WebSearch",
                "OpenAI Agents SDK",
                json!({"query": "OpenAI Agents SDK"}),
            ),
            (
                "WebFetch",
                "https://example.com",
                json!({"url": "https://example.com"}),
            ),
            (
                "InitMiniApp",
                "Markdown Viewer",
                json!({"name": "Markdown Viewer"}),
            ),
        ];

        for (tool_name, raw_arguments, expected) in cases {
            let mut pending = PendingToolCall::default();
            pending.start_new("call_1".to_string(), Some(tool_name.to_string()));
            pending.append_arguments(raw_arguments);

            let finalized = pending
                .finalize(ToolCallBoundary::FinishReason)
                .expect("finalized tool");

            assert_eq!(finalized.arguments, expected, "tool={tool_name}");
            assert!(!finalized.is_error, "tool={tool_name}");
        }
    }

    #[test]
    fn repairs_json_string_arguments_for_single_field_tools() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Bash".to_string()));
        pending.append_arguments("\"git status\"");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({"command": "git status"}));
        assert!(!finalized.is_error);
    }

    #[test]
    fn repairs_fenced_raw_arguments_for_single_field_tools() {
        let mut pending = PendingToolCall::default();
        pending.start_new("call_1".to_string(), Some("Bash".to_string()));
        pending.append_arguments("```bash\npnpm run lint:web\n```");

        let finalized = pending
            .finalize(ToolCallBoundary::FinishReason)
            .expect("finalized tool");

        assert_eq!(finalized.arguments, json!({"command": "pnpm run lint:web"}));
        assert!(!finalized.is_error);
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

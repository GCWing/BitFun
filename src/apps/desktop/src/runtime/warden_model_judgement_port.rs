//! Desktop implementation of the Warden model judgement port.
//!
//! Bridges `bitfun_runtime_ports::WardenModelJudgementPort` to a real model
//! call through the desktop `AIClientFactory` (fast model). The judgement
//! prompt embeds the candidate rule ids and the evidence summary; the model
//! response is parsed as JSON into `WardenAuditJudgementResponse`. Any model
//! failure, parse failure, or timeout returns `Err` so the audit caller falls
//! back to the mechanical rule ladder — the judgement port must never block
//! the audit loop on a broken model response.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bitfun_core::infrastructure::ai::AIClientFactory;
use bitfun_core_types::Message;
use bitfun_runtime_ports::{
    PortError, PortErrorKind, PortResult, WardenAuditJudgementRequest,
    WardenAuditJudgementResponse, WardenModelJudgementPort,
};

/// Time budget for one judgement model call.
///
/// WARDEN-02: reduced from 30s so a model judgement cannot block an agent
/// turn for a long round-trip; on timeout the caller falls back to the
/// mechanical rule ladder (the audit loop never depends on the model).
const WARDEN_JUDGEMENT_TIMEOUT: Duration = Duration::from_secs(8);

/// Upper bound for the serialized tool-args summary embedded in the prompt.
///
/// Defense-in-depth behind the core-side tool-args summarization (WARDEN-08):
/// a summarized args value that still exceeds this is replaced by a length
/// marker so the prompt budget stays bounded.
const WARDEN_JUDGEMENT_PROMPT_ARGS_MAX_CHARS: usize = 2048;

/// System prompt instructing the model to emit only the judgement JSON.
///
/// WARDEN-03: this prompt must not hard-code the "first failure of a scene is
/// exploratory and must not poke" rule — that is the runtime's counting
/// semantics, and the evidence passed in already reflects it (the caller
/// supplies the consecutive failure count in `evidence`). The model judges
/// strictly from the provided tool facts and the evidence field; asking it to
/// re-derive exploratory status would make the verdict depend on a rule the
/// model can only guess at.
const WARDEN_JUDGEMENT_SYSTEM_PROMPT: &str = "You are the Warden audit judgement engine \
of an AI agent host. Given one finished agent action (tool call or turn) and a \
list of candidate discipline rules, decide whether the agent deserves a poke \
reminder. Judge strictly from the provided tool facts: the toolName and \
toolArgs of the action, and the evidence field, which carries the failure \
context (consecutive failure count and the last error summary when \
available). A poke is warranted when the evidence shows a repeated failure of \
the same kind; do not infer exploratory status or first-failure rules that the \
evidence does not state. Respond with a single JSON object of the shape \
{\"shouldPoke\": bool, \"ruleIds\": [string], \"evidenceRequested\": [string]}. \
Do not include any text outside the JSON object.";

/// Desktop implementation of [`WardenModelJudgementPort`] over the global AI
/// client factory.
pub(crate) struct DesktopWardenModelJudgementPort {
    ai_client_factory: Arc<AIClientFactory>,
}

impl std::fmt::Debug for DesktopWardenModelJudgementPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopWardenModelJudgementPort")
            .field("ai_client_factory", &"<AIClientFactory>")
            .finish()
    }
}

impl DesktopWardenModelJudgementPort {
    pub(crate) fn new(ai_client_factory: Arc<AIClientFactory>) -> Self {
        Self { ai_client_factory }
    }

    /// Build the user prompt embedding every judgement input.
    fn judgement_prompt(request: &WardenAuditJudgementRequest) -> String {
        let tool_args = request
            .tool_args
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok())
            .map(|serialized| {
                if serialized.len() > WARDEN_JUDGEMENT_PROMPT_ARGS_MAX_CHARS {
                    format!("{{ \"summaryLength\": {} }}", serialized.len())
                } else {
                    serialized
                }
            })
            .unwrap_or_else(|| "null".to_string());
        let evidence = request
            .evidence
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok())
            .unwrap_or_else(|| "null".to_string());
        format!(
            "sessionId: {}\ntoolName: {}\ntoolArgs: {}\ncandidateRuleIds: {}\nevidence: {}",
            request.session_id,
            request.tool_name,
            tool_args,
            request.rule_ids.join(", "),
            evidence
        )
    }

    /// Parse a model judgement response into
    /// [`WardenAuditJudgementResponse`].
    ///
    /// WARDEN-07: a ```` ```json ```` fence around the JSON is stripped before
    /// parsing, and the verdict is parsed strictly — a missing or non-boolean
    /// `shouldPoke` is an error, never a silent default of `false` that would
    /// suppress a poke. Any error here makes the caller fall back to the
    /// mechanical rule ladder.
    fn parse_judgement_response(text: &str) -> PortResult<WardenAuditJudgementResponse> {
        let text = text.trim();
        if text.is_empty() {
            return Err(PortError::new(
                PortErrorKind::Backend,
                "warden judgement model returned an empty response",
            ));
        }
        let stripped = strip_json_fence(text);
        let json: serde_json::Value = serde_json::from_str(stripped).map_err(|error| {
            PortError::new(
                PortErrorKind::Backend,
                format!("warden judgement response is not valid JSON: {error}"),
            )
        })?;
        match json.get("shouldPoke") {
            Some(serde_json::Value::Bool(_)) => {}
            _ => {
                return Err(PortError::new(
                    PortErrorKind::Backend,
                    "warden judgement response is missing a boolean \"shouldPoke\" field",
                ));
            }
        }
        serde_json::from_value(json).map_err(|error| {
            PortError::new(
                PortErrorKind::Backend,
                format!("warden judgement response does not match the expected shape: {error}"),
            )
        })
    }
}

/// Strip a ```` ```json ```` or ```` ``` ```` fence around the model response.
///
/// A model that wraps the JSON in markdown fences still parses; a plain
/// response is returned unchanged.
fn strip_json_fence(text: &str) -> &str {
    let trimmed = text.trim();
    let body = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim();
    body.strip_suffix("```").unwrap_or(body).trim()
}

#[async_trait]
impl WardenModelJudgementPort for DesktopWardenModelJudgementPort {
    async fn judge_audit(
        &self,
        request: WardenAuditJudgementRequest,
    ) -> PortResult<WardenAuditJudgementResponse> {
        let client = self
            .ai_client_factory
            .get_client_resolved("fast")
            .await
            .map_err(|error| {
                PortError::new(
                    PortErrorKind::Backend,
                    format!("failed to resolve warden judgement model: {error}"),
                )
            })?;

        let messages = vec![
            Message::system(WARDEN_JUDGEMENT_SYSTEM_PROMPT.to_string()),
            Message::user(Self::judgement_prompt(&request)),
        ];

        let response = tokio::time::timeout(
            WARDEN_JUDGEMENT_TIMEOUT,
            client.send_message(messages, None),
        )
        .await
        .map_err(|_| {
            PortError::new(
                PortErrorKind::Timeout,
                "warden judgement timed out; caller falls back to mechanical rules",
            )
        })?
        .map_err(|error| {
            PortError::new(
                PortErrorKind::Backend,
                format!("warden judgement model call failed: {error}"),
            )
        })?;

        Self::parse_judgement_response(&response.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judgement_prompt_embeds_all_inputs() {
        let request = WardenAuditJudgementRequest {
            session_id: "sess-1".to_string(),
            tool_name: "ExecCommand".to_string(),
            tool_args: Some(serde_json::json!({"cmd": "pwd"})),
            rule_ids: vec!["iron-rules-compliance".to_string()],
            evidence: Some(serde_json::json!({"consecutiveFailures": 2})),
        };
        let prompt = DesktopWardenModelJudgementPort::judgement_prompt(&request);
        assert!(prompt.contains("sess-1"));
        assert!(prompt.contains("ExecCommand"));
        assert!(prompt.contains("pwd"));
        assert!(prompt.contains("iron-rules-compliance"));
        assert!(prompt.contains("consecutiveFailures"));
    }

    #[test]
    fn judgement_prompt_handles_missing_optional_inputs() {
        let request = WardenAuditJudgementRequest {
            session_id: "sess-2".to_string(),
            tool_name: "Read".to_string(),
            tool_args: None,
            rule_ids: Vec::new(),
            evidence: None,
        };
        let prompt = DesktopWardenModelJudgementPort::judgement_prompt(&request);
        assert!(prompt.contains("toolName: Read"));
        assert!(prompt.contains("toolArgs: null"));
        assert!(prompt.contains("candidateRuleIds: "));
        assert!(prompt.contains("evidence: null"));
    }

    #[test]
    fn judgement_prompt_caps_oversized_tool_args_summary() {
        // WARDEN-08 (desktop defense-in-depth): an oversized tool-args summary
        // is replaced by a length marker instead of bloating the prompt.
        let request = WardenAuditJudgementRequest {
            session_id: "sess-3".to_string(),
            tool_name: "Write".to_string(),
            tool_args: Some(serde_json::json!({ "data": "x".repeat(4096) })),
            rule_ids: Vec::new(),
            evidence: None,
        };
        let prompt = DesktopWardenModelJudgementPort::judgement_prompt(&request);
        assert!(prompt.contains("summaryLength"), "oversized args are capped");
        assert!(
            !prompt.contains(&"x".repeat(1024)),
            "bulk payload must not reach the prompt"
        );
    }

    #[test]
    fn parse_judgement_response_accepts_fenced_json() {
        // WARDEN-07: a ```json fence around the verdict is stripped and parsed.
        let verdict = r#"```json
        {"shouldPoke": true, "ruleIds": ["R2: execution_safety"], "evidenceRequested": ["tool_call_log"]}
        ```"#;
        let parsed = DesktopWardenModelJudgementPort::parse_judgement_response(verdict)
            .expect("fenced JSON parses");
        assert!(parsed.should_poke);
        assert_eq!(parsed.rule_ids, vec!["R2: execution_safety"]);
        assert_eq!(parsed.evidence_requested, vec!["tool_call_log"]);
    }

    #[test]
    fn parse_judgement_response_rejects_empty_and_missing_should_poke() {
        // WARDEN-07: empty responses and verdicts missing a boolean
        // shouldPoke are errors so the caller falls back to mechanical rules
        // instead of silently suppressing the poke.
        let empty = DesktopWardenModelJudgementPort::parse_judgement_response("   ");
        assert!(empty.is_err(), "empty response is a parse error");

        let empty_object =
            DesktopWardenModelJudgementPort::parse_judgement_response("{}");
        assert!(
            empty_object.is_err(),
            "an empty object must not default shouldPoke to false"
        );

        let missing_field = DesktopWardenModelJudgementPort::parse_judgement_response(
            r#"{"ruleIds": ["R1"]}"#,
        );
        assert!(
            missing_field.is_err(),
            "a missing shouldPoke must not default to false"
        );

        let wrong_type = DesktopWardenModelJudgementPort::parse_judgement_response(
            r#"{"shouldPoke": "yes"}"#,
        );
        assert!(
            wrong_type.is_err(),
            "a non-boolean shouldPoke is not a valid verdict"
        );
    }

    #[test]
    fn parse_judgement_response_accepts_plain_verdict_with_defaults() {
        // A bare `shouldPoke` verdict parses; absent rule/evidence lists
        // default to empty (which resolve_audit_poke_from_judgement fills
        // from the mechanical candidates).
        let parsed = DesktopWardenModelJudgementPort::parse_judgement_response(
            r#"{"shouldPoke": false}"#,
        )
        .expect("bare verdict parses");
        assert!(!parsed.should_poke);
        assert!(parsed.rule_ids.is_empty());
        assert!(parsed.evidence_requested.is_empty());
    }
}

//! Per-round execution-model selection contracts.
//!
//! It exposes a narrow execution-engine hook plus an opt-in HTTP implementation while
//! keeping the existing agent loop, retry lifecycle, and tool pipeline authoritative.

use crate::agentic::core::{Message, MessageContent, MessageRole, MessageSemanticKind};
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const SIMPLE_TOKEN_ID: u32 = 22_944;
const NON_SIMPLE_FIRST_TOKEN_ID: u32 = 6_280;
const ROUTER_TOP_LOGPROBS: usize = 20;

/// The configured execution-model slot to use for one logical model round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundModelRoute {
    Primary,
    Fast,
}

impl RoundModelRoute {
    pub fn model_selector(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Fast => "fast",
        }
    }
}

/// Immutable input visible to a per-round model router.
///
/// `messages` contains only history available before the round begins. A router must
/// treat it as read-only and must not inject its prediction into the agent transcript.
#[derive(Debug, Clone)]
pub struct RoundModelRouteRequest {
    pub session_id: String,
    pub dialog_turn_id: String,
    pub turn_index: usize,
    pub round_index: usize,
    pub agent_type: String,
    pub original_user_input: String,
    pub messages: Vec<Message>,
}

/// Selects the execution model exactly once for each logical model round.
///
/// Request retries and context-overflow recovery remain inside that logical round and
/// therefore reuse the selected model without calling this hook again.
#[async_trait::async_trait]
pub trait RoundModelRouter: Send + Sync {
    async fn select_model(
        &self,
        request: RoundModelRouteRequest,
    ) -> OpenBitFunResult<RoundModelRoute>;

    /// Record the concrete execution model selected after resolving a route.
    fn record_execution_model(&self, _record: RoundModelExecutionRecord) {}
}

#[derive(Debug, Clone, Serialize)]
pub struct RoundModelExecutionRecord {
    pub session_id: String,
    pub dialog_turn_id: String,
    pub round_index: usize,
    pub route: RoundModelRoute,
    pub model_config_id: String,
    pub effective_model_name: String,
}

#[derive(Debug, Clone)]
pub struct HttpRoundModelRouterConfig {
    pub endpoint: String,
    pub model: String,
    pub system_prompt: String,
    pub api_key: Option<String>,
    pub timeout: Duration,
    pub recent_rounds: usize,
    pub simple_threshold: f64,
    pub trace_path: Option<PathBuf>,
}

impl HttpRoundModelRouterConfig {
    /// Build opt-in configuration from process environment.
    ///
    /// Routing remains disabled unless `OPENBITFUN_ROUND_ROUTER_URL` is set.
    pub fn from_env() -> OpenBitFunResult<Option<Self>> {
        let Some(endpoint) = std::env::var("OPENBITFUN_ROUND_ROUTER_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
            return Err(OpenBitFunError::Configuration(
                "OPENBITFUN_ROUND_ROUTER_URL must be an http(s) URL".to_string(),
            ));
        }

        let prompt_path = std::env::var("OPENBITFUN_ROUND_ROUTER_PROMPT")
            .map(PathBuf::from)
            .map_err(|_| {
                OpenBitFunError::Configuration(
                    "OPENBITFUN_ROUND_ROUTER_PROMPT is required when routing is enabled"
                        .to_string(),
                )
            })?;
        let system_prompt = std::fs::read_to_string(&prompt_path).map_err(|error| {
            OpenBitFunError::Configuration(format!(
                "Failed to read Router system prompt {}: {}",
                prompt_path.display(),
                error
            ))
        })?;
        let timeout_ms = parse_env_usize("OPENBITFUN_ROUND_ROUTER_TIMEOUT_MS", 10_000)?;
        let recent_rounds = parse_env_usize("OPENBITFUN_ROUND_ROUTER_RECENT_ROUNDS", 3)?;
        let simple_threshold = parse_env_f64("OPENBITFUN_ROUND_ROUTER_SIMPLE_THRESHOLD", 0.7)?;
        if recent_rounds == 0 {
            return Err(OpenBitFunError::Configuration(
                "OPENBITFUN_ROUND_ROUTER_RECENT_ROUNDS must be positive".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&simple_threshold) {
            return Err(OpenBitFunError::Configuration(
                "OPENBITFUN_ROUND_ROUTER_SIMPLE_THRESHOLD must be between 0 and 1".to_string(),
            ));
        }

        Ok(Some(Self {
            endpoint,
            model: std::env::var("OPENBITFUN_ROUND_ROUTER_MODEL")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "router-best".to_string()),
            system_prompt,
            api_key: std::env::var("OPENBITFUN_ROUND_ROUTER_API_KEY")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            timeout: Duration::from_millis(timeout_ms as u64),
            recent_rounds,
            simple_threshold,
            trace_path: std::env::var("OPENBITFUN_ROUND_ROUTER_TRACE")
                .ok()
                .map(PathBuf::from),
        }))
    }
}

fn parse_env_usize(name: &str, default: usize) -> OpenBitFunResult<usize> {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|error| OpenBitFunError::Configuration(format!("Invalid {name}: {error}")))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn parse_env_f64(name: &str, default: f64) -> OpenBitFunResult<f64> {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|error| OpenBitFunError::Configuration(format!("Invalid {name}: {error}")))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

pub struct HttpRoundModelRouter {
    config: HttpRoundModelRouterConfig,
    client: reqwest::Client,
}

impl HttpRoundModelRouter {
    pub fn new(config: HttpRoundModelRouterConfig) -> OpenBitFunResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|error| OpenBitFunError::Http(error.to_string()))?;
        Ok(Self { config, client })
    }
}

#[derive(Debug, Serialize)]
struct OpenAiChatRequest<'a> {
    model: &'a str,
    messages: [OpenAiChatMessage<'a>; 2],
    temperature: f32,
    top_p: f32,
    max_tokens: usize,
    stop: [&'a str; 1],
    logprobs: bool,
    top_logprobs: usize,
    return_token_ids: bool,
    return_tokens_as_token_ids: bool,
    chat_template_kwargs: Value,
}

#[derive(Debug, Serialize)]
struct OpenAiChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChatChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatChoice {
    message: OpenAiResponseMessage,
    #[serde(default)]
    logprobs: Option<OpenAiChatLogprobs>,
    #[serde(default)]
    token_ids: Option<Vec<u32>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatLogprobs {
    content: Option<Vec<OpenAiChatLogprobContent>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatLogprobContent {
    bytes: Option<Vec<u8>>,
    top_logprobs: Vec<OpenAiTokenLogprob>,
}

#[derive(Debug, Deserialize)]
struct OpenAiTokenLogprob {
    token: String,
    logprob: f64,
}

#[async_trait::async_trait]
impl RoundModelRouter for HttpRoundModelRouter {
    async fn select_model(
        &self,
        request: RoundModelRouteRequest,
    ) -> OpenBitFunResult<RoundModelRoute> {
        let user_prompt = build_router_user_prompt(&request, self.config.recent_rounds)?;
        let body = OpenAiChatRequest {
            model: &self.config.model,
            messages: [
                OpenAiChatMessage {
                    role: "system",
                    content: &self.config.system_prompt,
                },
                OpenAiChatMessage {
                    role: "user",
                    content: &user_prompt,
                },
            ],
            temperature: 0.0,
            top_p: 1.0,
            max_tokens: 128,
            stop: ["\n"],
            logprobs: true,
            top_logprobs: ROUTER_TOP_LOGPROBS,
            return_token_ids: true,
            return_tokens_as_token_ids: true,
            chat_template_kwargs: json!({"enable_thinking": false}),
        };
        let started_at = Instant::now();
        let mut http_request = self.client.post(&self.config.endpoint).json(&body);
        if let Some(api_key) = self.config.api_key.as_deref() {
            http_request = http_request.bearer_auth(api_key);
        }
        let response = http_request
            .send()
            .await
            .map_err(|error| OpenBitFunError::Http(format!("Router request failed: {error}")))?
            .error_for_status()
            .map_err(|error| OpenBitFunError::Http(format!("Router returned an error: {error}")))?
            .json::<OpenAiChatResponse>()
            .await
            .map_err(|error| {
                OpenBitFunError::Deserialization(format!(
                    "Failed to decode Router response: {error}"
                ))
            })?;
        let choice = response.choices.first().ok_or_else(|| {
            OpenBitFunError::Deserialization("Router response did not contain a choice".to_string())
        })?;
        let raw_output = choice.message.content.as_deref().ok_or_else(|| {
            OpenBitFunError::Deserialization(
                "Router response did not contain assistant content".to_string(),
            )
        })?;
        let (generated_route, recovered) = parse_router_route(raw_output)?;
        let simple_probability = extract_simple_probability(choice, raw_output, generated_route);
        let route = match simple_probability {
            Some(probability) if probability > self.config.simple_threshold => {
                RoundModelRoute::Fast
            }
            _ => RoundModelRoute::Primary,
        };
        if simple_probability.is_none() {
            warn!(
                "Router logprobs did not contain both class tokens; conservatively selecting primary: turn_id={}, round_index={}",
                request.dialog_turn_id, request.round_index
            );
        }
        append_trace_record(
            self.config.trace_path.as_ref(),
            &json!({
                "event": "router_decision",
                "session_id": request.session_id,
                "dialog_turn_id": request.dialog_turn_id,
                "turn_index": request.turn_index,
                "round_index": request.round_index,
                "agent_type": request.agent_type,
                "router_model": self.config.model,
                "router_input": user_prompt,
                "router_output": raw_output,
                "generated_route": generated_route,
                "simple_probability": simple_probability,
                "simple_threshold": self.config.simple_threshold,
                "route": route,
                "recovered": recovered,
                "latency_ms": started_at.elapsed().as_millis(),
            }),
        );
        info!(
            "Router decision completed: session_id={}, turn_id={}, round_index={}, generated_route={:?}, simple_probability={:?}, threshold={}, route={:?}, recovered={}, latency_ms={}",
            request.session_id,
            request.dialog_turn_id,
            request.round_index,
            generated_route,
            simple_probability,
            self.config.simple_threshold,
            route,
            recovered,
            started_at.elapsed().as_millis()
        );
        debug!(
            "Router response protocol status: turn_id={}, round_index={}, output_chars={}",
            request.dialog_turn_id,
            request.round_index,
            raw_output.len()
        );
        Ok(route)
    }

    fn record_execution_model(&self, record: RoundModelExecutionRecord) {
        append_trace_record(
            self.config.trace_path.as_ref(),
            &json!({
                "event": "execution_model_selected",
                "session_id": record.session_id,
                "dialog_turn_id": record.dialog_turn_id,
                "round_index": record.round_index,
                "route": record.route,
                "model_config_id": record.model_config_id,
                "effective_model_name": record.effective_model_name,
            }),
        );
    }
}

fn extract_simple_probability(
    choice: &OpenAiChatChoice,
    raw_output: &str,
    generated_route: RoundModelRoute,
) -> Option<f64> {
    let token_ids = choice.token_ids.as_deref()?;
    let logprobs = choice.logprobs.as_ref()?.content.as_deref()?;
    if token_ids.len() != logprobs.len() {
        return None;
    }

    let generated_label_token = match generated_route {
        RoundModelRoute::Fast => SIMPLE_TOKEN_ID,
        RoundModelRoute::Primary => NON_SIMPLE_FIRST_TOKEN_ID,
    };
    let label_byte_offset = simple_type_value_byte_offset(raw_output)?;
    let label_index = token_index_at_byte_offset(logprobs, label_byte_offset)?;
    if token_ids.get(label_index).copied()? != generated_label_token {
        return None;
    }
    let candidates = &logprobs.get(label_index)?.top_logprobs;
    let simple_logprob = find_token_logprob(candidates, SIMPLE_TOKEN_ID)?;
    let non_simple_logprob = find_token_logprob(candidates, NON_SIMPLE_FIRST_TOKEN_ID)?;

    // Renormalize over the two routing classes only. Log-probabilities differ
    // from raw logits by the same log-softmax constant, so this is equivalent
    // to a two-class softmax over their logits.
    let maximum = simple_logprob.max(non_simple_logprob);
    let simple_weight = (simple_logprob - maximum).exp();
    let non_simple_weight = (non_simple_logprob - maximum).exp();
    Some(simple_weight / (simple_weight + non_simple_weight))
}

fn simple_type_value_byte_offset(raw_output: &str) -> Option<usize> {
    let first_line = raw_output.lines().next()?;
    let key_offset = first_line.find("\"simple_type\"")?;
    let after_key_offset = key_offset + "\"simple_type\"".len();
    let after_key = &first_line[after_key_offset..];
    let colon_offset = after_key.find(':')?;
    let after_colon_offset = after_key_offset + colon_offset + 1;
    let whitespace_bytes = first_line[after_colon_offset..]
        .len()
        .saturating_sub(first_line[after_colon_offset..].trim_start().len());
    let opening_quote_offset = after_colon_offset + whitespace_bytes;
    if first_line.as_bytes().get(opening_quote_offset) != Some(&b'"') {
        return None;
    }
    Some(opening_quote_offset + 1)
}

fn token_index_at_byte_offset(
    logprobs: &[OpenAiChatLogprobContent],
    target_offset: usize,
) -> Option<usize> {
    let mut current_offset: usize = 0;
    for (index, token) in logprobs.iter().enumerate() {
        let token_bytes = token.bytes.as_deref()?;
        let next_offset = current_offset.checked_add(token_bytes.len())?;
        if (current_offset..next_offset).contains(&target_offset) {
            return Some(index);
        }
        current_offset = next_offset;
    }
    None
}

fn find_token_logprob(candidates: &[OpenAiTokenLogprob], token_id: u32) -> Option<f64> {
    let expected = format!("token_id:{token_id}");
    candidates
        .iter()
        .find(|candidate| candidate.token == expected)
        .map(|candidate| candidate.logprob)
}

static TRACE_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn append_trace_record(trace_path: Option<&PathBuf>, record: &Value) {
    let Some(trace_path) = trace_path else {
        return;
    };
    let lock = TRACE_WRITE_LOCK.get_or_init(|| Mutex::new(()));
    let Ok(_guard) = lock.lock() else {
        warn!(
            "Router trace lock is poisoned: path={}",
            trace_path.display()
        );
        return;
    };
    let result = (|| -> std::io::Result<()> {
        if let Some(parent) = trace_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(trace_path)?;
        serde_json::to_writer(&mut file, record).map_err(std::io::Error::other)?;
        file.write_all(b"\n")?;
        Ok(())
    })();
    if let Err(error) = result {
        warn!(
            "Failed to append Router trace: path={}, error={}",
            trace_path.display(),
            error
        );
    }
}

#[derive(Debug, Serialize)]
struct RouterTrajectoryRound {
    round_id: usize,
    assistant: RouterAssistant,
    tool_results: Vec<RouterToolResult>,
}

#[derive(Debug, Serialize)]
struct RouterAssistant {
    reasoning_content: String,
    text: String,
    tool_calls: Vec<RouterToolCall>,
}

#[derive(Debug, Serialize)]
struct RouterToolCall {
    tool_id: String,
    tool_name: String,
    arguments: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_arguments: Option<String>,
    is_error: bool,
}

#[derive(Debug, Serialize)]
struct RouterToolResult {
    #[serde(skip)]
    tool_id: String,
    tool_name: String,
    result_for_assistant: String,
}

fn build_router_user_prompt(
    request: &RoundModelRouteRequest,
    recent_round_count: usize,
) -> OpenBitFunResult<String> {
    let rounds = collect_router_rounds(&request.messages);
    let recent_start = rounds.len().saturating_sub(recent_round_count);
    let recent_rounds = &rounds[recent_start..];
    let existing_summary = request.messages.iter().rev().find_map(|message| {
        if message.metadata.semantic_kind == Some(MessageSemanticKind::CompressionSummary) {
            message_text(message)
        } else {
            None
        }
    });
    let earlier_history = if let Some(summary) = existing_summary {
        summary
    } else if recent_start > 0 {
        format!(
            "Earlier completed rounds: {}",
            serde_json::to_string(&rounds[..recent_start])?
        )
    } else {
        "(none)".to_string()
    };
    let recent_json = serde_json::to_string(recent_rounds)?;
    let task_description = build_task_description(request);
    Ok(format!(
        "## Task\n{}\n\n## Earlier history summary\n{}\n\n## Recent trajectory\n{}",
        task_description, earlier_history, recent_json
    ))
}

fn build_task_description(request: &RoundModelRouteRequest) -> String {
    let mut instructions: Vec<String> = request
        .messages
        .iter()
        .filter(|message| {
            message.role == MessageRole::User
                && message.metadata.semantic_kind == Some(MessageSemanticKind::ActualUserInput)
        })
        .filter_map(message_text)
        .filter(|text| !text.trim().is_empty())
        .collect();
    if !request.original_user_input.trim().is_empty()
        && instructions.last() != Some(&request.original_user_input)
    {
        instructions.push(request.original_user_input.clone());
    }
    if instructions.is_empty() {
        "(none)".to_string()
    } else {
        instructions.join("\n\nUser update:\n")
    }
}

fn push_completed_round(
    rounds: &mut Vec<RouterTrajectoryRound>,
    current: Option<RouterTrajectoryRound>,
) {
    let Some(mut round) = current else {
        return;
    };
    let call_order: std::collections::HashMap<String, usize> = round
        .assistant
        .tool_calls
        .iter()
        .enumerate()
        .map(|(index, call)| (call.tool_id.clone(), index))
        .collect();
    round.tool_results.sort_by_key(|result| {
        call_order
            .get(result.tool_id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    rounds.push(round);
}

fn collect_router_rounds(messages: &[Message]) -> Vec<RouterTrajectoryRound> {
    let mut rounds = Vec::new();
    let mut current: Option<RouterTrajectoryRound> = None;
    for message in messages {
        match (&message.role, &message.content) {
            (MessageRole::Assistant, MessageContent::Text(text)) => {
                push_completed_round(&mut rounds, current.take());
                current = Some(RouterTrajectoryRound {
                    round_id: rounds.len(),
                    assistant: RouterAssistant {
                        reasoning_content: String::new(),
                        text: text.clone(),
                        tool_calls: Vec::new(),
                    },
                    tool_results: Vec::new(),
                });
            }
            (
                MessageRole::Assistant,
                MessageContent::Mixed {
                    reasoning_content,
                    text,
                    tool_calls,
                },
            ) => {
                push_completed_round(&mut rounds, current.take());
                current = Some(RouterTrajectoryRound {
                    round_id: rounds.len(),
                    assistant: RouterAssistant {
                        reasoning_content: reasoning_content.clone().unwrap_or_default(),
                        text: text.clone(),
                        tool_calls: tool_calls
                            .iter()
                            .map(|call| RouterToolCall {
                                tool_id: call.tool_id.clone(),
                                tool_name: call.tool_name.clone(),
                                arguments: call.arguments.clone(),
                                raw_arguments: call.raw_arguments.clone(),
                                is_error: call.is_error,
                            })
                            .collect(),
                    },
                    tool_results: Vec::new(),
                });
            }
            (
                MessageRole::Tool,
                MessageContent::ToolResult {
                    tool_id,
                    tool_name,
                    effective_tool_name,
                    result,
                    result_for_assistant,
                    ..
                },
            ) => {
                if let Some(round) = current.as_mut() {
                    round.tool_results.push(RouterToolResult {
                        tool_id: tool_id.clone(),
                        tool_name: effective_tool_name
                            .clone()
                            .unwrap_or_else(|| tool_name.clone()),
                        result_for_assistant: result_for_assistant
                            .clone()
                            .unwrap_or_else(|| result.to_string()),
                    });
                }
            }
            _ => {}
        }
    }
    push_completed_round(&mut rounds, current);
    rounds
}

fn message_text(message: &Message) -> Option<String> {
    match &message.content {
        MessageContent::Text(text) => Some(text.clone()),
        MessageContent::Multimodal { text, .. } => Some(text.clone()),
        MessageContent::Mixed { text, .. } => Some(text.clone()),
        MessageContent::ToolResult { .. } => None,
    }
}

fn parse_router_route(raw_output: &str) -> OpenBitFunResult<(RoundModelRoute, bool)> {
    let first_line = raw_output.lines().next().unwrap_or_default().trim();
    if first_line.is_empty() || !first_line.starts_with('{') {
        return Err(OpenBitFunError::Deserialization(
            "Router output is not a JSON object".to_string(),
        ));
    }
    if first_line.matches("\"simple_type\"").count() != 1 {
        return Err(OpenBitFunError::Deserialization(
            "Router output must contain exactly one simple_type key".to_string(),
        ));
    }
    let (prediction, recovered) = match serde_json::from_str::<Value>(first_line) {
        Ok(value) => (value, false),
        Err(_) if !first_line.ends_with('}') => {
            let repaired = format!("{first_line}}}");
            let value = serde_json::from_str::<Value>(&repaired).map_err(|error| {
                OpenBitFunError::Deserialization(format!("Router output is invalid JSON: {error}"))
            })?;
            (value, true)
        }
        Err(error) => {
            return Err(OpenBitFunError::Deserialization(format!(
                "Router output is invalid JSON: {error}"
            )));
        }
    };
    let simple_type = prediction
        .as_object()
        .and_then(|object| object.get("simple_type"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            OpenBitFunError::Deserialization(
                "Router output is missing a string simple_type".to_string(),
            )
        })?;
    match simple_type {
        "simple" => Ok((RoundModelRoute::Fast, recovered)),
        "non_simple" => Ok((RoundModelRoute::Primary, recovered)),
        value => Err(OpenBitFunError::Deserialization(format!(
            "Router output has unsupported simple_type: {value}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        extract_simple_probability, parse_router_route, OpenAiChatChoice, OpenAiChatLogprobContent,
        OpenAiChatLogprobs, OpenAiResponseMessage, OpenAiTokenLogprob, RoundModelRoute,
        NON_SIMPLE_FIRST_TOKEN_ID, SIMPLE_TOKEN_ID,
    };

    #[test]
    fn routes_use_existing_model_slots() {
        assert_eq!(RoundModelRoute::Primary.model_selector(), "primary");
        assert_eq!(RoundModelRoute::Fast.model_selector(), "fast");
    }

    #[test]
    fn parser_maps_simple_type_and_repairs_only_a_missing_closing_brace() {
        let simple = r#"{"phase":"editing","phase_detail":"production_change_applied","simple_type":"simple"}"#;
        let non_simple = r#"{"phase":"localization","phase_detail":"root_cause_closed","simple_type":"non_simple""#;

        assert_eq!(
            parse_router_route(simple).unwrap(),
            (RoundModelRoute::Fast, false)
        );
        assert_eq!(
            parse_router_route(non_simple).unwrap(),
            (RoundModelRoute::Primary, true)
        );
        assert!(parse_router_route("explanation first").is_err());
    }

    #[test]
    fn confidence_is_renormalized_over_simple_and_non_simple() {
        let raw_output = r#"{"simple_type":"simple"}"#;
        let choice = OpenAiChatChoice {
            message: OpenAiResponseMessage { content: None },
            logprobs: Some(OpenAiChatLogprobs {
                content: Some(vec![
                    OpenAiChatLogprobContent {
                        bytes: Some(br#"{"simple_type":""#.to_vec()),
                        top_logprobs: Vec::new(),
                    },
                    OpenAiChatLogprobContent {
                        bytes: Some(b"simple".to_vec()),
                        top_logprobs: vec![
                            OpenAiTokenLogprob {
                                token: format!("token_id:{SIMPLE_TOKEN_ID}"),
                                logprob: -0.2,
                            },
                            OpenAiTokenLogprob {
                                token: format!("token_id:{NON_SIMPLE_FIRST_TOKEN_ID}"),
                                logprob: -1.4,
                            },
                        ],
                    },
                    OpenAiChatLogprobContent {
                        bytes: Some(br#""}"#.to_vec()),
                        top_logprobs: Vec::new(),
                    },
                ]),
            }),
            token_ids: Some(vec![1, SIMPLE_TOKEN_ID, 2]),
        };

        let probability =
            extract_simple_probability(&choice, raw_output, RoundModelRoute::Fast).unwrap();
        assert!((probability - 0.768_524_783_5).abs() < 1e-9);
    }

    #[test]
    fn missing_class_logprob_has_no_confidence() {
        let raw_output = r#"{"simple_type":"simple"}"#;
        let choice = OpenAiChatChoice {
            message: OpenAiResponseMessage { content: None },
            logprobs: Some(OpenAiChatLogprobs {
                content: Some(vec![
                    OpenAiChatLogprobContent {
                        bytes: Some(br#"{"simple_type":""#.to_vec()),
                        top_logprobs: Vec::new(),
                    },
                    OpenAiChatLogprobContent {
                        bytes: Some(b"simple".to_vec()),
                        top_logprobs: vec![OpenAiTokenLogprob {
                            token: format!("token_id:{SIMPLE_TOKEN_ID}"),
                            logprob: -0.2,
                        }],
                    },
                    OpenAiChatLogprobContent {
                        bytes: Some(br#""}"#.to_vec()),
                        top_logprobs: Vec::new(),
                    },
                ]),
            }),
            token_ids: Some(vec![1, SIMPLE_TOKEN_ID, 2]),
        };

        assert!(extract_simple_probability(&choice, raw_output, RoundModelRoute::Fast).is_none());
    }
}

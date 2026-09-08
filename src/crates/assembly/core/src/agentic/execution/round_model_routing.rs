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
    pub max_input_chars: usize,
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
        let max_input_chars = parse_env_usize("OPENBITFUN_ROUND_ROUTER_MAX_INPUT_CHARS", 80_000)?;
        if recent_rounds == 0 {
            return Err(OpenBitFunError::Configuration(
                "OPENBITFUN_ROUND_ROUTER_RECENT_ROUNDS must be positive".to_string(),
            ));
        }
        if max_input_chars < 4_096 {
            return Err(OpenBitFunError::Configuration(
                "OPENBITFUN_ROUND_ROUTER_MAX_INPUT_CHARS must be at least 4096".to_string(),
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
            max_input_chars,
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
    #[serde(default)]
    usage: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatChoice {
    message: OpenAiResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    content: Option<String>,
}

#[async_trait::async_trait]
impl RoundModelRouter for HttpRoundModelRouter {
    async fn select_model(
        &self,
        request: RoundModelRouteRequest,
    ) -> OpenBitFunResult<RoundModelRoute> {
        let user_prompt = build_router_user_prompt(
            &request,
            self.config.recent_rounds,
            self.config.max_input_chars,
        )?;
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
            chat_template_kwargs: json!({"enable_thinking": false}),
        };
        let started_at = Instant::now();
        let mut http_request = self.client.post(&self.config.endpoint).json(&body);
        if let Some(api_key) = self.config.api_key.as_deref() {
            http_request = http_request.bearer_auth(api_key);
        }
        let route_result = async {
            let response = http_request
                .send()
                .await
                .map_err(|error| OpenBitFunError::Http(format!("Router request failed: {error}")))?
                .error_for_status()
                .map_err(|error| {
                    OpenBitFunError::Http(format!("Router returned an error: {error}"))
                })?
                .json::<OpenAiChatResponse>()
                .await
                .map_err(|error| {
                    OpenBitFunError::Deserialization(format!(
                        "Failed to decode Router response: {error}"
                    ))
                })?;
            let raw_output = response
                .choices
                .first()
                .and_then(|choice| choice.message.content.as_deref())
                .ok_or_else(|| {
                    OpenBitFunError::Deserialization(
                        "Router response did not contain assistant content".to_string(),
                    )
                })?;
            let (route, recovered) = parse_router_route(raw_output)?;
            Ok::<_, OpenBitFunError>((route, recovered, raw_output.to_string(), response.usage))
        }
        .await;
        let (route, recovered, raw_output, router_usage) = match route_result {
            Ok(result) => result,
            Err(error) => {
                append_trace_record(
                    self.config.trace_path.as_ref(),
                    &json!({
                        "event": "router_failure",
                        "session_id": request.session_id,
                        "dialog_turn_id": request.dialog_turn_id,
                        "turn_index": request.turn_index,
                        "round_index": request.round_index,
                        "agent_type": request.agent_type,
                        "router_model": self.config.model,
                        "router_input": user_prompt,
                        "error": error.to_string(),
                        "fallback_route": RoundModelRoute::Primary,
                        "latency_ms": started_at.elapsed().as_millis(),
                    }),
                );
                return Err(error);
            }
        };
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
                "router_usage": router_usage,
                "route": route,
                "recovered": recovered,
                "latency_ms": started_at.elapsed().as_millis(),
            }),
        );
        info!(
            "Router decision completed: session_id={}, turn_id={}, round_index={}, route={:?}, recovered={}, latency_ms={}",
            request.session_id,
            request.dialog_turn_id,
            request.round_index,
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
    max_input_chars: usize,
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
    let section_limit = (max_input_chars / 8).max(512);
    let earlier_history = if let Some(summary) = existing_summary {
        truncate_router_text(&summary, section_limit)
    } else if recent_start > 0 {
        format!(
            "({recent_start} earlier completed rounds omitted; no compression summary available)"
        )
    } else {
        "(none)".to_string()
    };
    let task_description = truncate_router_text(&build_task_description(request), section_limit);
    let fixed_chars = task_description.chars().count()
        + earlier_history.chars().count()
        + "## Task\n\n\n## Earlier history summary\n\n\n## Recent trajectory\n".len();
    let recent_budget = max_input_chars.saturating_sub(fixed_chars).max(512);
    let recent_json = bounded_recent_rounds_json(recent_rounds, recent_budget)?;
    Ok(format!(
        "## Task\n{}\n\n## Earlier history summary\n{}\n\n## Recent trajectory\n{}",
        task_description, earlier_history, recent_json
    ))
}

fn truncate_router_text(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }
    let marker = format!("\n...[truncated {} chars]...\n", char_count - max_chars);
    let marker_chars = marker.chars().count();
    if max_chars <= marker_chars + 2 {
        return value.chars().take(max_chars).collect();
    }
    let retained = max_chars - marker_chars;
    let head_chars = retained * 2 / 3;
    let tail_chars = retained - head_chars;
    let head: String = value.chars().take(head_chars).collect();
    let tail: String = value
        .chars()
        .rev()
        .take(tail_chars)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{head}{marker}{tail}")
}

fn bounded_json_value(value: &Value, max_chars: usize) -> Value {
    let rendered = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
    if rendered.chars().count() <= max_chars {
        value.clone()
    } else {
        json!({
            "truncated": true,
            "preview": truncate_router_text(&rendered, max_chars),
        })
    }
}

fn bounded_recent_rounds_json(
    rounds: &[RouterTrajectoryRound],
    max_chars: usize,
) -> OpenBitFunResult<String> {
    let mut first_round = 0;
    let mut field_limit = 1_536usize.min((max_chars / 12).max(128));
    let mut item_limit = 8usize;

    loop {
        let compact: Vec<Value> = rounds[first_round..]
            .iter()
            .map(|round| {
                json!({
                    "round_id": round.round_id,
                    "assistant": {
                        "reasoning_content": truncate_router_text(
                            &round.assistant.reasoning_content,
                            field_limit,
                        ),
                        "text": truncate_router_text(&round.assistant.text, field_limit),
                        "tool_calls": round.assistant.tool_calls.iter().take(item_limit).map(|call| {
                            json!({
                                "tool_id": call.tool_id,
                                "tool_name": call.tool_name,
                                "arguments": bounded_json_value(&call.arguments, field_limit),
                                "raw_arguments": call.raw_arguments.as_deref().map(|value| {
                                    truncate_router_text(value, field_limit)
                                }),
                                "is_error": call.is_error,
                            })
                        }).collect::<Vec<_>>(),
                        "omitted_tool_calls": round.assistant.tool_calls.len().saturating_sub(item_limit),
                    },
                    "tool_results": round.tool_results.iter().take(item_limit).map(|result| {
                        json!({
                            "tool_name": result.tool_name,
                            "result_for_assistant": truncate_router_text(
                                &result.result_for_assistant,
                                field_limit,
                            ),
                        })
                    }).collect::<Vec<_>>(),
                    "omitted_tool_results": round.tool_results.len().saturating_sub(item_limit),
                })
            })
            .collect();
        let rendered = serde_json::to_string(&compact)?;
        if rendered.chars().count() <= max_chars {
            return Ok(rendered);
        }
        if rounds.len().saturating_sub(first_round) > 1 {
            first_round += 1;
        } else if field_limit > 128 {
            field_limit = (field_limit / 2).max(128);
        } else if item_limit > 1 {
            item_limit = (item_limit / 2).max(1);
        } else {
            return Ok("[{\"trajectory_truncated\":true}]".to_string());
        }
    }
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
        bounded_recent_rounds_json, parse_router_route, RoundModelRoute, RouterAssistant,
        RouterToolCall, RouterToolResult, RouterTrajectoryRound,
    };
    use serde_json::json;

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
    fn bounded_recent_trajectory_stays_valid_json() {
        let oversized = "x".repeat(20_000);
        let rounds = vec![RouterTrajectoryRound {
            round_id: 7,
            assistant: RouterAssistant {
                reasoning_content: oversized.clone(),
                text: oversized.clone(),
                tool_calls: (0..20)
                    .map(|index| RouterToolCall {
                        tool_id: format!("tool-{index}"),
                        tool_name: "ExecCommand".to_string(),
                        arguments: json!({"output": oversized}),
                        raw_arguments: Some(oversized.clone()),
                        is_error: false,
                    })
                    .collect(),
            },
            tool_results: (0..20)
                .map(|index| RouterToolResult {
                    tool_id: format!("tool-{index}"),
                    tool_name: "ExecCommand".to_string(),
                    result_for_assistant: oversized.clone(),
                })
                .collect(),
        }];

        let rendered = bounded_recent_rounds_json(&rounds, 4_096).unwrap();

        assert!(rendered.chars().count() <= 4_096);
        assert!(serde_json::from_str::<serde_json::Value>(&rendered).is_ok());
        assert!(rendered.contains("truncated"));
    }
}

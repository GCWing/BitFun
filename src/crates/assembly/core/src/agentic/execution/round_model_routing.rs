//! Per-round execution-model selection contracts.
//!
//! It exposes a narrow execution-engine hook plus an opt-in HTTP implementation while
//! keeping the existing agent loop, retry lifecycle, and tool pipeline authoritative.

pub mod context;

use crate::agentic::core::{Message, MessageContent};
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use context::{RoundRouterContext, RouterContextConfig, RouterContextFactory};
use log::{debug, info, warn};
use openbitfun_agent_runtime::router_context::PreparedRouterContext;
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
    /// HTTP routing receives an isolated, bounded snapshot instead of cloning main history.
    pub prepared_context: Option<PreparedRouterContext>,
}

/// Selects the execution model exactly once for each logical model round.
///
/// Request retries and context-overflow recovery remain inside that logical round and
/// therefore reuse the selected model without calling this hook again.
#[async_trait::async_trait]
pub trait RoundModelRouter: Send + Sync {
    /// Optional router-owned incremental state. Custom routers retain their read-only history API.
    fn create_context(
        &self,
        _session_id: &str,
        _dialog_turn_id: &str,
        _task: &str,
    ) -> Option<RoundRouterContext> {
        None
    }

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
    pub simple_threshold: f64,
    pub trace_path: Option<PathBuf>,
    pub context: RouterContextConfig,
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
        let simple_threshold = parse_env_f64("OPENBITFUN_ROUND_ROUTER_SIMPLE_THRESHOLD", 0.7)?;
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
            max_input_chars,
            simple_threshold,
            trace_path: std::env::var("OPENBITFUN_ROUND_ROUTER_TRACE")
                .ok()
                .map(PathBuf::from),
            context: RouterContextConfig::from_env()?,
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
    context_factory: RouterContextFactory,
}

impl HttpRoundModelRouter {
    pub fn new(config: HttpRoundModelRouterConfig) -> OpenBitFunResult<Self> {
        openbitfun_services_core::tls_provider::ensure_ring_crypto_provider();
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|error| OpenBitFunError::Http(error.to_string()))?;
        let context_factory = RouterContextFactory::new(
            config.context.clone(),
            config.recent_rounds,
            config.max_input_chars,
            &config.system_prompt,
            config.trace_path.clone(),
        )?;
        Ok(Self {
            config,
            client,
            context_factory,
        })
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
    #[serde(default)]
    usage: Option<Value>,
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
    fn create_context(
        &self,
        session_id: &str,
        dialog_turn_id: &str,
        task: &str,
    ) -> Option<RoundRouterContext> {
        Some(
            self.context_factory
                .create(session_id, dialog_turn_id, task),
        )
    }

    async fn select_model(
        &self,
        request: RoundModelRouteRequest,
    ) -> OpenBitFunResult<RoundModelRoute> {
        let prepared = if let Some(prepared) = request.prepared_context.as_ref() {
            prepared.clone()
        } else {
            // Direct callers also get isolated context, never the main compression summary.
            let preparation_started = Instant::now();
            let mut context = self.context_factory.create(
                &request.session_id,
                &request.dialog_turn_id,
                &request.original_user_input,
            );
            context.observe(&request.messages);
            let mut prepared = context.render();
            prepared.preparation_ms = preparation_started.elapsed().as_millis() as u64;
            prepared
        };
        let user_prompt = &prepared.user_prompt;
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
            let choice = response.choices.first().ok_or_else(|| {
                OpenBitFunError::Deserialization(
                    "Router response did not contain a choice".to_string(),
                )
            })?;
            let raw_output = choice.message.content.as_deref().ok_or_else(|| {
                OpenBitFunError::Deserialization(
                    "Router response did not contain assistant content".to_string(),
                )
            })?;
            let (generated_route, recovered) = parse_router_route(raw_output)?;
            let simple_probability =
                extract_simple_probability(choice, raw_output, generated_route);
            let route = route_for_simple_probability(
                simple_probability,
                self.config.simple_threshold,
            );
            if simple_probability.is_none() {
                warn!(
                    "Router logprobs did not contain both class tokens; conservatively selecting primary: turn_id={}, round_index={}",
                    request.dialog_turn_id, request.round_index
                );
            }
            Ok::<_, OpenBitFunError>((
                route,
                generated_route,
                simple_probability,
                recovered,
                raw_output.to_string(),
                response.usage,
            ))
        }
        .await;
        let (route, generated_route, simple_probability, recovered, raw_output, router_usage) =
            match route_result {
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
                            "router_context": prepared,
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
                "router_context": prepared,
                "router_output": raw_output,
                "router_usage": router_usage,
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

fn route_for_simple_probability(
    simple_probability: Option<f64>,
    simple_threshold: f64,
) -> RoundModelRoute {
    match simple_probability {
        Some(probability) if probability > simple_threshold => RoundModelRoute::Fast,
        _ => RoundModelRoute::Primary,
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
        extract_simple_probability, parse_router_route, route_for_simple_probability,
        OpenAiChatChoice, OpenAiChatLogprobContent, OpenAiChatLogprobs, OpenAiResponseMessage,
        OpenAiTokenLogprob, RoundModelRoute, NON_SIMPLE_FIRST_TOKEN_ID, SIMPLE_TOKEN_ID,
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
    fn threshold_routes_only_confident_simple_predictions_to_fast() {
        assert_eq!(
            route_for_simple_probability(Some(0.700_001), 0.7),
            RoundModelRoute::Fast
        );
        assert_eq!(
            route_for_simple_probability(Some(0.7), 0.7),
            RoundModelRoute::Primary
        );
        assert_eq!(
            route_for_simple_probability(None, 0.7),
            RoundModelRoute::Primary
        );
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

    #[tokio::test]
    async fn http_request_preserves_system_and_protocol_with_isolated_bounded_input() {
        use super::{
            HttpRoundModelRouter, HttpRoundModelRouterConfig, RoundModelRouteRequest,
            RoundModelRouter,
        };
        use crate::agentic::core::{Message, MessageSemanticKind};
        use serde_json::{json, Value};
        use std::io::{BufRead, BufReader, Read, Write};
        use std::time::Duration;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let router = HttpRoundModelRouter::new(HttpRoundModelRouterConfig {
            endpoint,
            model: "router-fixture".into(),
            system_prompt: "EXACT_FIXED_SYSTEM".into(),
            api_key: None,
            timeout: Duration::from_secs(3),
            recent_rounds: 3,
            max_input_chars: 80_000,
            simple_threshold: 0.7,
            trace_path: None,
            context: super::RouterContextConfig {
                summary_enabled: false,
                ..Default::default()
            },
        })
        .unwrap();
        let server = std::thread::spawn(move || {
            let mut payloads = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    assert!(reader.read_line(&mut line).unwrap() > 0);
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse::<usize>().unwrap();
                    }
                }
                assert!(length > 0 && length < 100_000);
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                payloads.push(serde_json::from_slice::<Value>(&body).unwrap());
                let response = json!({"choices": [{
                    "message": {"content": "{\"simple_type\":\"simple\"}"},
                    "logprobs": {"content": [
                        {"bytes": b"{\"simple_type\":\"".to_vec(), "top_logprobs": []},
                        {"bytes": b"simple".to_vec(), "top_logprobs": [
                            {"token": "token_id:22944", "logprob": -0.2},
                            {"token": "token_id:6280", "logprob": -1.4}
                        ]},
                        {"bytes": b"\"}".to_vec(), "top_logprobs": []}
                    ]}, "token_ids": [1, 22944, 2]
                }], "usage": {"prompt_tokens": 100, "completion_tokens": 8}})
                .to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
            }
            payloads
        });
        let messages = vec![
            Message::user("DO_NOT_REUSE_MAIN_SUMMARY".into())
                .with_semantic_kind(MessageSemanticKind::CompressionSummary),
            Message::assistant("long evidence 世界 ".repeat(8_000)),
            Message::assistant("LATEST_VISIBLE_RESULT".into()),
        ];
        for prepared in [false, true] {
            let prepared_context = if prepared {
                Some(
                    router
                        .create_context("session", "turn", "fix bug")
                        .unwrap()
                        .prepare(&messages)
                        .await,
                )
            } else {
                None
            };
            let request = RoundModelRouteRequest {
                session_id: "session".into(),
                dialog_turn_id: "turn".into(),
                turn_index: 0,
                round_index: 2,
                agent_type: "code".into(),
                original_user_input: "fix bug".into(),
                messages: if prepared {
                    Vec::new()
                } else {
                    messages.clone()
                },
                prepared_context,
            };
            assert_eq!(
                router.select_model(request).await.unwrap(),
                RoundModelRoute::Fast
            );
        }
        let payloads = server.join().unwrap();
        assert_eq!(payloads[0], payloads[1]);
        let body = &payloads[0];
        assert_eq!(body["messages"].as_array().unwrap().len(), 2);
        assert_eq!(body["messages"][0]["content"], "EXACT_FIXED_SYSTEM");
        assert_eq!(body["temperature"].as_f64(), Some(0.0));
        assert_eq!(body["max_tokens"], 128);
        assert_eq!(body["stop"], json!(["\n"]));
        assert!(body.get("tools").is_none());
        let prompt = body["messages"][1]["content"].as_str().unwrap();
        assert!(prompt.len() <= 4_096);
        assert!(prompt.contains("LATEST_VISIBLE_RESULT"));
        assert!(!prompt.contains("DO_NOT_REUSE_MAIN_SUMMARY"));
        assert!(prompt.starts_with("## Task\n"));
        let trajectory = prompt.split("## Recent trajectory\n").nth(1).unwrap();
        serde_json::from_str::<Value>(trajectory).unwrap();
    }
}

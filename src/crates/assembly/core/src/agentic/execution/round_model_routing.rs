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
use openbitfun_ai_adapters::round_router::{RouterClientError, RouterHttpClient};
use serde::Serialize;
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
        let simple_threshold = parse_env_f64("OPENBITFUN_ROUND_ROUTER_SIMPLE_THRESHOLD", 0.75)?;
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
    client: RouterHttpClient,
    context_factory: RouterContextFactory,
}

impl HttpRoundModelRouter {
    pub fn new(config: HttpRoundModelRouterConfig) -> OpenBitFunResult<Self> {
        let client = RouterHttpClient::new(
            config.endpoint.clone(),
            config.model.clone(),
            config.system_prompt.clone(),
            config.api_key.clone(),
            config.timeout,
        )
        .map_err(map_router_error)?;
        let context_factory = RouterContextFactory::new(
            config.context.clone(),
            config.recent_rounds,
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
        let started_at = Instant::now();
        let mut trace_guard = RouterTraceGuard::start(
            self.config.trace_path.as_ref(),
            "router_request_started",
            "router_cancelled",
            json!({
                "session_id": request.session_id,
                "dialog_turn_id": request.dialog_turn_id,
                "turn_index": request.turn_index,
                "round_index": request.round_index,
                "agent_type": request.agent_type,
                "router_model": self.config.model,
            }),
        );
        let route_result = self.client.predict(user_prompt).await.map_err(map_router_error).map(|prediction| {
            let generated_route = if prediction.generated_simple {
                RoundModelRoute::Fast
            } else {
                RoundModelRoute::Primary
            };
            let route = route_for_simple_probability(
                prediction.simple_probability,
                self.config.simple_threshold,
            );
            if prediction.simple_probability.is_none() {
                warn!(
                    "Router logprobs did not contain both class tokens; conservatively selecting primary: turn_id={}, round_index={}",
                    request.dialog_turn_id, request.round_index
                );
            }
            (
                route,
                generated_route,
                prediction.simple_probability,
                prediction.recovered,
                prediction.raw_output,
                prediction.usage,
            )
        });
        let (route, generated_route, simple_probability, recovered, raw_output, router_usage) =
            match route_result {
                Ok(result) => result,
                Err(error) => {
                    append_trace_record(
                        self.config.trace_path.as_ref(),
                        &json!({
                            "event": "router_failure",
                            "request_id": trace_guard.request_id(),
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
                    trace_guard.finish();
                    return Err(error);
                }
            };
        append_trace_record(
            self.config.trace_path.as_ref(),
            &json!({
                "event": "router_decision",
                "request_id": trace_guard.request_id(),
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
        trace_guard.finish();
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

fn map_router_error(error: RouterClientError) -> OpenBitFunError {
    match error {
        RouterClientError::Http(message) => OpenBitFunError::Http(message),
        RouterClientError::Deserialization(message) => OpenBitFunError::Deserialization(message),
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

static TRACE_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Keep cancelled in-flight auxiliary calls visible without waiting for a model
/// during cancellation or changing the Agent's shutdown behavior.
struct RouterTraceGuard {
    path: Option<PathBuf>,
    request_id: String,
    started: Instant,
    cancellation: Value,
    finished: bool,
}

impl RouterTraceGuard {
    fn start(
        path: Option<&PathBuf>,
        started_event: &str,
        cancelled_event: &str,
        mut record: Value,
    ) -> Self {
        let started = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();
        record["request_id"] = json!(request_id);
        record["event"] = json!(started_event);
        append_trace_record(path, &record);
        record["event"] = json!(cancelled_event);
        record["error"] = json!("Request cancelled or dropped before completion");
        Self {
            path: path.cloned(),
            request_id,
            started,
            cancellation: record,
            finished: false,
        }
    }

    fn request_id(&self) -> &str {
        &self.request_id
    }

    fn finish(&mut self) {
        self.finished = true;
    }
}

impl Drop for RouterTraceGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.cancellation["latency_ms"] = json!(self.started.elapsed().as_millis());
            append_trace_record(self.path.as_ref(), &self.cancellation);
        }
    }
}

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
        #[derive(Serialize)]
        struct TraceEnvelope<'a> {
            recorded_at: chrono::DateTime<chrono::Utc>,
            #[serde(flatten)]
            record: &'a Value,
        }
        serde_json::to_writer(
            &mut file,
            &TraceEnvelope {
                recorded_at: chrono::Utc::now(),
                record,
            },
        )
        .map_err(std::io::Error::other)?;
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

#[cfg(test)]
mod tests {
    use super::{route_for_simple_probability, RoundModelRoute, RouterTraceGuard};

    #[test]
    fn dropped_router_call_retains_identity_timestamps_and_unknown_usage() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("router.jsonl");
        let guard = RouterTraceGuard::start(
            Some(&path),
            "router_request_started",
            "router_cancelled",
            serde_json::json!({"session_id": "s", "round_index": 1}),
        );
        let request_id = guard.request_id().to_string();
        drop(guard);
        let events: Vec<serde_json::Value> = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["event"], "router_request_started");
        assert_eq!(events[1]["event"], "router_cancelled");
        for event in &events {
            assert_eq!(event["request_id"], request_id);
            assert!(event["recorded_at"].is_string());
        }
        assert!(events[1]["latency_ms"].is_u64());
        assert!(events[1]["router_usage"].is_null());
    }

    #[test]
    fn completed_router_call_does_not_emit_cancellation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("router.jsonl");
        let mut guard = RouterTraceGuard::start(
            Some(&path),
            "router_request_started",
            "router_cancelled",
            serde_json::json!({"round_index": 0}),
        );
        guard.finish();
        drop(guard);
        assert_eq!(std::fs::read_to_string(path).unwrap().lines().count(), 1);
    }

    #[test]
    fn routes_use_existing_model_slots() {
        assert_eq!(RoundModelRoute::Primary.model_selector(), "primary");
        assert_eq!(RoundModelRoute::Fast.model_selector(), "fast");
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

use super::{BitFunError, BitFunResult, SpeechService};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bitfun_core_types::speech::{
    SpeechAppendRealtimeAudioRequest, SpeechRealtimeEvent, SpeechRealtimeEventKind,
    SpeechRealtimeFunctionCall, SpeechRealtimeSession, SpeechRealtimeSessionRequest,
    SpeechRealtimeSpeakRequest, SpeechRealtimeToolResultRequest,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    http::{header::HeaderName, HeaderValue},
    Message,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const VOLCENGINE_REALTIME_URL: &str =
    "wss://openspeech.bytedance.com/api/v3/duplex/realtime/dialogue";
const VOLCENGINE_REALTIME_MODEL: &str = "1.2.6.1";
const INPUT_SAMPLE_RATE: u32 = 16_000;
const OUTPUT_SAMPLE_RATE: u32 = 24_000;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const COMMAND_QUEUE_CAPACITY: usize = 256;
const MAX_AUDIO_CHUNK_BYTES: usize = 64 * 1024;
const MAX_TOOL_RESULT_BYTES: usize = 16 * 1024;
const MAX_SPOKEN_PROGRESS_CHARS: usize = 600;
const MAX_CLIENT_CONTEXT_CHARS: usize = 16 * 1024;

const BITFUN_VOICE_INSTRUCTIONS: &str = r#"You are BitFun's client-level realtime voice assistant. Reply naturally and concisely in the user's language. Your voice call belongs to the whole BitFun client, not to one chat session or one workspace.

Use get_bitfun_client_context whenever the user asks about the current client, open workspaces/projects, visible sessions, running tasks, or names a workspace whose exact id is not already known from a fresh context result. Never guess a workspace id. Use switch_bitfun_workspace for navigation-only requests. When the user asks you to inspect, create, change, run, debug, research, or otherwise complete work, call run_bitfun_task with a complete standalone task description and the intended workspace_id. Omit workspace_id only when the user clearly means the active workspace. Set activate_workspace to true when the user asks to enter, switch to, or visibly work in that workspace; use false only for an explicit background request.

If the user asks to stop, cancel, abort, or interrupt the BitFun task currently running through this client voice assistant, call stop_bitfun_task immediately. A stop request is a control operation, not a new task: never pass it to run_bitfun_task and never claim the task stopped before the stop_bitfun_task result confirms it. Do not claim that work is complete before the tool result arrives. BitFun will speak brief public progress summaries while the Agent task is running; do not expose private reasoning, raw logs, or tool payloads. BitFun also speaks a concise final outcome itself. When a task tool result contains outcome_spoken=true, do not repeat that outcome; wait for the user's next request. If outcome_spoken is false or absent, summarize the outcome clearly and mention any user action still required. Never invent client state or task results."#;

#[derive(Debug, Clone)]
pub struct VolcengineRealtimeSpeechConfig {
    pub api_key: String,
    pub voice: String,
    pub speed: i32,
    pub loudness: i32,
    pub client_context: Option<String>,
}

#[derive(Clone, Default)]
pub(crate) struct RealtimeSpeechRegistry {
    sessions: Arc<Mutex<HashMap<String, RealtimeSpeechSessionHandle>>>,
}

#[derive(Clone)]
struct RealtimeSpeechSessionHandle {
    sender: mpsc::Sender<RealtimeSpeechCommand>,
    cancel: CancellationToken,
}

enum RealtimeSpeechCommand {
    Send(Value),
    Close,
}

type RealtimeEventHandler = Arc<dyn Fn(SpeechRealtimeEvent) + Send + Sync + 'static>;

impl SpeechService {
    pub async fn start_realtime_session<F>(
        &self,
        config: VolcengineRealtimeSpeechConfig,
        on_event: F,
    ) -> BitFunResult<SpeechRealtimeSession>
    where
        F: Fn(SpeechRealtimeEvent) + Send + Sync + 'static,
    {
        let config = validate_config(config)?;
        let session_id = Uuid::new_v4().to_string();
        let connect_id = Uuid::new_v4().to_string();
        let (sender, receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
        let cancel = CancellationToken::new();
        let handle = RealtimeSpeechSessionHandle {
            sender,
            cancel: cancel.clone(),
        };

        self.realtime
            .sessions
            .lock()
            .await
            .insert(session_id.clone(), handle);

        let (ready_sender, ready_receiver) = oneshot::channel();
        let event_handler: RealtimeEventHandler = Arc::new(on_event);
        let registry = self.realtime.clone();
        let actor_session_id = session_id.clone();
        tokio::spawn(async move {
            run_realtime_actor(
                actor_session_id.clone(),
                connect_id,
                config,
                receiver,
                cancel,
                ready_sender,
                event_handler,
            )
            .await;
            registry.sessions.lock().await.remove(&actor_session_id);
        });

        match timeout(CONNECT_TIMEOUT + Duration::from_secs(2), ready_receiver).await {
            Ok(Ok(Ok(()))) => Ok(SpeechRealtimeSession {
                session_id,
                input_sample_rate: INPUT_SAMPLE_RATE,
                output_sample_rate: OUTPUT_SAMPLE_RATE,
            }),
            Ok(Ok(Err(message))) => {
                self.realtime.sessions.lock().await.remove(&session_id);
                Err(BitFunError::service(message))
            }
            Ok(Err(_)) => {
                self.realtime.sessions.lock().await.remove(&session_id);
                Err(BitFunError::service(
                    "Realtime speech connection ended before it became ready",
                ))
            }
            Err(_) => {
                if let Some(handle) = self.realtime.sessions.lock().await.remove(&session_id) {
                    handle.cancel.cancel();
                }
                Err(BitFunError::service(
                    "Timed out connecting to the realtime speech service",
                ))
            }
        }
    }

    pub async fn append_realtime_audio(
        &self,
        request: SpeechAppendRealtimeAudioRequest,
    ) -> BitFunResult<()> {
        let audio = BASE64_STANDARD
            .decode(request.pcm16_base64.as_bytes())
            .map_err(|error| {
                BitFunError::validation(format!("Invalid base64 realtime audio chunk: {error}"))
            })?;
        if audio.is_empty() || audio.len() % 2 != 0 {
            return Err(BitFunError::validation(
                "Realtime PCM16 audio must contain complete samples",
            ));
        }
        if audio.len() > MAX_AUDIO_CHUNK_BYTES {
            return Err(BitFunError::validation(format!(
                "Realtime audio chunk exceeds {MAX_AUDIO_CHUNK_BYTES} bytes"
            )));
        }
        self.send_realtime_command(
            &request.session_id,
            json!({
                "type": "input_audio_buffer.append",
                "event_id": Uuid::new_v4().simple().to_string(),
                "audio": request.pcm16_base64,
            }),
        )
        .await
    }

    pub async fn commit_realtime_audio(
        &self,
        request: SpeechRealtimeSessionRequest,
    ) -> BitFunResult<()> {
        self.send_realtime_command(
            &request.session_id,
            json!({
                "type": "input_audio_buffer.commit",
                "event_id": Uuid::new_v4().simple().to_string(),
            }),
        )
        .await
    }

    pub async fn send_realtime_tool_result(
        &self,
        request: SpeechRealtimeToolResultRequest,
    ) -> BitFunResult<()> {
        let call_id = request.call_id.trim();
        if call_id.is_empty() {
            return Err(BitFunError::validation("Function call id cannot be empty"));
        }
        if request.result.len() > MAX_TOOL_RESULT_BYTES {
            return Err(BitFunError::validation(format!(
                "Realtime function result exceeds {MAX_TOOL_RESULT_BYTES} bytes"
            )));
        }
        self.send_realtime_command(
            &request.session_id,
            tool_result_payload(call_id, &request.result),
        )
        .await
    }

    pub async fn speak_realtime_text(
        &self,
        request: SpeechRealtimeSpeakRequest,
    ) -> BitFunResult<()> {
        let text = request.text.trim();
        if text.is_empty() {
            return Err(BitFunError::validation(
                "Spoken progress text cannot be empty",
            ));
        }
        if text.chars().count() > MAX_SPOKEN_PROGRESS_CHARS {
            return Err(BitFunError::validation(format!(
                "Spoken progress text exceeds {MAX_SPOKEN_PROGRESS_CHARS} characters"
            )));
        }
        self.send_realtime_command(&request.session_id, spoken_text_payload(text))
            .await
    }

    pub async fn cancel_realtime_response(
        &self,
        request: SpeechRealtimeSessionRequest,
    ) -> BitFunResult<()> {
        self.send_realtime_command(
            &request.session_id,
            json!({
                "type": "response.cancel",
                "event_id": Uuid::new_v4().simple().to_string(),
            }),
        )
        .await
    }

    pub async fn close_realtime_session(
        &self,
        request: SpeechRealtimeSessionRequest,
    ) -> BitFunResult<()> {
        let handle = self
            .realtime
            .sessions
            .lock()
            .await
            .remove(&request.session_id)
            .ok_or_else(|| {
                BitFunError::NotFound("Realtime speech session not found".to_string())
            })?;
        handle
            .sender
            .send(RealtimeSpeechCommand::Close)
            .await
            .map_err(|_| BitFunError::service("Realtime speech session is already closed"))
    }

    async fn send_realtime_command(&self, session_id: &str, payload: Value) -> BitFunResult<()> {
        let handle = self
            .realtime
            .sessions
            .lock()
            .await
            .get(session_id)
            .cloned()
            .ok_or_else(|| {
                BitFunError::NotFound("Realtime speech session not found".to_string())
            })?;
        handle
            .sender
            .send(RealtimeSpeechCommand::Send(payload))
            .await
            .map_err(|_| BitFunError::service("Realtime speech session is already closed"))
    }
}

fn tool_result_payload(call_id: &str, result: &str) -> Value {
    json!({
        "type": "conversation.item.create",
        "event_id": Uuid::new_v4().simple().to_string(),
        "items": [{
            "type": "message",
            "call_id": call_id,
            "role": "tool",
            "content": [{ "type": "input_text", "text": result }],
        }],
    })
}

fn spoken_text_payload(text: &str) -> Value {
    json!({
        "type": "speech_text_buffer.commit",
        "event_id": Uuid::new_v4().simple().to_string(),
        "speech_id": Uuid::new_v4().to_string(),
        "text": text,
    })
}

fn validate_config(
    mut config: VolcengineRealtimeSpeechConfig,
) -> BitFunResult<VolcengineRealtimeSpeechConfig> {
    config.api_key = config.api_key.trim().to_string();
    config.voice = config.voice.trim().to_string();
    if config.api_key.is_empty() {
        return Err(BitFunError::validation(
            "Volcengine realtime speech API key is not configured",
        ));
    }
    if config.voice.is_empty() {
        return Err(BitFunError::validation(
            "Volcengine realtime speech voice is not configured",
        ));
    }
    if !(-50..=100).contains(&config.speed) || !(-50..=100).contains(&config.loudness) {
        return Err(BitFunError::validation(
            "Realtime speech speed and loudness must be between -50 and 100",
        ));
    }
    config.client_context = config.client_context.and_then(|context| {
        let trimmed = context.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trim_chars(trimmed, MAX_CLIENT_CONTEXT_CHARS))
        }
    });
    Ok(config)
}

fn trim_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars).collect()
}

async fn run_realtime_actor(
    session_id: String,
    connect_id: String,
    config: VolcengineRealtimeSpeechConfig,
    mut receiver: mpsc::Receiver<RealtimeSpeechCommand>,
    cancel: CancellationToken,
    ready_sender: oneshot::Sender<Result<(), String>>,
    on_event: RealtimeEventHandler,
) {
    bitfun_services_core::tls_provider::ensure_ring_crypto_provider();
    let mut request = match VOLCENGINE_REALTIME_URL.into_client_request() {
        Ok(request) => request,
        Err(error) => {
            let message = format!("Failed to build realtime speech request: {error}");
            let _ = ready_sender.send(Err(message.clone()));
            emit_error(&on_event, &session_id, message);
            return;
        }
    };
    let api_key_header = HeaderName::from_static("x-api-key");
    let connect_id_header = HeaderName::from_static("x-api-connect-id");
    let api_key = match HeaderValue::from_str(&config.api_key) {
        Ok(value) => value,
        Err(_) => {
            let message =
                "Volcengine realtime speech API key contains invalid header characters".to_string();
            let _ = ready_sender.send(Err(message.clone()));
            emit_error(&on_event, &session_id, message);
            return;
        }
    };
    let connect_header = match HeaderValue::from_str(&connect_id) {
        Ok(value) => value,
        Err(_) => {
            let message = "Failed to construct realtime speech connection id".to_string();
            let _ = ready_sender.send(Err(message.clone()));
            emit_error(&on_event, &session_id, message);
            return;
        }
    };
    request.headers_mut().insert(api_key_header, api_key);
    request
        .headers_mut()
        .insert(connect_id_header, connect_header);

    let websocket = match timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(request)).await
    {
        Ok(Ok((stream, _response))) => stream,
        Ok(Err(error)) => {
            let message = format!("Failed to connect to realtime speech service: {error}");
            let _ = ready_sender.send(Err(message.clone()));
            emit_error(&on_event, &session_id, message);
            return;
        }
        Err(_) => {
            let message = "Timed out connecting to the realtime speech service".to_string();
            let _ = ready_sender.send(Err(message.clone()));
            emit_error(&on_event, &session_id, message);
            return;
        }
    };

    emit_event(
        &on_event,
        SpeechRealtimeEvent {
            session_id: session_id.clone(),
            kind: SpeechRealtimeEventKind::Connected,
            text: None,
            audio_base64: None,
            function_calls: Vec::new(),
            provider_session_id: None,
            status_code: None,
            message: None,
        },
    );

    let (mut sink, mut stream) = websocket.split();
    let create_payload = session_create_payload(&config, &session_id);
    if let Err(error) = send_json(&mut sink, create_payload).await {
        let message = format!("Failed to create realtime speech session: {error}");
        let _ = ready_sender.send(Err(message.clone()));
        emit_error(&on_event, &session_id, message);
        return;
    }
    let mut ready_sender = Some(ready_sender);
    let mut closed_emitted = false;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = send_json(&mut sink, json!({
                    "type": "session.close",
                    "event_id": Uuid::new_v4().simple().to_string(),
                })).await;
                break;
            }
            command = receiver.recv() => {
                match command {
                    Some(RealtimeSpeechCommand::Send(payload)) => {
                        if let Err(error) = send_json(&mut sink, payload).await {
                            emit_error(
                                &on_event,
                                &session_id,
                                format!("Failed to send realtime speech request: {error}"),
                            );
                            break;
                        }
                    }
                    Some(RealtimeSpeechCommand::Close) => {
                        let _ = send_json(&mut sink, json!({
                            "type": "session.close",
                            "event_id": Uuid::new_v4().simple().to_string(),
                        })).await;
                        break;
                    }
                    None => break,
                }
            }
            incoming = stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let session_created = provider_frame_is_type(text.as_ref(), "session.created");
                        if let Some(was_closed) = process_provider_frame(
                            &session_id,
                            text.as_ref(),
                            &on_event,
                        ) {
                            if let Some(sender) = ready_sender.take() {
                                let _ = sender.send(Err(provider_startup_error(text.as_ref())));
                            }
                            closed_emitted = was_closed;
                            break;
                        }
                        if session_created {
                            if let Some(sender) = ready_sender.take() {
                                let _ = sender.send(Ok(()));
                            }
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        let text = match std::str::from_utf8(bytes.as_ref()) {
                            Ok(text) => text,
                            Err(error) => {
                                emit_error(
                                    &on_event,
                                    &session_id,
                                    format!("Realtime speech service returned a non-UTF-8 binary event: {error}"),
                                );
                                break;
                            }
                        };
                        let session_created = provider_frame_is_type(text, "session.created");
                        if let Some(was_closed) = process_provider_frame(
                            &session_id,
                            text,
                            &on_event,
                        ) {
                            if let Some(sender) = ready_sender.take() {
                                let _ = sender.send(Err(provider_startup_error(text)));
                            }
                            closed_emitted = was_closed;
                            break;
                        }
                        if session_created {
                            if let Some(sender) = ready_sender.take() {
                                let _ = sender.send(Ok(()));
                            }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if sink.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(error)) => {
                        emit_error(
                            &on_event,
                            &session_id,
                            format!("Realtime speech connection failed: {error}"),
                        );
                        break;
                    }
                }
            }
        }
    }

    if let Some(sender) = ready_sender.take() {
        let _ = sender.send(Err(
            "Realtime speech connection ended before session.created".to_string(),
        ));
    }
    let _ = sink.close().await;
    if !closed_emitted {
        emit_event(
            &on_event,
            SpeechRealtimeEvent {
                session_id,
                kind: SpeechRealtimeEventKind::Closed,
                text: None,
                audio_base64: None,
                function_calls: Vec::new(),
                provider_session_id: None,
                status_code: None,
                message: None,
            },
        );
    }
}

fn provider_frame_is_type(text: &str, expected: &str) -> bool {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|payload| {
            payload
                .get("type")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .as_deref()
        == Some(expected)
}

fn provider_startup_error(text: &str) -> String {
    let payload = serde_json::from_str::<Value>(text).ok();
    payload
        .as_ref()
        .and_then(|value| {
            value
                .get("message")
                .or_else(|| value.pointer("/error/message"))
                .and_then(Value::as_str)
        })
        .map(|message| format!("Realtime speech session failed: {message}"))
        .unwrap_or_else(|| "Realtime speech session ended before it became ready".to_string())
}

fn session_create_payload(
    config: &VolcengineRealtimeSpeechConfig,
    provider_session_id: &str,
) -> Value {
    let instructions = match config.client_context.as_deref() {
        Some(context) => format!(
            "{BITFUN_VOICE_INSTRUCTIONS}\n\nCLIENT CONTEXT SNAPSHOT AT CALL START (JSON; refresh with get_bitfun_client_context before relying on changing state):\n{context}"
        ),
        None => BITFUN_VOICE_INSTRUCTIONS.to_string(),
    };
    json!({
        "type": "session.create",
        "event_id": Uuid::new_v4().simple().to_string(),
        "session": {
            "id": provider_session_id,
            "model": VOLCENGINE_REALTIME_MODEL,
            "instructions": instructions,
            "audio": {
                "input": { "format": { "type": "pcm", "rate": INPUT_SAMPLE_RATE } },
                "output": {
                    "format": { "type": "pcm_s16le", "rate": OUTPUT_SAMPLE_RATE },
                    "voice": config.voice,
                    "speed": config.speed,
                    "loudness": config.loudness,
                },
            },
            // Architecture boundary for future agents:
            //
            // This is the provider-hosted Voice model's client control-plane
            // tool list. It is intentionally independent from the tool registry
            // assembled for a normal BitFun Agent session. In particular,
            // `run_bitfun_task` delegates one complete user intent to a newly
            // created Agent session; Voice never receives, mirrors, or proxies
            // that session's filesystem, terminal, MCP, browser, or other tools.
            // The delegated session resolves its own tools and permissions in
            // the normal workspace execution path.
            //
            // Add a Voice tool only for a direct client-level operation that
            // cannot be expressed as an Agent task. A new Voice tool requires
            // coordinated changes in all of these places:
            // 1. its provider schema below;
            // 2. `VoiceFunctionCommand`, `parseFunctionCall`, and
            //    `handleFunctionCall` in
            //    `src/web-ui/src/flow_chat/components/voice/useRealtimeVoiceCall.ts`;
            // 3. focused Rust payload-contract and Web UI dispatch tests.
            // Workspace execution capabilities belong in the normal Agent tool
            // registry and become available to delegated tasks without being
            // copied into this list.
            "tools": [
                {
                    "type": "function",
                    "name": "get_bitfun_client_context",
                    "description": "Return a fresh snapshot of the BitFun client: active scene, active and opened workspaces, visible sessions, running Agent tasks, and the task owned by this voice assistant. Call this before resolving a workspace name or answering questions about current client state.",
                    "parameters": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {}
                    }
                },
                {
                    "type": "function",
                    "name": "switch_bitfun_workspace",
                    "description": "Activate one currently opened BitFun workspace in the client. Obtain the exact workspace_id from get_bitfun_client_context and never guess it.",
                    "parameters": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "workspace_id": {
                                "type": "string",
                                "description": "Exact id of an opened workspace from get_bitfun_client_context."
                            }
                        },
                        "required": ["workspace_id"]
                    }
                },
                {
                    "type": "function",
                    "name": "run_bitfun_task",
                    "description": "Start a new BitFun Agent session in the intended opened workspace, autonomously complete the requested task with normal tools and permissions, and return the final result. Use get_bitfun_client_context first when the user names a workspace or project.",
                    "parameters": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "task": {
                                "type": "string",
                                "description": "A complete standalone description of the work BitFun should perform."
                            },
                            "workspace_id": {
                                "type": "string",
                                "description": "Exact opened workspace id from get_bitfun_client_context. Omit only when the user clearly means the active workspace."
                            },
                            "activate_workspace": {
                                "type": "boolean",
                                "description": "Whether to activate the target workspace and show the new Agent session. Defaults to true; use false only for an explicit background request."
                            }
                        },
                        "required": ["task"]
                    }
                },
                {
                    "type": "function",
                    "name": "stop_bitfun_task",
                    "description": "Stop the BitFun Agent task currently owned by this client-level voice assistant. Use this for any user request to stop, cancel, abort, or interrupt the current task.",
                    "parameters": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {}
                    }
                }
            ],
        },
        "extension": {
            "asr": { "extra": {} },
            "tts": { "extra": {} },
            "dialog": {
                "extra": {
                    "enable_loudness_norm": true,
                    "enable_music": false,
                }
            },
            "extra": { "enable_proactive_speak": true },
        },
    })
}

/// Process either a WebSocket text frame or a UTF-8 JSON payload carried in a
/// binary frame. The official Volcengine clients accept both frame kinds.
/// `Some(true)` means a provider close event was emitted; `Some(false)` means
/// the actor should terminate after an error event.
fn process_provider_frame(
    session_id: &str,
    text: &str,
    on_event: &RealtimeEventHandler,
) -> Option<bool> {
    let payload = match serde_json::from_str::<Value>(text) {
        Ok(payload) => payload,
        Err(error) => {
            emit_error(
                on_event,
                session_id,
                format!("Realtime speech service returned invalid JSON: {error}"),
            );
            return Some(false);
        }
    };
    let event = parse_provider_event(session_id, &payload)?;
    let was_closed = event.kind == SpeechRealtimeEventKind::Closed;
    let was_error = event.kind == SpeechRealtimeEventKind::Error;
    emit_event(on_event, event);
    if was_closed {
        Some(true)
    } else if was_error {
        Some(false)
    } else {
        None
    }
}

async fn send_json<S>(sink: &mut S, payload: Value) -> Result<(), String>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let text = serde_json::to_string(&payload).map_err(|error| error.to_string())?;
    sink.send(Message::Text(text.into()))
        .await
        .map_err(|error| error.to_string())
}

fn parse_provider_event(session_id: &str, payload: &Value) -> Option<SpeechRealtimeEvent> {
    let event_type = payload.get("type")?.as_str()?;
    let mut event = SpeechRealtimeEvent {
        session_id: session_id.to_string(),
        kind: SpeechRealtimeEventKind::Ready,
        text: None,
        audio_base64: None,
        function_calls: Vec::new(),
        provider_session_id: None,
        status_code: status_code(payload),
        message: payload
            .get("message")
            .or_else(|| payload.pointer("/error/message"))
            .and_then(Value::as_str)
            .map(str::to_string),
    };

    match event_type {
        "session.created" => {
            event.kind = SpeechRealtimeEventKind::Ready;
            event.provider_session_id = payload
                .pointer("/session/id")
                .or_else(|| payload.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        "conversation.item.input_audio_transcription.started" => {
            event.kind = SpeechRealtimeEventKind::UserSpeechStarted;
        }
        "conversation.item.input_audio_transcription.delta" => {
            event.kind = SpeechRealtimeEventKind::UserTranscriptDelta;
            event.text = event_text(payload);
        }
        "conversation.item.input_audio_transcription.completed" => {
            event.kind = SpeechRealtimeEventKind::UserTranscriptCompleted;
            event.text = event_text(payload);
        }
        "response.output_text.delta" => {
            event.kind = SpeechRealtimeEventKind::AssistantTextDelta;
            event.text = event_text(payload);
        }
        "response.output_text.done" => {
            event.kind = SpeechRealtimeEventKind::AssistantTextCompleted;
            event.text = event_text(payload);
        }
        "response.output_audio.started" => {
            event.kind = SpeechRealtimeEventKind::AssistantAudioStarted;
        }
        "response.output_audio.delta" => {
            event.kind = SpeechRealtimeEventKind::AssistantAudioDelta;
            event.audio_base64 = payload
                .get("audio")
                .or_else(|| payload.get("delta"))
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        "response.output_audio.done" => {
            event.kind = SpeechRealtimeEventKind::AssistantAudioCompleted;
        }
        "response.function_call_arguments.done" => {
            event.kind = SpeechRealtimeEventKind::FunctionCall;
            let items = match payload.get("items") {
                Some(Value::Array(items)) => items.clone(),
                Some(Value::Object(_)) => vec![payload["items"].clone()],
                _ => Vec::new(),
            };
            event.function_calls = items
                .iter()
                .filter_map(|item| {
                    let call_id = item.get("call_id")?.as_str()?.trim();
                    let name = item
                        .get("name")
                        .or_else(|| item.pointer("/function/name"))?
                        .as_str()?
                        .trim();
                    if call_id.is_empty() || name.is_empty() {
                        return None;
                    }
                    let arguments = item
                        .get("arguments")
                        .or_else(|| item.pointer("/function/arguments"))
                        .map(|value| {
                            value
                                .as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| value.to_string())
                        })
                        .unwrap_or_else(|| "{}".to_string());
                    Some(SpeechRealtimeFunctionCall {
                        call_id: call_id.to_string(),
                        name: name.to_string(),
                        arguments,
                    })
                })
                .collect();
        }
        "session.closed" => {
            event.kind = SpeechRealtimeEventKind::Closed;
        }
        "error" => {
            event.kind = SpeechRealtimeEventKind::Error;
            if event.message.is_none() {
                event.message = Some("Realtime speech service reported an error".to_string());
            }
        }
        _ => return None,
    }

    Some(event)
}

fn event_text(payload: &Value) -> Option<String> {
    ["delta", "transcript", "text", "content"]
        .into_iter()
        .find_map(|key| payload.get(key).and_then(Value::as_str))
        .or_else(|| payload.pointer("/item/transcript").and_then(Value::as_str))
        .map(str::to_string)
}

fn status_code(payload: &Value) -> Option<i64> {
    payload.get("status_code").and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|code| code.parse().ok()))
    })
}

fn emit_event(handler: &RealtimeEventHandler, event: SpeechRealtimeEvent) {
    handler(event);
}

fn emit_error(handler: &RealtimeEventHandler, session_id: &str, message: String) {
    emit_event(
        handler,
        SpeechRealtimeEvent {
            session_id: session_id.to_string(),
            kind: SpeechRealtimeEventKind::Error,
            text: None,
            audio_base64: None,
            function_calls: Vec::new(),
            provider_session_id: None,
            status_code: None,
            message: Some(message),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> VolcengineRealtimeSpeechConfig {
        VolcengineRealtimeSpeechConfig {
            api_key: "fixture-key".to_string(),
            voice: "zh_female_vv_jupiter_bigtts".to_string(),
            speed: 0,
            loudness: 0,
            client_context: Some(r#"{"active_workspace":{"id":"workspace-1"}}"#.to_string()),
        }
    }

    #[test]
    fn session_create_uses_documented_pcm_contract_and_bitfun_tool() {
        let payload = session_create_payload(&config(), "local-session");
        assert_eq!(
            payload.pointer("/session/id"),
            Some(&json!("local-session"))
        );
        assert_eq!(payload.pointer("/session/model"), Some(&json!("1.2.6.1")));
        assert_eq!(
            payload.pointer("/session/audio/input/format"),
            Some(&json!({"type": "pcm", "rate": 16000}))
        );
        assert_eq!(
            payload.pointer("/session/audio/output/format"),
            Some(&json!({"type": "pcm_s16le", "rate": 24000}))
        );
        assert_eq!(
            payload.pointer("/session/tools/0/name"),
            Some(&json!("get_bitfun_client_context"))
        );
        assert_eq!(
            payload.pointer("/session/tools/1/name"),
            Some(&json!("switch_bitfun_workspace"))
        );
        assert_eq!(
            payload.pointer("/session/tools/2/name"),
            Some(&json!("run_bitfun_task"))
        );
        assert_eq!(
            payload.pointer("/session/tools/3/name"),
            Some(&json!("stop_bitfun_task"))
        );
        let instructions = payload
            .pointer("/session/instructions")
            .and_then(Value::as_str)
            .unwrap();
        assert!(instructions.contains("never claim the task stopped"));
        assert!(instructions.contains("outcome_spoken=true"));
        assert!(instructions.contains("workspace-1"));
        assert_eq!(
            payload.pointer("/extension/extra/enable_proactive_speak"),
            Some(&json!(true))
        );
        assert_eq!(
            payload.pointer("/extension/dialog/extra/enable_loudness_norm"),
            Some(&json!(true))
        );
    }

    #[test]
    fn function_call_event_preserves_call_identity_and_arguments() {
        let event = parse_provider_event(
            "local-session",
            &json!({
                "type": "response.function_call_arguments.done",
                "items": [{
                    "call_id": "call-1",
                    "name": "run_bitfun_task",
                    "arguments": "{\"task\":\"run tests\"}"
                }]
            }),
        )
        .unwrap();

        assert_eq!(event.kind, SpeechRealtimeEventKind::FunctionCall);
        assert_eq!(event.function_calls.len(), 1);
        assert_eq!(event.function_calls[0].call_id, "call-1");
        assert_eq!(
            event.function_calls[0].arguments,
            "{\"task\":\"run tests\"}"
        );

        let single_item_event = parse_provider_event(
            "local-session",
            &json!({
                "type": "response.function_call_arguments.done",
                "items": {
                    "call_id": "call-2",
                    "function": {
                        "name": "run_bitfun_task",
                        "arguments": {"task": "inspect the workspace"}
                    }
                }
            }),
        )
        .unwrap();
        assert_eq!(single_item_event.function_calls[0].call_id, "call-2");
        assert_eq!(
            single_item_event.function_calls[0].arguments,
            "{\"task\":\"inspect the workspace\"}"
        );
    }

    #[test]
    fn documented_audio_delta_is_forwarded_from_json_frames() {
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = events.clone();
        let handler: RealtimeEventHandler = Arc::new(move |event| {
            captured.lock().unwrap().push(event);
        });

        let outcome = process_provider_frame(
            "local-session",
            r#"{"type":"response.output_audio.delta","delta":"AQIDBA=="}"#,
            &handler,
        );

        assert_eq!(outcome, None);
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, SpeechRealtimeEventKind::AssistantAudioDelta);
        assert_eq!(events[0].audio_base64.as_deref(), Some("AQIDBA=="));
    }

    #[test]
    fn tool_results_and_spoken_progress_match_the_provider_contract() {
        let tool_result = tool_result_payload("call-1", r#"{"ok":true}"#);
        assert_eq!(
            tool_result.pointer("/items/0/type"),
            Some(&json!("message"))
        );
        assert_eq!(tool_result.pointer("/items/0/role"), Some(&json!("tool")));
        assert_eq!(
            tool_result.pointer("/items/0/call_id"),
            Some(&json!("call-1"))
        );

        let spoken = spoken_text_payload("A short progress update.");
        assert_eq!(
            spoken.get("type"),
            Some(&json!("speech_text_buffer.commit"))
        );
        assert!(spoken
            .get("speech_id")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty()));
        assert_eq!(spoken.get("text"), Some(&json!("A short progress update.")));
    }
}

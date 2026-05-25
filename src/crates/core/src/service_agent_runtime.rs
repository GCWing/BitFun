//! Core-owned bindings for service and agent runtime ports.
//!
//! Owner crates keep portable contracts and orchestration policy. This module
//! centralizes the concrete core adapters that still own scheduler execution,
//! session restore, terminal pre-warm, remote image conversion, and runtime-port
//! implementations until a reviewed port/provider migration proves equivalence.

use bitfun_runtime_ports::{
    AgentSubmissionPort, AgentSubmissionSource, AgentTurnCancellationPort,
    AgentTurnCancellationRequest, RemoteControlStatePort, RemoteControlStateRequest,
    RemoteControlStateSnapshot,
};
use bitfun_services_integrations::remote_connect::{
    ChatImageAttachment, ChatMessage, ChatMessageItem, RemoteCancelRuntimeHost,
    RemoteConnectSubmissionSource, RemoteDefaultModelsConfig, RemoteDialogQueuePriority,
    RemoteDialogResolvedSubmission, RemoteDialogRuntimeHost, RemoteDialogSubmissionPolicy,
    RemoteDialogSubmitOutcome, RemoteImageContext, RemoteImageContextAdapter, RemoteModelCatalog,
    RemoteModelConfig, RemoteSessionStateTracker, RemoteSessionTrackerHost,
    RemoteTerminalPrewarmRequest, RemoteToolStatus, RemoteWorkspaceFileRuntimeHost,
};
use log::{debug, info};
use std::sync::Arc;

use crate::agentic::coordination::{
    get_global_coordinator, get_global_scheduler, ConversationCoordinator, DialogQueuePriority,
    DialogScheduler, DialogSubmissionPolicy, DialogSubmitOutcome, DialogTriggerSource,
};
use crate::agentic::image_analysis::ImageContextData;
use crate::service::remote_connect::remote_server::RemoteExecutionDispatcher;

use crate::service::config::types::{AIConfig, GlobalConfig, ModelCapability, ReasoningMode};
use crate::service::session::{DialogTurnData, TurnStatus};

/// Max thumbnail size per remote chat image sent to mobile (100 KB).
const MOBILE_IMAGE_MAX_BYTES: usize = 100 * 1024;

fn current_workspace_path() -> Option<std::path::PathBuf> {
    crate::service::workspace::get_global_workspace_service()
        .and_then(|service| service.try_get_current_workspace_path())
}

fn normalize_remote_session_model_id(model_id: Option<String>) -> Option<String> {
    match model_id {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() || trimmed == "default" {
                Some("auto".to_string())
            } else {
                Some(trimmed.to_string())
            }
        }
        None => Some("auto".to_string()),
    }
}

fn normalize_remote_model_selection(
    requested_model_id: &str,
    ai_config: Option<&AIConfig>,
) -> Result<String, String> {
    let requested_model_id = requested_model_id.trim();
    if requested_model_id.is_empty() {
        return Err("model_id is required".to_string());
    }

    if matches!(requested_model_id, "auto" | "default" | "primary" | "fast") {
        return Ok(if requested_model_id == "default" {
            "auto".to_string()
        } else {
            requested_model_id.to_string()
        });
    }

    let Some(ai_config) = ai_config else {
        return Err("Config service not available".to_string());
    };
    ai_config
        .resolve_model_reference(requested_model_id)
        .ok_or_else(|| format!("Unknown model selection: {requested_model_id}"))
}

fn remote_model_selection_needs_config(requested_model_id: &str) -> bool {
    let requested_model_id = requested_model_id.trim();
    !requested_model_id.is_empty()
        && !matches!(requested_model_id, "auto" | "default" | "primary" | "fast")
}

/// Compress a base64 data-URL image to a small thumbnail for mobile display.
/// Falls back to the original if decoding/compression fails or the image is
/// already within `max_bytes`.
fn compress_remote_chat_data_url_for_mobile(data_url: &str, max_bytes: usize) -> String {
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;
    use image::imageops::FilterType;

    const MAX_THUMBNAIL_DIM: u32 = 400;

    let Some(comma_pos) = data_url.find(',') else {
        return data_url.to_string();
    };
    let b64_data = &data_url[comma_pos + 1..];

    if b64_data.len() * 3 / 4 <= max_bytes {
        return data_url.to_string();
    }

    let Ok(raw_bytes) = BASE64.decode(b64_data) else {
        return data_url.to_string();
    };

    let Ok(img) = image::load_from_memory(&raw_bytes) else {
        return data_url.to_string();
    };

    let resized = if img.width() > MAX_THUMBNAIL_DIM || img.height() > MAX_THUMBNAIL_DIM {
        img.resize(MAX_THUMBNAIL_DIM, MAX_THUMBNAIL_DIM, FilterType::Triangle)
    } else {
        img
    };

    fn encode_jpeg(img: &image::DynamicImage, quality: u8) -> Option<Vec<u8>> {
        let mut buf = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
        img.write_with_encoder(encoder).ok()?;
        Some(buf)
    }

    for quality in [75u8, 60, 45, 30] {
        if let Some(buf) = encode_jpeg(&resized, quality) {
            if buf.len() <= max_bytes || quality == 30 {
                let b64 = BASE64.encode(&buf);
                return format!("data:image/jpeg;base64,{b64}");
            }
        }
    }

    data_url.to_string()
}

/// Convert persisted turns into mobile ChatMessages.
/// This is the same data source the desktop frontend uses.
fn remote_chat_messages_from_turns(turns: &[DialogTurnData]) -> Vec<ChatMessage> {
    let mut result = Vec::new();

    for turn in turns {
        if !turn.kind.is_model_visible() {
            continue;
        }

        let images = turn
            .user_message
            .metadata
            .as_ref()
            .and_then(|m| m.get("images"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        let name = v.get("name")?.as_str()?.to_string();
                        let raw_url = v.get("data_url")?.as_str()?;
                        let data_url = compress_remote_chat_data_url_for_mobile(
                            raw_url,
                            MOBILE_IMAGE_MAX_BYTES,
                        );
                        Some(ChatImageAttachment { name, data_url })
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());

        // Prefer original_text from metadata (pre-enhancement) for display.
        let display_content = turn
            .user_message
            .metadata
            .as_ref()
            .and_then(|m| m.get("original_text"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| strip_remote_user_input_tags(&turn.user_message.content));

        result.push(ChatMessage {
            id: turn.user_message.id.clone(),
            role: "user".to_string(),
            content: display_content,
            timestamp: (turn.user_message.timestamp / 1000).to_string(),
            metadata: None,
            tools: None,
            thinking: None,
            items: None,
            images,
        });

        // Skip assistant message for in-progress turns. The active turn's
        // content is delivered via the real-time overlay, not the historical
        // list. Including an empty or partial assistant message here would
        // consume a slot in the count-based skip cursor and prevent the final
        // version from ever being delivered.
        if turn.status == TurnStatus::InProgress {
            continue;
        }

        // Collect ordered items across all rounds, preserving interleaved order.
        struct OrderedEntry {
            order_index: Option<usize>,
            sequence: usize,
            round_idx: usize,
            item: ChatMessageItem,
        }

        let mut ordered: Vec<OrderedEntry> = Vec::new();
        let mut tools_flat = Vec::new();
        let mut thinking_parts = Vec::new();
        let mut text_parts = Vec::new();
        let mut sequence = 0usize;

        for (round_idx, round) in turn.model_rounds.iter().enumerate() {
            // Iterate in streaming order: thinking, text, tools.
            // The model first thinks, then outputs text, and finally the tools
            // are detected/executed. This matches the real-time tracker order.
            for t in &round.thinking_items {
                if t.is_subagent_item.unwrap_or(false) {
                    continue;
                }
                if !t.content.is_empty() {
                    thinking_parts.push(t.content.clone());
                    ordered.push(OrderedEntry {
                        order_index: t.order_index,
                        sequence,
                        round_idx,
                        item: ChatMessageItem {
                            item_type: "thinking".to_string(),
                            content: Some(t.content.clone()),
                            tool: None,
                            is_subagent: None,
                        },
                    });
                    sequence += 1;
                }
            }
            for t in &round.text_items {
                if t.is_subagent_item.unwrap_or(false) {
                    continue;
                }
                if !t.content.is_empty() {
                    text_parts.push(t.content.clone());
                    ordered.push(OrderedEntry {
                        order_index: t.order_index,
                        sequence,
                        round_idx,
                        item: ChatMessageItem {
                            item_type: "text".to_string(),
                            content: Some(t.content.clone()),
                            tool: None,
                            is_subagent: None,
                        },
                    });
                    sequence += 1;
                }
            }
            for t in &round.tool_items {
                if t.is_subagent_item.unwrap_or(false) {
                    continue;
                }
                let status_str = t.status.as_deref().unwrap_or(if t.tool_result.is_some() {
                    "completed"
                } else {
                    "running"
                });
                let tool_status = RemoteToolStatus {
                    id: t.id.clone(),
                    name: t.tool_name.clone(),
                    status: status_str.to_string(),
                    duration_ms: t.duration_ms,
                    start_ms: Some(t.start_time),
                    input_preview:
                        bitfun_services_integrations::remote_connect::make_slim_tool_params(
                            &t.tool_call.input,
                        ),
                    tool_input: if t.tool_name == "AskUserQuestion"
                        || t.tool_name == "Task"
                        || t.tool_name == "TodoWrite"
                    {
                        Some(t.tool_call.input.clone())
                    } else {
                        None
                    },
                };
                tools_flat.push(tool_status.clone());
                ordered.push(OrderedEntry {
                    order_index: t.order_index,
                    sequence,
                    round_idx,
                    item: ChatMessageItem {
                        item_type: "tool".to_string(),
                        content: None,
                        tool: Some(tool_status),
                        is_subagent: None,
                    },
                });
                sequence += 1;
            }
        }

        // Sort by round first (rounds are strictly sequential), then by
        // order_index within each round. order_index is per-round, so it must
        // not be compared across rounds.
        ordered.sort_by(|a, b| {
            let round_cmp = a.round_idx.cmp(&b.round_idx);
            if round_cmp != std::cmp::Ordering::Equal {
                return round_cmp;
            }
            match (a.order_index, b.order_index) {
                (Some(a_idx), Some(b_idx)) => a_idx.cmp(&b_idx),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.sequence.cmp(&b.sequence),
            }
        });
        let items: Vec<ChatMessageItem> = ordered.into_iter().map(|e| e.item).collect();

        let ts = turn
            .model_rounds
            .last()
            .map(|r| r.end_time.unwrap_or(r.start_time))
            .unwrap_or(turn.start_time);

        result.push(ChatMessage {
            id: format!("{}_assistant", turn.turn_id),
            role: "assistant".to_string(),
            content: text_parts.join("\n\n"),
            timestamp: (ts / 1000).to_string(),
            metadata: None,
            tools: if tools_flat.is_empty() {
                None
            } else {
                Some(tools_flat)
            },
            thinking: if thinking_parts.is_empty() {
                None
            } else {
                Some(thinking_parts.join("\n\n"))
            },
            items: if items.is_empty() { None } else { Some(items) },
            images: None,
        });
    }

    result
}

fn strip_remote_user_input_tags(content: &str) -> String {
    let s = crate::agentic::core::strip_prompt_markup(content);
    if s.starts_with("User uploaded") {
        if let Some(pos) = s.find("User's question:\n") {
            return s[pos + "User's question:\n".len()..].trim().to_string();
        }
    }
    s
}

async fn resolve_session_model_id(session_id: &str) -> Option<String> {
    let coordinator = get_global_coordinator()?;
    let session_manager = coordinator.get_session_manager();

    if let Some(session) = session_manager.get_session(session_id) {
        return normalize_remote_session_model_id(session.config.model_id.clone());
    }

    let workspace_path =
        CoreServiceAgentRuntime::resolve_session_workspace_path(session_id).await?;
    coordinator
        .restore_session(&workspace_path, session_id)
        .await
        .ok()
        .and_then(|session| normalize_remote_session_model_id(session.config.model_id.clone()))
}

fn core_dialog_submission_policy(policy: RemoteDialogSubmissionPolicy) -> DialogSubmissionPolicy {
    let trigger_source = match policy.source {
        RemoteConnectSubmissionSource::Relay => DialogTriggerSource::RemoteRelay,
        RemoteConnectSubmissionSource::Bot => DialogTriggerSource::Bot,
    };
    let queue_priority = match policy.queue_priority {
        RemoteDialogQueuePriority::Low => DialogQueuePriority::Low,
        RemoteDialogQueuePriority::Normal => DialogQueuePriority::Normal,
        RemoteDialogQueuePriority::High => DialogQueuePriority::High,
    };

    DialogSubmissionPolicy::new(
        trigger_source,
        queue_priority,
        policy.skip_tool_confirmation,
    )
}

impl RemoteImageContextAdapter for ImageContextData {
    fn from_remote_image_context(context: RemoteImageContext) -> Self {
        Self {
            id: context.id,
            image_path: context.image_path,
            data_url: context.data_url,
            mime_type: context.mime_type,
            metadata: context.metadata,
        }
    }
}

pub(crate) struct CoreServiceAgentRuntime;

impl CoreServiceAgentRuntime {
    pub(crate) async fn resolve_session_workspace_path(
        session_id: &str,
    ) -> Option<std::path::PathBuf> {
        let coordinator = get_global_coordinator()?;
        coordinator.resolve_session_workspace_path(session_id).await
    }

    pub(crate) async fn resolve_remote_file_workspace_root(
        session_id: Option<&str>,
    ) -> Option<std::path::PathBuf> {
        if let Some(session_id) = session_id {
            if let Some(workspace_path) = Self::resolve_session_workspace_path(session_id).await {
                return Some(workspace_path);
            }
        }

        current_workspace_path()
    }

    pub(crate) fn remote_dialog_host(
        dispatcher: &RemoteExecutionDispatcher,
    ) -> Result<CoreRemoteDialogRuntimeHost<'_>, String> {
        CoreRemoteDialogRuntimeHost::new(dispatcher)
    }

    pub(crate) fn remote_cancel_host() -> Result<CoreRemoteCancelRuntimeHost, String> {
        CoreRemoteCancelRuntimeHost::new()
    }

    pub(crate) fn remote_workspace_file_host() -> CoreRemoteWorkspaceFileRuntimeHost {
        CoreRemoteWorkspaceFileRuntimeHost::new()
    }

    pub(crate) fn remote_image_context(context: RemoteImageContext) -> ImageContextData {
        ImageContextData::from_remote_image_context(context)
    }

    pub(crate) async fn load_remote_chat_messages(
        workspace_path: &std::path::Path,
        session_id: &str,
    ) -> (Vec<ChatMessage>, bool) {
        let Ok(pm) = crate::infrastructure::PathManager::new() else {
            return (vec![], false);
        };
        let pm = std::sync::Arc::new(pm);
        let Ok(store) = crate::agentic::persistence::PersistenceManager::new(pm) else {
            return (vec![], false);
        };
        let Ok(turns) = store.load_session_turns(workspace_path, session_id).await else {
            return (vec![], false);
        };
        (remote_chat_messages_from_turns(&turns), false)
    }

    pub(crate) async fn load_remote_model_catalog(
        session_id: Option<&str>,
    ) -> Result<RemoteModelCatalog, String> {
        let config_service = crate::service::config::get_global_config_service()
            .await
            .map_err(|e| format!("Config service not available: {e}"))?;
        let global_config: GlobalConfig = config_service
            .get_config(None)
            .await
            .map_err(|e| format!("Failed to load global config: {e}"))?;
        let ai_config: AIConfig = global_config.ai;

        let models: Vec<RemoteModelConfig> = ai_config
            .models
            .into_iter()
            .map(|model| {
                let reasoning_mode = model.effective_reasoning_mode();

                RemoteModelConfig {
                    id: model.id,
                    name: model.name,
                    provider: model.provider,
                    base_url: model.base_url,
                    model_name: model.model_name,
                    context_window: model.context_window,
                    enabled: model.enabled,
                    capabilities: model
                        .capabilities
                        .into_iter()
                        .map(|capability| {
                            match capability {
                                ModelCapability::TextChat => "text_chat",
                                ModelCapability::ImageUnderstanding => "image_understanding",
                                ModelCapability::ImageGeneration => "image_generation",
                                ModelCapability::Embedding => "embedding",
                                ModelCapability::Search => "search",
                                ModelCapability::CodeSpecialized => "code_specialized",
                                ModelCapability::FunctionCalling => "function_calling",
                                ModelCapability::SpeechRecognition => "speech_recognition",
                            }
                            .to_string()
                        })
                        .collect(),
                    enable_thinking_process: model.enable_thinking_process,
                    reasoning_mode: Some(
                        match reasoning_mode {
                            ReasoningMode::Default => "default",
                            ReasoningMode::Enabled => "enabled",
                            ReasoningMode::Disabled => "disabled",
                            ReasoningMode::Adaptive => "adaptive",
                        }
                        .to_string(),
                    ),
                    reasoning_effort: model.reasoning_effort,
                    thinking_budget_tokens: model.thinking_budget_tokens,
                }
            })
            .collect();

        let session_model_id = if let Some(session_id) = session_id {
            resolve_session_model_id(session_id).await
        } else {
            None
        };
        Ok(RemoteModelCatalog {
            version: global_config.last_modified.timestamp_millis().max(0) as u64,
            models,
            default_models: RemoteDefaultModelsConfig {
                primary: ai_config.default_models.primary,
                fast: ai_config.default_models.fast,
                search: ai_config.default_models.search,
                image_understanding: ai_config.default_models.image_understanding,
                image_generation: ai_config.default_models.image_generation,
                speech_recognition: ai_config.default_models.speech_recognition,
            },
            session_model_id,
        })
    }

    pub(crate) async fn update_remote_session_model(
        coordinator: &ConversationCoordinator,
        session_id: &str,
        model_id: &str,
    ) -> Result<String, String> {
        let ai_config = if remote_model_selection_needs_config(model_id) {
            let config_service = crate::service::config::get_global_config_service()
                .await
                .map_err(|_| "Config service not available".to_string())?;
            Some(
                config_service
                    .get_config::<AIConfig>(Some("ai"))
                    .await
                    .map_err(|e| format!("Failed to load AI config: {e}"))?,
            )
        } else {
            None
        };
        let normalized_model_id = normalize_remote_model_selection(model_id, ai_config.as_ref())?;

        if coordinator
            .get_session_manager()
            .get_session(session_id)
            .is_none()
        {
            let Some(workspace_path) = Self::resolve_session_workspace_path(session_id).await
            else {
                return Err(format!(
                    "Workspace path not available for session: {session_id}"
                ));
            };
            coordinator
                .restore_session(&workspace_path, session_id)
                .await
                .map_err(|e| format!("Failed to restore session: {e}"))?;
        }

        coordinator
            .get_session_manager()
            .update_session_model_id(session_id, &normalized_model_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(normalized_model_id)
    }

    pub(crate) fn agent_submission_port(
        coordinator: &ConversationCoordinator,
    ) -> &(dyn AgentSubmissionPort + '_) {
        coordinator
    }

    pub(crate) fn agent_turn_cancellation_port(
        coordinator: &ConversationCoordinator,
    ) -> &(dyn AgentTurnCancellationPort + '_) {
        coordinator
    }

    pub(crate) fn remote_control_state_port(
        coordinator: &ConversationCoordinator,
    ) -> &(dyn RemoteControlStatePort + '_) {
        coordinator
    }
}

pub(crate) struct CoreRemoteSessionTrackerHost;

#[async_trait::async_trait]
impl crate::agentic::events::EventSubscriber for Arc<RemoteSessionStateTracker> {
    async fn on_event(
        &self,
        event: &crate::agentic::events::AgenticEvent,
    ) -> crate::util::errors::BitFunResult<()> {
        self.handle_agentic_event(event);
        Ok(())
    }
}

impl RemoteSessionTrackerHost for CoreRemoteSessionTrackerHost {
    fn subscribe_tracker(&self, session_id: &str, tracker: Arc<RemoteSessionStateTracker>) {
        if let Some(coordinator) = get_global_coordinator() {
            let sub_id = format!("remote_tracker_{}", session_id);
            coordinator.subscribe_internal(sub_id, tracker);
            info!("Registered state tracker for session {session_id}");
        }
    }

    fn unsubscribe_tracker(&self, session_id: &str) {
        if let Some(coordinator) = get_global_coordinator() {
            let sub_id = format!("remote_tracker_{}", session_id);
            coordinator.unsubscribe_internal(&sub_id);
        }
    }

    fn active_turn_id(&self, session_id: &str) -> Option<String> {
        let coordinator = get_global_coordinator()?;
        let session_mgr = coordinator.get_session_manager();
        let session = session_mgr.get_session(session_id)?;
        match &session.state {
            crate::agentic::core::SessionState::Processing {
                current_turn_id, ..
            } => {
                info!(
                    "Seeded tracker with existing active turn {} for session {}",
                    current_turn_id, session_id
                );
                Some(current_turn_id.clone())
            }
            _ => None,
        }
    }
}

pub(crate) struct CoreRemoteDialogRuntimeHost<'a> {
    dispatcher: &'a RemoteExecutionDispatcher,
    coordinator: Arc<ConversationCoordinator>,
    scheduler: Arc<DialogScheduler>,
}

impl<'a> CoreRemoteDialogRuntimeHost<'a> {
    pub(crate) fn new(dispatcher: &'a RemoteExecutionDispatcher) -> Result<Self, String> {
        let coordinator = get_global_coordinator()
            .ok_or_else(|| "Desktop session system not ready".to_string())?;
        let scheduler = get_global_scheduler()
            .ok_or_else(|| "Dialog scheduler is not initialized".to_string())?;

        Ok(Self {
            dispatcher,
            coordinator,
            scheduler,
        })
    }
}

pub(crate) struct CoreRemoteCancelRuntimeHost {
    coordinator: Arc<ConversationCoordinator>,
}

impl CoreRemoteCancelRuntimeHost {
    pub(crate) fn new() -> Result<Self, String> {
        let coordinator = get_global_coordinator()
            .ok_or_else(|| "Desktop session system not ready".to_string())?;
        Ok(Self { coordinator })
    }
}

pub(crate) struct CoreRemoteWorkspaceFileRuntimeHost;

impl CoreRemoteWorkspaceFileRuntimeHost {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RemoteDialogRuntimeHost for CoreRemoteDialogRuntimeHost<'_> {
    type ImageContext = ImageContextData;

    fn ensure_tracker(&self, session_id: &str) {
        self.dispatcher.ensure_tracker(session_id);
    }

    async fn resolve_binding_workspace(&self, session_id: &str) -> Option<String> {
        self.coordinator
            .resolve_session_workspace_path(session_id)
            .await
            .map(|path| path.to_string_lossy().into_owned())
    }

    async fn remote_session_exists(&self, session_id: &str) -> Result<bool, String> {
        Ok(self
            .coordinator
            .get_session_manager()
            .get_session(session_id)
            .is_some())
    }

    async fn restore_remote_session(
        &self,
        session_id: &str,
        workspace_path: &str,
    ) -> Result<(), String> {
        self.coordinator
            .restore_session(std::path::Path::new(workspace_path), session_id)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn prewarm_remote_terminal(&self, request: RemoteTerminalPrewarmRequest) {
        use terminal_core::session::SessionSource;
        use terminal_core::{TerminalApi, TerminalBindingOptions};

        let sid = request.session_id;
        let binding_workspace_for_terminal = request.binding_workspace;
        tokio::spawn(async move {
            let Ok(api) = TerminalApi::from_singleton() else {
                return;
            };
            let binding = api.session_manager().binding();
            if binding.get(&sid).is_some() {
                return;
            }
            let workspace = binding_workspace_for_terminal;
            let name = format!("Chat-{}", &sid[..8.min(sid.len())]);
            match binding
                .get_or_create(
                    &sid,
                    TerminalBindingOptions {
                        working_directory: workspace,
                        session_id: Some(sid.clone()),
                        session_name: Some(name),
                        env: Some(
                            crate::agentic::tools::implementations::bash_tool::BashTool::noninteractive_env(),
                        ),
                        source: Some(SessionSource::Agent),
                        ..Default::default()
                    },
                )
                .await
            {
                Ok(_) => info!("Terminal pre-warmed for remote session {sid}"),
                Err(e) => debug!("Terminal pre-warm skipped for {sid}: {e}"),
            }
        });
    }

    fn generate_turn_id(&self) -> String {
        format!("turn_{}", chrono::Utc::now().timestamp_millis())
    }

    async fn submit_dialog(
        &self,
        submission: RemoteDialogResolvedSubmission<Self::ImageContext>,
    ) -> Result<RemoteDialogSubmitOutcome, String> {
        let image_payload = if submission.image_contexts.is_empty() {
            None
        } else {
            Some(submission.image_contexts)
        };
        let policy = core_dialog_submission_policy(submission.policy);

        self.scheduler
            .submit(
                submission.session_id,
                submission.content,
                None,
                Some(submission.turn_id),
                submission.resolved_agent_type,
                submission.binding_workspace,
                policy,
                None,
                None,
                image_payload,
            )
            .await
            .map(|outcome| match outcome {
                DialogSubmitOutcome::Started {
                    session_id,
                    turn_id,
                } => RemoteDialogSubmitOutcome::Started {
                    session_id,
                    turn_id,
                },
                DialogSubmitOutcome::Queued {
                    session_id,
                    turn_id,
                } => RemoteDialogSubmitOutcome::Queued {
                    session_id,
                    turn_id,
                },
            })
    }
}

#[async_trait::async_trait]
impl RemoteWorkspaceFileRuntimeHost for CoreRemoteWorkspaceFileRuntimeHost {
    async fn resolve_remote_file_workspace_root(
        &self,
        session_id: Option<&str>,
    ) -> Option<std::path::PathBuf> {
        CoreServiceAgentRuntime::resolve_remote_file_workspace_root(session_id).await
    }
}

#[async_trait::async_trait]
impl RemoteCancelRuntimeHost for CoreRemoteCancelRuntimeHost {
    async fn resolve_restore_workspace(&self, session_id: &str) -> Option<String> {
        self.coordinator
            .resolve_session_workspace_path(session_id)
            .await
            .map(|path| path.to_string_lossy().into_owned())
    }

    async fn remote_control_state(
        &self,
        session_id: &str,
    ) -> Result<Option<RemoteControlStateSnapshot>, String> {
        let state_port =
            CoreServiceAgentRuntime::remote_control_state_port(self.coordinator.as_ref());
        state_port
            .read_remote_control_state(RemoteControlStateRequest {
                session_id: session_id.to_string(),
            })
            .await
            .map_err(|error| error.message)
    }

    async fn restore_remote_session(
        &self,
        session_id: &str,
        workspace_path: &str,
    ) -> Result<(), String> {
        self.coordinator
            .restore_session(std::path::Path::new(workspace_path), session_id)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    async fn cancel_remote_turn(&self, session_id: &str, turn_id: &str) -> Result<(), String> {
        let cancellation_port =
            CoreServiceAgentRuntime::agent_turn_cancellation_port(self.coordinator.as_ref());
        cancellation_port
            .cancel_turn(AgentTurnCancellationRequest {
                session_id: session_id.to_string(),
                turn_id: Some(turn_id.to_string()),
                source: Some(AgentSubmissionSource::RemoteRelay),
                reason: None,
                wait_timeout_ms: None,
            })
            .await
            .map(|_| ())
            .map_err(|error| error.message)
    }
}

#[cfg(test)]
mod tests {
    use bitfun_runtime_ports::SessionTranscriptReader;

    use super::*;
    use crate::service::session::{
        DialogTurnData, DialogTurnKind, ModelRoundData, TextItemData, ThinkingItemData,
        ToolCallData, ToolItemData, TurnStatus, UserMessageData,
    };

    #[test]
    fn core_service_agent_runtime_owner_keeps_coordinator_port_contracts() {
        fn assert_runtime_ports<T>()
        where
            T: AgentSubmissionPort
                + AgentTurnCancellationPort
                + RemoteControlStatePort
                + SessionTranscriptReader,
        {
        }

        assert_runtime_ports::<ConversationCoordinator>();
    }

    #[test]
    fn core_service_agent_runtime_owner_exposes_remote_control_ports() {
        fn assert_port_accessors(
            coordinator: &ConversationCoordinator,
        ) -> (
            &(dyn AgentTurnCancellationPort + '_),
            &(dyn RemoteControlStatePort + '_),
        ) {
            (
                CoreServiceAgentRuntime::agent_turn_cancellation_port(coordinator),
                CoreServiceAgentRuntime::remote_control_state_port(coordinator),
            )
        }

        let _ = assert_port_accessors;
    }

    #[test]
    fn core_service_agent_runtime_owner_maps_remote_dialog_policy() {
        let relay = core_dialog_submission_policy(RemoteDialogSubmissionPolicy {
            source: RemoteConnectSubmissionSource::Relay,
            queue_priority: RemoteDialogQueuePriority::High,
            skip_tool_confirmation: true,
        });
        assert_eq!(relay.trigger_source, DialogTriggerSource::RemoteRelay);
        assert_eq!(relay.queue_priority, DialogQueuePriority::High);
        assert!(relay.skip_tool_confirmation);

        let bot = core_dialog_submission_policy(RemoteDialogSubmissionPolicy {
            source: RemoteConnectSubmissionSource::Bot,
            queue_priority: RemoteDialogQueuePriority::Low,
            skip_tool_confirmation: false,
        });
        assert_eq!(bot.trigger_source, DialogTriggerSource::Bot);
        assert_eq!(bot.queue_priority, DialogQueuePriority::Low);
        assert!(!bot.skip_tool_confirmation);
    }

    #[test]
    fn core_service_agent_runtime_owner_normalizes_remote_session_model_ids() {
        assert_eq!(
            normalize_remote_session_model_id(None),
            Some("auto".to_string())
        );
        assert_eq!(
            normalize_remote_session_model_id(Some("".to_string())),
            Some("auto".to_string())
        );
        assert_eq!(
            normalize_remote_session_model_id(Some("  default  ".to_string())),
            Some("auto".to_string())
        );
        assert_eq!(
            normalize_remote_session_model_id(Some(" model-1 ".to_string())),
            Some("model-1".to_string())
        );
    }

    #[test]
    fn core_service_agent_runtime_owner_normalizes_remote_model_selection_aliases() {
        assert_eq!(
            normalize_remote_model_selection("auto", None).unwrap(),
            "auto"
        );
        assert_eq!(
            normalize_remote_model_selection("default", None).unwrap(),
            "auto"
        );
        assert_eq!(
            normalize_remote_model_selection("primary", None).unwrap(),
            "primary"
        );
        assert_eq!(
            normalize_remote_model_selection("fast", None).unwrap(),
            "fast"
        );
        assert_eq!(
            normalize_remote_model_selection("   ", None).unwrap_err(),
            "model_id is required"
        );
    }

    #[test]
    fn core_service_agent_runtime_owner_preserves_remote_chat_history_shape() {
        let turn = remote_history_test_turn(
            TurnStatus::Completed,
            Some(serde_json::json!({
                "original_text": "original question",
                "images": [
                    {
                        "name": "screenshot.png",
                        "data_url": "data:image/png;base64,abcd"
                    }
                ]
            })),
        );

        let messages = remote_chat_messages_from_turns(&[turn]);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "original question");
        assert_eq!(
            messages[0].images.as_ref().unwrap()[0].name,
            "screenshot.png"
        );

        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "visible text");
        assert_eq!(messages[1].thinking.as_deref(), Some("visible thought"));
        let items = messages[1].items.as_ref().expect("assistant items");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].item_type, "thinking");
        assert_eq!(items[1].item_type, "text");
        assert_eq!(items[2].item_type, "tool");
        assert_eq!(
            messages[1].tools.as_ref().unwrap()[0].name,
            "AskUserQuestion"
        );
    }

    #[test]
    fn core_service_agent_runtime_owner_skips_in_progress_remote_assistant_history() {
        let turn = remote_history_test_turn(TurnStatus::InProgress, None);

        let messages = remote_chat_messages_from_turns(&[turn]);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
    }

    #[test]
    fn core_service_agent_runtime_owner_strips_enhanced_remote_user_input() {
        let content = "User uploaded a file.\nUser's question:\n  explain this  ";

        assert_eq!(strip_remote_user_input_tags(content), "explain this");
    }

    fn remote_history_test_turn(
        status: TurnStatus,
        metadata: Option<serde_json::Value>,
    ) -> DialogTurnData {
        DialogTurnData {
            turn_id: "turn-1".to_string(),
            turn_index: 0,
            session_id: "session-1".to_string(),
            timestamp: 1_000,
            kind: DialogTurnKind::UserDialog,
            agent_type: None,
            user_message: UserMessageData {
                id: "user-1".to_string(),
                content: "fallback text".to_string(),
                timestamp: 1_000,
                metadata,
            },
            model_rounds: vec![ModelRoundData {
                id: "round-1".to_string(),
                turn_id: "turn-1".to_string(),
                round_index: 0,
                timestamp: 1_100,
                text_items: vec![
                    TextItemData {
                        id: "text-hidden".to_string(),
                        content: "hidden text".to_string(),
                        is_streaming: false,
                        timestamp: 1_111,
                        is_markdown: true,
                        order_index: Some(1),
                        is_subagent_item: Some(true),
                        parent_task_tool_id: None,
                        subagent_session_id: None,
                        status: None,
                    },
                    TextItemData {
                        id: "text-1".to_string(),
                        content: "visible text".to_string(),
                        is_streaming: false,
                        timestamp: 1_112,
                        is_markdown: true,
                        order_index: Some(1),
                        is_subagent_item: None,
                        parent_task_tool_id: None,
                        subagent_session_id: None,
                        status: None,
                    },
                ],
                tool_items: vec![ToolItemData {
                    id: "tool-1".to_string(),
                    tool_name: "AskUserQuestion".to_string(),
                    tool_call: ToolCallData {
                        input: serde_json::json!({ "question": "confirm?" }),
                        id: "call-1".to_string(),
                    },
                    tool_result: None,
                    ai_intent: None,
                    start_time: 1_130,
                    end_time: None,
                    duration_ms: Some(25),
                    queue_wait_ms: None,
                    preflight_ms: None,
                    confirmation_wait_ms: None,
                    execution_ms: None,
                    order_index: Some(2),
                    is_subagent_item: None,
                    parent_task_tool_id: None,
                    subagent_session_id: None,
                    subagent_model_id: None,
                    subagent_model_alias: None,
                    status: Some("running".to_string()),
                    interruption_reason: None,
                }],
                thinking_items: vec![ThinkingItemData {
                    id: "thinking-1".to_string(),
                    content: "visible thought".to_string(),
                    is_streaming: false,
                    is_collapsed: false,
                    timestamp: 1_105,
                    order_index: Some(0),
                    status: None,
                    is_subagent_item: None,
                    parent_task_tool_id: None,
                    subagent_session_id: None,
                }],
                start_time: 1_100,
                end_time: Some(1_200),
                duration_ms: Some(100),
                provider_id: None,
                model_id: None,
                model_alias: None,
                first_chunk_ms: None,
                first_visible_output_ms: None,
                stream_duration_ms: None,
                attempt_count: None,
                failure_category: None,
                token_details: None,
                status: "completed".to_string(),
            }],
            start_time: 1_000,
            end_time: Some(1_250),
            duration_ms: Some(250),
            status,
        }
    }
}

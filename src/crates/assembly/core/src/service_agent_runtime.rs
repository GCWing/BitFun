//! Core-owned bindings for service and agent runtime ports.
//!
//! Owner crates keep portable contracts and orchestration policy. This module
//! centralizes the concrete core adapters that still own scheduler execution,
//! session restore, terminal pre-warm, remote image conversion, and runtime-port
//! implementations until a reviewed port/provider migration proves equivalence.

use bitfun_runtime_ports::{
    AgentSessionCreateRequest, AgentSubmissionPort, AgentSubmissionSource,
    AgentTurnCancellationPort, AgentTurnCancellationRequest, RemoteControlStatePort,
    RemoteControlStateRequest, RemoteControlStateSnapshot, RuntimeServiceCapability,
    RuntimeServicePort,
};
use bitfun_services_integrations::remote_connect::{
    build_remote_chat_messages, build_remote_model_catalog,
    normalize_remote_model_selection as normalize_remote_model_selection_contract,
    normalize_remote_session_model_id as normalize_remote_session_model_id_contract,
    remote_dialog_submit_outcome_from_scheduler,
    remote_model_selection_needs_config as remote_model_selection_needs_config_contract,
    ChatImageAttachment, ChatMessage, RemoteAssistantWorkspaceFacts, RemoteCancelRuntimeHost,
    RemoteChatHistoryRound, RemoteChatHistoryTextItem, RemoteChatHistoryThinkingItem,
    RemoteChatHistoryToolCall, RemoteChatHistoryToolItem, RemoteChatHistoryTurn,
    RemoteConnectSubmissionSource, RemoteDefaultModelsConfig, RemoteDialogQueuePriority,
    RemoteDialogResolvedSubmission, RemoteDialogRuntimeHost, RemoteDialogSchedulerOutcomeFact,
    RemoteDialogSubmissionPolicy, RemoteDialogSubmitOutcome, RemoteImageContext,
    RemoteImageContextAdapter, RemoteInitialSyncRuntimeHost, RemoteInteractionRuntimeHost,
    RemoteModelCapabilityFact, RemoteModelCatalog, RemoteModelCatalogFacts, RemoteModelFacts,
    RemotePollRuntimeHost, RemoteReasoningModeFact, RemoteRecentWorkspaceFacts,
    RemoteSessionMetadata, RemoteSessionRuntimeHost, RemoteSessionStateTracker,
    RemoteSessionTrackerHost, RemoteTerminalPrewarmRequest, RemoteWorkspaceFacts,
    RemoteWorkspaceFileRuntimeHost, RemoteWorkspaceKind as RemoteConnectWorkspaceKind,
    RemoteWorkspaceRuntimeHost, RemoteWorkspaceUpdate,
};
use log::{debug, error, info};
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

fn remote_workspace_kind(
    kind: crate::service::workspace::WorkspaceKind,
) -> RemoteConnectWorkspaceKind {
    match kind {
        crate::service::workspace::WorkspaceKind::Normal => RemoteConnectWorkspaceKind::Normal,
        crate::service::workspace::WorkspaceKind::Assistant => {
            RemoteConnectWorkspaceKind::Assistant
        }
        crate::service::workspace::WorkspaceKind::Remote => RemoteConnectWorkspaceKind::Remote,
    }
}

fn git_branch_for_workspace_path(path: &std::path::Path) -> Option<String> {
    git2::Repository::open(path).ok().and_then(|repo| {
        repo.head()
            .ok()
            .and_then(|head| head.shorthand().ok().map(String::from))
    })
}

async fn current_remote_workspace_facts() -> Option<RemoteWorkspaceFacts> {
    let workspace_service = crate::service::workspace::get_global_workspace_service()?;
    workspace_service
        .get_current_workspace()
        .await
        .map(|workspace| {
            let root_path = workspace.root_path.clone();
            RemoteWorkspaceFacts {
                path: root_path.to_string_lossy().to_string(),
                name: workspace.name,
                git_branch: git_branch_for_workspace_path(&root_path),
                kind: remote_workspace_kind(workspace.workspace_kind),
                assistant_id: workspace.assistant_id,
            }
        })
}

async fn open_workspace_with_snapshot(
    path: &str,
    snapshot_log_context: &str,
) -> Result<RemoteWorkspaceUpdate, String> {
    let workspace_service = crate::service::workspace::get_global_workspace_service()
        .ok_or_else(|| "Workspace service not available".to_string())?;
    let path_buf = std::path::PathBuf::from(path);
    let info = workspace_service
        .open_workspace(path_buf)
        .await
        .map_err(|error| error.to_string())?;
    if let Err(error) = crate::service::snapshot::initialize_snapshot_manager_for_workspace(
        info.root_path.clone(),
        None,
    )
    .await
    {
        error!("Failed to initialize snapshot after {snapshot_log_context}: {error}");
    }
    Ok(RemoteWorkspaceUpdate {
        path: info.root_path.to_string_lossy().to_string(),
        name: info.name,
    })
}

async fn load_remote_session_metadata_for_workspace(
    workspace_path: &std::path::Path,
) -> Result<Vec<RemoteSessionMetadata>, String> {
    let workspace_path_display = workspace_path.to_string_lossy().to_string();
    let path_manager = crate::infrastructure::PathManager::new()
        .map_err(|_| "Failed to initialize path manager".to_string())?;
    let path_manager = std::sync::Arc::new(path_manager);
    let store =
        crate::agentic::persistence::PersistenceManager::new(path_manager).map_err(|error| {
            debug!("PersistenceManager init failed for {workspace_path_display}: {error}");
            format!("Failed to initialize session storage: {error}")
        })?;
    let metadata = store
        .list_session_metadata(workspace_path)
        .await
        .map_err(|error| {
            debug!("Session list read failed for {workspace_path_display}: {error}");
            format!("Failed to list sessions for workspace: {error}")
        })?;

    Ok(metadata
        .into_iter()
        .map(|session| RemoteSessionMetadata {
            session_id: session.session_id,
            name: session.session_name,
            agent_type: session.agent_type,
            created_at_ms: session.created_at,
            last_active_at_ms: session.last_active_at,
            turn_count: session.turn_count,
        })
        .collect())
}

fn normalize_remote_session_model_id(model_id: Option<String>) -> Option<String> {
    normalize_remote_session_model_id_contract(model_id.as_deref())
}

fn normalize_remote_model_selection(
    requested_model_id: &str,
    ai_config: Option<&AIConfig>,
) -> Result<String, String> {
    if remote_model_selection_needs_config(requested_model_id) && ai_config.is_none() {
        return Err("Config service not available".to_string());
    }

    normalize_remote_model_selection_contract(requested_model_id, |model_id| {
        ai_config.and_then(|config| config.resolve_model_reference(model_id))
    })
}

fn remote_model_selection_needs_config(requested_model_id: &str) -> bool {
    remote_model_selection_needs_config_contract(requested_model_id)
}

fn remote_model_capability_fact(capability: ModelCapability) -> RemoteModelCapabilityFact {
    match capability {
        ModelCapability::TextChat => RemoteModelCapabilityFact::TextChat,
        ModelCapability::ImageUnderstanding => RemoteModelCapabilityFact::ImageUnderstanding,
        ModelCapability::ImageGeneration => RemoteModelCapabilityFact::ImageGeneration,
        ModelCapability::Embedding => RemoteModelCapabilityFact::Embedding,
        ModelCapability::Search => RemoteModelCapabilityFact::Search,
        ModelCapability::CodeSpecialized => RemoteModelCapabilityFact::CodeSpecialized,
        ModelCapability::FunctionCalling => RemoteModelCapabilityFact::FunctionCalling,
        ModelCapability::SpeechRecognition => RemoteModelCapabilityFact::SpeechRecognition,
    }
}

fn remote_reasoning_mode_fact(reasoning_mode: ReasoningMode) -> RemoteReasoningModeFact {
    match reasoning_mode {
        ReasoningMode::Default => RemoteReasoningModeFact::Default,
        ReasoningMode::Enabled => RemoteReasoningModeFact::Enabled,
        ReasoningMode::Disabled => RemoteReasoningModeFact::Disabled,
        ReasoningMode::Adaptive => RemoteReasoningModeFact::Adaptive,
    }
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
    let projected_turns = turns
        .iter()
        .filter(|turn| turn.kind.is_model_visible())
        .map(remote_chat_history_turn_from_core_turn)
        .collect::<Vec<_>>();
    build_remote_chat_messages(projected_turns)
}

fn remote_chat_history_turn_from_core_turn(turn: &DialogTurnData) -> RemoteChatHistoryTurn {
    let user_images = turn
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
                    let data_url =
                        compress_remote_chat_data_url_for_mobile(raw_url, MOBILE_IMAGE_MAX_BYTES);
                    Some(ChatImageAttachment { name, data_url })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Prefer original_text from metadata (pre-enhancement) for display.
    let user_display_content = turn
        .user_message
        .metadata
        .as_ref()
        .and_then(|m| m.get("original_text"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| strip_remote_user_input_tags(&turn.user_message.content));

    let rounds = turn
        .model_rounds
        .iter()
        .map(|round| RemoteChatHistoryRound {
            start_time_ms: round.start_time,
            end_time_ms: round.end_time,
            text_items: round
                .text_items
                .iter()
                .map(|item| RemoteChatHistoryTextItem {
                    content: item.content.clone(),
                    order_index: item.order_index,
                    is_subagent: item.is_subagent_item.unwrap_or(false),
                })
                .collect(),
            thinking_items: round
                .thinking_items
                .iter()
                .map(|item| RemoteChatHistoryThinkingItem {
                    content: item.content.clone(),
                    order_index: item.order_index,
                    is_subagent: item.is_subagent_item.unwrap_or(false),
                })
                .collect(),
            tool_items: round
                .tool_items
                .iter()
                .map(|item| RemoteChatHistoryToolItem {
                    id: item.id.clone(),
                    name: item.tool_name.clone(),
                    call: RemoteChatHistoryToolCall {
                        id: item.tool_call.id.clone(),
                        input: item.tool_call.input.clone(),
                    },
                    has_result: item.tool_result.is_some(),
                    status: item.status.clone(),
                    duration_ms: item.duration_ms,
                    start_ms: item.start_time,
                    order_index: item.order_index,
                    is_subagent: item.is_subagent_item.unwrap_or(false),
                })
                .collect(),
        })
        .collect();

    RemoteChatHistoryTurn {
        turn_id: turn.turn_id.clone(),
        user_message_id: turn.user_message.id.clone(),
        user_display_content,
        user_timestamp_ms: turn.user_message.timestamp,
        user_images,
        is_in_progress: turn.status == TurnStatus::InProgress,
        start_time_ms: turn.start_time,
        rounds,
    }
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

fn remote_dialog_scheduler_outcome_fact(
    outcome: DialogSubmitOutcome,
) -> RemoteDialogSchedulerOutcomeFact {
    match outcome {
        DialogSubmitOutcome::Started {
            session_id,
            turn_id,
        } => RemoteDialogSchedulerOutcomeFact::Started {
            session_id,
            turn_id,
        },
        DialogSubmitOutcome::Queued {
            session_id,
            turn_id,
        } => RemoteDialogSchedulerOutcomeFact::Queued {
            session_id,
            turn_id,
        },
    }
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

    pub(crate) fn remote_workspace_host() -> CoreRemoteWorkspaceRuntimeHost {
        CoreRemoteWorkspaceRuntimeHost::new()
    }

    pub(crate) fn remote_initial_sync_host() -> CoreRemoteWorkspaceRuntimeHost {
        CoreRemoteWorkspaceRuntimeHost::new()
    }

    pub(crate) fn remote_session_host() -> Result<CoreRemoteSessionRuntimeHost, String> {
        CoreRemoteSessionRuntimeHost::new()
    }

    pub(crate) fn remote_poll_host(
        dispatcher: &RemoteExecutionDispatcher,
    ) -> CoreRemotePollRuntimeHost<'_> {
        CoreRemotePollRuntimeHost::new(dispatcher)
    }

    pub(crate) fn remote_interaction_host() -> CoreRemoteInteractionRuntimeHost {
        CoreRemoteInteractionRuntimeHost::new()
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

        let models: Vec<RemoteModelFacts> = ai_config
            .models
            .into_iter()
            .map(|model| {
                let reasoning_mode = model.effective_reasoning_mode();

                RemoteModelFacts {
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
                        .map(remote_model_capability_fact)
                        .collect(),
                    enable_thinking_process: model.enable_thinking_process,
                    reasoning_mode: Some(remote_reasoning_mode_fact(reasoning_mode)),
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
        Ok(build_remote_model_catalog(RemoteModelCatalogFacts {
            last_modified_ms: global_config.last_modified.timestamp_millis(),
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
        }))
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

pub(crate) struct CoreRemoteWorkspaceRuntimeHost;

impl CoreRemoteWorkspaceRuntimeHost {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl RuntimeServicePort for CoreRemoteWorkspaceFileRuntimeHost {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::RemoteProjection
    }
}

impl RuntimeServicePort for CoreRemoteWorkspaceRuntimeHost {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::RemoteWorkspace
    }
}

pub(crate) struct CoreRemoteSessionRuntimeHost {
    coordinator: Arc<ConversationCoordinator>,
}

impl CoreRemoteSessionRuntimeHost {
    pub(crate) fn new() -> Result<Self, String> {
        let coordinator = get_global_coordinator()
            .ok_or_else(|| "Desktop session system not ready".to_string())?;
        Ok(Self { coordinator })
    }
}

pub(crate) struct CoreRemotePollRuntimeHost<'a> {
    dispatcher: &'a RemoteExecutionDispatcher,
}

impl<'a> CoreRemotePollRuntimeHost<'a> {
    pub(crate) fn new(dispatcher: &'a RemoteExecutionDispatcher) -> Self {
        Self { dispatcher }
    }
}

pub(crate) struct CoreRemoteInteractionRuntimeHost {
    coordinator: Option<Arc<ConversationCoordinator>>,
}

impl CoreRemoteInteractionRuntimeHost {
    pub(crate) fn new() -> Self {
        Self {
            coordinator: get_global_coordinator(),
        }
    }

    fn coordinator(&self) -> Result<&ConversationCoordinator, String> {
        self.coordinator
            .as_deref()
            .ok_or_else(|| "Desktop session system not ready".to_string())
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
            .map(remote_dialog_scheduler_outcome_fact)
            .map(remote_dialog_submit_outcome_from_scheduler)
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
impl RemoteWorkspaceRuntimeHost for CoreRemoteWorkspaceRuntimeHost {
    async fn current_workspace(&self) -> Option<RemoteWorkspaceFacts> {
        current_remote_workspace_facts().await
    }

    async fn recent_workspaces(&self) -> Vec<RemoteRecentWorkspaceFacts> {
        let Some(workspace_service) = crate::service::workspace::get_global_workspace_service()
        else {
            return Vec::new();
        };
        workspace_service
            .get_recent_workspaces()
            .await
            .into_iter()
            .map(|workspace| RemoteRecentWorkspaceFacts {
                path: workspace.root_path.to_string_lossy().to_string(),
                name: workspace.name,
                last_opened: workspace.last_accessed.to_rfc3339(),
                kind: remote_workspace_kind(workspace.workspace_kind),
            })
            .collect()
    }

    async fn open_workspace(&self, path: &str) -> Result<RemoteWorkspaceUpdate, String> {
        open_workspace_with_snapshot(path, "remote workspace set").await
    }

    async fn assistant_workspaces(&self) -> Vec<RemoteAssistantWorkspaceFacts> {
        let Some(workspace_service) = crate::service::workspace::get_global_workspace_service()
        else {
            return Vec::new();
        };
        workspace_service
            .get_assistant_workspaces()
            .await
            .into_iter()
            .map(|workspace| RemoteAssistantWorkspaceFacts {
                path: workspace.root_path.to_string_lossy().to_string(),
                name: workspace.name,
                assistant_id: workspace.assistant_id,
            })
            .collect()
    }

    async fn open_assistant_workspace(&self, path: &str) -> Result<RemoteWorkspaceUpdate, String> {
        open_workspace_with_snapshot(path, "remote assistant set").await
    }
}

#[async_trait::async_trait]
impl RemoteInitialSyncRuntimeHost for CoreRemoteWorkspaceRuntimeHost {
    async fn current_workspace(&self) -> Option<RemoteWorkspaceFacts> {
        current_remote_workspace_facts().await
    }

    async fn list_session_metadata(
        &self,
        workspace_path: &std::path::Path,
    ) -> Result<Vec<RemoteSessionMetadata>, String> {
        load_remote_session_metadata_for_workspace(workspace_path).await
    }
}

#[async_trait::async_trait]
impl RemoteSessionRuntimeHost for CoreRemoteSessionRuntimeHost {
    async fn list_session_metadata(
        &self,
        workspace_path: &std::path::Path,
    ) -> Result<Vec<RemoteSessionMetadata>, String> {
        load_remote_session_metadata_for_workspace(workspace_path).await
    }

    async fn resolve_default_assistant_workspace_path(&self) -> Result<String, String> {
        let workspace_service = crate::service::workspace::get_global_workspace_service()
            .ok_or_else(|| "Workspace service not available".to_string())?;
        let workspaces = workspace_service.get_assistant_workspaces().await;
        if let Some(default_workspace) = workspaces
            .into_iter()
            .find(|workspace| workspace.assistant_id.is_none())
        {
            return Ok(default_workspace.root_path.to_string_lossy().to_string());
        }

        workspace_service
            .create_assistant_workspace(None)
            .await
            .map(|workspace| workspace.root_path.to_string_lossy().to_string())
            .map_err(|error| format!("Failed to create assistant workspace: {}", error))
    }

    async fn create_session(&self, request: AgentSessionCreateRequest) -> Result<String, String> {
        let submission_port =
            CoreServiceAgentRuntime::agent_submission_port(self.coordinator.as_ref());
        submission_port
            .create_session(request)
            .await
            .map(|session| session.session_id)
            .map_err(|error| error.message)
    }

    async fn load_model_catalog(
        &self,
        session_id: Option<&str>,
    ) -> Result<RemoteModelCatalog, String> {
        CoreServiceAgentRuntime::load_remote_model_catalog(session_id).await
    }

    async fn update_session_model(
        &self,
        session_id: &str,
        model_id: &str,
    ) -> Result<String, String> {
        CoreServiceAgentRuntime::update_remote_session_model(
            self.coordinator.as_ref(),
            session_id,
            model_id,
        )
        .await
    }

    async fn ensure_session_loaded(&self, session_id: &str) -> Result<(), String> {
        if self
            .coordinator
            .get_session_manager()
            .get_session(session_id)
            .is_some()
        {
            return Ok(());
        }

        let Some(workspace_path) =
            CoreServiceAgentRuntime::resolve_session_workspace_path(session_id).await
        else {
            return Err(format!(
                "Workspace path not available for session: {}",
                session_id
            ));
        };
        self.coordinator
            .restore_session(&workspace_path, session_id)
            .await
            .map(|_| ())
            .map_err(|error| format!("Failed to restore session: {error}"))
    }

    async fn update_session_title(&self, session_id: &str, title: &str) -> Result<String, String> {
        self.coordinator
            .update_session_title(session_id, title)
            .await
            .map_err(|error| error.to_string())
    }

    async fn resolve_session_workspace_path(&self, session_id: &str) -> Option<std::path::PathBuf> {
        CoreServiceAgentRuntime::resolve_session_workspace_path(session_id).await
    }

    async fn load_remote_chat_messages(
        &self,
        workspace_path: &std::path::Path,
        session_id: &str,
    ) -> (Vec<ChatMessage>, bool) {
        CoreServiceAgentRuntime::load_remote_chat_messages(workspace_path, session_id).await
    }

    async fn delete_session(
        &self,
        workspace_path: &std::path::Path,
        session_id: &str,
    ) -> Result<(), String> {
        self.coordinator
            .delete_session(workspace_path, session_id)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn remove_tracker(&self, session_id: &str) {
        crate::service::remote_connect::remote_server::get_or_init_global_dispatcher()
            .remove_tracker(session_id);
    }
}

#[async_trait::async_trait]
impl RemotePollRuntimeHost for CoreRemotePollRuntimeHost<'_> {
    fn ensure_tracker(&self, session_id: &str) -> Arc<RemoteSessionStateTracker> {
        self.dispatcher.ensure_tracker(session_id)
    }

    async fn load_model_catalog(&self, session_id: &str) -> Option<RemoteModelCatalog> {
        CoreServiceAgentRuntime::load_remote_model_catalog(Some(session_id))
            .await
            .ok()
    }

    async fn resolve_session_workspace_path(&self, session_id: &str) -> Option<std::path::PathBuf> {
        CoreServiceAgentRuntime::resolve_session_workspace_path(session_id).await
    }

    async fn load_remote_chat_messages(
        &self,
        workspace_path: &std::path::Path,
        session_id: &str,
    ) -> (Vec<ChatMessage>, bool) {
        CoreServiceAgentRuntime::load_remote_chat_messages(workspace_path, session_id).await
    }
}

#[async_trait::async_trait]
impl RemoteInteractionRuntimeHost for CoreRemoteInteractionRuntimeHost {
    async fn confirm_tool(
        &self,
        tool_id: &str,
        updated_input: Option<serde_json::Value>,
    ) -> Result<(), String> {
        self.coordinator()?
            .confirm_tool(tool_id, updated_input)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    async fn reject_tool(&self, tool_id: &str, reason: String) -> Result<(), String> {
        self.coordinator()?
            .reject_tool(tool_id, reason)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    async fn cancel_tool(&self, tool_id: &str, reason: String) -> Result<(), String> {
        self.coordinator()?
            .cancel_tool(tool_id, reason)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn answer_question(&self, tool_id: &str, answers: serde_json::Value) -> Result<(), String> {
        crate::agentic::tools::user_input_manager::get_user_input_manager()
            .send_answer(tool_id, answers)
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
        assert_eq!(
            normalize_remote_model_selection("custom-alias", None).unwrap_err(),
            "Config service not available"
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

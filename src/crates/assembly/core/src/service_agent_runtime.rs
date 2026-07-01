//! Core-owned bindings for service and agent runtime ports.
//!
//! Owner crates keep portable contracts and orchestration policy. This module
//! centralizes the concrete core adapters that still own scheduler execution,
//! session restore, terminal pre-warm, remote image conversion, and runtime-port
//! implementations until a reviewed port/provider migration proves equivalence.

use bitfun_agent_runtime::sdk::{AgentRuntime, AgentRuntimeBuilder, RuntimeError};
use bitfun_runtime_ports::{
    AgentDialogTurnPort, AgentDialogTurnRequest, AgentInputAttachment, AgentLifecycleDeliveryPort,
    AgentSessionCreateRequest, AgentSessionManagementPort, AgentSubmissionPort,
    AgentSubmissionSource, AgentThreadGoalManagementPort, AgentTurnCancellationPort,
    AgentTurnCancellationRequest, RemoteControlStatePort, RemoteControlStateRequest,
    RemoteControlStateSnapshot, RemoteSessionWorkspaceIdentity, RuntimeServiceCapability,
    RuntimeServicePort, SessionStoragePathRequest, SessionStorePort,
};
use bitfun_services_integrations::remote_connect::{
    agent_input_attachment_from_remote_image_context, build_remote_chat_messages,
    build_remote_model_catalog,
    normalize_remote_model_selection as normalize_remote_model_selection_contract,
    normalize_remote_session_model_id, project_remote_chat_user,
    remote_dialog_submit_outcome_from_scheduler, remote_model_selection_needs_config, ChatMessage,
    RemoteAssistantWorkspaceFacts, RemoteCancelRuntimeHost, RemoteChatHistoryRound,
    RemoteChatHistoryTextItem, RemoteChatHistoryThinkingItem, RemoteChatHistoryToolCall,
    RemoteChatHistoryToolItem, RemoteChatHistoryTurn, RemoteConnectSubmissionSource,
    RemoteDefaultModelsConfig, RemoteDialogQueuePriority, RemoteDialogResolvedSubmission,
    RemoteDialogRuntimeHost, RemoteDialogSchedulerOutcomeFact, RemoteDialogSubmissionPolicy,
    RemoteDialogSubmitOutcome, RemoteDialogWorkspaceBinding, RemoteImageContext,
    RemoteInitialSyncRuntimeHost, RemoteInteractionRuntimeHost, RemoteModelCapabilityFact,
    RemoteModelCatalog, RemoteModelCatalogFacts, RemoteModelFacts, RemotePollRuntimeHost,
    RemoteReasoningModeFact, RemoteRecentWorkspaceFacts, RemoteSessionMetadata,
    RemoteSessionRuntimeHost, RemoteSessionStateTracker, RemoteSessionTrackerHost,
    RemoteTerminalPrewarmRequest, RemoteWorkspaceFacts, RemoteWorkspaceFileRuntimeHost,
    RemoteWorkspaceKind as RemoteConnectWorkspaceKind, RemoteWorkspaceRuntimeHost,
    RemoteWorkspaceUpdate,
};
use log::{debug, error, info};
use std::sync::Arc;

use crate::agentic::coordination::{
    get_global_coordinator, get_global_scheduler, ConversationCoordinator, DialogQueuePriority,
    DialogScheduler, DialogSubmissionPolicy, DialogSubmitOutcome, DialogTriggerSource,
};
use crate::agentic::image_analysis::ImageContextData;
use crate::agentic::session::session_store_port::CoreSessionStorePort;
use crate::agentic::workspace::WorkspaceBinding;
use crate::service::remote_connect::remote_server::RemoteExecutionDispatcher;

use crate::service::config::types::{AIConfig, GlobalConfig, ModelCapability, ReasoningMode};
use crate::service::session::{DialogTurnData, TurnStatus};

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
    let path_str = path.to_string_lossy();
    bitfun_services_integrations::git::execute_git_command_sync(
        &path_str,
        &["rev-parse", "--abbrev-ref", "HEAD"],
    )
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty() && s != "HEAD")
}

fn workspace_metadata_string(
    metadata: &std::collections::HashMap<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    metadata
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
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
                remote_connection_id: workspace_metadata_string(
                    &workspace.metadata,
                    "connectionId",
                ),
                remote_ssh_host: workspace_metadata_string(&workspace.metadata, "sshHost"),
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
    workspace_identity: RemoteSessionWorkspaceIdentity,
) -> Result<Vec<RemoteSessionMetadata>, String> {
    let workspace_path_display = workspace_path.to_string_lossy().to_string();
    let session_storage_dir = CoreSessionStorePort::default()
        .resolve_session_storage_path(SessionStoragePathRequest {
            workspace_path: workspace_path.to_path_buf(),
            remote_connection_id: workspace_identity.remote_connection_id,
            remote_ssh_host: workspace_identity.remote_ssh_host,
        })
        .await
        .map(|resolution| resolution.effective_storage_path)
        .map_err(|error| {
            debug!("Session storage path resolution failed for {workspace_path_display}: {error}");
            format!("Failed to resolve session storage for workspace: {error}")
        })?;
    let path_manager = crate::infrastructure::PathManager::new()
        .map_err(|_| "Failed to initialize path manager".to_string())?;
    let path_manager = std::sync::Arc::new(path_manager);
    let store =
        crate::agentic::persistence::PersistenceManager::new(path_manager).map_err(|error| {
            debug!("PersistenceManager init failed for {workspace_path_display}: {error}");
            format!("Failed to initialize session storage: {error}")
        })?;
    let metadata = store
        .list_session_metadata(&session_storage_dir)
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
    let prompt_visible_content =
        crate::agentic::core::strip_prompt_markup(&turn.user_message.content);
    let user_projection =
        project_remote_chat_user(turn.user_message.metadata.as_ref(), &prompt_visible_content);

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
        user_display_content: user_projection.content,
        user_timestamp_ms: turn.user_message.timestamp,
        user_images: user_projection.images,
        is_in_progress: turn.status == TurnStatus::InProgress,
        start_time_ms: turn.start_time,
        rounds,
    }
}

async fn resolve_session_model_id(session_id: &str) -> Option<String> {
    let coordinator = get_global_coordinator()?;
    let session_manager = coordinator.get_session_manager();

    if let Some(session) = session_manager.get_session(session_id) {
        return normalize_remote_session_model_id(session.config.model_id.as_deref());
    }

    let session_storage_dir =
        CoreServiceAgentRuntime::resolve_session_storage_dir(session_id).await?;
    coordinator
        .restore_session_from_storage_path(&session_storage_dir, session_id)
        .await
        .ok()
        .and_then(|session| normalize_remote_session_model_id(session.config.model_id.as_deref()))
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

fn remote_image_context_from_image_context(context: ImageContextData) -> RemoteImageContext {
    RemoteImageContext {
        id: context.id,
        image_path: context.image_path,
        data_url: context.data_url,
        mime_type: context.mime_type,
        metadata: context.metadata,
    }
}

fn image_context_from_remote_image_context(context: RemoteImageContext) -> ImageContextData {
    ImageContextData {
        id: context.id,
        image_path: context.image_path,
        data_url: context.data_url,
        mime_type: context.mime_type,
        metadata: context.metadata,
    }
}

fn agent_input_attachment_from_image_context(context: ImageContextData) -> AgentInputAttachment {
    agent_input_attachment_from_remote_image_context(remote_image_context_from_image_context(
        context,
    ))
}

fn core_agent_runtime_builder(
    submission: Arc<dyn AgentSubmissionPort>,
    session_management: Arc<dyn AgentSessionManagementPort>,
    thread_goal_management: Arc<dyn AgentThreadGoalManagementPort>,
    cancellation: Arc<dyn AgentTurnCancellationPort>,
) -> AgentRuntimeBuilder {
    let agent_registry: Arc<dyn bitfun_agent_runtime::sdk::RuntimeAgentRegistry> =
        crate::agentic::agents::get_agent_registry();
    AgentRuntimeBuilder::new()
        .with_submission_port(submission)
        .with_session_management_port(session_management)
        .with_thread_goal_management_port(thread_goal_management)
        .with_cancellation_port(cancellation)
        .with_agent_registry(agent_registry)
}

pub(crate) struct CoreServiceAgentRuntime;

impl CoreServiceAgentRuntime {
    async fn resolve_session_workspace_binding(session_id: &str) -> Option<WorkspaceBinding> {
        let coordinator = get_global_coordinator()?;
        coordinator
            .get_session_manager()
            .resolve_session_workspace_binding(session_id)
            .await
    }

    pub(crate) async fn resolve_session_workspace_paths(
        session_id: &str,
    ) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
        Self::resolve_session_workspace_binding(session_id)
            .await
            .map(|binding| {
                (
                    binding.logical_workspace_path().to_path_buf(),
                    binding.session_storage_dir(),
                )
            })
    }

    pub(crate) async fn resolve_session_storage_dir(
        session_id: &str,
    ) -> Option<std::path::PathBuf> {
        Self::resolve_session_workspace_paths(session_id)
            .await
            .map(|(_, storage_dir)| storage_dir)
    }

    pub(crate) async fn resolve_session_logical_workspace_path(
        session_id: &str,
    ) -> Option<std::path::PathBuf> {
        Self::resolve_session_workspace_paths(session_id)
            .await
            .map(|(workspace_path, _)| workspace_path)
    }

    pub(crate) async fn resolve_remote_file_workspace_root(
        session_id: Option<&str>,
    ) -> Option<std::path::PathBuf> {
        if let Some(session_id) = session_id {
            if let Some(workspace_path) =
                Self::resolve_session_logical_workspace_path(session_id).await
            {
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
        image_context_from_remote_image_context(context)
    }

    pub(crate) async fn load_remote_chat_messages(
        session_storage_dir: &std::path::Path,
        session_id: &str,
    ) -> (Vec<ChatMessage>, bool) {
        let Ok(pm) = crate::infrastructure::PathManager::new() else {
            return (vec![], false);
        };
        let pm = std::sync::Arc::new(pm);
        let Ok(store) = crate::agentic::persistence::PersistenceManager::new(pm) else {
            return (vec![], false);
        };
        let Ok(turns) = store
            .load_session_turns(session_storage_dir, session_id)
            .await
        else {
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
            let Some(session_storage_dir) = Self::resolve_session_storage_dir(session_id).await
            else {
                return Err(format!(
                    "Session storage directory not available for session: {session_id}"
                ));
            };
            coordinator
                .restore_session_from_storage_path(&session_storage_dir, session_id)
                .await
                .map_err(|e| format!("Failed to restore session: {e}"))?;
        }

        coordinator
            .get_session_manager()
            .update_session_model_id(session_id, &normalized_model_id)
            .await
            .map_err(|e| e.to_string())?;

        // Propagate the model choice to every agent type already present in
        // `ai.agent_models` so that newly created sessions of any type
        // (including different agent types like Cowork/Claw) inherit it.
        // Also ensure the current session's agent type is present.  This
        // covers mobile-web and IM-bot paths; the desktop client handles
        // its own `ai.agent_models` writing in the frontend.
        Self::persist_model_for_all_agents(&normalized_model_id, || {
            coordinator
                .get_session_manager()
                .get_session(session_id)
                .map(|s| s.agent_type.clone())
        })
        .await;

        Ok(normalized_model_id)
    }

    /// Write `model_id` to `ai.agent_models` for **every** agent type already
    /// present in the config, plus the current session's agent type if it is
    /// not yet listed.  This ensures newly created sessions of any type pick
    /// up the same model without hardcoding a fixed list of agent types.
    async fn persist_model_for_all_agents<F>(model_id: &str, current_agent_type: F)
    where
        F: FnOnce() -> Option<String>,
    {
        let Ok(config_service) = crate::service::config::get_global_config_service().await else {
            return;
        };
        let mut current: std::collections::HashMap<String, String> = config_service
            .get_config(Some("ai.agent_models"))
            .await
            .unwrap_or_default();
        for value in current.values_mut() {
            *value = model_id.to_string();
        }
        if let Some(agent_type) = current_agent_type() {
            current.insert(agent_type, model_id.to_string());
        }
        let _ = config_service.set_config("ai.agent_models", &current).await;
    }

    pub(crate) fn remote_control_state_port(
        coordinator: &ConversationCoordinator,
    ) -> &(dyn RemoteControlStatePort + '_) {
        coordinator
    }

    pub(crate) fn agent_runtime(
        coordinator: Arc<ConversationCoordinator>,
    ) -> Result<AgentRuntime, String> {
        let submission: Arc<dyn AgentSubmissionPort> = coordinator.clone();
        let session_management: Arc<dyn AgentSessionManagementPort> = coordinator.clone();
        let thread_goal_management: Arc<dyn AgentThreadGoalManagementPort> = coordinator.clone();
        let cancellation: Arc<dyn AgentTurnCancellationPort> = coordinator;
        core_agent_runtime_builder(
            submission,
            session_management,
            thread_goal_management,
            cancellation,
        )
        .build()
        .map_err(|error| error.to_string())
    }

    pub(crate) fn agent_runtime_with_dialog_turns(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
    ) -> Result<AgentRuntime, String> {
        let submission: Arc<dyn AgentSubmissionPort> = coordinator.clone();
        let session_management: Arc<dyn AgentSessionManagementPort> = coordinator.clone();
        let thread_goal_management: Arc<dyn AgentThreadGoalManagementPort> = coordinator.clone();
        let cancellation: Arc<dyn AgentTurnCancellationPort> = coordinator;
        let dialog_turn: Arc<dyn AgentDialogTurnPort> = scheduler.clone();
        let lifecycle_delivery: Arc<dyn AgentLifecycleDeliveryPort> = scheduler;
        core_agent_runtime_builder(
            submission,
            session_management,
            thread_goal_management,
            cancellation,
        )
        .with_dialog_turn_port(dialog_turn)
        .with_lifecycle_delivery_port(lifecycle_delivery)
        .build()
        .map_err(|error| error.to_string())
    }

    pub(crate) fn agent_runtime_with_lifecycle_delivery(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
    ) -> Result<AgentRuntime, String> {
        let submission: Arc<dyn AgentSubmissionPort> = coordinator.clone();
        let session_management: Arc<dyn AgentSessionManagementPort> = coordinator.clone();
        let thread_goal_management: Arc<dyn AgentThreadGoalManagementPort> = coordinator.clone();
        let cancellation: Arc<dyn AgentTurnCancellationPort> = coordinator;
        let lifecycle_delivery: Arc<dyn AgentLifecycleDeliveryPort> = scheduler;
        core_agent_runtime_builder(
            submission,
            session_management,
            thread_goal_management,
            cancellation,
        )
        .with_lifecycle_delivery_port(lifecycle_delivery)
        .build()
        .map_err(|error| error.to_string())
    }

    pub(crate) fn agent_runtime_with_scheduler_ports(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
    ) -> Result<AgentRuntime, String> {
        let submission: Arc<dyn AgentSubmissionPort> = coordinator.clone();
        let session_management: Arc<dyn AgentSessionManagementPort> = coordinator.clone();
        let thread_goal_management: Arc<dyn AgentThreadGoalManagementPort> = coordinator;
        let cancellation: Arc<dyn AgentTurnCancellationPort> = scheduler.clone();
        let dialog_turn: Arc<dyn AgentDialogTurnPort> = scheduler.clone();
        let lifecycle_delivery: Arc<dyn AgentLifecycleDeliveryPort> = scheduler;
        core_agent_runtime_builder(
            submission,
            session_management,
            thread_goal_management,
            cancellation,
        )
        .with_dialog_turn_port(dialog_turn)
        .with_lifecycle_delivery_port(lifecycle_delivery)
        .build()
        .map_err(|error| error.to_string())
    }

    pub(crate) fn global_agent_runtime_with_lifecycle_delivery() -> Result<AgentRuntime, String> {
        let coordinator = get_global_coordinator()
            .ok_or_else(|| "Desktop session system not ready".to_string())?;
        let scheduler = get_global_scheduler()
            .ok_or_else(|| "Dialog scheduler is not initialized".to_string())?;
        Self::agent_runtime_with_lifecycle_delivery(coordinator, scheduler)
    }

    pub(crate) fn runtime_error_message(error: RuntimeError) -> String {
        match error {
            RuntimeError::Port(error) => error.message,
            other => other.to_string(),
        }
    }
}

pub(crate) struct CoreRemoteSessionTrackerHost;

struct CoreRemoteSessionStateTrackerSubscriber(Arc<RemoteSessionStateTracker>);

#[async_trait::async_trait]
impl crate::agentic::events::EventSubscriber for CoreRemoteSessionStateTrackerSubscriber {
    async fn on_event(
        &self,
        event: &crate::agentic::events::AgenticEvent,
    ) -> bitfun_agent_runtime::event_bus::EventSubscriberResult {
        self.0.handle_agentic_event(event);
        Ok(())
    }
}

impl RemoteSessionTrackerHost for CoreRemoteSessionTrackerHost {
    fn subscribe_tracker(&self, session_id: &str, tracker: Arc<RemoteSessionStateTracker>) {
        if let Some(coordinator) = get_global_coordinator() {
            let sub_id = format!("remote_tracker_{}", session_id);
            coordinator
                .subscribe_internal(sub_id, CoreRemoteSessionStateTrackerSubscriber(tracker));
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
    runtime: AgentRuntime,
}

impl<'a> CoreRemoteDialogRuntimeHost<'a> {
    pub(crate) fn new(dispatcher: &'a RemoteExecutionDispatcher) -> Result<Self, String> {
        let coordinator = get_global_coordinator()
            .ok_or_else(|| "Desktop session system not ready".to_string())?;
        let scheduler = get_global_scheduler()
            .ok_or_else(|| "Dialog scheduler is not initialized".to_string())?;
        let runtime = CoreServiceAgentRuntime::agent_runtime_with_dialog_turns(
            coordinator.clone(),
            scheduler,
        )?;

        Ok(Self {
            dispatcher,
            coordinator,
            runtime,
        })
    }
}

pub(crate) struct CoreRemoteCancelRuntimeHost {
    coordinator: Arc<ConversationCoordinator>,
    runtime: AgentRuntime,
}

impl CoreRemoteCancelRuntimeHost {
    pub(crate) fn new() -> Result<Self, String> {
        let coordinator = get_global_coordinator()
            .ok_or_else(|| "Desktop session system not ready".to_string())?;
        let runtime = CoreServiceAgentRuntime::agent_runtime(coordinator.clone())?;
        Ok(Self {
            coordinator,
            runtime,
        })
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
    runtime: AgentRuntime,
}

impl CoreRemoteSessionRuntimeHost {
    pub(crate) fn new() -> Result<Self, String> {
        let coordinator = get_global_coordinator()
            .ok_or_else(|| "Desktop session system not ready".to_string())?;
        let runtime = CoreServiceAgentRuntime::agent_runtime(coordinator.clone())?;
        Ok(Self {
            coordinator,
            runtime,
        })
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

    async fn resolve_binding_workspace(
        &self,
        session_id: &str,
    ) -> Option<RemoteDialogWorkspaceBinding> {
        self.coordinator
            .get_session_manager()
            .resolve_session_workspace_binding(session_id)
            .await
            .map(|binding| RemoteDialogWorkspaceBinding {
                workspace_path: binding.logical_workspace_path_string(),
                remote_connection_id: binding.connection_id().map(ToOwned::to_owned),
                remote_ssh_host: if binding.is_remote() {
                    Some(binding.session_identity.hostname.clone())
                        .filter(|value| !value.trim().is_empty())
                } else {
                    None
                },
            })
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
        workspace: RemoteDialogWorkspaceBinding,
    ) -> Result<(), String> {
        if let Some(session_storage_dir) =
            CoreServiceAgentRuntime::resolve_session_storage_dir(session_id).await
        {
            self.coordinator
                .restore_session_from_storage_path(&session_storage_dir, session_id)
                .await
        } else {
            self.coordinator
                .restore_session_for_workspace(
                    SessionStoragePathRequest {
                        workspace_path: std::path::PathBuf::from(workspace.workspace_path),
                        remote_connection_id: workspace.remote_connection_id,
                        remote_ssh_host: workspace.remote_ssh_host,
                    },
                    session_id,
                )
                .await
        }
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
        let policy = core_dialog_submission_policy(submission.policy);
        let attachments = submission
            .image_contexts
            .into_iter()
            .map(agent_input_attachment_from_image_context)
            .collect();

        let binding_workspace = submission.binding_workspace;
        let workspace_path = binding_workspace
            .as_ref()
            .map(|binding| binding.workspace_path.clone());
        let remote_connection_id = binding_workspace
            .as_ref()
            .and_then(|binding| binding.remote_connection_id.clone());
        let remote_ssh_host = binding_workspace
            .as_ref()
            .and_then(|binding| binding.remote_ssh_host.clone());

        self.runtime
            .submit_dialog_turn(AgentDialogTurnRequest {
                session_id: submission.session_id,
                message: submission.content,
                original_message: None,
                turn_id: Some(submission.turn_id),
                agent_type: submission.resolved_agent_type,
                workspace_path,
                remote_connection_id,
                remote_ssh_host,
                policy,
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments,
                metadata: serde_json::Map::new(),
            })
            .await
            .map(remote_dialog_scheduler_outcome_fact)
            .map(remote_dialog_submit_outcome_from_scheduler)
            .map_err(CoreServiceAgentRuntime::runtime_error_message)
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
        workspace_identity: RemoteSessionWorkspaceIdentity,
    ) -> Result<Vec<RemoteSessionMetadata>, String> {
        load_remote_session_metadata_for_workspace(workspace_path, workspace_identity).await
    }
}

#[async_trait::async_trait]
impl RemoteSessionRuntimeHost for CoreRemoteSessionRuntimeHost {
    async fn list_session_metadata(
        &self,
        workspace_path: &std::path::Path,
        workspace_identity: RemoteSessionWorkspaceIdentity,
    ) -> Result<Vec<RemoteSessionMetadata>, String> {
        load_remote_session_metadata_for_workspace(workspace_path, workspace_identity).await
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
        self.runtime
            .create_session(request)
            .await
            .map(|session| session.session_id)
            .map_err(CoreServiceAgentRuntime::runtime_error_message)
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

        let Some(session_storage_dir) =
            CoreServiceAgentRuntime::resolve_session_storage_dir(session_id).await
        else {
            return Err(format!(
                "Session storage directory not available for session: {}",
                session_id
            ));
        };
        self.coordinator
            .restore_session_from_storage_path(&session_storage_dir, session_id)
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

    async fn resolve_session_storage_dir(&self, session_id: &str) -> Option<std::path::PathBuf> {
        CoreServiceAgentRuntime::resolve_session_storage_dir(session_id).await
    }

    async fn load_remote_chat_messages(
        &self,
        session_storage_dir: &std::path::Path,
        session_id: &str,
    ) -> (Vec<ChatMessage>, bool) {
        CoreServiceAgentRuntime::load_remote_chat_messages(session_storage_dir, session_id).await
    }

    async fn delete_session(
        &self,
        session_storage_dir: &std::path::Path,
        session_id: &str,
    ) -> Result<(), String> {
        self.coordinator
            .delete_session(session_storage_dir, session_id)
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

    async fn resolve_session_storage_dir(&self, session_id: &str) -> Option<std::path::PathBuf> {
        CoreServiceAgentRuntime::resolve_session_storage_dir(session_id).await
    }

    async fn load_remote_chat_messages(
        &self,
        session_storage_dir: &std::path::Path,
        session_id: &str,
    ) -> (Vec<ChatMessage>, bool) {
        CoreServiceAgentRuntime::load_remote_chat_messages(session_storage_dir, session_id).await
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
    async fn resolve_session_storage_dir(&self, session_id: &str) -> Option<String> {
        CoreServiceAgentRuntime::resolve_session_storage_dir(session_id)
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
        restore_path_hint: &str,
    ) -> Result<(), String> {
        let restore_path = CoreServiceAgentRuntime::resolve_session_storage_dir(session_id)
            .await
            .unwrap_or_else(|| std::path::PathBuf::from(restore_path_hint));
        self.coordinator
            .restore_session_from_storage_path(&restore_path, session_id)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    async fn cancel_remote_turn(&self, session_id: &str, turn_id: &str) -> Result<(), String> {
        self.runtime
            .cancel_turn(AgentTurnCancellationRequest {
                session_id: session_id.to_string(),
                turn_id: Some(turn_id.to_string()),
                source: Some(AgentSubmissionSource::RemoteRelay),
                requester_session_id: None,
                reason: None,
                wait_timeout_ms: None,
            })
            .await
            .map(|_| ())
            .map_err(CoreServiceAgentRuntime::runtime_error_message)
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
                + AgentSessionManagementPort
                + AgentThreadGoalManagementPort
                + AgentTurnCancellationPort
                + RemoteControlStatePort
                + SessionTranscriptReader,
        {
        }

        assert_runtime_ports::<ConversationCoordinator>();
    }

    #[test]
    fn core_service_agent_runtime_owner_keeps_scheduler_lifecycle_port_contracts() {
        fn assert_scheduler_ports<T>()
        where
            T: AgentDialogTurnPort + AgentLifecycleDeliveryPort + AgentTurnCancellationPort,
        {
        }

        assert_scheduler_ports::<DialogScheduler>();
    }

    #[test]
    fn core_service_agent_runtime_owner_exposes_agent_runtime_and_remote_control_port() {
        fn assert_agent_runtime(
            coordinator: Arc<ConversationCoordinator>,
        ) -> Result<AgentRuntime, String> {
            CoreServiceAgentRuntime::agent_runtime(coordinator)
        }

        fn assert_agent_runtime_with_dialog_turns(
            coordinator: Arc<ConversationCoordinator>,
            scheduler: Arc<DialogScheduler>,
        ) -> Result<AgentRuntime, String> {
            CoreServiceAgentRuntime::agent_runtime_with_dialog_turns(coordinator, scheduler)
        }

        fn assert_agent_runtime_with_lifecycle_delivery(
            coordinator: Arc<ConversationCoordinator>,
            scheduler: Arc<DialogScheduler>,
        ) -> Result<AgentRuntime, String> {
            CoreServiceAgentRuntime::agent_runtime_with_lifecycle_delivery(coordinator, scheduler)
        }

        fn assert_agent_runtime_with_scheduler_ports(
            coordinator: Arc<ConversationCoordinator>,
            scheduler: Arc<DialogScheduler>,
        ) -> Result<AgentRuntime, String> {
            CoreServiceAgentRuntime::agent_runtime_with_scheduler_ports(coordinator, scheduler)
        }

        fn assert_remote_control_port(
            coordinator: &ConversationCoordinator,
        ) -> &(dyn RemoteControlStatePort + '_) {
            CoreServiceAgentRuntime::remote_control_state_port(coordinator)
        }

        let _ = assert_agent_runtime;
        let _ = assert_agent_runtime_with_dialog_turns;
        let _ = assert_agent_runtime_with_lifecycle_delivery;
        let _ = assert_agent_runtime_with_scheduler_ports;
        let _ = assert_remote_control_port;
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
    fn core_service_agent_runtime_owner_maps_image_context_to_lifecycle_attachment() {
        let attachment = agent_input_attachment_from_image_context(ImageContextData {
            id: "ctx-1".to_string(),
            image_path: Some("/workspace/clip.png".to_string()),
            data_url: Some("data:image/png;base64,abc".to_string()),
            mime_type: "image/png".to_string(),
            metadata: Some(serde_json::json!({ "name": "clip.png" })),
        });

        assert_eq!(attachment.kind, "remote_image");
        assert_eq!(attachment.id, "ctx-1");
        assert_eq!(
            attachment.metadata.get("imagePath"),
            Some(&serde_json::json!("/workspace/clip.png"))
        );
        assert_eq!(
            attachment.metadata.get("dataUrl"),
            Some(&serde_json::json!("data:image/png;base64,abc"))
        );
        assert_eq!(
            attachment.metadata.get("mimeType"),
            Some(&serde_json::json!("image/png"))
        );
        assert_eq!(
            attachment
                .metadata
                .get("metadata")
                .and_then(|value| value.get("name")),
            Some(&serde_json::json!("clip.png"))
        );
    }

    #[test]
    fn core_service_agent_runtime_owner_normalizes_remote_session_model_ids() {
        assert_eq!(
            normalize_remote_session_model_id(None),
            Some("auto".to_string())
        );
        assert_eq!(
            normalize_remote_session_model_id(Some("")),
            Some("auto".to_string())
        );
        assert_eq!(
            normalize_remote_session_model_id(Some("  default  ")),
            Some("auto".to_string())
        );
        assert_eq!(
            normalize_remote_session_model_id(Some(" model-1 ")),
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
        let mut turn = remote_history_test_turn(TurnStatus::Completed, None);
        turn.user_message.content =
            "User uploaded a file.\nUser's question:\n  explain this  ".to_string();

        let messages = remote_chat_messages_from_turns(&[turn]);

        assert_eq!(messages[0].content, "explain this");
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
                round_group_id: None,
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
                        attempt_id: None,
                        attempt_index: None,
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
                        attempt_id: None,
                        attempt_index: None,
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
                    subagent_dialog_turn_id: None,
                    attempt_id: None,
                    attempt_index: None,
                    subagent_model_id: None,
                    subagent_model_display_name: None,
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
                    attempt_id: None,
                    attempt_index: None,
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
            token_usage: None,
            finish_reason: None,
            has_final_response: None,
            status,
        }
    }
}

mod external_editor;
mod prompt_stash;
/// Chat mode implementation
///
/// Interactive chat mode with TUI interface.
/// Events are observed through an independent runtime broadcast subscription.
mod resize;
mod transcript;

use anyhow::{anyhow, Result};
use arboard::Clipboard;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::{
    mpsc::{self, Receiver, TryRecvError as MpscTryRecvError},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::broadcast::error::TryRecvError;

use bitfun_agent_runtime::sdk::{
    AgentLocalCommandTurnRecordRequest, AgentSessionComposerUpdate, AgentSessionLineageEntry,
    AgentSessionLineageInspection, AgentSessionLineageSnapshot, AgentSessionUsageRequest,
    SessionTranscript, SessionUsageReport,
};
use bitfun_events::{AgenticEvent, ToolEventData, ToolEventIdentity};
use resize::ResizeRedrawState;

use crate::actions::{
    action_by_id, action_conflict_behavior_version, action_for_alias,
    removed_management_command_hint, slash_actions, ActionContext, ActionHandler, ActionSpec,
    ActionState, ResolvedKeymap, IMAGE_ATTACHMENTS_REQUIRE_MESSAGE, SHARED_TUI_EMBEDDED_HANDOFF,
    SHARED_TUI_HELP_NOTE,
};
use crate::agent::context_reload_client::CliContextReloadClient;
use crate::agent::runtime_client::{CliAgentRuntimeClient, SessionOperationError};
use crate::chat_state::{ChatState, ModelTokenUsageSnapshot};
use crate::config::CliConfig;
use crate::ui::agent_selector::{AgentItem, AgentSelectorAction};
use crate::ui::chat::{session_status_text, ChatView, MouseGestureOutcome};
use crate::ui::command_menu::{ExternalCommandProjection, NativeCommandCollisionProjection};
use crate::ui::command_palette::PaletteAction;
use crate::ui::fork_selector::{ForkAction, ForkTarget};
use crate::ui::image_paste::{self, ImagePaste};
use crate::ui::login_form::LoginFormAction;
use crate::ui::mcp_add_dialog::McpAddAction;
use crate::ui::mcp_selector::{McpItem, McpItemAction};
use crate::ui::model_config_form::{ModelFormAction, ModelFormResult};
use crate::ui::model_selector::ModelItem;
use crate::ui::permission::PermissionAction;
use crate::ui::prompt_command_shell_review::PromptCommandShellReviewAction;
use crate::ui::prompt_stash_selector::PromptStashAction;
use crate::ui::provider_selector::ProviderSelection;
use crate::ui::question::QuestionAction;
use crate::ui::session_lineage_selector::SessionLineageAction;
use crate::ui::session_selector::{SessionAction, SessionItem};
use crate::ui::skill_selector::{SkillItem, SkillSelectorAction};
use crate::ui::subagent_selector::{SubagentItem, SubagentSelectorAction};
use crate::ui::theme::{
    builtin_theme_ids, builtin_theme_json, resolve_appearance, resolve_effective_color_scheme,
    Appearance, EffectiveColorScheme, Theme,
};
use crate::ui::theme_selector::ThemeItem;
use crate::ui::{init_terminal, restore_terminal, TerminalGuard};
use bitfun_core::agentic::agents::{
    get_agent_registry, AgentInfo, SubAgentSource, SubagentListScope, SubagentQueryContext,
};
use bitfun_core::agentic::tools::implementations::skills::{
    mode_overrides::{
        load_project_mode_skills_document_local, save_project_mode_skills_document_local,
        set_mode_skill_disabled_in_document, set_user_mode_skill_state,
    },
    registry::SkillRegistry,
    ModeSkillInfo, SkillInfo,
};
use bitfun_core::external_hooks::{
    ExternalHookCatalogSnapshotV1, ExternalHookMatcherSummary, ExternalHookNativeActivation,
    ExternalHookProjectionStatus,
};
use bitfun_core::external_sources::{
    apply_external_source_control_action, choose_external_subagent_conflict,
    expand_external_prompt_command, external_source_conflict_choices, external_source_snapshot,
    get_external_source_control_snapshot, native_prompt_command_conflict_key,
    sanitize_external_source_operation_error, set_external_prompt_command_conflict_choice,
    set_external_subagent_activation, set_external_subagent_model_binding,
    set_external_tool_conflict_choice, set_external_tool_target_decision,
    set_native_prompt_command_conflict_choice, subscribe_external_source_updates,
    ExternalSourceAssetKind, ExternalSourceCatalogSnapshot, ExternalSourceControlActionV1,
    ExternalSourceControlRequestV1, ExternalSourceDiagnosticSeverity,
    ExternalSourceHostCapabilities, ExternalSourceOperationError, ExternalSourceOperationErrorCode,
    ExternalSubagentActivationState, ExternalSubagentCompatibilityState,
    ExternalSubagentModelBindingMethod, ExternalSubagentModelBindingTarget,
    ExternalSubagentModelProfileRequest, ExternalSubagentModelRequest, ExternalToolActivationState,
    ExternalToolCapability, ExternalToolCatalogEntry, ExternalToolRuntimeKind,
    NativePromptCommandDescriptor, PromptCommandAvailability, PromptCommandExecutionTarget,
    PromptCommandInvocationOutcome, PromptCommandShellReviewDecision, PromptCommandShellReviewMode,
    PromptCommandShellReviewPlan, EXTERNAL_SOURCE_CONTROL_SCHEMA_V1,
};
use bitfun_core::native_hooks::{
    overview as native_hook_overview, NativeHookOverview, NativeHookRuleView,
};
use bitfun_core::product_runtime::CoreAgentRuntimeCompatibility;
use bitfun_core::service::config::GlobalConfigManager;
use bitfun_core::service::session_usage::render_usage_report_markdown;
use bitfun_product_domains::external_hook_import::{
    ExternalHookImportApplyOutcomeV1, ExternalHookImportApplyRequestV1,
    ExternalHookImportMutationV1, ExternalHookImportPlanV1, ExternalHookImportSnapshotV1,
    EXTERNAL_HOOK_IMPORT_SCHEMA_V1,
};
use bitfun_product_domains::external_sources::{
    ExternalSourceHealth, ExternalSourceScope, SourceKey,
};

/// Spinner/UI redraw interval while a turn is processing.
const SPINNER_REDRAW_INTERVAL_MS: u64 = 100;
/// Coalesce rapid resize bursts to reduce flicker during window drag.
const RESIZE_REDRAW_DEBOUNCE_MS: u64 = 75;

include!("chat/external_review.rs");
include!("chat/external_sources.rs");
include!("chat/external_hooks.rs");
include!("chat/native_hooks.rs");

fn agent_event_stream_failure(error: TryRecvError) -> Option<String> {
    match error {
        TryRecvError::Empty => None,
        TryRecvError::Lagged(skipped) => Some(format!(
            "Agent event stream lagged by {skipped} events; chat state can no longer be trusted"
        )),
        TryRecvError::Closed => {
            Some("Agent event stream closed; chat state can no longer be trusted".to_string())
        }
    }
}

fn mark_active_turn_failed(chat_state: &mut ChatState, error: &str) -> bool {
    if chat_state.current_turn_id().is_none() {
        return false;
    }

    chat_state.handle_turn_failed(error);
    true
}

#[derive(Debug, Default)]
struct TranscriptProjectionOutcome {
    changed: bool,
    requested_input: bool,
    terminal: Option<TranscriptTerminalOutcome>,
}

#[derive(Debug)]
enum TranscriptTerminalOutcome {
    Completed,
    Failed(String),
    Cancelled,
    SystemError(String),
}

/// Shared event-to-transcript projection used by the root conversation and a
/// read-only inspected descendant. Host notifications and git refresh remain
/// root-only side effects in the caller.
fn project_transcript_event(
    chat_state: &mut ChatState,
    event: &AgenticEvent,
    interactive: bool,
) -> TranscriptProjectionOutcome {
    if event
        .turn_id()
        .is_some_and(|turn_id| chat_state.should_ignore_turn_event(turn_id))
    {
        return TranscriptProjectionOutcome::default();
    }

    let mut outcome = TranscriptProjectionOutcome::default();
    match event {
        AgenticEvent::DialogTurnStarted {
            turn_id,
            user_input,
            ..
        } if chat_state.current_turn_id() != Some(turn_id.as_str()) => {
            chat_state.handle_turn_started(turn_id, user_input);
            outcome.changed = true;
        }
        AgenticEvent::TextChunk { turn_id, text, .. }
            if chat_state.current_turn_id() == Some(turn_id.as_str()) =>
        {
            chat_state.handle_text_chunk(text);
            outcome.changed = true;
        }
        AgenticEvent::ThinkingChunk {
            turn_id, content, ..
        } if chat_state.current_turn_id() == Some(turn_id.as_str()) => {
            chat_state.handle_thinking_chunk(content);
            outcome.changed = true;
        }
        AgenticEvent::ToolEvent {
            turn_id,
            tool_event,
            ..
        } if chat_state.current_turn_id() == Some(turn_id.as_str()) => {
            let question_pending = chat_state.question_prompt.is_some();
            chat_state.handle_tool_event(tool_event);
            outcome.requested_input = !question_pending && chat_state.question_prompt.is_some();
            if !interactive {
                chat_state.question_prompt = None;
                chat_state.permission_prompt = None;
            }
            outcome.changed = true;
        }
        AgenticEvent::UserSteeringInjected {
            turn_id,
            steering_id,
            display_content,
            ..
        } if chat_state.current_turn_id() == Some(turn_id.as_str()) => {
            chat_state.handle_user_steering(steering_id, display_content, false);
            outcome.changed = true;
        }
        AgenticEvent::ContextCompressionStarted { .. }
        | AgenticEvent::ContextCompressionCompleted { .. }
        | AgenticEvent::ContextCompressionFailed { .. } => {
            if let Some(tool_event) = context_compression_tool_event(event, chat_state) {
                chat_state.handle_tool_event(&tool_event);
                outcome.changed = true;
            }
        }
        AgenticEvent::DialogTurnCompleted {
            turn_id,
            total_rounds,
            total_tools,
            ..
        } if chat_state.current_turn_id() == Some(turn_id.as_str()) => {
            chat_state.handle_turn_completed(*total_rounds, *total_tools);
            outcome.changed = true;
            outcome.terminal = Some(TranscriptTerminalOutcome::Completed);
        }
        AgenticEvent::DialogTurnFailed { turn_id, error, .. }
            if chat_state.current_turn_id() == Some(turn_id.as_str()) =>
        {
            chat_state.handle_turn_failed(error);
            outcome.changed = true;
            outcome.terminal = Some(TranscriptTerminalOutcome::Failed(error.clone()));
        }
        AgenticEvent::DialogTurnCancelled { turn_id, .. }
            if chat_state.should_apply_turn_cancelled(turn_id) =>
        {
            chat_state.handle_turn_cancelled();
            outcome.changed = true;
            outcome.terminal = Some(TranscriptTerminalOutcome::Cancelled);
        }
        AgenticEvent::TokenUsageUpdated { .. } => {
            if let Some(usage) = primary_model_usage_for_active_turn(event, chat_state) {
                chat_state.handle_primary_model_usage(usage);
                outcome.changed = true;
            }
        }
        AgenticEvent::SystemError { error, .. } => {
            chat_state.add_system_message(format!("[System error: {error}]"));
            outcome.changed = true;
            outcome.terminal = Some(TranscriptTerminalOutcome::SystemError(error.clone()));
        }
        _ => {}
    }
    outcome
}

/// Chat mode exit reason
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ChatExitReason {
    /// User exits program
    Quit,
    /// Switch to a different session
    SwitchSession(String),
    /// Fork the current session at the selected OpenCode-compatible boundary.
    ForkSession(crate::ui::fork_selector::ForkTarget),
    /// Create a new session
    NewSession,
}

/// Pending MCP operation (deferred to allow a render frame for loading state)
enum PendingMcpOp {
    Toggle(String),
    External(McpItem),
    Add { name: String, config_json: String },
    Delete(String),
}

enum PendingMcpTask {
    Toggle {
        server_id: String,
        handle: tokio::task::JoinHandle<bitfun_core::util::errors::BitFunResult<()>>,
    },
    Add {
        name: String,
        handle: tokio::task::JoinHandle<bitfun_core::util::errors::BitFunResult<()>>,
    },
    Delete {
        server_id: String,
        handle: tokio::task::JoinHandle<bitfun_core::util::errors::BitFunResult<()>>,
    },
    External {
        item_id: String,
        item_name: String,
        handle: tokio::task::JoinHandle<std::result::Result<ExternalSourceCatalogSnapshot, String>>,
    },
}

enum PendingSessionOperationKind {
    Mode {
        mode_id: String,
    },
    Model {
        model_id: String,
        display_name: String,
    },
    Rename {
        session_name: String,
    },
    Delete {
        session_name: String,
    },
}

impl PendingSessionOperationKind {
    fn name(&self) -> &'static str {
        match self {
            Self::Mode { .. } => "agent mode",
            Self::Model { .. } => "model",
            Self::Rename { .. } => "name",
            Self::Delete { .. } => "deletion",
        }
    }

    fn selected_id(&self) -> &str {
        match self {
            Self::Mode { mode_id } => mode_id,
            Self::Model { model_id, .. } => model_id,
            Self::Rename { session_name } => session_name,
            Self::Delete { session_name } => session_name,
        }
    }
}

struct PendingSessionOperation {
    session_id: String,
    kind: PendingSessionOperationKind,
    started_at: Instant,
    slow_notice_shown: bool,
    exit_warning_shown: bool,
    handle: tokio::task::JoinHandle<std::result::Result<(), SessionOperationError>>,
}

struct PendingWorkspaceReferenceSearch {
    generation: u64,
    query: String,
    handle: tokio::task::JoinHandle<
        std::result::Result<bitfun_agent_runtime::sdk::AgentWorkspaceReferenceSearchResult, String>,
    >,
}

struct PendingWorkspaceDiff {
    handle: tokio::task::JoinHandle<
        std::result::Result<bitfun_agent_runtime::sdk::WorkspaceDiffSnapshot, String>,
    >,
}

#[derive(Debug, Clone)]
struct ExternalPromptCommandInvocation {
    command_name: String,
    arguments: String,
    native_commands: Vec<NativePromptCommandDescriptor>,
    candidate_id: Option<String>,
    content_version: Option<String>,
    native_conflict_key: Option<String>,
    expected_preference_revision: Option<u64>,
}

struct PendingPromptCommandShellInvocation {
    invocation: ExternalPromptCommandInvocation,
    review: PromptCommandShellReviewPlan,
}

enum PendingLocalEffect {
    EditComposer {
        command: external_editor::EditorCommand,
        draft: crate::ui::composer::ComposerDraft,
    },
    ExportTranscript {
        markdown: String,
        target: Option<std::path::PathBuf>,
        editor_command: Option<external_editor::EditorCommand>,
        editor_error: Option<String>,
        overwrite_confirmed: bool,
    },
}

fn terminal_event_allowed_while_local_effect_pending(event: &Event) -> bool {
    matches!(event, Event::Resize(_, _))
}

const SESSION_OPERATION_SLOW_NOTICE: Duration = Duration::from_secs(15);
const SHARED_TUI_CHAT_STATUS: &str = "Shared TUI preview: this view controls sessions, including deleting an idle Session, turns, the current Session name, current Session Agent mode, current Session model, and declarative context via /reload [skills|instructions]; model management remains Embedded, along with local extension, MCP, account-sync, and Agent/Subagent management.";

#[derive(Default)]
struct NonKeyEventOutcome {
    request_redraw: bool,
    resize_observed: bool,
}

struct ChatEventContext<'a> {
    this: &'a mut ChatMode,
    chat_view: &'a mut ChatView,
    chat_state: &'a mut ChatState,
    session_id: &'a mut String,
    rt_handle: &'a tokio::runtime::Handle,
    should_quit: &'a mut bool,
    exit_reason: &'a mut ChatExitReason,
}

struct AgentSessionInspection {
    selected_session_id: String,
    chat_state: ChatState,
    /// Terminal events invalidate this read model. Live chunks are projected
    /// directly; Runtime transcript reads reconcile the settled turn.
    refresh_pending: bool,
    refresh_due_at: Instant,
    refresh_deadline: Option<Instant>,
    /// A terminal event precedes persistence. An otherwise successful Runtime
    /// read is stale while it still reports one of these Turns as active.
    settling_turn_ids: BTreeSet<String>,
    /// Failed/cancelled persistence omits partial model output, so settlement
    /// confirmation must retain the already projected terminal view.
    preserve_live_terminal: bool,
}

#[derive(Clone)]
struct LineageSettlement {
    turn_id: String,
    preserve_live_terminal: bool,
}

struct BufferedLineageEvent {
    event: AgenticEvent,
    encoded_bytes: usize,
}

const LINEAGE_EVENT_BUFFER_MAX_BYTES: usize = 1024 * 1024;
const LINEAGE_EVENT_BUFFER_MAX_EVENTS: usize = 4096;
const LINEAGE_SETTLEMENT_RETRY_WINDOW: Duration = Duration::from_secs(5);

pub(crate) struct ChatMode {
    config: CliConfig,
    keymap: ResolvedKeymap,
    /// Current agent type (e.g. "agentic", "plan", "debug")
    agent_type: String,
    workspace: Option<String>,
    local_cwd: std::path::PathBuf,
    agent: Arc<CliAgentRuntimeClient>,
    context_reload: CliContextReloadClient,
    compatibility: Option<CoreAgentRuntimeCompatibility>,
    /// User-level default resolved from shared config for this TUI run.
    auto_approve_ask_default: bool,
    /// Temporary override for the current session only.
    auto_approve_ask_override: Option<bool>,
    /// If set, restore this existing session instead of creating a new one
    restore_session_id: Option<String>,
    /// If set, send this prompt automatically when the session starts
    initial_prompt: Option<crate::ui::composer::ComposerDraft>,
    /// Pending MCP operation — set in key handler, executed after one render frame
    pending_mcp_op: Option<PendingMcpOp>,
    /// Running MCP tasks (non-blocking, polled in main loop)
    pending_mcp_tasks: Vec<PendingMcpTask>,
    /// One Session operation in flight. The event loop remains responsive while
    /// the Runtime owner updates or deletes Session state.
    pending_session_operation: Option<PendingSessionOperation>,
    pending_workspace_diff: Option<PendingWorkspaceDiff>,
    pending_local_effect: Option<PendingLocalEffect>,
    pending_workspace_reference_search: Option<PendingWorkspaceReferenceSearch>,
    /// Last authoritative flat lineage read. This is a presentation cache only;
    /// Services remains the membership/order owner.
    lineage_snapshot: Option<AgentSessionLineageSnapshot>,
    lineage_inspection: Option<AgentSessionInspection>,
    /// Bounded presentation tail for active descendants. It bridges live
    /// broadcast output until the Runtime-owned transcript settles.
    lineage_event_buffer: VecDeque<BufferedLineageEvent>,
    lineage_event_buffer_bytes: usize,
    /// At most one Turn can settle per Session before the next Turn starts.
    /// Keeping this outside the selected inspection closes selector-time races.
    lineage_settlements: BTreeMap<String, LineageSettlement>,
    workspace_reference_search_generation: u64,
    last_workspace_reference_query: Option<String>,
    /// One explicit native slash-menu choice waiting for its parameterized submission.
    selected_native_command_once: Option<String>,
    pending_prompt_command_shell_invocation: Option<PendingPromptCommandShellInvocation>,
    external_source_snapshot: Option<ExternalSourceCatalogSnapshot>,
    external_source_conflict_choices: BTreeMap<String, String>,
    external_source_conflict_lineage_current_keys: BTreeMap<String, String>,
    external_source_conflicted_candidate_ids: BTreeSet<String>,
    external_tool_notice_key: Option<String>,
    external_tool_review_snapshot: Option<ExternalSourceCatalogSnapshot>,
    external_tool_mutation_rx: Option<Receiver<ExternalToolMutationResult>>,
    external_control_mutation_rx: Option<Receiver<ExternalControlMutationResult>>,
    external_agent_notice_key: Option<String>,
    external_agent_review_snapshot: Option<ExternalSourceCatalogSnapshot>,
    external_agent_mutation_rx: Option<Receiver<ExternalAgentMutationResult>>,
    hook_management_rx: Option<
        Receiver<
            std::result::Result<
                HookManagementResult,
                bitfun_core::external_sources::ExternalSourceOperationError,
            >,
        >,
    >,
    hook_management_snapshot: Option<HookManagementSnapshot>,
    pending_hook_plan: Option<ExternalHookImportPlanV1>,
}

/// Map agent_type to a display name for status messages
fn agent_display_name(agent_type: &str) -> &'static str {
    match agent_type {
        "agentic" => "Fang",
        _ => "AI Assistant",
    }
}

impl ChatMode {
    pub(crate) fn new(
        config: CliConfig,
        agent_type: String,
        workspace: Option<String>,
        agent: Arc<CliAgentRuntimeClient>,
        context_reload: CliContextReloadClient,
        compatibility: Option<CoreAgentRuntimeCompatibility>,
    ) -> Self {
        let keymap = ResolvedKeymap::new(&config.shortcuts);
        Self {
            config,
            keymap,
            agent_type,
            workspace,
            local_cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            agent,
            context_reload,
            compatibility,
            auto_approve_ask_default: false,
            auto_approve_ask_override: None,
            restore_session_id: None,
            initial_prompt: None,
            pending_mcp_op: None,
            pending_mcp_tasks: Vec::new(),
            pending_session_operation: None,
            pending_workspace_diff: None,
            pending_local_effect: None,
            pending_workspace_reference_search: None,
            lineage_snapshot: None,
            lineage_inspection: None,
            lineage_event_buffer: VecDeque::new(),
            lineage_event_buffer_bytes: 0,
            lineage_settlements: BTreeMap::new(),
            workspace_reference_search_generation: 0,
            last_workspace_reference_query: None,
            selected_native_command_once: None,
            pending_prompt_command_shell_invocation: None,
            external_source_snapshot: None,
            external_source_conflict_choices: BTreeMap::new(),
            external_source_conflict_lineage_current_keys: BTreeMap::new(),
            external_source_conflicted_candidate_ids: BTreeSet::new(),
            external_tool_notice_key: None,
            external_tool_review_snapshot: None,
            external_tool_mutation_rx: None,
            external_control_mutation_rx: None,
            external_agent_notice_key: None,
            external_agent_review_snapshot: None,
            external_agent_mutation_rx: None,
            hook_management_rx: None,
            hook_management_snapshot: None,
            pending_hook_plan: None,
        }
    }

    /// Set a session ID to restore (for "Continue Last Session")
    pub(crate) fn with_restore_session(mut self, session_id: String) -> Self {
        self.restore_session_id = Some(session_id);
        self
    }

    /// Set an initial prompt to send automatically when the session starts
    pub(crate) fn with_initial_prompt(
        mut self,
        prompt: crate::ui::composer::ComposerDraft,
    ) -> Self {
        self.initial_prompt = Some(prompt);
        self
    }

    fn action_state(&self, is_processing: bool, popup_open: bool) -> ActionState {
        ActionState::chat(is_processing, popup_open)
            .with_shared_tui(self.agent.is_shared())
            .with_lineage_inspection(self.lineage_inspection.is_some())
    }
}

include!("chat/account.rs");
include!("chat/run.rs");
include!("chat/input.rs");
include!("chat/commands.rs");
include!("chat/worktree.rs");
include!("chat/selection.rs");
include!("chat/mcp.rs");
include!("chat/sessions.rs");
include!("chat/workspace_references.rs");
include!("chat/capabilities.rs");
include!("chat/session_lineage.rs");
include!("chat/provider_models.rs");
include!("chat/tests.rs");

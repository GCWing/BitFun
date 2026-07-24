/// Chat mode implementation
///
/// Interactive chat mode with TUI interface.
/// Events are observed through an independent runtime broadcast subscription.
mod resize;

use anyhow::{anyhow, Result};
use arboard::Clipboard;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{
    mpsc::{self, Receiver, TryRecvError as MpscTryRecvError},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::broadcast::error::TryRecvError;

use bitfun_agent_runtime::sdk::{
    AgentLocalCommandTurnRecordRequest, AgentSessionUsageRequest, SessionUsageReport,
};
use bitfun_events::AgenticEvent;
use resize::ResizeRedrawState;

use crate::actions::{
    action_by_id, action_conflict_behavior_version, action_for_alias,
    removed_management_command_hint, slash_actions, ActionContext, ActionHandler, ActionSpec,
    ActionState, ResolvedKeymap,
};
use crate::agent::runtime_client::CliAgentRuntimeClient;
use crate::chat_state::ChatState;
use crate::config::CliConfig;
use crate::runtime::CliRuntimeContext;
use crate::ui::agent_selector::{AgentItem, AgentSelectorAction};
use crate::ui::chat::{ChatView, MouseGestureOutcome};
use crate::ui::command_menu::{ExternalCommandProjection, NativeCommandCollisionProjection};
use crate::ui::command_palette::PaletteAction;
use crate::ui::login_form::LoginFormAction;
use crate::ui::mcp_add_dialog::McpAddAction;
use crate::ui::mcp_selector::{McpItem, McpItemAction};
use crate::ui::model_config_form::{ModelFormAction, ModelFormResult};
use crate::ui::model_selector::ModelItem;
use crate::ui::permission::PermissionAction;
use crate::ui::provider_selector::ProviderSelection;
use crate::ui::question::QuestionAction;
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
    local_external_hook_catalog_snapshot, ExternalHookCatalogSnapshotV1,
    ExternalHookMatcherSummary, ExternalHookNativeActivation, ExternalHookProjectionStatus,
};
use bitfun_core::external_sources::{
    apply_external_source_control_action, choose_external_subagent_conflict,
    expand_external_prompt_command, external_source_conflict_choices, external_source_snapshot,
    get_external_source_control_snapshot, native_prompt_command_conflict_key,
    sanitize_external_source_operation_error, set_external_prompt_command_conflict_choice,
    set_external_subagent_activation, set_external_tool_conflict_choice,
    set_external_tool_target_decision, set_native_prompt_command_conflict_choice,
    subscribe_external_source_updates, ExternalSourceAssetKind, ExternalSourceCatalogSnapshot,
    ExternalSourceControlActionV1, ExternalSourceControlRequestV1,
    ExternalSourceDiagnosticSeverity, ExternalSourceHostCapabilities, ExternalSourceOperationError,
    ExternalSourceOperationErrorCode, ExternalSubagentActivationState,
    ExternalSubagentCompatibilityState, ExternalToolActivationState, ExternalToolCapability,
    ExternalToolCatalogEntry, ExternalToolRuntimeKind, NativePromptCommandDescriptor,
    PromptCommandAvailability, EXTERNAL_SOURCE_CONTROL_SCHEMA_V1,
};
use bitfun_core::service::config::GlobalConfigManager;
use bitfun_core::service::session_usage::render_usage_report_markdown;
use bitfun_product_domains::external_sources::{ExternalSourceHealth, ExternalSourceScope};

/// Spinner/UI redraw interval while a turn is processing.
const SPINNER_REDRAW_INTERVAL_MS: u64 = 100;
/// Coalesce rapid resize bursts to reduce flicker during window drag.
const RESIZE_REDRAW_DEBOUNCE_MS: u64 = 75;

include!("chat/external_review.rs");
include!("chat/external_sources.rs");
include!("chat/external_hooks.rs");

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

/// Chat mode exit reason
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ChatExitReason {
    /// User exits program
    Quit,
    /// Switch to a different session
    SwitchSession(String),
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

struct PendingModeChange {
    session_id: String,
    mode_id: String,
    started_at: Instant,
    slow_notice_shown: bool,
    exit_warning_shown: bool,
    handle: tokio::task::JoinHandle<anyhow::Result<()>>,
}

const MODE_CHANGE_SLOW_NOTICE: Duration = Duration::from_secs(15);

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

pub(crate) struct ChatMode {
    config: CliConfig,
    keymap: ResolvedKeymap,
    /// Current agent type (e.g. "agentic", "plan", "debug")
    agent_type: String,
    workspace: Option<String>,
    agent: Arc<CliAgentRuntimeClient>,
    runtime: Arc<CliRuntimeContext>,
    /// User-level default resolved from shared config for this TUI run.
    auto_approve_ask_default: bool,
    /// Temporary override for the current session only.
    auto_approve_ask_override: Option<bool>,
    /// If set, restore this existing session instead of creating a new one
    restore_session_id: Option<String>,
    /// If set, send this prompt automatically when the session starts
    initial_prompt: Option<String>,
    /// Pending MCP operation — set in key handler, executed after one render frame
    pending_mcp_op: Option<PendingMcpOp>,
    /// Running MCP tasks (non-blocking, polled in main loop)
    pending_mcp_tasks: Vec<PendingMcpTask>,
    /// One durable mode update in flight. The event loop remains responsive
    /// while the runtime owner writes session metadata.
    pending_mode_change: Option<PendingModeChange>,
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
    external_hook_catalog_rx: Option<
        Receiver<
            std::result::Result<
                ExternalHookCatalogSnapshotV1,
                bitfun_core::external_sources::ExternalSourceOperationError,
            >,
        >,
    >,
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
        runtime: Arc<CliRuntimeContext>,
    ) -> Self {
        let agent = Arc::new(CliAgentRuntimeClient::new(
            runtime.as_ref(),
            workspace.clone().map(PathBuf::from),
        ));

        let keymap = ResolvedKeymap::new(&config.shortcuts);
        Self {
            config,
            keymap,
            agent_type,
            workspace,
            agent,
            runtime,
            auto_approve_ask_default: false,
            auto_approve_ask_override: None,
            restore_session_id: None,
            initial_prompt: None,
            pending_mcp_op: None,
            pending_mcp_tasks: Vec::new(),
            pending_mode_change: None,
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
            external_hook_catalog_rx: None,
        }
    }

    /// Set a session ID to restore (for "Continue Last Session")
    pub(crate) fn with_restore_session(mut self, session_id: String) -> Self {
        self.restore_session_id = Some(session_id);
        self
    }

    /// Set an initial prompt to send automatically when the session starts
    pub(crate) fn with_initial_prompt(mut self, prompt: String) -> Self {
        self.initial_prompt = Some(prompt);
        self
    }
}

include!("chat/account.rs");
include!("chat/run.rs");
include!("chat/input.rs");
include!("chat/commands.rs");
include!("chat/selection.rs");
include!("chat/mcp.rs");
include!("chat/sessions.rs");
include!("chat/capabilities.rs");
include!("chat/provider_models.rs");
include!("chat/tests.rs");

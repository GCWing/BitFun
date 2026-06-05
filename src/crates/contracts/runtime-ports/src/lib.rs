//! Thin runtime ports for boundaries that currently cross service and agentic
//! concrete implementations.
//!
//! This crate intentionally contains only DTOs and traits. It must not depend
//! on concrete managers, platform adapters, `bitfun-core`, or app crates.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub type PortResult<T> = Result<T, PortError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortErrorKind {
    NotAvailable,
    NotFound,
    InvalidRequest,
    PermissionDenied,
    Cancelled,
    Timeout,
    Backend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortError {
    pub kind: PortErrorKind,
    pub message: String,
}

impl PortError {
    pub fn new(kind: PortErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for PortError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeServiceCapability {
    FileSystem,
    Workspace,
    SessionStore,
    Permission,
    Events,
    Clock,
    Terminal,
    Network,
    Git,
    McpCatalog,
    RemoteConnection,
    RemoteWorkspace,
    RemoteProjection,
    RemoteCapabilities,
}

impl RuntimeServiceCapability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FileSystem => "filesystem",
            Self::Workspace => "workspace",
            Self::SessionStore => "session_store",
            Self::Permission => "permission",
            Self::Events => "events",
            Self::Clock => "clock",
            Self::Terminal => "terminal",
            Self::Network => "network",
            Self::Git => "git",
            Self::McpCatalog => "mcp_catalog",
            Self::RemoteConnection => "remote_connection",
            Self::RemoteWorkspace => "remote_workspace",
            Self::RemoteProjection => "remote_projection",
            Self::RemoteCapabilities => "remote_capabilities",
        }
    }
}

impl std::fmt::Display for RuntimeServiceCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub trait RuntimeServicePort: Send + Sync {
    fn capability(&self) -> RuntimeServiceCapability;
}

pub trait FileSystemPort: RuntimeServicePort {}

pub trait WorkspacePort: RuntimeServicePort {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStoragePathRequest {
    pub workspace_path: PathBuf,
    pub remote_connection_id: Option<String>,
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStorageKind {
    Local,
    Remote,
    UnresolvedRemote,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStoragePathResolution {
    pub requested_workspace_path: PathBuf,
    pub effective_storage_path: PathBuf,
    pub storage_kind: SessionStorageKind,
    pub remote_connection_id: Option<String>,
    pub remote_ssh_host: Option<String>,
}

impl SessionStoragePathResolution {
    pub fn new(
        requested_workspace_path: PathBuf,
        effective_storage_path: PathBuf,
        storage_kind: SessionStorageKind,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
    ) -> Self {
        Self {
            requested_workspace_path,
            effective_storage_path,
            storage_kind,
            remote_connection_id,
            remote_ssh_host,
        }
    }

    pub fn is_remote_storage(&self) -> bool {
        matches!(
            self.storage_kind,
            SessionStorageKind::Remote | SessionStorageKind::UnresolvedRemote
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionViewRestoreRequest {
    pub workspace_path: PathBuf,
    pub session_id: String,
    pub include_internal: bool,
    pub tail_turn_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTurnLoadRequest {
    pub workspace_path: PathBuf,
    pub session_id: String,
    pub tail_turn_count: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTurnLoadTiming {
    pub requested_tail_turn_count: Option<usize>,
    pub loaded_turn_count: usize,
    pub total_turn_count: usize,
    pub turn_file_count: usize,
    pub missing_turn_file_count: usize,
    pub fast_path: bool,
    pub metadata_duration_ms: u64,
    pub state_duration_ms: u64,
    pub scan_duration_ms: u64,
    pub read_duration_ms: u64,
    pub max_turn_read_duration_ms: u64,
    pub build_session_duration_ms: u64,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionViewRestoreTiming {
    pub resolve_storage_path_duration_ms: u64,
    pub visibility_metadata_duration_ms: u64,
    pub load_session_with_turns_duration_ms: u64,
    pub normalize_turn_ids_duration_ms: u64,
    pub total_duration_ms: u64,
    pub turn_load: SessionTurnLoadTiming,
}

#[async_trait::async_trait]
pub trait SessionStorePort: RuntimeServicePort {
    async fn resolve_session_storage_path(
        &self,
        request: SessionStoragePathRequest,
    ) -> PortResult<SessionStoragePathResolution>;
}

/// One row from [`WorkspaceFileSystem::read_dir`] (POSIX paths when the backend is remote SSH).
#[derive(Debug, Clone)]
pub struct WorkspaceDirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
}

/// Unified file system operations that work for both local and remote workspaces.
#[async_trait::async_trait]
pub trait WorkspaceFileSystem: Send + Sync {
    async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>>;
    async fn read_file_text(&self, path: &str) -> anyhow::Result<String>;
    async fn write_file(&self, path: &str, contents: &[u8]) -> anyhow::Result<()>;
    async fn exists(&self, path: &str) -> anyhow::Result<bool>;
    async fn is_file(&self, path: &str) -> anyhow::Result<bool>;
    async fn is_dir(&self, path: &str) -> anyhow::Result<bool>;
    /// List immediate children (non-recursive). Symlinks may be included; callers often skip them.
    async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>>;
}

/// Unified shell execution options for local and remote workspaces.
#[derive(Clone, Default)]
pub struct WorkspaceCommandOptions {
    pub timeout_ms: Option<u64>,
    pub cancellation_token: Option<CancellationToken>,
}

impl std::fmt::Debug for WorkspaceCommandOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceCommandOptions")
            .field("timeout_ms", &self.timeout_ms)
            .field(
                "cancellation_token",
                &self
                    .cancellation_token
                    .as_ref()
                    .map(|_| "<CancellationToken>"),
            )
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceCommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub interrupted: bool,
    pub timed_out: bool,
}

impl WorkspaceCommandResult {
    pub fn combined_output(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout, self.stderr)
        }
    }
}

/// Unified shell execution for both local and remote workspaces.
#[async_trait::async_trait]
pub trait WorkspaceShell: Send + Sync {
    /// Execute a command and return a structured result.
    async fn exec_with_options(
        &self,
        command: &str,
        options: WorkspaceCommandOptions,
    ) -> anyhow::Result<WorkspaceCommandResult>;

    /// Execute a command and return (stdout, stderr, exit_code).
    async fn exec(
        &self,
        command: &str,
        timeout_ms: Option<u64>,
    ) -> anyhow::Result<(String, String, i32)> {
        let result = self
            .exec_with_options(
                command,
                WorkspaceCommandOptions {
                    timeout_ms,
                    ..Default::default()
                },
            )
            .await?;

        if result.timed_out {
            anyhow::bail!(
                "Command timed out after {}ms",
                timeout_ms.unwrap_or_default()
            );
        }
        if result.interrupted {
            anyhow::bail!("Command was cancelled");
        }

        Ok((result.stdout, result.stderr, result.exit_code))
    }
}

/// Bundle of workspace I/O services injected into tool runtime context.
pub struct WorkspaceServices {
    pub fs: Arc<dyn WorkspaceFileSystem>,
    pub shell: Arc<dyn WorkspaceShell>,
}

impl Clone for WorkspaceServices {
    fn clone(&self) -> Self {
        Self {
            fs: Arc::clone(&self.fs),
            shell: Arc::clone(&self.shell),
        }
    }
}

impl std::fmt::Debug for WorkspaceServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceServices")
            .field("fs", &"<dyn WorkspaceFileSystem>")
            .field("shell", &"<dyn WorkspaceShell>")
            .finish()
    }
}

/// Runtime handles injected into tool execution contexts.
///
/// This bundle is intentionally handle-only. Concrete local or remote
/// implementations are still assembled by product/runtime owners outside this
/// crate.
#[derive(Clone, Default)]
pub struct ToolRuntimeHandles {
    workspace_services: Option<WorkspaceServices>,
    cancellation_token: Option<CancellationToken>,
}

impl ToolRuntimeHandles {
    pub fn new(
        workspace_services: Option<WorkspaceServices>,
        cancellation_token: Option<CancellationToken>,
    ) -> Self {
        Self {
            workspace_services,
            cancellation_token,
        }
    }

    pub fn workspace_services(&self) -> Option<&WorkspaceServices> {
        self.workspace_services.as_ref()
    }

    pub fn cancellation_token(&self) -> Option<&CancellationToken> {
        self.cancellation_token.as_ref()
    }
}

impl std::fmt::Debug for ToolRuntimeHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRuntimeHandles")
            .field(
                "workspace_services",
                &self
                    .workspace_services
                    .as_ref()
                    .map(|_| "<WorkspaceServices>"),
            )
            .field(
                "cancellation_token",
                &self
                    .cancellation_token
                    .as_ref()
                    .map(|_| "<CancellationToken>"),
            )
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub scope: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    Allow,
    Deny { reason: String },
}

#[async_trait::async_trait]
pub trait PermissionPort: RuntimeServicePort {
    async fn request_permission(
        &self,
        request: PermissionRequest,
    ) -> PortResult<PermissionDecision>;
}

pub trait ClockPort: RuntimeServicePort {
    fn now_unix_millis(&self) -> i64;
}

pub trait TerminalPort: RuntimeServicePort {}

pub trait NetworkPort: RuntimeServicePort {}

pub trait GitPort: RuntimeServicePort {}

pub trait McpCatalogPort: RuntimeServicePort {}

/// Typed registration boundary for remote connection providers.
///
/// PR1 intentionally keeps this trait handle-free; PR2 adds owner-specific
/// lifecycle methods once behavior-equivalence tests are in place.
pub trait RemoteConnectionPort: RuntimeServicePort {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteWorkspaceKind {
    Normal,
    Assistant,
    Remote,
}

impl RemoteWorkspaceKind {
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Assistant => "assistant",
            Self::Remote => "remote",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspaceFacts {
    pub path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    pub kind: RemoteWorkspaceKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRecentWorkspaceFacts {
    pub path: String,
    pub name: String,
    pub last_opened: String,
    pub kind: RemoteWorkspaceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAssistantWorkspaceFacts {
    pub path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspaceUpdate {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSessionMetadata {
    pub session_id: String,
    pub name: String,
    pub agent_type: String,
    pub created_at_ms: u64,
    pub last_active_at_ms: u64,
    pub turn_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorkspaceFileContent {
    pub name: String,
    pub bytes: Vec<u8>,
    pub mime_type: &'static str,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorkspaceFileChunk {
    pub name: String,
    pub bytes: Vec<u8>,
    pub offset: u64,
    pub chunk_size: u64,
    pub total_size: u64,
    pub mime_type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorkspaceFileInfo {
    pub name: String,
    pub size: u64,
    pub mime_type: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteFileChunkRange {
    pub start: usize,
    pub end: usize,
    pub chunk_size: u64,
}

/// Old remote-connect host compatibility trait for workspace commands.
#[async_trait::async_trait]
pub trait RemoteWorkspaceRuntimeHost: Send + Sync {
    async fn current_workspace(&self) -> Option<RemoteWorkspaceFacts>;
    async fn recent_workspaces(&self) -> Vec<RemoteRecentWorkspaceFacts>;
    async fn open_workspace(&self, path: &str) -> Result<RemoteWorkspaceUpdate, String>;
    async fn assistant_workspaces(&self) -> Vec<RemoteAssistantWorkspaceFacts>;
    async fn open_assistant_workspace(&self, path: &str) -> Result<RemoteWorkspaceUpdate, String>;
}

/// Typed registration boundary for remote workspace providers.
pub trait RemoteWorkspacePort: RuntimeServicePort + RemoteWorkspaceRuntimeHost {}

impl<T> RemoteWorkspacePort for T where T: RuntimeServicePort + RemoteWorkspaceRuntimeHost + ?Sized {}

/// Old remote-connect host compatibility trait for initial sync.
#[async_trait::async_trait]
pub trait RemoteInitialSyncRuntimeHost: Send + Sync {
    async fn current_workspace(&self) -> Option<RemoteWorkspaceFacts>;
    async fn list_session_metadata(
        &self,
        workspace_path: &Path,
    ) -> Result<Vec<RemoteSessionMetadata>, String>;
}

/// Old remote-connect host compatibility trait for remote file projection.
#[async_trait::async_trait]
pub trait RemoteWorkspaceFileRuntimeHost: Send + Sync {
    async fn resolve_remote_file_workspace_root(&self, session_id: Option<&str>)
        -> Option<PathBuf>;
}

/// Typed registration boundary for remote filesystem/terminal/image projection providers.
pub trait RemoteProjectionPort: RuntimeServicePort + RemoteWorkspaceFileRuntimeHost {}

impl<T> RemoteProjectionPort for T where
    T: RuntimeServicePort + RemoteWorkspaceFileRuntimeHost + ?Sized
{
}

/// Typed registration boundary for remote host capability facts.
pub trait RemoteCapabilityPort: RuntimeServicePort {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionCreateRequest {
    pub session_name: String,
    pub agent_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionCreateResult {
    pub session_id: String,
    pub agent_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSubmissionRequest {
    pub session_id: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AgentSubmissionSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<AgentInputAttachment>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSubmissionSource {
    DesktopUi,
    DesktopApi,
    AgentSession,
    ScheduledJob,
    RemoteRelay,
    Bot,
    Cli,
}

pub type DialogTriggerSource = AgentSubmissionSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogQueuePriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogSubmissionPolicy {
    pub trigger_source: DialogTriggerSource,
    pub queue_priority: DialogQueuePriority,
    pub skip_tool_confirmation: bool,
}

impl DialogSubmissionPolicy {
    pub const fn new(
        trigger_source: DialogTriggerSource,
        queue_priority: DialogQueuePriority,
        skip_tool_confirmation: bool,
    ) -> Self {
        Self {
            trigger_source,
            queue_priority,
            skip_tool_confirmation,
        }
    }

    pub const fn for_source(trigger_source: DialogTriggerSource) -> Self {
        let (queue_priority, skip_tool_confirmation) = match trigger_source {
            DialogTriggerSource::AgentSession => (DialogQueuePriority::Low, true),
            DialogTriggerSource::ScheduledJob => (DialogQueuePriority::Low, true),
            DialogTriggerSource::DesktopUi
            | DialogTriggerSource::DesktopApi
            | DialogTriggerSource::Cli => (DialogQueuePriority::Normal, false),
            DialogTriggerSource::RemoteRelay | DialogTriggerSource::Bot => {
                (DialogQueuePriority::Normal, true)
            }
        };
        Self::new(trigger_source, queue_priority, skip_tool_confirmation)
    }

    pub const fn with_queue_priority(mut self, queue_priority: DialogQueuePriority) -> Self {
        self.queue_priority = queue_priority;
        self
    }

    pub const fn with_skip_tool_confirmation(mut self, skip_tool_confirmation: bool) -> Self {
        self.skip_tool_confirmation = skip_tool_confirmation;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogSubmitOutcome {
    Started { session_id: String, turn_id: String },
    Queued { session_id: String, turn_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogSessionStateFact {
    Missing,
    Idle,
    Processing,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogSubmitQueueFacts {
    pub session_state: DialogSessionStateFact,
    pub queue_has_items: bool,
    pub policy: DialogSubmissionPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogSubmitQueueAction {
    StartImmediately,
    ClearQueueAndStartImmediately,
    EnqueueThenStartNext,
    EnqueueForActiveTurn { request_yield: bool },
}

pub const fn dialog_policy_may_preempt(policy: &DialogSubmissionPolicy) -> bool {
    matches!(
        policy.trigger_source,
        DialogTriggerSource::DesktopUi
            | DialogTriggerSource::DesktopApi
            | DialogTriggerSource::Cli
            | DialogTriggerSource::RemoteRelay
            | DialogTriggerSource::Bot
    )
}

pub const fn resolve_dialog_submit_queue_action(
    facts: DialogSubmitQueueFacts,
) -> DialogSubmitQueueAction {
    match facts.session_state {
        DialogSessionStateFact::Missing => DialogSubmitQueueAction::StartImmediately,
        DialogSessionStateFact::Error => DialogSubmitQueueAction::ClearQueueAndStartImmediately,
        DialogSessionStateFact::Idle => {
            if facts.queue_has_items {
                DialogSubmitQueueAction::EnqueueThenStartNext
            } else {
                DialogSubmitQueueAction::StartImmediately
            }
        }
        DialogSessionStateFact::Processing => DialogSubmitQueueAction::EnqueueForActiveTurn {
            request_yield: dialog_policy_may_preempt(&facts.policy),
        },
    }
}

pub fn should_suppress_agent_session_cancelled_reply(
    policy: &DialogSubmissionPolicy,
    reply_source_session_id: Option<&str>,
    requester_session_id: &str,
) -> bool {
    policy.trigger_source == DialogTriggerSource::AgentSession
        && reply_source_session_id.is_some_and(|source| source == requester_session_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogTurnOutcomeKind {
    Completed,
    Cancelled,
    Failed,
}

pub const fn should_skip_agent_session_reply(
    outcome_kind: DialogTurnOutcomeKind,
    suppressed_cancelled_reply: bool,
) -> bool {
    matches!(outcome_kind, DialogTurnOutcomeKind::Cancelled) && suppressed_cancelled_reply
}

/// Source session route used when an agent-session request should reply to the
/// requester after the target session finishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionReplyRoute {
    pub source_session_id: String,
    pub source_workspace_path: String,
}

/// Outcome for steering a message into an already-running dialog turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogSteerOutcome {
    /// Steering was buffered for the running turn and will be consumed at the
    /// next model-round boundary.
    Buffered {
        session_id: String,
        turn_id: String,
        steering_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundInjectionKind {
    UserSteering,
    BackgroundResult,
    ThreadGoalObjectiveUpdated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundInjectionTarget {
    /// Only inject into the exact targeted running turn.
    ExactTurn(String),
    /// Inject into whichever turn is currently running for the session.
    CurrentRunningTurn,
}

/// A message to inject into the currently running dialog turn at the next
/// model-round boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundInjection {
    pub id: String,
    pub kind: RoundInjectionKind,
    pub target: RoundInjectionTarget,
    pub content: String,
    pub display_content: String,
    pub created_at: std::time::SystemTime,
}

/// Observes whether the current dialog turn should end after the latest model
/// round so a queued user message can start as a new turn.
pub trait DialogRoundPreemptSource: Send + Sync {
    fn should_yield_after_round(&self, session_id: &str) -> bool;
    fn clear_yield_after_round(&self, session_id: &str);
}

/// Observes round-boundary injections for a given running turn.
pub trait DialogRoundInjectionSource: Send + Sync {
    fn has_pending(&self, session_id: &str, turn_id: &str) -> bool;
    fn take_pending(&self, session_id: &str, turn_id: &str) -> Vec<RoundInjection>;
}

/// Legacy session metadata key for the pre-Codex goal mode experiment.
pub const GOAL_MODE_METADATA_KEY: &str = "goal_mode";

/// Persisted thread goal stored in session custom metadata.
pub const THREAD_GOAL_METADATA_KEY: &str = "thread_goal";

pub const MAX_THREAD_GOAL_OBJECTIVE_CHARS: usize = 4_000;

pub const MAX_CONTEXT_SUMMARY_CHARS: usize = 12_000;

/// Max automatic goal continuation dialog turns per objective (legacy goal_mode parity).
pub const MAX_THREAD_GOAL_AUTO_CONTINUATIONS: u32 = 100;

/// Alias retained for migration from legacy `goal_mode` metadata and docs.
pub const MAX_GOAL_CONTINUATIONS: u32 = MAX_THREAD_GOAL_AUTO_CONTINUATIONS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ThreadGoalStatus {
    Active,
    Paused,
    Blocked,
    UsageLimited,
    BudgetLimited,
    Complete,
}

impl ThreadGoalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Blocked => "blocked",
            Self::UsageLimited => "usageLimited",
            Self::BudgetLimited => "budgetLimited",
            Self::Complete => "complete",
        }
    }
}

pub fn validate_thread_goal_objective(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("goal objective must not be empty".to_string());
    }
    if value.chars().count() > MAX_THREAD_GOAL_OBJECTIVE_CHARS {
        return Err(format!(
            "goal objective must be at most {MAX_THREAD_GOAL_OBJECTIVE_CHARS} characters"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoal {
    pub goal_id: String,
    pub session_id: String,
    pub objective: String,
    pub status: ThreadGoalStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<i64>,
    #[serde(default)]
    pub tokens_used: i64,
    #[serde(default)]
    pub time_used_seconds: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// Auto-continuation dialog turns scheduled toward this goal (resets on new objective).
    #[serde(default)]
    pub auto_continuation_count: u32,
}

impl ThreadGoal {
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            ThreadGoalStatus::Active | ThreadGoalStatus::BudgetLimited
        )
    }

    pub fn remaining_tokens(&self) -> Option<i64> {
        self.token_budget
            .map(|budget| (budget - self.tokens_used).max(0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetThreadGoalResult {
    pub goal: ThreadGoal,
    pub replaced_existing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadGoalContinuationPlan {
    pub prepended_reminders: Vec<String>,
    pub display_message: String,
    pub user_message_metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalToolResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<ThreadGoal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_budget_report: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressionContract {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub touched_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_commands: Vec<CompressionContractItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocking_failures: Vec<CompressionContractItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subagent_statuses: Vec<CompressionContractItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressionContractItem {
    pub target: String,
    pub status: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
}

impl CompressionContract {
    pub fn is_empty(&self) -> bool {
        self.touched_files.is_empty()
            && self.verification_commands.is_empty()
            && self.blocking_failures.is_empty()
            && self.subagent_statuses.is_empty()
    }

    pub fn render_for_model(&self) -> String {
        let mut lines = vec![
            "Compaction contract: preserve these factual fields when continuing the task."
                .to_string(),
        ];

        if !self.touched_files.is_empty() {
            lines.push("Touched files:".to_string());
            for file in &self.touched_files {
                lines.push(format!("- {}", file));
            }
        }

        render_contract_items(
            &mut lines,
            "Verification commands:",
            &self.verification_commands,
        );
        render_contract_items(&mut lines, "Blocking failures:", &self.blocking_failures);
        render_contract_items(&mut lines, "Subagent statuses:", &self.subagent_statuses);

        lines.join("\n")
    }
}

fn render_contract_items(lines: &mut Vec<String>, title: &str, items: &[CompressionContractItem]) {
    if items.is_empty() {
        return;
    }

    lines.push(title.to_string());
    for item in items {
        let mut rendered = format!("- {} [{}]: {}", item.target, item.status, item.summary);
        if let Some(error_kind) = item.error_kind.as_ref() {
            rendered.push_str(&format!(" ({})", error_kind));
        }
        lines.push(rendered);
    }
}

/// User-managed related directory reference for request-context prompts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RelatedPath {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInputAttachment {
    pub kind: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

impl AgentInputAttachment {
    pub fn remote_image(
        id: impl Into<String>,
        name: impl Into<String>,
        data_url: impl Into<String>,
    ) -> Self {
        let mut metadata = serde_json::Map::new();
        metadata.insert("name".to_string(), serde_json::Value::String(name.into()));
        metadata.insert(
            "dataUrl".to_string(),
            serde_json::Value::String(data_url.into()),
        );

        Self {
            kind: "remote_image".to_string(),
            id: id.into(),
            metadata,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSubmissionResult {
    pub turn_id: String,
    #[serde(default)]
    pub accepted: bool,
}

#[async_trait::async_trait]
pub trait AgentSubmissionPort: Send + Sync {
    async fn create_session(
        &self,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult>;

    async fn submit_message(
        &self,
        request: AgentSubmissionRequest,
    ) -> PortResult<AgentSubmissionResult>;

    async fn resolve_session_agent_type(&self, session_id: &str) -> PortResult<Option<String>>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurnCancellationRequest {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AgentSubmissionSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurnCancellationResult {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub requested: bool,
}

#[async_trait::async_trait]
pub trait AgentTurnCancellationPort: Send + Sync {
    async fn cancel_turn(
        &self,
        request: AgentTurnCancellationRequest,
    ) -> PortResult<AgentTurnCancellationResult>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteControlSessionState {
    Idle,
    Processing,
    Error,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlStateRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlStateSnapshot {
    pub session_id: String,
    pub state: RemoteControlSessionState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_turn_id: Option<String>,
    #[serde(default)]
    pub queue_depth: usize,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

#[async_trait::async_trait]
pub trait RemoteControlStatePort: Send + Sync {
    async fn read_remote_control_state(
        &self,
        request: RemoteControlStateRequest,
    ) -> PortResult<Option<RemoteControlStateSnapshot>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEventType {
    TurnStarted,
    TurnCompleted,
    TurnFailed,
    TurnCancelled,
    SessionStateChanged,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEventEnvelope {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AgentSubmissionSource>,
    pub event_type: RuntimeEventType,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[async_trait::async_trait]
pub trait RuntimeEventSink: Send + Sync {
    async fn publish_runtime_event(&self, event: RuntimeEventEnvelope) -> PortResult<()>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
}

#[async_trait::async_trait]
pub trait DynamicToolProvider: Send + Sync {
    async fn list_dynamic_tools(&self) -> PortResult<Vec<DynamicToolDescriptor>>;
}

pub trait ToolDecorator<Tool>: Send + Sync {
    fn decorate(&self, tool: Tool) -> Tool;
}

#[async_trait::async_trait]
pub trait ConfigReadPort: Send + Sync {
    async fn get_config_value(&self, key: &str) -> PortResult<Option<serde_json::Value>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTranscriptRequest {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTranscript {
    pub session_id: String,
    #[serde(default)]
    pub messages: Vec<TranscriptMessage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub content: serde_json::Value,
}

#[async_trait::async_trait]
pub trait SessionTranscriptReader: Send + Sync {
    async fn read_session_transcript(
        &self,
        request: SessionTranscriptRequest,
    ) -> PortResult<SessionTranscript>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DelegationPolicy {
    pub allow_subagent_spawn: bool,
    pub nesting_depth: u8,
}

impl Default for DelegationPolicy {
    fn default() -> Self {
        Self::top_level()
    }
}

impl DelegationPolicy {
    pub fn top_level() -> Self {
        Self {
            allow_subagent_spawn: true,
            nesting_depth: 0,
        }
    }

    pub fn spawn_child(self) -> Self {
        Self {
            allow_subagent_spawn: false,
            nesting_depth: self.nesting_depth.saturating_add(1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentContextMode {
    #[default]
    Fresh,
    Fork,
}

impl SubagentContextMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Fork => "fork",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_error_display_keeps_kind_and_message() {
        let error = PortError::new(PortErrorKind::NotAvailable, "coordinator missing");

        assert_eq!(
            error.to_string(),
            "NotAvailable: coordinator missing".to_string()
        );
    }

    #[test]
    fn agent_submission_request_serializes_with_stable_camel_case() {
        let request = AgentSubmissionRequest {
            session_id: "session_1".to_string(),
            message: "hello".to_string(),
            turn_id: None,
            source: None,
            attachments: Vec::new(),
            metadata: serde_json::Map::new(),
        };

        let json = serde_json::to_value(request).expect("serialize request");

        assert_eq!(json["sessionId"], "session_1");
        assert_eq!(json["message"], "hello");
        assert!(json.get("source").is_none());
        assert!(json.get("attachments").is_none());
    }

    #[test]
    fn agent_submission_request_serializes_source_without_changing_field_case() {
        let request = AgentSubmissionRequest {
            session_id: "session_1".to_string(),
            message: "hello".to_string(),
            turn_id: None,
            source: Some(AgentSubmissionSource::RemoteRelay),
            attachments: Vec::new(),
            metadata: serde_json::Map::new(),
        };

        let json = serde_json::to_value(request).expect("serialize request");

        assert_eq!(json["source"], "remote_relay");
        assert!(json.get("turnId").is_none());
    }

    #[test]
    fn dialog_trigger_source_reuses_agent_submission_source_contract() {
        let json = serde_json::to_value(DialogTriggerSource::Cli)
            .expect("serialize dialog trigger source");

        assert_eq!(json, serde_json::json!("cli"));
    }

    #[test]
    fn dialog_submission_policy_preserves_current_surface_queue_defaults() {
        let remote = DialogSubmissionPolicy::for_source(DialogTriggerSource::RemoteRelay);
        assert_eq!(remote.queue_priority, DialogQueuePriority::Normal);
        assert!(remote.skip_tool_confirmation);

        let bot = DialogSubmissionPolicy::for_source(DialogTriggerSource::Bot);
        assert_eq!(bot.queue_priority, DialogQueuePriority::Normal);
        assert!(bot.skip_tool_confirmation);

        let agent_session = DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession);
        assert_eq!(agent_session.queue_priority, DialogQueuePriority::Low);
        assert!(agent_session.skip_tool_confirmation);

        let cli = DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli);
        assert_eq!(cli.queue_priority, DialogQueuePriority::Normal);
        assert!(!cli.skip_tool_confirmation);
    }

    #[test]
    fn dialog_submit_outcome_preserves_started_and_queued_fields() {
        let started = DialogSubmitOutcome::Started {
            session_id: "session_1".to_string(),
            turn_id: "turn_1".to_string(),
        };
        let queued = DialogSubmitOutcome::Queued {
            session_id: "session_1".to_string(),
            turn_id: "turn_2".to_string(),
        };

        assert_eq!(
            started,
            DialogSubmitOutcome::Started {
                session_id: "session_1".to_string(),
                turn_id: "turn_1".to_string(),
            }
        );
        assert_ne!(started, queued);
    }

    #[test]
    fn dialog_submit_queue_action_preserves_current_scheduler_routing_policy() {
        let remote = DialogSubmissionPolicy::for_source(DialogTriggerSource::RemoteRelay);
        assert!(dialog_policy_may_preempt(&remote));

        let agent_session = DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession);
        assert!(!dialog_policy_may_preempt(&agent_session));

        assert_eq!(
            resolve_dialog_submit_queue_action(DialogSubmitQueueFacts {
                session_state: DialogSessionStateFact::Missing,
                queue_has_items: true,
                policy: remote,
            }),
            DialogSubmitQueueAction::StartImmediately
        );
        assert_eq!(
            resolve_dialog_submit_queue_action(DialogSubmitQueueFacts {
                session_state: DialogSessionStateFact::Error,
                queue_has_items: true,
                policy: remote,
            }),
            DialogSubmitQueueAction::ClearQueueAndStartImmediately
        );
        assert_eq!(
            resolve_dialog_submit_queue_action(DialogSubmitQueueFacts {
                session_state: DialogSessionStateFact::Idle,
                queue_has_items: false,
                policy: remote,
            }),
            DialogSubmitQueueAction::StartImmediately
        );
        assert_eq!(
            resolve_dialog_submit_queue_action(DialogSubmitQueueFacts {
                session_state: DialogSessionStateFact::Idle,
                queue_has_items: true,
                policy: remote,
            }),
            DialogSubmitQueueAction::EnqueueThenStartNext
        );
        assert_eq!(
            resolve_dialog_submit_queue_action(DialogSubmitQueueFacts {
                session_state: DialogSessionStateFact::Processing,
                queue_has_items: false,
                policy: remote,
            }),
            DialogSubmitQueueAction::EnqueueForActiveTurn {
                request_yield: true
            }
        );
        assert_eq!(
            resolve_dialog_submit_queue_action(DialogSubmitQueueFacts {
                session_state: DialogSessionStateFact::Processing,
                queue_has_items: false,
                policy: agent_session,
            }),
            DialogSubmitQueueAction::EnqueueForActiveTurn {
                request_yield: false
            }
        );
    }

    #[test]
    fn agent_session_reply_decisions_preserve_cancel_suppression_boundary() {
        let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession);
        assert!(should_suppress_agent_session_cancelled_reply(
            &policy,
            Some("requester"),
            "requester",
        ));
        assert!(!should_suppress_agent_session_cancelled_reply(
            &policy,
            Some("requester"),
            "other",
        ));

        let remote = DialogSubmissionPolicy::for_source(DialogTriggerSource::RemoteRelay);
        assert!(!should_suppress_agent_session_cancelled_reply(
            &remote,
            Some("requester"),
            "requester",
        ));

        assert!(should_skip_agent_session_reply(
            DialogTurnOutcomeKind::Cancelled,
            true,
        ));
        assert!(!should_skip_agent_session_reply(
            DialogTurnOutcomeKind::Cancelled,
            false,
        ));
        assert!(!should_skip_agent_session_reply(
            DialogTurnOutcomeKind::Completed,
            true,
        ));
        assert!(!should_skip_agent_session_reply(
            DialogTurnOutcomeKind::Failed,
            true,
        ));
    }

    #[test]
    fn agent_session_reply_route_keeps_requester_fields() {
        let route = AgentSessionReplyRoute {
            source_session_id: "requester_session".to_string(),
            source_workspace_path: "/workspace/requester".to_string(),
        };

        assert_eq!(route.source_session_id, "requester_session");
        assert_eq!(route.source_workspace_path, "/workspace/requester");
    }

    #[test]
    fn remote_workspace_contracts_preserve_workspace_and_session_facts() {
        let workspace = RemoteWorkspaceFacts {
            path: "/workspace/project".to_string(),
            name: "project".to_string(),
            git_branch: Some("main".to_string()),
            kind: RemoteWorkspaceKind::Remote,
            assistant_id: Some("assistant_1".to_string()),
        };
        let session = RemoteSessionMetadata {
            session_id: "session_1".to_string(),
            name: "Research".to_string(),
            agent_type: "CodeAgent".to_string(),
            created_at_ms: 10,
            last_active_at_ms: 20,
            turn_count: 3,
        };

        assert_eq!(workspace.kind.as_wire_str(), "remote");
        assert_eq!(workspace.assistant_id.as_deref(), Some("assistant_1"));
        assert_eq!(session.turn_count, 3);
    }

    #[test]
    fn remote_projection_contract_preserves_file_chunk_identity() {
        let chunk = RemoteWorkspaceFileChunk {
            name: "report.md".to_string(),
            bytes: b"chunk".to_vec(),
            offset: 6,
            chunk_size: 5,
            total_size: 11,
            mime_type: "text/markdown",
        };

        assert_eq!(chunk.name, "report.md");
        assert_eq!(chunk.bytes, b"chunk");
        assert_eq!(chunk.offset + chunk.chunk_size, chunk.total_size);
    }

    #[test]
    fn dialog_steer_outcome_preserves_buffered_fields() {
        let outcome = DialogSteerOutcome::Buffered {
            session_id: "session_1".to_string(),
            turn_id: "turn_1".to_string(),
            steering_id: "steer_1".to_string(),
        };

        assert_eq!(
            outcome,
            DialogSteerOutcome::Buffered {
                session_id: "session_1".to_string(),
                turn_id: "turn_1".to_string(),
                steering_id: "steer_1".to_string(),
            }
        );
    }

    #[test]
    fn round_injection_contract_keeps_kind_and_target_identity() {
        assert_eq!(
            RoundInjectionKind::UserSteering,
            RoundInjectionKind::UserSteering
        );
        assert_ne!(
            RoundInjectionKind::UserSteering,
            RoundInjectionKind::BackgroundResult
        );

        let target = RoundInjectionTarget::ExactTurn("turn_1".to_string());
        assert_eq!(
            target,
            RoundInjectionTarget::ExactTurn("turn_1".to_string())
        );
        assert_ne!(target, RoundInjectionTarget::CurrentRunningTurn);
    }

    #[test]
    fn round_injection_source_contract_drains_portable_injections() {
        struct StaticInjectionSource {
            injection: RoundInjection,
        }

        impl DialogRoundInjectionSource for StaticInjectionSource {
            fn has_pending(&self, session_id: &str, turn_id: &str) -> bool {
                session_id == "session_1" && turn_id == "turn_1"
            }

            fn take_pending(&self, session_id: &str, turn_id: &str) -> Vec<RoundInjection> {
                if self.has_pending(session_id, turn_id) {
                    vec![self.injection.clone()]
                } else {
                    Vec::new()
                }
            }
        }

        let source = StaticInjectionSource {
            injection: RoundInjection {
                id: "injection_1".to_string(),
                kind: RoundInjectionKind::BackgroundResult,
                target: RoundInjectionTarget::CurrentRunningTurn,
                content: "result".to_string(),
                display_content: "result".to_string(),
                created_at: std::time::SystemTime::UNIX_EPOCH,
            },
        };

        assert!(source.has_pending("session_1", "turn_1"));
        assert!(!source.has_pending("session_2", "turn_1"));
        let drained = source.take_pending("session_1", "turn_1");
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "injection_1");
        assert_eq!(drained[0].kind, RoundInjectionKind::BackgroundResult);
    }

    #[test]
    fn thread_goal_active_status_includes_budget_limited() {
        let active = ThreadGoal {
            goal_id: "goal_1".to_string(),
            session_id: "session_1".to_string(),
            objective: "Ship feature".to_string(),
            status: ThreadGoalStatus::Active,
            token_budget: Some(10_000),
            tokens_used: 100,
            time_used_seconds: 5,
            created_at: 1,
            updated_at: 2,
            auto_continuation_count: 0,
        };
        assert!(active.is_active());
        assert_eq!(active.remaining_tokens(), Some(9_900));

        let budget_limited = ThreadGoal {
            status: ThreadGoalStatus::BudgetLimited,
            ..active.clone()
        };
        assert!(budget_limited.is_active());

        let paused = ThreadGoal {
            status: ThreadGoalStatus::Paused,
            ..active
        };
        assert!(!paused.is_active());
    }

    #[test]
    fn thread_goal_tool_response_serializes_optional_fields() {
        let response = ThreadGoalToolResponse {
            goal: None,
            remaining_tokens: Some(42),
            completion_budget_report: None,
        };
        let json = serde_json::to_value(response).expect("serialize thread goal tool response");
        assert!(json.get("goal").is_none());
        assert_eq!(json["remainingTokens"], 42);
        assert!(json.get("completionBudgetReport").is_none());
    }

    #[test]
    fn compression_contract_renders_model_visible_fields() {
        let contract = CompressionContract {
            touched_files: vec!["src/lib.rs".to_string()],
            verification_commands: vec![CompressionContractItem {
                target: "cargo test -p bitfun-runtime-ports".to_string(),
                status: "passed".to_string(),
                summary: "runtime ports contract tests passed".to_string(),
                error_kind: None,
            }],
            blocking_failures: vec![CompressionContractItem {
                target: "cargo check".to_string(),
                status: "failed".to_string(),
                summary: "compile error before migration".to_string(),
                error_kind: Some("compile".to_string()),
            }],
            subagent_statuses: Vec::new(),
        };

        let rendered = contract.render_for_model();

        assert!(rendered.contains("Compaction contract"));
        assert!(rendered.contains("Touched files:"));
        assert!(rendered.contains("- src/lib.rs"));
        assert!(rendered.contains(
            "- cargo test -p bitfun-runtime-ports [passed]: runtime ports contract tests passed"
        ));
        assert!(
            rendered.contains("- cargo check [failed]: compile error before migration (compile)")
        );
    }

    #[test]
    fn related_path_serializes_as_request_context_fact() {
        let related = RelatedPath {
            path: "/workspace/shared".to_string(),
            description: Some("shared fixtures".to_string()),
        };

        let json = serde_json::to_value(related).expect("serialize related path");

        assert_eq!(json["path"], "/workspace/shared");
        assert_eq!(json["description"], "shared fixtures");
        assert!(json.get("related_path").is_none());
    }

    #[test]
    fn agent_submission_request_serializes_explicit_turn_id_contract() {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "turnId".to_string(),
            serde_json::Value::String("legacy_metadata_turn".to_string()),
        );
        let request = AgentSubmissionRequest {
            session_id: "session_1".to_string(),
            message: "hello".to_string(),
            turn_id: Some("explicit_turn".to_string()),
            source: Some(AgentSubmissionSource::RemoteRelay),
            attachments: Vec::new(),
            metadata,
        };

        let json = serde_json::to_value(request).expect("serialize request");

        assert_eq!(json["turnId"], "explicit_turn");
        assert_eq!(json["metadata"]["turnId"], "legacy_metadata_turn");
    }

    #[test]
    fn remote_image_attachment_serializes_portable_metadata_contract() {
        let attachment =
            AgentInputAttachment::remote_image("image-1", "clip.png", "data:image/png;base64,abc");

        let json = serde_json::to_value(attachment).expect("serialize attachment");

        assert_eq!(json["kind"], "remote_image");
        assert_eq!(json["id"], "image-1");
        assert_eq!(json["metadata"]["name"], "clip.png");
        assert_eq!(json["metadata"]["dataUrl"], "data:image/png;base64,abc");
    }

    #[test]
    fn agent_turn_cancellation_request_serializes_current_contract() {
        let request = AgentTurnCancellationRequest {
            session_id: "session_1".to_string(),
            turn_id: Some("turn_1".to_string()),
            source: Some(AgentSubmissionSource::Bot),
            reason: Some("user_cancelled".to_string()),
            wait_timeout_ms: Some(1500),
        };

        let json = serde_json::to_value(request).expect("serialize cancel request");

        assert_eq!(json["sessionId"], "session_1");
        assert_eq!(json["turnId"], "turn_1");
        assert_eq!(json["source"], "bot");
        assert_eq!(json["reason"], "user_cancelled");
        assert_eq!(json["waitTimeoutMs"], 1500);
    }

    #[test]
    fn remote_control_state_snapshot_serializes_active_turn_contract() {
        let snapshot = RemoteControlStateSnapshot {
            session_id: "session_1".to_string(),
            state: RemoteControlSessionState::Processing,
            active_turn_id: Some("turn_1".to_string()),
            queue_depth: 2,
            metadata: serde_json::Map::new(),
        };

        let json = serde_json::to_value(snapshot).expect("serialize state snapshot");

        assert_eq!(json["sessionId"], "session_1");
        assert_eq!(json["state"], "processing");
        assert_eq!(json["activeTurnId"], "turn_1");
        assert_eq!(json["queueDepth"], 2);
    }

    #[test]
    fn runtime_event_envelope_serializes_observational_surface_facts() {
        let event = RuntimeEventEnvelope {
            session_id: "session_1".to_string(),
            turn_id: Some("turn_1".to_string()),
            source: Some(AgentSubmissionSource::RemoteRelay),
            event_type: RuntimeEventType::TurnCancelled,
            payload: serde_json::json!({ "reason": "user_cancelled" }),
        };

        let json = serde_json::to_value(event).expect("serialize event");

        assert_eq!(json["sessionId"], "session_1");
        assert_eq!(json["turnId"], "turn_1");
        assert_eq!(json["source"], "remote_relay");
        assert_eq!(json["eventType"], "turn_cancelled");
        assert_eq!(json["payload"]["reason"], "user_cancelled");
    }

    #[test]
    fn session_transcript_request_serializes_turn_id_contract() {
        let request = SessionTranscriptRequest {
            session_id: "session_1".to_string(),
            turn_id: Some("turn_1".to_string()),
        };

        let json = serde_json::to_value(request).expect("serialize transcript request");

        assert_eq!(json["sessionId"], "session_1");
        assert_eq!(json["turnId"], "turn_1");
        assert!(json.get("fromTurnId").is_none());
    }

    #[test]
    fn dynamic_tool_descriptor_serializes_current_wire_shape() {
        let descriptor = DynamicToolDescriptor {
            name: "external_search".to_string(),
            description: "Search external docs".to_string(),
            input_schema: serde_json::json!({ "type": "object" }),
            provider_id: Some("provider-a".to_string()),
        };

        let json = serde_json::to_value(descriptor).expect("serialize descriptor");

        assert_eq!(json["name"], "external_search");
        assert_eq!(json["description"], "Search external docs");
        assert_eq!(json["inputSchema"]["type"], "object");
        assert_eq!(json["providerId"], "provider-a");
        assert!(json.get("provider_id").is_none());
    }

    #[test]
    fn subagent_context_mode_preserves_fork_wire_value() {
        assert_eq!(SubagentContextMode::default(), SubagentContextMode::Fresh);
        assert_eq!(SubagentContextMode::Fresh.as_str(), "fresh");
        assert_eq!(SubagentContextMode::Fork.as_str(), "fork");

        let json = serde_json::to_value(SubagentContextMode::Fork)
            .expect("serialize subagent context mode");

        assert_eq!(json, serde_json::json!("fork"));
    }

    #[test]
    fn delegation_policy_child_blocks_recursive_spawn_without_losing_depth() {
        let top_level = DelegationPolicy::top_level();
        assert!(top_level.allow_subagent_spawn);
        assert_eq!(top_level.nesting_depth, 0);

        let child = top_level.spawn_child();

        assert!(!child.allow_subagent_spawn);
        assert_eq!(child.nesting_depth, 1);
        assert_eq!(child.spawn_child().nesting_depth, 2);
    }

    #[test]
    fn dynamic_tool_descriptor_omits_missing_provider_id() {
        let descriptor = DynamicToolDescriptor {
            name: "local_tool".to_string(),
            description: "Local tool".to_string(),
            input_schema: serde_json::json!({ "type": "object" }),
            provider_id: None,
        };

        let json = serde_json::to_value(descriptor).expect("serialize descriptor");

        assert!(json.get("providerId").is_none());
    }

    #[derive(Debug)]
    struct FakeWorkspaceFileSystem;

    #[async_trait::async_trait]
    impl WorkspaceFileSystem for FakeWorkspaceFileSystem {
        async fn read_file(&self, _path: &str) -> anyhow::Result<Vec<u8>> {
            Ok(b"hello".to_vec())
        }

        async fn read_file_text(&self, _path: &str) -> anyhow::Result<String> {
            Ok("hello".to_string())
        }

        async fn write_file(&self, _path: &str, _contents: &[u8]) -> anyhow::Result<()> {
            Ok(())
        }

        async fn exists(&self, _path: &str) -> anyhow::Result<bool> {
            Ok(true)
        }

        async fn is_file(&self, _path: &str) -> anyhow::Result<bool> {
            Ok(true)
        }

        async fn is_dir(&self, _path: &str) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn read_dir(&self, _path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
            Ok(vec![WorkspaceDirEntry {
                name: "file.txt".to_string(),
                path: "/workspace/file.txt".to_string(),
                is_dir: false,
                is_symlink: false,
            }])
        }
    }

    #[derive(Debug)]
    struct FakeWorkspaceShell;

    #[async_trait::async_trait]
    impl WorkspaceShell for FakeWorkspaceShell {
        async fn exec_with_options(
            &self,
            _command: &str,
            options: WorkspaceCommandOptions,
        ) -> anyhow::Result<WorkspaceCommandResult> {
            assert_eq!(options.timeout_ms, Some(100));
            assert!(options.cancellation_token.is_none());
            Ok(WorkspaceCommandResult {
                stdout: "ok".to_string(),
                stderr: String::new(),
                exit_code: 0,
                interrupted: false,
                timed_out: false,
            })
        }
    }

    #[test]
    fn workspace_services_contract_is_runtime_port_owned() {
        let services = WorkspaceServices {
            fs: std::sync::Arc::new(FakeWorkspaceFileSystem),
            shell: std::sync::Arc::new(FakeWorkspaceShell),
        };

        let cloned = services.clone();
        assert!(std::sync::Arc::ptr_eq(&services.fs, &cloned.fs));
        assert!(std::sync::Arc::ptr_eq(&services.shell, &cloned.shell));
        assert_eq!(
            format!("{:?}", services),
            "WorkspaceServices { fs: \"<dyn WorkspaceFileSystem>\", shell: \"<dyn WorkspaceShell>\" }"
        );
    }

    #[test]
    fn tool_runtime_handles_keep_workspace_services_and_cancellation_contracts() {
        let cancellation_token = tokio_util::sync::CancellationToken::new();
        let services = WorkspaceServices {
            fs: std::sync::Arc::new(FakeWorkspaceFileSystem),
            shell: std::sync::Arc::new(FakeWorkspaceShell),
        };

        let handles =
            ToolRuntimeHandles::new(Some(services.clone()), Some(cancellation_token.clone()));

        assert!(handles.cancellation_token().is_some());
        assert!(handles.workspace_services().is_some());
        assert!(std::sync::Arc::ptr_eq(
            &services.fs,
            &handles.workspace_services().expect("workspace services").fs
        ));

        let cloned = handles.clone();
        assert!(cloned.cancellation_token().is_some());
        assert!(std::sync::Arc::ptr_eq(
            &services.shell,
            &cloned
                .workspace_services()
                .expect("workspace services")
                .shell
        ));
        assert_eq!(
            format!("{:?}", handles),
            "ToolRuntimeHandles { workspace_services: Some(\"<WorkspaceServices>\"), cancellation_token: Some(\"<CancellationToken>\") }"
        );
    }
}

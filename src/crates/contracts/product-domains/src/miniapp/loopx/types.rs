//! Stable LoopX MiniApp wire types.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const LOOPX_BUILTIN_APP_ID: &str = "builtin-bitfun-loopx";
pub const LOOPX_PINNED_VERSION: &str = "0.2.13";
pub const LOOPX_CLI_SCHEMA_VERSION: u32 = 1;

pub type LoopxEventCursor = u64;

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxRepositoryKey {
    pub host: String,
    pub owner: String,
    pub repository: String,
}

impl LoopxRepositoryKey {
    pub fn canonical_id(&self) -> String {
        format!("{}/{}/{}", self.host, self.owner, self.repository)
    }

    pub fn label(&self) -> String {
        format!("{}/{}", self.owner, self.repository)
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum LoopxItemKind {
    #[default]
    Issue,
    #[serde(rename = "pr", alias = "pull_request")]
    PullRequest,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxIssueKey {
    pub repository: LoopxRepositoryKey,
    pub kind: LoopxItemKind,
    pub number: u64,
}

impl LoopxIssueKey {
    pub fn canonical_id(&self) -> String {
        let collection = match self.kind {
            LoopxItemKind::Issue => "issues",
            LoopxItemKind::PullRequest => "pull",
        };
        format!(
            "{}/{}/{}",
            self.repository.canonical_id(),
            collection,
            self.number
        )
    }

    pub fn canonical_url(&self) -> String {
        format!("https://{}", self.canonical_id())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "targetType", rename_all = "snake_case")]
pub enum LoopxIntakeTarget {
    Repository { repository: LoopxRepositoryKey },
    Item { item: LoopxIssueKey },
}

impl Default for LoopxIntakeTarget {
    fn default() -> Self {
        Self::Repository {
            repository: LoopxRepositoryKey::default(),
        }
    }
}

impl LoopxIntakeTarget {
    pub fn repository(&self) -> &LoopxRepositoryKey {
        match self {
            Self::Repository { repository } => repository,
            Self::Item { item } => &item.repository,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxRemoteItemState {
    Open,
    Closed,
    Merged,
    #[default]
    #[serde(other)]
    Unknown,
}

impl LoopxRemoteItemState {
    pub fn is_resolved(self) -> bool {
        matches!(self, Self::Closed | Self::Merged)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxIntakeCandidate {
    pub key: LoopxIssueKey,
    pub url: String,
    pub title: String,
    pub state: LoopxRemoteItemState,
    pub state_reason: Option<String>,
    pub from_repository: bool,
    pub has_images: bool,
    /// Repository/list intake must not silently select every candidate.
    pub default_selected: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxWorkspaceDisposition {
    ExistingWorktree,
    NewWorktree,
    CloneRequired,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxWorkspacePreview {
    pub disposition: LoopxWorkspaceDisposition,
    pub path: Option<String>,
    pub repository_verified: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxModelCapability {
    pub model_id: String,
    pub available: bool,
    pub supports_images: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxPermissionScope {
    WorkspaceRead,
    WorkspaceWrite,
    GitLocal,
    GithubRead,
    AgentExecution,
    Publish,
    PublicComment,
    PullRequest,
    Merge,
    ProductionAction,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxIntakePreview {
    pub fingerprint: String,
    pub target: LoopxIntakeTarget,
    pub repository: LoopxRepositoryKey,
    pub workspace: LoopxWorkspacePreview,
    pub candidates: Vec<LoopxIntakeCandidate>,
    pub truncated: bool,
    pub model: LoopxModelCapability,
    pub permission_scopes: Vec<LoopxPermissionScope>,
    pub resolved_at: i64,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxEnvironmentFactStatus {
    Checking,
    Available,
    Degraded,
    Unavailable,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxEnvironmentFact {
    pub status: LoopxEnvironmentFactStatus,
    pub version: Option<String>,
    pub detail: Option<String>,
    pub remediation: Option<String>,
    pub checked_at: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCoreEnvironmentFacts {
    pub sidecar: LoopxEnvironmentFact,
    pub git_worktree: LoopxEnvironmentFact,
    pub agent_model: LoopxEnvironmentFact,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxOptionalEnvironmentFacts {
    pub python_fallback: LoopxEnvironmentFact,
    pub open_viking: LoopxEnvironmentFact,
    pub github_auth: LoopxEnvironmentFact,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxEnvironmentStatus {
    Checking,
    Ready,
    Degraded,
    Blocked,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxEnvironmentSnapshot {
    pub revision: u64,
    pub status: LoopxEnvironmentStatus,
    pub core: LoopxCoreEnvironmentFacts,
    pub optional: LoopxOptionalEnvironmentFacts,
    pub checked_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxTaskState {
    Preparing,
    Queued,
    Running,
    WaitingForUser,
    RetryWait,
    Cancelling,
    Stopped,
    Completed,
    Failed,
    Archived,
    #[default]
    #[serde(other)]
    RecoveryRequired,
}

impl LoopxTaskState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Stopped | Self::Completed | Self::Failed | Self::Archived
        )
    }

    pub fn was_executing_at_shutdown(self) -> bool {
        matches!(
            self,
            Self::Preparing
                | Self::Queued
                | Self::Running
                | Self::WaitingForUser
                | Self::RetryWait
                | Self::Cancelling
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxPhase {
    ValidatingEnvironment,
    ResolvingIntake,
    PreparingWorkspace,
    CreatingGoal,
    Queued,
    InspectingGoal,
    BuildingTurn,
    StartingAgent,
    AgentRunning,
    ValidatingProgress,
    SettlingTurn,
    WaitingForApproval,
    RetryBackoff,
    Cancelling,
    Recovering,
    Finished,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxTaskIdentity {
    pub item: LoopxIssueKey,
    pub attempt: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxSettlementSummary {
    pub turn_id: Option<String>,
    pub receipt_id: Option<String>,
    pub durable_revision: Option<String>,
    pub settled_at: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxTaskSnapshot {
    #[serde(alias = "id")]
    pub task_id: String,
    pub batch_id: Option<String>,
    pub identity: LoopxTaskIdentity,
    pub generation: u64,
    pub revision: u64,
    pub goal_id: Option<String>,
    pub agent_id: Option<String>,
    #[serde(alias = "status")]
    pub state: LoopxTaskState,
    pub phase: LoopxPhase,
    pub workspace_path: Option<String>,
    pub model_id: Option<String>,
    pub granted_scopes: Vec<LoopxPermissionScope>,
    pub current_turn_id: Option<String>,
    pub current_tool: Option<String>,
    pub last_output_at: Option<i64>,
    pub deadline_at: Option<i64>,
    pub retry_at: Option<i64>,
    pub error: Option<String>,
    pub settlement: LoopxSettlementSummary,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxExecutionDomain {
    LocalDesktop,
    RemoteWorkspace,
    PeerDevice,
    RemoteControl,
    DetachedDispatch,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxExecutionSupport {
    Supported,
    #[default]
    #[serde(other)]
    UnsupportedExecutionDomain,
}

fn default_contract_schema_version() -> u32 {
    LOOPX_CLI_SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxSnapshot {
    #[serde(default = "default_contract_schema_version")]
    pub schema_version: u32,
    pub stream_id: String,
    pub cursor: LoopxEventCursor,
    pub revision: u64,
    pub execution_domain: LoopxExecutionDomain,
    pub execution_support: LoopxExecutionSupport,
    pub unsupported_reason: Option<String>,
    pub environment: LoopxEnvironmentSnapshot,
    pub tasks: Vec<LoopxTaskSnapshot>,
    pub generated_at: i64,
}

impl Default for LoopxSnapshot {
    fn default() -> Self {
        Self {
            schema_version: default_contract_schema_version(),
            stream_id: String::new(),
            cursor: 0,
            revision: 0,
            execution_domain: LoopxExecutionDomain::default(),
            execution_support: LoopxExecutionSupport::default(),
            unsupported_reason: None,
            environment: LoopxEnvironmentSnapshot::default(),
            tasks: Vec::new(),
            generated_at: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxEventLevel {
    Trace,
    Debug,
    Warning,
    Error,
    #[default]
    #[serde(other)]
    Info,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxEventSource {
    #[default]
    Controller,
    Sidecar,
    Agent,
    Git,
    Github,
    System,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxEventKind {
    #[default]
    Progress,
    TaskCreated,
    StateChanged,
    PhaseChanged,
    Log,
    ApprovalRequired,
    SettlementRecorded,
    EnvironmentChanged,
    OperationCancelled,
    SnapshotInvalidated,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxEvent {
    pub stream_id: String,
    pub cursor: LoopxEventCursor,
    pub task_id: Option<String>,
    pub generation: Option<u64>,
    pub revision: Option<u64>,
    pub kind: LoopxEventKind,
    pub level: LoopxEventLevel,
    pub source: LoopxEventSource,
    pub phase: Option<LoopxPhase>,
    pub message: String,
    pub important: bool,
    pub tool_name: Option<String>,
    pub deadline_at: Option<i64>,
    pub details: BTreeMap<String, String>,
    pub occurred_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxAttachRequest {
    pub known_stream_id: Option<String>,
    pub after_cursor: Option<LoopxEventCursor>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxAttachResponse {
    pub snapshot: LoopxSnapshot,
}

fn default_loopx_model_id() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxResolveIntakeRequest {
    pub input: String,
    #[serde(default = "default_loopx_model_id")]
    pub model_id: String,
}

impl Default for LoopxResolveIntakeRequest {
    fn default() -> Self {
        Self {
            input: String::new(),
            model_id: default_loopx_model_id(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxResolveIntakeResponse {
    pub preview: LoopxIntakePreview,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCreateTaskRequest {
    pub client_request_id: String,
    pub preview_fingerprint: String,
    pub selected_items: Vec<LoopxIssueKey>,
    pub model_id: String,
    pub granted_scopes: Vec<LoopxPermissionScope>,
    pub retry_terminal: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxCreateTaskOutcomeKind {
    #[default]
    Created,
    OpenedExisting,
    RetryConfirmationRequired,
    ClosedNoop,
    NeedsLiveVerification,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCreateTaskOutcome {
    pub item: LoopxIssueKey,
    pub kind: LoopxCreateTaskOutcomeKind,
    pub task_id: Option<String>,
    pub attempt: Option<u32>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCreateTaskResponse {
    pub outcomes: Vec<LoopxCreateTaskOutcome>,
    pub snapshot_revision: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxActionKind {
    #[default]
    Pause,
    Resume,
    Approve,
    Reject,
    Archive,
    Restore,
    RetryEnvironment,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxActionRequest {
    pub task_id: Option<String>,
    pub action: LoopxActionKind,
    pub client_request_id: String,
    pub expected_revision: u64,
    pub gate_id: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxActionStatus {
    #[default]
    Applied,
    Duplicate,
    RevisionConflict,
    Rejected,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxActionResponse {
    pub status: LoopxActionStatus,
    pub current_revision: u64,
    pub task: Option<LoopxTaskSnapshot>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxEventsSinceRequest {
    pub stream_id: String,
    pub after_cursor: LoopxEventCursor,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxEventsPageStatus {
    #[default]
    Current,
    SnapshotRequired,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxEventsSinceResponse {
    pub status: LoopxEventsPageStatus,
    pub stream_id: String,
    pub events: Vec<LoopxEvent>,
    pub next_cursor: LoopxEventCursor,
    pub has_more: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxExistingTask {
    pub task_id: String,
    pub identity: LoopxTaskIdentity,
    pub state: LoopxTaskState,
}

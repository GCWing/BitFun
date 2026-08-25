//! Narrow service boundary for the pinned LoopX CLI adapter.

use super::types::{
    LoopxIntakeCandidate, LoopxIntakeTarget, LoopxIssueKey, LoopxPermissionScope,
    LoopxRepositoryKey,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

pub type LoopxCliFuture<'a, T> = Pin<Box<dyn Future<Output = LoopxCliResult<T>> + Send + 'a>>;
pub type LoopxCliResult<T> = Result<T, LoopxCliError>;

fn default_loopx_version() -> String {
    super::types::LOOPX_PINNED_VERSION.to_string()
}

fn default_cli_schema_version() -> u32 {
    super::types::LOOPX_CLI_SCHEMA_VERSION
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxCliErrorKind {
    #[default]
    Backend,
    InvalidInput,
    NotFound,
    VersionMismatch,
    SchemaMismatch,
    Conflict,
    Cancelled,
    Timeout,
    Process,
    Io,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliError {
    pub kind: LoopxCliErrorKind,
    pub message: String,
    pub operation_id: Option<String>,
    pub retryable: bool,
}

impl LoopxCliError {
    pub fn new(kind: LoopxCliErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            operation_id: None,
            retryable: false,
        }
    }

    pub fn for_operation(mut self, operation_id: impl Into<String>) -> Self {
        self.operation_id = Some(operation_id.into());
        self
    }

    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }
}

impl std::fmt::Display for LoopxCliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for LoopxCliError {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliCallContext {
    pub operation_id: String,
    pub deadline_at: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliGoalContext {
    #[serde(flatten)]
    pub call: LoopxCliCallContext,
    pub task_id: String,
    pub generation: u64,
    pub worktree_path: String,
    pub registry_path: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxCliProgressStage {
    #[default]
    StartingSidecar,
    Handshake,
    ResolvingIntake,
    PlanningItem,
    CreatingGoal,
    InspectingGoal,
    BuildingTurn,
    AnsweringGate,
    SettlingTurn,
    Cancelling,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliProgress {
    pub operation_id: String,
    pub task_id: Option<String>,
    pub stage: LoopxCliProgressStage,
    pub message: String,
    pub occurred_at: i64,
}

/// Synchronous projection hook; the controller persists and broadcasts events.
pub trait LoopxCliProgressSink: Send + Sync {
    fn report(&self, progress: LoopxCliProgress);
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxCliSource {
    #[default]
    Unknown,
    Bundled,
    System,
    PythonFallback,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliExecutableIdentity {
    pub source: LoopxCliSource,
    /// Adapter-owned executable identifier; never an argv fragment.
    pub identity: String,
    pub path: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliHandshakeRequest {
    #[serde(flatten)]
    pub call: LoopxCliCallContext,
    #[serde(default = "default_loopx_version")]
    pub required_loopx_version: String,
    #[serde(default = "default_cli_schema_version")]
    pub required_schema_version: u32,
}

impl Default for LoopxCliHandshakeRequest {
    fn default() -> Self {
        Self {
            call: LoopxCliCallContext::default(),
            required_loopx_version: default_loopx_version(),
            required_schema_version: default_cli_schema_version(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliManifest {
    pub adapter_version: String,
    pub loopx_version: String,
    pub schema_version: u32,
    pub executable: LoopxCliExecutableIdentity,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliTodoPlan {
    pub role: String,
    pub task_class: String,
    pub action_kind: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliResolveIntakeRequest {
    #[serde(flatten)]
    pub call: LoopxCliCallContext,
    pub input: String,
    pub target: LoopxIntakeTarget,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliResolveIntakeResult {
    pub target: LoopxIntakeTarget,
    pub repository: LoopxRepositoryKey,
    pub candidates: Vec<LoopxIntakeCandidate>,
    pub truncated: bool,
    pub resolved_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxGithubAuthProbeRequest {
    #[serde(flatten)]
    pub call: LoopxCliCallContext,
}

/// Result of the pre-flight GitHub access probe. The host surfaces this as the
/// `github_auth` environment fact so an auth/rate-limit failure is visible
/// before the user submits an intake, instead of surfacing as a 403 later.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxGithubAuthProbe {
    /// Whether an authenticated GitHub identity is available.
    pub authenticated: bool,
    /// Remaining core API rate limit, when the endpoint reports one.
    pub rate_limit_remaining: Option<u64>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliPlanItemRequest {
    #[serde(flatten)]
    pub context: LoopxCliGoalContext,
    pub item: LoopxIssueKey,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliIntakePlan {
    pub item: LoopxIssueKey,
    pub objective: String,
    pub todos: Vec<LoopxCliTodoPlan>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliCreateGoalRequest {
    #[serde(flatten)]
    pub context: LoopxCliGoalContext,
    pub goal_id: String,
    pub agent_id: String,
    pub intake: LoopxCliIntakePlan,
    pub granted_scopes: Vec<LoopxPermissionScope>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliCreateGoalResult {
    pub goal_id: String,
    pub created: bool,
    pub durable_revision: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliInspectGoalRequest {
    #[serde(flatten)]
    pub context: LoopxCliGoalContext,
    pub goal_id: String,
    pub agent_id: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxCliGoalState {
    #[default]
    Unknown,
    Active,
    WaitingForUser,
    Completed,
    Failed,
    Archived,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxCliRunDecision {
    #[default]
    Wait,
    RunNow,
    WaitingForUser,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliGoalSnapshot {
    pub goal_id: String,
    pub state: LoopxCliGoalState,
    pub durable_revision: String,
    pub run_decision: LoopxCliRunDecision,
    pub scheduler_hint_ms: Option<u64>,
    pub open_todo_count: u32,
    pub waiting_user_todo_count: u32,
    pub last_turn_id: Option<String>,
    pub settlement_receipt_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliBuildTurnRequest {
    #[serde(flatten)]
    pub context: LoopxCliGoalContext,
    pub goal_id: String,
    pub agent_id: String,
    pub expected_durable_revision: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliBuildTurnResult {
    pub goal_id: String,
    pub turn_id: String,
    pub prompt: String,
    pub settlement_token: String,
    pub durable_revision: String,
    pub scheduler_hint_ms: Option<u64>,
    pub deadline_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxCliGateDecision {
    #[default]
    Approve,
    Reject,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliAnswerGateRequest {
    #[serde(flatten)]
    pub context: LoopxCliGoalContext,
    pub goal_id: String,
    pub agent_id: String,
    pub gate_id: String,
    pub decision: LoopxCliGateDecision,
    pub note: Option<String>,
    pub granted_scope: Option<LoopxPermissionScope>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliAnswerGateResult {
    pub goal_id: String,
    pub gate_id: String,
    pub applied: bool,
    pub durable_revision: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxAgentTurnStatus {
    #[default]
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliSettleTurnRequest {
    #[serde(flatten)]
    pub context: LoopxCliGoalContext,
    pub goal_id: String,
    pub agent_id: String,
    pub turn_id: String,
    pub settlement_token: String,
    pub expected_durable_revision: String,
    pub agent_status: LoopxAgentTurnStatus,
    pub agent_summary: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxCliSettlementStatus {
    #[default]
    Settled,
    AlreadySettled,
    NoDurableProgress,
    RetryRequired,
    GoalCompleted,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliSettleTurnResult {
    pub goal_id: String,
    pub turn_id: String,
    pub receipt_id: String,
    pub status: LoopxCliSettlementStatus,
    pub before_revision: String,
    pub after_revision: String,
    pub validation_succeeded: bool,
    pub scheduler_hint_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliCancelRequest {
    #[serde(flatten)]
    pub call: LoopxCliCallContext,
    pub target_operation_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxCliCancelResult {
    pub operation_id: String,
    pub target_operation_id: String,
    pub cancelled: bool,
}

/// Typed LoopX operations. No caller can pass raw CLI arguments through this port.
pub trait LoopxCliPort: Send + Sync {
    fn handshake<'a>(
        &'a self,
        request: LoopxCliHandshakeRequest,
        progress: &'a dyn LoopxCliProgressSink,
    ) -> LoopxCliFuture<'a, LoopxCliManifest>;

    fn resolve_intake<'a>(
        &'a self,
        request: LoopxCliResolveIntakeRequest,
        progress: &'a dyn LoopxCliProgressSink,
    ) -> LoopxCliFuture<'a, LoopxCliResolveIntakeResult>;

    fn probe_github_auth<'a>(
        &'a self,
        request: LoopxGithubAuthProbeRequest,
    ) -> LoopxCliFuture<'a, LoopxGithubAuthProbe>;

    fn plan_item<'a>(
        &'a self,
        request: LoopxCliPlanItemRequest,
        progress: &'a dyn LoopxCliProgressSink,
    ) -> LoopxCliFuture<'a, LoopxCliIntakePlan>;

    fn create_goal<'a>(
        &'a self,
        request: LoopxCliCreateGoalRequest,
        progress: &'a dyn LoopxCliProgressSink,
    ) -> LoopxCliFuture<'a, LoopxCliCreateGoalResult>;

    fn inspect_goal<'a>(
        &'a self,
        request: LoopxCliInspectGoalRequest,
        progress: &'a dyn LoopxCliProgressSink,
    ) -> LoopxCliFuture<'a, LoopxCliGoalSnapshot>;

    fn build_turn<'a>(
        &'a self,
        request: LoopxCliBuildTurnRequest,
        progress: &'a dyn LoopxCliProgressSink,
    ) -> LoopxCliFuture<'a, LoopxCliBuildTurnResult>;

    fn answer_gate<'a>(
        &'a self,
        request: LoopxCliAnswerGateRequest,
        progress: &'a dyn LoopxCliProgressSink,
    ) -> LoopxCliFuture<'a, LoopxCliAnswerGateResult>;

    fn settle_turn<'a>(
        &'a self,
        request: LoopxCliSettleTurnRequest,
        progress: &'a dyn LoopxCliProgressSink,
    ) -> LoopxCliFuture<'a, LoopxCliSettleTurnResult>;

    fn cancel<'a>(
        &'a self,
        request: LoopxCliCancelRequest,
        progress: &'a dyn LoopxCliProgressSink,
    ) -> LoopxCliFuture<'a, LoopxCliCancelResult>;
}

pub type LoopxHostFuture<'a, T> = Pin<Box<dyn Future<Output = LoopxHostResult<T>> + Send + 'a>>;
pub type LoopxHostResult<T> = Result<T, LoopxHostPortError>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxHostPortErrorKind {
    #[default]
    Backend,
    InvalidInput,
    NotFound,
    Unsupported,
    Conflict,
    Cancelled,
    Timeout,
    Io,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxHostPortError {
    pub kind: LoopxHostPortErrorKind,
    pub message: String,
    pub operation_id: Option<String>,
    pub retryable: bool,
}

impl LoopxHostPortError {
    pub fn new(kind: LoopxHostPortErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            operation_id: None,
            retryable: false,
        }
    }
}

impl std::fmt::Display for LoopxHostPortError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for LoopxHostPortError {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxWorkspacePrepareRequest {
    pub operation_id: String,
    pub task_id: String,
    pub item: LoopxIssueKey,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxWorkspacePrepareResult {
    pub worktree_path: String,
    pub registry_path: String,
    pub reused: bool,
    pub repository_verified: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxWorkspaceVerifyRequest {
    pub operation_id: String,
    pub task_id: String,
    pub item: LoopxIssueKey,
    pub worktree_path: String,
    pub registry_path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxWorkspaceVerifyResult {
    pub valid: bool,
    pub repository: Option<super::types::LoopxRepositoryKey>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxWorkspaceCancelRequest {
    pub operation_id: String,
    pub target_operation_id: String,
    pub task_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxWorkspaceCancelResult {
    pub target_operation_id: String,
    pub cancelled: bool,
}

/// Creates or reuses an isolated worktree for exactly one canonical item.
pub trait LoopxWorkspacePort: Send + Sync {
    fn prepare(
        &self,
        request: LoopxWorkspacePrepareRequest,
    ) -> LoopxHostFuture<'_, LoopxWorkspacePrepareResult>;

    fn verify(
        &self,
        request: LoopxWorkspaceVerifyRequest,
    ) -> LoopxHostFuture<'_, LoopxWorkspaceVerifyResult>;

    fn cancel(
        &self,
        request: LoopxWorkspaceCancelRequest,
    ) -> LoopxHostFuture<'_, LoopxWorkspaceCancelResult>;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxAgentTurnMetadata {
    pub goal_id: String,
    pub loopx_turn_id: String,
    pub item: LoopxIssueKey,
    pub attempt: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxAgentStartRequest {
    pub operation_id: String,
    pub task_id: String,
    pub generation: u64,
    pub worktree_path: String,
    pub prompt: String,
    pub model_id: String,
    pub metadata: LoopxAgentTurnMetadata,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxAgentStartResult {
    pub session_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxAgentCancelRequest {
    pub operation_id: String,
    pub target_operation_id: String,
    pub task_id: String,
    pub generation: u64,
    pub session_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxAgentCancelResult {
    pub target_operation_id: String,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxAgentFinishRequest {
    pub operation_id: String,
    pub task_id: String,
    pub generation: u64,
    pub worktree_path: String,
    pub session_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoopxAgentFinishResult {
    pub session_id: String,
    pub discarded: bool,
}

/// Starts fresh transient Agent sessions bound to the prepared worktree.
pub trait LoopxAgentPort: Send + Sync {
    fn start(&self, request: LoopxAgentStartRequest) -> LoopxHostFuture<'_, LoopxAgentStartResult>;

    fn cancel(
        &self,
        request: LoopxAgentCancelRequest,
    ) -> LoopxHostFuture<'_, LoopxAgentCancelResult>;

    /// Discards the fresh transient session after terminal settlement.
    fn finish(
        &self,
        request: LoopxAgentFinishRequest,
    ) -> LoopxHostFuture<'_, LoopxAgentFinishResult>;
}

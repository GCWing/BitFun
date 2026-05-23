# Local Agent API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a localhost-only, token-authenticated HTTP API that lets Codex submit a task to a BitFun session by `sessionId` or `sessionName`, wait for the turn result, and query timed-out turns by `turnId`.

**Architecture:** Put platform-agnostic request validation, session resolution, result tracking, and task submission in `bitfun-core::service::local_agent_api`. Put the Axum HTTP server, token file management, and desktop startup wiring in `bitfun-desktop`. Reuse `ConversationCoordinator`, `DialogScheduler`, and `TurnOutcome`; do not create a second agent runtime.

**Tech Stack:** Rust, Tokio, Axum 0.7, Serde, BitFun `ConversationCoordinator`, BitFun `DialogScheduler`, existing session persistence.

---

## File Structure

- Create `src/crates/core/src/service/local_agent_api/mod.rs`: public module exports.
- Create `src/crates/core/src/service/local_agent_api/types.rs`: request, response, status, and error DTOs shared by service and HTTP layer.
- Create `src/crates/core/src/service/local_agent_api/tracker.rs`: in-memory `turnId` result registry and waiters.
- Create `src/crates/core/src/service/local_agent_api/service.rs`: validation, session resolution, scheduler submission, timeout wait, and query behavior.
- Modify `src/crates/core/src/service/mod.rs`: export `local_agent_api`.
- Modify `src/crates/core/src/agentic/coordination/scheduler.rs`: allow `TaskResultTracker` to observe `TurnOutcome` without stealing scheduler's existing outcome receiver.
- Create `src/apps/desktop/src/local_agent_api/mod.rs`: desktop module exports.
- Create `src/apps/desktop/src/local_agent_api/auth.rs`: token load/generate/verify logic.
- Create `src/apps/desktop/src/local_agent_api/http.rs`: Axum routes, auth middleware-style checks, and response mapping.
- Create `src/apps/desktop/src/local_agent_api/server.rs`: bind `127.0.0.1:17373` and spawn server task.
- Modify `src/apps/desktop/src/lib.rs`: create tracker, attach it to scheduler, and start local API server after agentic runtime initialization.
- Modify `src/apps/desktop/Cargo.toml`: add `axum = { workspace = true }`.
- Tests live beside the implementation files under `#[cfg(test)]` modules.

## Task 1: Core DTOs And Error Model

**Files:**
- Create: `src/crates/core/src/service/local_agent_api/mod.rs`
- Create: `src/crates/core/src/service/local_agent_api/types.rs`
- Modify: `src/crates/core/src/service/mod.rs`

- [ ] **Step 1: Write DTO tests**

Add this test module to `types.rs` after defining the DTOs in Step 3:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn task_run_request_accepts_session_id_payload() {
        let request: TaskRunRequest = serde_json::from_value(json!({
            "sessionId": "session-1",
            "workspacePath": "D:\\\\BitFun",
            "message": "Run tests",
            "timeoutMs": 1000
        }))
        .expect("request should deserialize");

        assert_eq!(request.session_id.as_deref(), Some("session-1"));
        assert_eq!(request.session_name, None);
        assert_eq!(request.workspace_path, "D:\\BitFun");
        assert_eq!(request.message, "Run tests");
        assert_eq!(request.timeout_ms, Some(1000));
    }

    #[test]
    fn api_error_serializes_stable_code_and_message() {
        let error = LocalAgentApiError::invalid_request("message is required");
        let value = serde_json::to_value(error.to_error_response()).expect("serialize error");

        assert_eq!(value["error"]["code"], "INVALID_REQUEST");
        assert_eq!(value["error"]["message"], "message is required");
    }
}
```

- [ ] **Step 2: Run the new tests and verify they fail**

Run:

```bash
cargo test -p bitfun-core local_agent_api::types -- --nocapture
```

Expected: compile failure because `local_agent_api` and the DTO types do not exist yet.

- [ ] **Step 3: Add DTOs and errors**

Create `src/crates/core/src/service/local_agent_api/mod.rs`:

```rust
pub mod service;
pub mod tracker;
pub mod types;

pub use service::LocalAgentApiService;
pub use tracker::TaskResultTracker;
pub use types::{
    LocalAgentApiError, LocalAgentErrorCode, LocalAgentTaskStatus, SessionCandidate,
    TaskQueryResponse, TaskRunRequest, TaskRunResponse,
};
```

Create `src/crates/core/src/service/local_agent_api/types.rs`:

```rust
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRunRequest {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub session_name: Option<String>,
    pub workspace_path: String,
    pub message: String,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LocalAgentTaskStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    NotFound,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionCandidate {
    pub session_id: String,
    pub session_name: String,
    pub agent_type: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskRunResponse {
    pub status: LocalAgentTaskStatus,
    pub session_id: String,
    pub session_name: String,
    pub turn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_response: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskQueryResponse {
    pub status: LocalAgentTaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,
    pub turn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_response: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalAgentErrorCode {
    Unauthorized,
    InvalidRequest,
    SessionNotFound,
    SessionNameAmbiguous,
    SessionMismatch,
    SubmitFailed,
    TaskNotFound,
    InternalError,
}

impl LocalAgentErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unauthorized => "UNAUTHORIZED",
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::SessionNotFound => "SESSION_NOT_FOUND",
            Self::SessionNameAmbiguous => "SESSION_NAME_AMBIGUOUS",
            Self::SessionMismatch => "SESSION_MISMATCH",
            Self::SubmitFailed => "SUBMIT_FAILED",
            Self::TaskNotFound => "TASK_NOT_FOUND",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAgentApiError {
    pub code: LocalAgentErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub details: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalAgentErrorResponse {
    pub error: LocalAgentApiError,
}

impl LocalAgentApiError {
    pub fn new(code: LocalAgentErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: Map::new(),
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(LocalAgentErrorCode::InvalidRequest, message)
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: Value) -> Self {
        self.details.insert(key.into(), value);
        self
    }

    pub fn to_error_response(&self) -> LocalAgentErrorResponse {
        LocalAgentErrorResponse {
            error: self.clone(),
        }
    }
}
```

Modify `src/crates/core/src/service/mod.rs` near the other service module declarations:

```rust
pub mod local_agent_api; // Localhost-only Agent API for external desktop callers
```

- [ ] **Step 4: Run DTO tests**

Run:

```bash
cargo test -p bitfun-core local_agent_api::types -- --nocapture
```

Expected: tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/crates/core/src/service/mod.rs src/crates/core/src/service/local_agent_api
git commit -m "feat: add local agent api DTOs"
```

## Task 2: Task Result Tracker

**Files:**
- Create: `src/crates/core/src/service/local_agent_api/tracker.rs`
- Modify: `src/crates/core/src/service/local_agent_api/mod.rs`

- [ ] **Step 1: Write tracker tests**

Add these tests to `tracker.rs` after the implementation in Step 3:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::coordination::turn_outcome::TurnOutcome;

    #[tokio::test]
    async fn registered_task_is_running_until_completed() {
        let tracker = TaskResultTracker::default();
        tracker.register(TaskRegistration {
            turn_id: "turn-1".to_string(),
            session_id: "session-1".to_string(),
            session_name: "Worker".to_string(),
        });

        let before = tracker.query("turn-1").expect("task should exist");
        assert_eq!(before.status, LocalAgentTaskStatus::Running);

        tracker.record_outcome(
            "session-1",
            TurnOutcome::Completed {
                turn_id: "turn-1".to_string(),
                final_response: "done".to_string(),
            },
        );

        let after = tracker.query("turn-1").expect("task should exist");
        assert_eq!(after.status, LocalAgentTaskStatus::Completed);
        assert_eq!(after.final_response.as_deref(), Some("done"));
    }

    #[tokio::test]
    async fn wait_returns_none_when_timeout_expires() {
        let tracker = TaskResultTracker::default();
        tracker.register(TaskRegistration {
            turn_id: "turn-2".to_string(),
            session_id: "session-2".to_string(),
            session_name: "Worker".to_string(),
        });

        let result = tracker
            .wait_for("turn-2", std::time::Duration::from_millis(1))
            .await;

        assert!(result.is_none());
    }
}
```

- [ ] **Step 2: Run tracker tests and verify they fail**

Run:

```bash
cargo test -p bitfun-core local_agent_api::tracker -- --nocapture
```

Expected: compile failure because tracker types are missing.

- [ ] **Step 3: Implement tracker**

Create `src/crates/core/src/service/local_agent_api/tracker.rs`:

```rust
use super::types::{LocalAgentTaskStatus, TaskQueryResponse};
use crate::agentic::coordination::turn_outcome::TurnOutcome;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Notify;

#[derive(Debug, Clone)]
pub struct TaskRegistration {
    pub turn_id: String,
    pub session_id: String,
    pub session_name: String,
}

#[derive(Debug, Clone)]
struct TrackedTask {
    response: TaskQueryResponse,
    created_at: SystemTime,
    notify: Arc<Notify>,
}

#[derive(Debug, Default)]
pub struct TaskResultTracker {
    tasks: DashMap<String, TrackedTask>,
}

impl TaskResultTracker {
    pub fn register(&self, registration: TaskRegistration) {
        self.tasks.insert(
            registration.turn_id.clone(),
            TrackedTask {
                response: TaskQueryResponse {
                    status: LocalAgentTaskStatus::Running,
                    session_id: Some(registration.session_id),
                    session_name: Some(registration.session_name),
                    turn_id: registration.turn_id,
                    final_response: None,
                    error: None,
                },
                created_at: SystemTime::now(),
                notify: Arc::new(Notify::new()),
            },
        );
    }

    pub fn query(&self, turn_id: &str) -> Option<TaskQueryResponse> {
        self.tasks.get(turn_id).map(|entry| entry.response.clone())
    }

    pub fn query_or_not_found(&self, turn_id: &str) -> TaskQueryResponse {
        self.query(turn_id).unwrap_or_else(|| TaskQueryResponse {
            status: LocalAgentTaskStatus::NotFound,
            session_id: None,
            session_name: None,
            turn_id: turn_id.to_string(),
            final_response: None,
            error: None,
        })
    }

    pub async fn wait_for(&self, turn_id: &str, timeout: Duration) -> Option<TaskQueryResponse> {
        if let Some(existing) = self.query(turn_id) {
            if existing.status != LocalAgentTaskStatus::Running {
                return Some(existing);
            }
        }

        let notify = self.tasks.get(turn_id).map(|entry| entry.notify.clone())?;
        let notified = notify.notified();
        tokio::select! {
            _ = notified => self.query(turn_id),
            _ = tokio::time::sleep(timeout) => None,
        }
    }

    pub fn record_outcome(&self, session_id: &str, outcome: TurnOutcome) {
        let turn_id = outcome.turn_id().to_string();
        let Some(mut entry) = self.tasks.get_mut(&turn_id) else {
            return;
        };

        if entry.response.session_id.as_deref() != Some(session_id) {
            return;
        }

        match outcome {
            TurnOutcome::Completed { final_response, .. } => {
                entry.response.status = LocalAgentTaskStatus::Completed;
                entry.response.final_response = Some(final_response);
                entry.response.error = None;
            }
            TurnOutcome::Cancelled { .. } => {
                entry.response.status = LocalAgentTaskStatus::Cancelled;
                entry.response.final_response = None;
                entry.response.error = None;
            }
            TurnOutcome::Failed { error, .. } => {
                entry.response.status = LocalAgentTaskStatus::Failed;
                entry.response.final_response = None;
                entry.response.error = Some(error);
            }
        }

        entry.notify.notify_waiters();
    }

    pub fn prune_older_than(&self, max_age: Duration) {
        let now = SystemTime::now();
        let expired: Vec<String> = self
            .tasks
            .iter()
            .filter_map(|entry| {
                now.duration_since(entry.created_at)
                    .ok()
                    .filter(|age| *age > max_age)
                    .map(|_| entry.key().clone())
            })
            .collect();

        for turn_id in expired {
            self.tasks.remove(&turn_id);
        }
    }
}
```

Modify `mod.rs` exports:

```rust
pub use tracker::{TaskRegistration, TaskResultTracker};
```

- [ ] **Step 4: Run tracker tests**

Run:

```bash
cargo test -p bitfun-core local_agent_api::tracker -- --nocapture
```

Expected: tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/crates/core/src/service/local_agent_api
git commit -m "feat: track local agent task results"
```

## Task 3: Scheduler Outcome Observer

**Files:**
- Modify: `src/crates/core/src/agentic/coordination/scheduler.rs`
- Test: existing scheduler test module in the same file.

- [ ] **Step 1: Write scheduler observer test**

In `scheduler.rs` test module, add:

```rust
#[tokio::test]
async fn outcome_observer_receives_completed_turn() {
    let observer = Arc::new(crate::service::local_agent_api::TaskResultTracker::default());
    observer.register(crate::service::local_agent_api::TaskRegistration {
        turn_id: "turn-observed".to_string(),
        session_id: "session-observed".to_string(),
        session_name: "Observed".to_string(),
    });

    observer.record_outcome(
        "session-observed",
        TurnOutcome::Completed {
            turn_id: "turn-observed".to_string(),
            final_response: "ok".to_string(),
        },
    );

    let response = observer
        .query("turn-observed")
        .expect("observer should store outcome");
    assert_eq!(
        response.status,
        crate::service::local_agent_api::LocalAgentTaskStatus::Completed
    );
    assert_eq!(response.final_response.as_deref(), Some("ok"));
}
```

- [ ] **Step 2: Run the narrow scheduler test**

Run:

```bash
cargo test -p bitfun-core outcome_observer_receives_completed_turn -- --nocapture
```

Expected: pass after Task 2. This test protects the tracker/outcome integration type path before adding scheduler wiring.

- [ ] **Step 3: Add scheduler observer storage**

In `DialogScheduler`, add a field:

```rust
task_result_tracker: Arc<DashMap<String, Arc<crate::service::local_agent_api::TaskResultTracker>>>,
```

Initialize it in `DialogScheduler::new`:

```rust
task_result_tracker: Arc::new(DashMap::new()),
```

Add methods in `impl DialogScheduler`:

```rust
pub fn attach_task_result_tracker(
    &self,
    name: impl Into<String>,
    tracker: Arc<crate::service::local_agent_api::TaskResultTracker>,
) {
    self.task_result_tracker.insert(name.into(), tracker);
}

fn notify_task_result_trackers(&self, session_id: &str, outcome: &TurnOutcome) {
    for tracker in self.task_result_tracker.iter() {
        tracker.value().record_outcome(session_id, outcome.clone());
    }
}
```

In `run_outcome_handler`, immediately after `while let Some((session_id, outcome)) = outcome_rx.recv().await {`, add:

```rust
self.notify_task_result_trackers(&session_id, &outcome);
```

- [ ] **Step 4: Run scheduler tests**

Run:

```bash
cargo test -p bitfun-core outcome_observer_receives_completed_turn -- --nocapture
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src/crates/core/src/agentic/coordination/scheduler.rs
git commit -m "feat: expose scheduler turn outcomes to local task tracker"
```

## Task 4: LocalAgentApiService Validation And Session Resolution

**Files:**
- Create: `src/crates/core/src/service/local_agent_api/service.rs`
- Modify: `src/crates/core/src/service/local_agent_api/mod.rs`

- [ ] **Step 1: Write pure validation tests**

Add these tests to `service.rs` after implementation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn request(session_id: Option<&str>, session_name: Option<&str>) -> TaskRunRequest {
        TaskRunRequest {
            session_id: session_id.map(str::to_string),
            session_name: session_name.map(str::to_string),
            workspace_path: "D:\\BitFun".to_string(),
            message: "Do work".to_string(),
            agent_type: None,
            timeout_ms: Some(1000),
        }
    }

    #[test]
    fn validate_rejects_missing_session_identifier() {
        let error = validate_task_request(&request(None, None)).expect_err("must fail");
        assert_eq!(error.code, LocalAgentErrorCode::InvalidRequest);
        assert_eq!(
            error.message,
            "sessionId or sessionName is required"
        );
    }

    #[test]
    fn validate_rejects_empty_message() {
        let mut req = request(Some("session-1"), None);
        req.message = "   ".to_string();
        let error = validate_task_request(&req).expect_err("must fail");
        assert_eq!(error.message, "message is required");
    }

    #[test]
    fn validate_accepts_session_name_request() {
        validate_task_request(&request(None, Some("Worker"))).expect("valid request");
    }
}
```

- [ ] **Step 2: Run validation tests and verify they fail**

Run:

```bash
cargo test -p bitfun-core local_agent_api::service::tests::validate -- --nocapture
```

Expected: compile failure because `service.rs` is not implemented.

- [ ] **Step 3: Implement validation and service shell**

Create `src/crates/core/src/service/local_agent_api/service.rs`:

```rust
use super::tracker::{TaskRegistration, TaskResultTracker};
use super::types::{
    LocalAgentApiError, LocalAgentErrorCode, LocalAgentTaskStatus, SessionCandidate,
    TaskQueryResponse, TaskRunRequest, TaskRunResponse,
};
use crate::agentic::coordination::{ConversationCoordinator, DialogScheduler, DialogSubmissionPolicy, DialogTriggerSource};
use crate::agentic::core::SessionSummary;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const DEFAULT_TIMEOUT_MS: u64 = 600_000;
const MAX_TIMEOUT_MS: u64 = 3_600_000;

#[derive(Clone)]
pub struct LocalAgentApiService {
    coordinator: Arc<ConversationCoordinator>,
    scheduler: Arc<DialogScheduler>,
    tracker: Arc<TaskResultTracker>,
}

#[derive(Debug, Clone)]
struct ResolvedSession {
    session_id: String,
    session_name: String,
    agent_type: String,
}

impl LocalAgentApiService {
    pub fn new(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
        tracker: Arc<TaskResultTracker>,
    ) -> Self {
        Self {
            coordinator,
            scheduler,
            tracker,
        }
    }

    pub async fn run_task(
        &self,
        request: TaskRunRequest,
    ) -> Result<TaskRunResponse, LocalAgentApiError> {
        validate_task_request(&request)?;
        let workspace_path = PathBuf::from(request.workspace_path.trim());
        let session = self.resolve_session(&workspace_path, &request).await?;
        let turn_id = format!("local-agent-{}", Uuid::new_v4());
        let agent_type = request
            .agent_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&session.agent_type)
            .to_string();

        self.tracker.register(TaskRegistration {
            turn_id: turn_id.clone(),
            session_id: session.session_id.clone(),
            session_name: session.session_name.clone(),
        });

        self.scheduler
            .submit(
                session.session_id.clone(),
                request.message.clone(),
                Some(request.message.clone()),
                Some(turn_id.clone()),
                agent_type,
                Some(request.workspace_path.clone()),
                DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopApi),
                None,
                None,
                None,
            )
            .await
            .map_err(|error| {
                LocalAgentApiError::new(LocalAgentErrorCode::SubmitFailed, error)
            })?;

        let timeout = Duration::from_millis(resolve_timeout_ms(request.timeout_ms));
        let waited = self.tracker.wait_for(&turn_id, timeout).await;
        let query = waited.unwrap_or_else(|| self.tracker.query_or_not_found(&turn_id));
        Ok(task_run_response_from_query(query, true))
    }

    pub fn query_task(&self, turn_id: &str) -> TaskQueryResponse {
        self.tracker.query_or_not_found(turn_id)
    }

    async fn resolve_session(
        &self,
        workspace_path: &PathBuf,
        request: &TaskRunRequest,
    ) -> Result<ResolvedSession, LocalAgentApiError> {
        let sessions = self.coordinator.list_sessions(workspace_path).await.map_err(|error| {
            LocalAgentApiError::new(LocalAgentErrorCode::InternalError, error.to_string())
        })?;

        resolve_session_from_summaries(&sessions, request)
    }
}

fn resolve_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    timeout_ms
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(1, MAX_TIMEOUT_MS)
}

fn system_time_to_unix_secs(value: SystemTime) -> u64 {
    value
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn candidate_from_summary(summary: &SessionSummary) -> SessionCandidate {
    SessionCandidate {
        session_id: summary.session_id.clone(),
        session_name: summary.session_name.clone(),
        agent_type: summary.agent_type.clone(),
        created_at: system_time_to_unix_secs(summary.created_at),
    }
}

pub(crate) fn validate_task_request(request: &TaskRunRequest) -> Result<(), LocalAgentApiError> {
    let has_session_id = request
        .session_id
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let has_session_name = request
        .session_name
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());

    if !has_session_id && !has_session_name {
        return Err(LocalAgentApiError::invalid_request(
            "sessionId or sessionName is required",
        ));
    }
    if request.workspace_path.trim().is_empty() {
        return Err(LocalAgentApiError::invalid_request("workspacePath is required"));
    }
    if request.message.trim().is_empty() {
        return Err(LocalAgentApiError::invalid_request("message is required"));
    }

    Ok(())
}

fn resolve_session_from_summaries(
    sessions: &[SessionSummary],
    request: &TaskRunRequest,
) -> Result<ResolvedSession, LocalAgentApiError> {
    let by_id = request.session_id.as_deref().map(str::trim).filter(|value| !value.is_empty());
    let by_name = request.session_name.as_deref().map(str::trim).filter(|value| !value.is_empty());

    if let Some(session_id) = by_id {
        let session = sessions
            .iter()
            .find(|summary| summary.session_id == session_id)
            .ok_or_else(|| {
                LocalAgentApiError::new(
                    LocalAgentErrorCode::SessionNotFound,
                    format!("sessionId '{}' was not found", session_id),
                )
            })?;

        if let Some(session_name) = by_name {
            if session.session_name != session_name {
                return Err(LocalAgentApiError::new(
                    LocalAgentErrorCode::SessionMismatch,
                    "sessionId and sessionName do not refer to the same session",
                ));
            }
        }

        return Ok(ResolvedSession {
            session_id: session.session_id.clone(),
            session_name: session.session_name.clone(),
            agent_type: session.agent_type.clone(),
        });
    }

    let session_name = by_name.expect("validated sessionName exists");
    let matches: Vec<&SessionSummary> = sessions
        .iter()
        .filter(|summary| summary.session_name == session_name)
        .collect();

    match matches.as_slice() {
        [] => Err(LocalAgentApiError::new(
            LocalAgentErrorCode::SessionNotFound,
            format!("sessionName '{}' was not found", session_name),
        )),
        [session] => Ok(ResolvedSession {
            session_id: session.session_id.clone(),
            session_name: session.session_name.clone(),
            agent_type: session.agent_type.clone(),
        }),
        _ => {
            let candidates: Vec<SessionCandidate> =
                matches.into_iter().map(candidate_from_summary).collect();
            Err(LocalAgentApiError::new(
                LocalAgentErrorCode::SessionNameAmbiguous,
                "multiple sessions match sessionName in this workspace",
            )
            .with_detail("candidates", json!(candidates)))
        }
    }
}

fn task_run_response_from_query(query: TaskQueryResponse, include_timeout: bool) -> TaskRunResponse {
    let timed_out = include_timeout && query.status == LocalAgentTaskStatus::Running;
    TaskRunResponse {
        status: query.status,
        session_id: query.session_id.unwrap_or_default(),
        session_name: query.session_name.unwrap_or_default(),
        turn_id: query.turn_id,
        final_response: query.final_response,
        error: query.error,
        timed_out,
    }
}
```

Update `mod.rs` export:

```rust
pub use service::LocalAgentApiService;
```

- [ ] **Step 4: Run validation tests**

Run:

```bash
cargo test -p bitfun-core local_agent_api::service::tests::validate -- --nocapture
```

Expected: validation tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/crates/core/src/service/local_agent_api
git commit -m "feat: resolve local agent api task requests"
```

## Task 5: Desktop Token Management

**Files:**
- Create: `src/apps/desktop/src/local_agent_api/mod.rs`
- Create: `src/apps/desktop/src/local_agent_api/auth.rs`
- Modify: `src/apps/desktop/src/lib.rs` later in Task 7 only.

- [ ] **Step 1: Write token tests**

Add to `auth.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_bearer_token_accepts_exact_value() {
        assert!(verify_authorization_header(
            Some("Bearer abc123"),
            "abc123"
        ));
    }

    #[test]
    fn verify_bearer_token_rejects_missing_or_wrong_value() {
        assert!(!verify_authorization_header(None, "abc123"));
        assert!(!verify_authorization_header(Some("Bearer wrong"), "abc123"));
        assert!(!verify_authorization_header(Some("Basic abc123"), "abc123"));
    }
}
```

- [ ] **Step 2: Run token tests and verify they fail**

Run:

```bash
cargo test -p bitfun-desktop local_agent_api::auth -- --nocapture
```

Expected: compile failure because the desktop module does not exist.

- [ ] **Step 3: Add desktop module and token helpers**

Create `src/apps/desktop/src/local_agent_api/mod.rs`:

```rust
pub mod auth;
pub mod http;
pub mod server;
```

Create `src/apps/desktop/src/local_agent_api/auth.rs`:

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAgentApiAuthConfig {
    pub token: String,
}

pub fn verify_authorization_header(header: Option<&str>, expected_token: &str) -> bool {
    let Some(header) = header else {
        return false;
    };
    let Some(token) = header.strip_prefix("Bearer ") else {
        return false;
    };
    token == expected_token
}

pub async fn load_or_create_token(config_path: PathBuf) -> Result<String> {
    if let Ok(content) = tokio::fs::read_to_string(&config_path).await {
        let config: LocalAgentApiAuthConfig =
            serde_json::from_str(&content).context("Failed to parse Local Agent API config")?;
        if !config.token.trim().is_empty() {
            return Ok(config.token);
        }
    }

    if let Some(parent) = config_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create Local Agent API config directory")?;
    }

    let token = uuid::Uuid::new_v4().to_string().replace('-', "");
    let config = LocalAgentApiAuthConfig {
        token: token.clone(),
    };
    let content = serde_json::to_string_pretty(&config)
        .context("Failed to serialize Local Agent API config")?;
    tokio::fs::write(&config_path, content)
        .await
        .context("Failed to write Local Agent API config")?;
    Ok(token)
}
```

Modify `src/apps/desktop/src/lib.rs` near other top-level modules:

```rust
mod local_agent_api;
```

- [ ] **Step 4: Run token tests**

Run:

```bash
cargo test -p bitfun-desktop local_agent_api::auth -- --nocapture
```

Expected: tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/apps/desktop/src/lib.rs src/apps/desktop/src/local_agent_api
git commit -m "feat: add local agent api token config"
```

## Task 6: Desktop HTTP Handlers

**Files:**
- Create: `src/apps/desktop/src/local_agent_api/http.rs`
- Modify: `src/apps/desktop/Cargo.toml`

- [ ] **Step 1: Add Axum dependency**

Modify `src/apps/desktop/Cargo.toml` under inherited dependencies:

```toml
axum = { workspace = true }
```

- [ ] **Step 2: Write handler mapping tests**

Add this test module to `http.rs` after implementation:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_core::service::local_agent_api::{
        LocalAgentApiError, LocalAgentErrorCode,
    };

    #[test]
    fn status_for_session_name_ambiguous_is_conflict() {
        let error = LocalAgentApiError::new(
            LocalAgentErrorCode::SessionNameAmbiguous,
            "multiple sessions match sessionName in this workspace",
        );
        assert_eq!(status_for_error(&error), axum::http::StatusCode::CONFLICT);
    }

    #[test]
    fn status_for_unauthorized_is_unauthorized() {
        let error = LocalAgentApiError::new(
            LocalAgentErrorCode::Unauthorized,
            "missing bearer token",
        );
        assert_eq!(status_for_error(&error), axum::http::StatusCode::UNAUTHORIZED);
    }
}
```

- [ ] **Step 3: Run handler tests and verify they fail**

Run:

```bash
cargo test -p bitfun-desktop local_agent_api::http -- --nocapture
```

Expected: compile failure because `http.rs` has not been implemented.

- [ ] **Step 4: Implement HTTP handlers**

Create `src/apps/desktop/src/local_agent_api/http.rs`:

```rust
use crate::local_agent_api::auth::verify_authorization_header;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bitfun_core::service::local_agent_api::{
    LocalAgentApiError, LocalAgentApiService, LocalAgentErrorCode, TaskRunRequest,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct LocalAgentHttpState {
    pub service: Arc<LocalAgentApiService>,
    pub token: Arc<String>,
}

pub fn router(state: LocalAgentHttpState) -> Router {
    Router::new()
        .route("/api/local-agent/tasks:run", post(run_task))
        .route("/api/local-agent/tasks/:turn_id", get(query_task))
        .with_state(state)
}

async fn run_task(
    State(state): State<LocalAgentHttpState>,
    headers: HeaderMap,
    Json(request): Json<TaskRunRequest>,
) -> Response {
    if let Err(error) = authorize(&headers, state.token.as_str()) {
        return error_response(error);
    }

    match state.service.run_task(request).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => error_response(error),
    }
}

async fn query_task(
    State(state): State<LocalAgentHttpState>,
    headers: HeaderMap,
    Path(turn_id): Path<String>,
) -> Response {
    if let Err(error) = authorize(&headers, state.token.as_str()) {
        return error_response(error);
    }

    (StatusCode::OK, Json(state.service.query_task(&turn_id))).into_response()
}

fn authorize(headers: &HeaderMap, expected_token: &str) -> Result<(), LocalAgentApiError> {
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    if verify_authorization_header(header, expected_token) {
        Ok(())
    } else {
        Err(LocalAgentApiError::new(
            LocalAgentErrorCode::Unauthorized,
            "missing or invalid bearer token",
        ))
    }
}

pub(crate) fn status_for_error(error: &LocalAgentApiError) -> StatusCode {
    match error.code {
        LocalAgentErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
        LocalAgentErrorCode::InvalidRequest | LocalAgentErrorCode::SessionMismatch => {
            StatusCode::BAD_REQUEST
        }
        LocalAgentErrorCode::SessionNotFound | LocalAgentErrorCode::TaskNotFound => {
            StatusCode::NOT_FOUND
        }
        LocalAgentErrorCode::SessionNameAmbiguous => StatusCode::CONFLICT,
        LocalAgentErrorCode::SubmitFailed | LocalAgentErrorCode::InternalError => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

fn error_response(error: LocalAgentApiError) -> Response {
    let status = status_for_error(&error);
    (status, Json(error.to_error_response())).into_response()
}
```

- [ ] **Step 5: Run handler tests**

Run:

```bash
cargo test -p bitfun-desktop local_agent_api::http -- --nocapture
```

Expected: tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/apps/desktop/Cargo.toml src/apps/desktop/src/local_agent_api/http.rs
git commit -m "feat: add local agent api HTTP routes"
```

## Task 7: Desktop Server Startup Wiring

**Files:**
- Create: `src/apps/desktop/src/local_agent_api/server.rs`
- Modify: `src/apps/desktop/src/lib.rs`

- [ ] **Step 1: Implement server start function**

Create `src/apps/desktop/src/local_agent_api/server.rs`:

```rust
use super::auth::load_or_create_token;
use super::http::{router, LocalAgentHttpState};
use anyhow::{Context, Result};
use bitfun_core::agentic::coordination::{ConversationCoordinator, DialogScheduler};
use bitfun_core::service::local_agent_api::{LocalAgentApiService, TaskResultTracker};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

pub const DEFAULT_LOCAL_AGENT_API_PORT: u16 = 17_373;

pub async fn start_local_agent_api_server(
    config_path: PathBuf,
    coordinator: Arc<ConversationCoordinator>,
    scheduler: Arc<DialogScheduler>,
    tracker: Arc<TaskResultTracker>,
) -> Result<()> {
    let token = load_or_create_token(config_path).await?;
    let service = Arc::new(LocalAgentApiService::new(coordinator, scheduler, tracker));
    let state = LocalAgentHttpState {
        service,
        token: Arc::new(token),
    };
    let app = router(state);
    let addr = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        DEFAULT_LOCAL_AGENT_API_PORT,
    );
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind Local Agent API server at {}", addr))?;

    log::info!("Local Agent API server started at http://{}", addr);
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            log::error!("Local Agent API server stopped with error: {}", error);
        }
    });

    Ok(())
}
```

- [ ] **Step 2: Wire startup after scheduler initialization**

In `src/apps/desktop/src/lib.rs`, after `coordination::set_global_scheduler(scheduler.clone());`, add:

```rust
    let local_agent_task_tracker = Arc::new(bitfun_core::service::local_agent_api::TaskResultTracker::default());
    scheduler.attach_task_result_tracker(
        "local_agent_api",
        local_agent_task_tracker.clone(),
    );
    let local_agent_config_path = path_manager
        .get_app_data_dir()
        .join("local-agent-api.json");
    if let Err(error) = crate::local_agent_api::server::start_local_agent_api_server(
        local_agent_config_path,
        coordinator.clone(),
        scheduler.clone(),
        local_agent_task_tracker,
    )
    .await
    {
        log::error!("Failed to start Local Agent API server: {}", error);
    }
```

If `PathManager` does not expose `get_app_data_dir()`, use the existing method in that type that returns BitFun app data/config root. Search with:

```bash
rg -n "struct PathManager|fn .*app.*dir|config.*dir|data.*dir" src/crates/core/src src/apps/desktop/src -g "*.rs"
```

Use the existing method name exactly; do not invent a new path manager API unless no suitable method exists.

- [ ] **Step 3: Compile desktop**

Run:

```bash
cargo check -p bitfun-desktop
```

Expected: compile succeeds. If it fails because the path manager method name differs, adjust only the method call from Step 2.

- [ ] **Step 4: Commit**

```bash
git add src/apps/desktop/src/lib.rs src/apps/desktop/src/local_agent_api/server.rs
git commit -m "feat: start localhost local agent api"
```

## Task 8: Service Session Resolution Tests

**Files:**
- Modify: `src/crates/core/src/service/local_agent_api/service.rs`

- [ ] **Step 1: Add pure session resolution tests**

Add to the `service.rs` test module:

```rust
use crate::agentic::core::{SessionKind, SessionState, SessionSummary};
use std::time::SystemTime;

fn summary(id: &str, name: &str, agent_type: &str) -> SessionSummary {
    SessionSummary {
        session_id: id.to_string(),
        session_name: name.to_string(),
        agent_type: agent_type.to_string(),
        created_by: None,
        kind: SessionKind::Standard,
        turn_count: 0,
        created_at: SystemTime::UNIX_EPOCH,
        last_activity_at: SystemTime::UNIX_EPOCH,
        state: SessionState::Idle,
    }
}

#[test]
fn resolve_by_session_name_rejects_ambiguous_names() {
    let sessions = vec![
        summary("s1", "Worker", "agentic"),
        summary("s2", "Worker", "Plan"),
    ];
    let req = request(None, Some("Worker"));

    let error = resolve_session_from_summaries(&sessions, &req).expect_err("must fail");
    assert_eq!(error.code, LocalAgentErrorCode::SessionNameAmbiguous);
    assert!(error.details.contains_key("candidates"));
}

#[test]
fn resolve_by_session_id_and_name_requires_same_session() {
    let sessions = vec![summary("s1", "Worker", "agentic")];
    let req = request(Some("s1"), Some("Other"));

    let error = resolve_session_from_summaries(&sessions, &req).expect_err("must fail");
    assert_eq!(error.code, LocalAgentErrorCode::SessionMismatch);
}

#[test]
fn resolve_by_unique_session_name_returns_session() {
    let sessions = vec![summary("s1", "Worker", "agentic")];
    let req = request(None, Some("Worker"));

    let resolved = resolve_session_from_summaries(&sessions, &req).expect("must resolve");
    assert_eq!(resolved.session_id, "s1");
    assert_eq!(resolved.session_name, "Worker");
    assert_eq!(resolved.agent_type, "agentic");
}
```

- [ ] **Step 2: Run resolution tests**

Run:

```bash
cargo test -p bitfun-core local_agent_api::service::tests::resolve -- --nocapture
```

Expected: tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/crates/core/src/service/local_agent_api/service.rs
git commit -m "test: cover local agent api session resolution"
```

## Task 9: End-To-End Compile And Focused Verification

**Files:**
- No new files.

- [ ] **Step 1: Run focused core tests**

Run:

```bash
cargo test -p bitfun-core local_agent_api -- --nocapture
```

Expected: all Local Agent API core tests pass.

- [ ] **Step 2: Run focused desktop tests**

Run:

```bash
cargo test -p bitfun-desktop local_agent_api -- --nocapture
```

Expected: all Local Agent API desktop tests pass.

- [ ] **Step 3: Run desktop compile check**

Run:

```bash
cargo check -p bitfun-desktop
```

Expected: compile succeeds.

- [ ] **Step 4: Run broader Rust check if shared core was touched**

Run:

```bash
cargo check --workspace
```

Expected: workspace compile succeeds.

- [ ] **Step 5: Commit verification-only fixes**

Only if Step 1-4 require compile fixes, commit them:

```bash
git add src/crates/core/src/service/local_agent_api src/crates/core/src/agentic/coordination/scheduler.rs src/apps/desktop/src/local_agent_api src/apps/desktop/src/lib.rs src/apps/desktop/Cargo.toml
git commit -m "fix: stabilize local agent api integration"
```

If no fixes are required, do not create an empty commit.

## Task 10: Manual Smoke Test Notes

**Files:**
- Modify: `docs/superpowers/specs/2026-05-23-local-agent-api-design.md` only if behavior changed during implementation.

- [ ] **Step 1: Start desktop dev runtime**

Run:

```bash
pnpm run desktop:dev
```

Expected: logs include `Local Agent API server started at http://127.0.0.1:17373`.

- [ ] **Step 2: Read token from local config**

Open the generated `local-agent-api.json` under the app data directory used in Task 7. Confirm it has this shape:

```json
{
  "token": "hex-or-uuid-token"
}
```

- [ ] **Step 3: Submit a task with curl**

Replace `<token>`, `<workspace>`, and `<session_id>` with real values:

```bash
curl -s -X POST http://127.0.0.1:17373/api/local-agent/tasks:run ^
  -H "Authorization: Bearer <token>" ^
  -H "Content-Type: application/json" ^
  -d "{\"sessionId\":\"<session_id>\",\"workspacePath\":\"<workspace>\",\"message\":\"Reply with the word pong only.\",\"timeoutMs\":30000}"
```

Expected: JSON response contains `status` as `completed` or `running`, plus `sessionId` and `turnId`.

- [ ] **Step 4: Query by turn id after a running response**

Replace `<token>` and `<turn_id>`:

```bash
curl -s http://127.0.0.1:17373/api/local-agent/tasks/<turn_id> ^
  -H "Authorization: Bearer <token>"
```

Expected: JSON response contains the same `turnId` and a stable status.

- [ ] **Step 5: Commit documentation adjustment if needed**

If implementation changed the design document:

```bash
git add docs/superpowers/specs/2026-05-23-local-agent-api-design.md
git commit -m "docs: align local agent api design with implementation"
```

If the design document still matches implementation, do not create a docs commit.

## Self-Review

Spec coverage:

- Localhost-only HTTP API: covered by Tasks 6 and 7.
- Bearer token auth: covered by Tasks 5 and 6.
- `sessionId` / `sessionName` targeting: covered by Tasks 4 and 8.
- 409 for ambiguous `sessionName`: covered by Tasks 4, 6, and 8.
- Synchronous wait with timeout: covered by Tasks 2 and 4.
- Query by `turnId`: covered by Tasks 2, 4, and 6.
- Reuse existing runtime: covered by Tasks 3, 4, and 7.
- No streaming API: no task adds streaming routes.

Placeholder scan:

- The plan contains concrete files, functions, request shapes, code snippets, commands, and expected results.
- The only conditional branch is the path manager method lookup in Task 7, because the exact method name must be verified against the implementation at execution time before editing that line.

Type consistency:

- `TaskRunRequest`, `TaskRunResponse`, `TaskQueryResponse`, `LocalAgentApiService`, and `TaskResultTracker` are introduced before use.
- HTTP handlers use the same camelCase JSON DTOs defined in core.
- Scheduler observer uses `TaskResultTracker::record_outcome(session_id, outcome.clone())`, matching Task 2.

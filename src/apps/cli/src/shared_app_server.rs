//! Multi-client Shared App Server host for `bitfun --shared`.

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use agent_client_protocol::Lines;
use anyhow::{anyhow, Context, Result};
use bitfun_app_server::{
    AppManagementService, AppServerHostPolicy, AppServerHostPolicyError, AppServerOperationKind,
    AppServerOperationObserver, BitfunAppRuntime, BitfunAppServer,
};
use bitfun_app_server_client::AppServerClient;
use bitfun_app_server_protocol::app::TransportLimits;
use bitfun_app_server_protocol::PROTOCOL_VERSION;
use bitfun_core::runtime_ownership::CoreRuntimeOwnership;
use bitfun_services_core::runtime_ownership::{RuntimeDeployment, RuntimeOwnershipKey};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(45);
const AUTH_TIMEOUT: Duration = Duration::from_secs(2);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CONNECTIONS: usize = 64;
const MAX_REQUEST_BYTES: usize = 128 * 1024;
const MAX_RESPONSE_EVENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_DISCOVERY_BYTES: u64 = 16 * 1024;

struct SharedHostPolicy {
    canonical_workspace: PathBuf,
    execution_roots: std::sync::RwLock<HashSet<PathBuf>>,
    session_bindings: std::sync::RwLock<HashMap<String, SharedSessionBinding>>,
}

#[derive(Debug, Clone)]
struct SharedSessionBinding {
    project_root: PathBuf,
    execution_root: PathBuf,
}

impl SharedHostPolicy {
    fn new(workspace: &Path) -> Result<Self> {
        let canonical_workspace = dunce::canonicalize(workspace).with_context(|| {
            format!("canonicalize Shared Host workspace {}", workspace.display())
        })?;
        Ok(Self {
            execution_roots: std::sync::RwLock::new(HashSet::from([canonical_workspace.clone()])),
            canonical_workspace,
            session_bindings: std::sync::RwLock::new(HashMap::new()),
        })
    }

    fn authorize_value(
        &self,
        method: &str,
        value: &serde_json::Value,
        require_session_binding: bool,
    ) -> Result<(), AppServerHostPolicyError> {
        let mut workspace_paths = Vec::new();
        let mut session_ids = Vec::new();
        self.inspect_request_value(
            value,
            &mut workspace_paths,
            &mut session_ids,
            require_session_binding || !method_requires_bound_session(method),
        )?;

        if require_session_binding
            && method_requires_bound_session(method)
            && !session_ids.is_empty()
        {
            let bindings = self
                .session_bindings
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if session_ids
                .iter()
                .any(|session_id| !bindings.contains_key(session_id))
            {
                return Err(AppServerHostPolicyError::invalid_request(
                    "The Session is outside this Shared Host workspace scope",
                ));
            }
            for session_id in &session_ids {
                let binding = bindings.get(session_id).expect("checked above");
                if workspace_paths
                    .iter()
                    .any(|path| path != &binding.project_root && path != &binding.execution_root)
                {
                    return Err(AppServerHostPolicyError::invalid_request(
                        "The requested workspace does not match the Session execution scope",
                    ));
                }
            }
        }

        if method == "agent/submitDialogTurn"
            && value
                .get("turnId")
                .or_else(|| value.get("turn_id"))
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Err(AppServerHostPolicyError::invalid_request(
                "Shared turn submission requires an exact turnId",
            ));
        }
        if method.starts_with("agent/") && method.contains("ProjectPermission") {
            let project_id = value
                .get("projectId")
                .or_else(|| value.get("project_id"))
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    AppServerHostPolicyError::invalid_request(
                        "Permission project scope requires projectId",
                    )
                })?;
            let canonical_root = self
                .canonical_workspace
                .to_string_lossy()
                .replace('\\', "/");
            let expected = bitfun_core::service::remote_ssh::workspace_state::
                local_workspace_stable_storage_id(&canonical_root);
            if project_id != expected && project_id != "__bitfun_account_actions__" {
                return Err(AppServerHostPolicyError::invalid_request(
                    "Permission project scope is outside this Shared Host workspace",
                ));
            }
        }
        Ok(())
    }

    fn inspect_request_value(
        &self,
        value: &serde_json::Value,
        workspace_paths: &mut Vec<PathBuf>,
        session_ids: &mut Vec<String>,
        validate_execution_paths: bool,
    ) -> Result<(), AppServerHostPolicyError> {
        match value {
            serde_json::Value::Array(values) => {
                for value in values {
                    self.inspect_request_value(
                        value,
                        workspace_paths,
                        session_ids,
                        validate_execution_paths,
                    )?;
                }
            }
            serde_json::Value::Object(object) => {
                for (key, value) in object {
                    if is_opaque_request_payload(key) {
                        continue;
                    }
                    match key.as_str() {
                        "remoteConnectionId" | "remoteSshHost"
                            if !value.is_null()
                                && value.as_str().is_none_or(|value| !value.is_empty()) =>
                        {
                            return Err(AppServerHostPolicyError::unsupported(
                                "shared.localWorkspace",
                                "Remote execution is unavailable in the local Shared Host",
                            ));
                        }
                        "workspacePath" | "workspace_path" => {
                            if !value.is_null() && validate_execution_paths {
                                let path = canonical_request_path_value(value)?;
                                self.require_execution_root_path(&path)?;
                                workspace_paths.push(path);
                            }
                        }
                        "projectWorkspacePath" | "project_workspace_path" => {
                            if !value.is_null() {
                                let path = canonical_request_path_value(value)?;
                                self.require_project_root_path(&path)?;
                                workspace_paths.push(path);
                            }
                        }
                        "rootPath" if object.contains_key("kind") => {
                            if validate_execution_paths {
                                let path = canonical_request_path_value(value)?;
                                self.require_execution_root_path(&path)?;
                                workspace_paths.push(path);
                            }
                        }
                        key if key == "sessionId"
                            || key == "sourceSessionId"
                            || key == "rootSessionId"
                            || key == "anchorSessionId" =>
                        {
                            if let Some(session_id) = value.as_str() {
                                session_ids.push(session_id.to_string());
                            }
                        }
                        _ => self.inspect_request_value(
                            value,
                            workspace_paths,
                            session_ids,
                            validate_execution_paths,
                        )?,
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn require_project_root(
        &self,
        value: &serde_json::Value,
    ) -> Result<(), AppServerHostPolicyError> {
        if value.is_null() {
            return Ok(());
        }
        let path = value.as_str().ok_or_else(|| {
            AppServerHostPolicyError::invalid_request("Workspace path must be a string")
        })?;
        let canonical = canonical_request_path(path)?;
        self.require_project_root_path(&canonical)
    }

    fn require_project_root_path(&self, canonical: &Path) -> Result<(), AppServerHostPolicyError> {
        if canonical == self.canonical_workspace {
            Ok(())
        } else {
            Err(AppServerHostPolicyError::invalid_request(
                "The requested project workspace is outside this Shared Host scope",
            ))
        }
    }

    fn require_execution_root_path(
        &self,
        canonical: &Path,
    ) -> Result<(), AppServerHostPolicyError> {
        let roots = self
            .execution_roots
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if roots.contains(canonical) {
            Ok(())
        } else {
            Err(AppServerHostPolicyError::invalid_request(
                "The requested execution workspace is outside this Shared Host scope",
            ))
        }
    }
}

fn is_opaque_request_payload(key: &str) -> bool {
    matches!(
        key,
        "answers"
            | "attachments"
            | "config"
            | "displayMetadata"
            | "display_metadata"
            | "metadata"
            | "value"
    )
}

impl AppServerHostPolicy for SharedHostPolicy {
    fn allows_method(&self, method: &str) -> bool {
        is_shared_method(method)
    }

    fn authorize_preflight(
        &self,
        method: &str,
        request: &serde_json::Value,
    ) -> Result<(), AppServerHostPolicyError> {
        if !self.allows_method(method) {
            return Err(AppServerHostPolicyError::unsupported(
                "shared.method",
                format!("The Shared Host does not expose {method}"),
            ));
        }
        self.authorize_value(method, request, false)
    }

    fn authorize_request(
        &self,
        method: &str,
        request: &serde_json::Value,
    ) -> Result<(), AppServerHostPolicyError> {
        if !self.allows_method(method) {
            return Err(AppServerHostPolicyError::unsupported(
                "shared.method",
                format!("The Shared Host does not expose {method}"),
            ));
        }
        self.authorize_value(method, request, true)
    }

    fn allows_capability(&self, capability: &str) -> bool {
        !matches!(capability, "git")
    }

    fn allows_external_source_workspace(&self, workspace_path: &str) -> bool {
        canonical_request_path(workspace_path).is_ok_and(|path| path == self.canonical_workspace)
    }

    fn register_session_binding(
        &self,
        session_id: &str,
        binding: &bitfun_runtime_ports::AgentSessionWorkspaceBinding,
    ) -> Result<(), AppServerHostPolicyError> {
        if binding.remote_connection_id.is_some() || binding.remote_ssh_host.is_some() {
            return Err(AppServerHostPolicyError::unsupported(
                "shared.localWorkspace",
                "Remote Sessions are unavailable in the local Shared Host",
            ));
        }
        if let Some(project_workspace_path) = binding.project_workspace_path.as_deref() {
            self.require_project_root(&serde_json::Value::String(
                project_workspace_path.to_string(),
            ))?;
        }
        let execution_root = canonical_request_path(&binding.workspace_path)?;
        if binding.project_workspace_path.is_none() && execution_root != self.canonical_workspace {
            return Err(AppServerHostPolicyError::invalid_request(
                "A Session outside the canonical Shared Host workspace requires a project binding",
            ));
        }
        if let Some(target) = &binding.execution_target {
            let target_root = canonical_request_path(&target.root_path)?;
            if target_root != execution_root {
                return Err(AppServerHostPolicyError::invalid_request(
                    "The Session execution target does not match its workspace binding",
                ));
            }
        }
        self.execution_roots
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(execution_root.clone());
        let project_root = binding
            .project_workspace_path
            .as_deref()
            .map(canonical_request_path)
            .transpose()?
            .unwrap_or_else(|| self.canonical_workspace.clone());
        self.session_bindings
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                session_id.to_string(),
                SharedSessionBinding {
                    project_root,
                    execution_root,
                },
            );
        Ok(())
    }
}

fn canonical_request_path_value(
    value: &serde_json::Value,
) -> Result<PathBuf, AppServerHostPolicyError> {
    let path = value.as_str().ok_or_else(|| {
        AppServerHostPolicyError::invalid_request("Workspace path must be a string")
    })?;
    canonical_request_path(path)
}

fn canonical_request_path(path: &str) -> Result<PathBuf, AppServerHostPolicyError> {
    dunce::canonicalize(path).map_err(|_| {
        AppServerHostPolicyError::invalid_request("The requested workspace path is unavailable")
    })
}

fn method_requires_bound_session(method: &str) -> bool {
    matches!(
        method,
        "agent/deleteSession"
            | "agent/submitTurn"
            | "agent/submitDialogTurn"
            | "agent/steerTurn"
            | "agent/runUserShellCommand"
            | "agent/cancelTurn"
            | "session/subscribe"
            | "session/unsubscribe"
            | "session/readTranscript"
            | "session/resolveWorkspace"
            | "session/rename"
            | "session/setArchived"
            | "session/updateModel"
            | "session/updateMode"
            | "session/fork"
            | "session/forkAtTurn"
            | "session/forkBeforeTurn"
            | "session/compact"
            | "session/undo"
            | "session/redo"
            | "session/reloadContext"
            | "session/usage"
            | "session/waitForSettlement"
            | "session/lineage"
            | "session/inspectLineage"
            | "session/cancelLineage"
            | "workspace/searchReferences"
            | "workspace/messageReferences"
            | "worktree/bindSession"
            | "worktree/releaseSession"
    )
}

fn is_shared_method(method: &str) -> bool {
    matches!(
        method,
        "app/initialize"
            | "app/health"
            | "app/syncEvents"
            | "agent/createSession"
            | "agent/listSessions"
            | "agent/deleteSession"
            | "agent/submitDialogTurn"
            | "agent/steerTurn"
            | "agent/runUserShellCommand"
            | "agent/submitUserAnswers"
            | "agent/cancelTurn"
            | "agent/listModes"
            | "agent/respondPermission"
            | "agent/respondPermissionBatch"
            | "agent/listPendingPermissionRequests"
            | "agent/listProjectPermissionGrants"
            | "agent/removeProjectPermissionGrant"
            | "agent/clearProjectPermissionGrants"
            | "agent/listProjectPermissionAudit"
            | "agent/event"
            | "agent/permissionEvent"
            | "session/sync"
            | "session/subscribe"
            | "session/unsubscribe"
            | "session/readTranscript"
            | "session/resolveWorkspace"
            | "session/rename"
            | "session/setArchived"
            | "session/updateModel"
            | "session/updateMode"
            | "session/fork"
            | "session/forkAtTurn"
            | "session/forkBeforeTurn"
            | "session/restore"
            | "session/compact"
            | "session/undo"
            | "session/redo"
            | "session/reloadContext"
            | "session/usage"
            | "session/waitForSettlement"
            | "session/lineage"
            | "session/inspectLineage"
            | "session/cancelLineage"
            | "workspace/diff"
            | "workspace/searchReferences"
            | "workspace/messageReferences"
            | "config/getAgentProfileConfigs"
            | "config/getAgentProfileConfig"
            | "config/getModelConfigs"
            | "config/getTuiModelCatalog"
            | "model/projectReasoningCatalog"
            | "config/getConfig"
            | "config/getConfigs"
            | "config/setConfig"
            | "config/saveCloudSpeechConfig"
            | "config/validateConfig"
            | "config/setAgentProfileConfig"
            | "config/resetAgentProfileConfig"
            | "config/event"
            | "i18n/getCurrentLanguage"
            | "i18n/setLanguage"
            | "i18n/getConfig"
            | "i18n/setConfig"
            | "i18n/getSupportedLanguages"
            | "model/list"
            | "model/get"
            | "model/add"
            | "model/update"
            | "model/delete"
            | "model/setDefault"
            | "skill/list"
            | "skill/setEnabled"
            | "subagent/list"
            | "subagent/setEnabled"
            | "mcp/list"
            | "mcp/toggle"
            | "mcp/add"
            | "mcp/delete"
            | "mcp/externalDecision"
            | "mcp/conflictChoice"
            | "externalSource/snapshot"
            | "externalSource/control"
            | "externalSource/review"
            | "externalSource/setNativeCommandChoice"
            | "externalSource/expandCommand"
            | "externalSource/event"
            | "nativeHook/overview"
            | "externalHook/snapshot"
            | "externalHook/plan"
            | "externalHook/apply"
            | "externalHook/mutate"
            | "account/snapshot"
            | "account/login"
            | "account/finalizeLogin"
            | "account/logout"
            | "settingsSync/start"
            | "settingsSync/snapshot"
            | "settingsSync/cancel"
            | "settingsSync/localChanged"
            | "worktree/repositoryStatus"
            | "worktree/bindSession"
            | "worktree/releaseSession"
            | "app/eventStreamState"
    )
}

#[derive(Default)]
struct SharedOperationState {
    pending: HashSet<SharedOperationKey>,
    active: HashSet<SharedOperationKey>,
    terminal_before_admission: HashSet<SharedOperationKey>,
    tracking_uncertain: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SharedOperationKey {
    session_id: String,
    turn_id: String,
    kind: AppServerOperationKind,
}

impl SharedOperationKey {
    fn new(session_id: &str, turn_id: &str, kind: AppServerOperationKind) -> Self {
        Self {
            session_id: session_id.to_string(),
            turn_id: turn_id.to_string(),
            kind,
        }
    }
}

#[derive(Default)]
struct SharedOperationTracker {
    state: std::sync::Mutex<SharedOperationState>,
}

impl SharedOperationTracker {
    fn is_idle(&self) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.pending.is_empty() && state.active.is_empty() && !state.tracking_uncertain
    }

    fn terminal(&self, session_id: &str, turn_id: &str, kind: AppServerOperationKind) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let key = SharedOperationKey::new(session_id, turn_id, kind);
        if state.pending.remove(&key) {
            state.terminal_before_admission.insert(key.clone());
        }
        state.active.remove(&key);
    }

    fn mark_uncertain(&self) {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .tracking_uncertain = true;
    }
}

fn can_idle_exit(connection_count: usize, tracker: &SharedOperationTracker) -> bool {
    connection_count == 0 && tracker.is_idle()
}

impl AppServerOperationObserver for SharedOperationTracker {
    fn operation_started(&self, session_id: &str, turn_id: &str, kind: AppServerOperationKind) {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pending
            .insert(SharedOperationKey::new(session_id, turn_id, kind));
    }

    fn operation_admitted(&self, session_id: &str, turn_id: &str, kind: AppServerOperationKind) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let key = SharedOperationKey::new(session_id, turn_id, kind);
        state.pending.remove(&key);
        if !state.terminal_before_admission.remove(&key) {
            state.active.insert(key);
        }
    }

    fn operation_rejected(&self, session_id: &str, turn_id: &str, kind: AppServerOperationKind) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let key = SharedOperationKey::new(session_id, turn_id, kind);
        state.pending.remove(&key);
        state.terminal_before_admission.remove(&key);
    }
}

async fn observe_operations(
    tracker: std::sync::Arc<SharedOperationTracker>,
    mut events: bitfun_agent_runtime::sdk::AgentEventReceiver,
) {
    loop {
        match events.recv().await {
            Ok(envelope) => match &envelope.event {
                bitfun_events::AgenticEvent::DialogTurnCompleted {
                    session_id,
                    turn_id,
                    ..
                }
                | bitfun_events::AgenticEvent::DialogTurnCancelled {
                    session_id,
                    turn_id,
                }
                | bitfun_events::AgenticEvent::DialogTurnFailed {
                    session_id,
                    turn_id,
                    ..
                } => tracker.terminal(session_id, turn_id, AppServerOperationKind::DialogTurn),
                bitfun_events::AgenticEvent::ContextCompressionCompleted {
                    session_id,
                    turn_id,
                    ..
                }
                | bitfun_events::AgenticEvent::ContextCompressionFailed {
                    session_id,
                    turn_id,
                    ..
                } => tracker.terminal(
                    session_id,
                    turn_id,
                    AppServerOperationKind::ContextCompaction,
                ),
                _ => {}
            },
            Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                tracing::error!(missed, "Shared operation tracker lost Runtime events");
                tracker.mark_uncertain();
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                tracker.mark_uncertain();
                break;
            }
        }
    }
}

struct SharedInstanceLock {
    file: File,
}

impl SharedInstanceLock {
    fn try_acquire(root: &Path, identity: &RuntimeOwnershipKey) -> Result<Self, InstanceLockError> {
        ensure_private_directory(root).map_err(InstanceLockError::PrepareDirectory)?;
        let path = root.join(format!("{}.instance.lock", identity.as_str()));
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        configure_private_file(&mut options);
        let file = options
            .open(&path)
            .map_err(|source| InstanceLockError::Open { path, source })?;
        fs2::FileExt::try_lock_exclusive(&file).map_err(InstanceLockError::AlreadyOwned)?;
        Ok(Self { file })
    }
}

impl Drop for SharedInstanceLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

#[derive(Debug, thiserror::Error)]
enum InstanceLockError {
    #[error("prepare Shared App Server instance directory")]
    PrepareDirectory(#[source] anyhow::Error),
    #[error("open Shared App Server instance lock {path}")]
    Open {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Shared App Server instance is already owned")]
    AlreadyOwned(#[source] io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SharedAppServerDiscovery {
    protocol_version: u32,
    instance_identity: String,
    endpoint: String,
    process_id: u32,
    token: String,
    owner_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuthRequest {
    protocol_version: u32,
    instance_identity: String,
    token: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuthResponse {
    accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

pub(crate) async fn connect_or_start(workspace: &Path) -> Result<AppServerClient> {
    prepare_client_environment().await?;
    let identity = instance_identity(workspace)?;
    let root = shared_root()?;
    let mut last_connect_error = None;

    match connect_existing(&root, &identity).await {
        Ok(Some(client)) => return Ok(client),
        Ok(None) => {}
        Err(error) => last_connect_error = Some(error),
    }

    let mut child = StartupChild::spawn(workspace, identity.as_str())?;
    let mut started = Instant::now();
    let mut respawned = false;
    loop {
        match connect_existing(&root, &identity).await {
            Ok(Some(client)) => {
                child.disarm();
                return Ok(client);
            }
            Ok(None) => {}
            Err(error) => last_connect_error = Some(error),
        }

        if let Some(status) = child.try_wait().context("poll Shared App Server startup")? {
            if embedded_runtime_owner_present(workspace)? {
                return Err(anyhow!(
                    "Agent Runtime ownership failed (runtime_ownership_unavailable): an Embedded Runtime owns this workspace; close it before starting --shared ({status})"
                ));
            }
            if shared_instance_present(&root, &identity)? {
                // A concurrent Shared child may hold its instance lock before
                // it acquires Runtime ownership and publishes discovery.
            } else if runtime_owner_present(workspace)? {
                return Err(anyhow!(
                    "Agent Runtime ownership failed (runtime_ownership_unavailable): another Shared Runtime deployment owns this workspace; close its clients before starting --shared ({status})"
                ));
            } else {
                if respawned {
                    return Err(anyhow!(
                        "Shared App Server exited before becoming ready ({status})"
                    ));
                }
                child = StartupChild::spawn(workspace, identity.as_str())?;
                respawned = true;
                started = Instant::now();
            }
        }

        if started.elapsed() >= STARTUP_TIMEOUT {
            let detail = last_connect_error
                .as_ref()
                .map(|error| format!("; last connection error: {error:#}"))
                .unwrap_or_default();
            return Err(anyhow!(
                "Shared App Server did not become ready within {} seconds{detail}",
                STARTUP_TIMEOUT.as_secs()
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub(crate) fn run_service(workspace: PathBuf, expected_identity: String) -> Result<()> {
    // The App Server handler graph and the first dialog-turn future are deep
    // enough that the default Tokio worker stack is not a reliable host for
    // this service on Windows. Keep the process-level host topology explicit,
    // matching the Embedded App Server's dedicated large-stack thread.
    let thread = std::thread::Builder::new()
        .name("bitfun-shared-app-server".to_string())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("build Shared App Server Tokio runtime")?;
            runtime.block_on(run_service_inner(workspace, expected_identity))
        })
        .context("start Shared App Server service thread")?;

    match thread.join() {
        Ok(result) => result,
        Err(_) => Err(anyhow!("Shared App Server service thread panicked")),
    }
}

async fn run_service_inner(workspace: PathBuf, expected_identity: String) -> Result<()> {
    bitfun_services_core::process_manager::contain_current_process_tree()
        .context("contain Shared App Server process tree")?;
    prepare_client_environment().await?;
    let identity = instance_identity(&workspace)?;
    if identity.as_str() != expected_identity {
        return Err(anyhow!(
            "Shared App Server identity does not match its workspace"
        ));
    }

    let root = shared_root()?;
    ensure_private_directory(&root)?;
    let _instance_lock = SharedInstanceLock::try_acquire(&root, &identity)
        .context("acquire Shared App Server instance lock")?;
    remove_discovery(&root, &identity)?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .context("bind Shared App Server loopback endpoint")?;

    let runtime = crate::initialize_core_services_for_deployment(
        &workspace,
        crate::runtime::approval::CliApprovalPolicy::Ask,
        crate::BootstrapProfile::Interactive,
        RuntimeDeployment::Shared,
    )
    .await?;
    let event_source = runtime.agent_event_source();
    let operation_tracker = std::sync::Arc::new(SharedOperationTracker::default());
    let operation_task = tokio::spawn(observe_operations(
        operation_tracker.clone(),
        event_source.subscribe(),
    ));
    let app_runtime = BitfunAppRuntime::new(runtime.agent_runtime().clone(), event_source)
        .with_context_reload(std::sync::Arc::new(runtime.compatibility().clone()));
    let account = runtime.account_runtime().clone();
    if let Some(user_id) = account.try_restore_session().await {
        tracing::info!(user_id, "Shared App Server restored account session");
    }
    account.start_settings_sync_loop();
    let management =
        std::sync::Arc::new(AppManagementService::load_for_local_host(Some(account)).await?);
    let host_policy = std::sync::Arc::new(SharedHostPolicy::new(&workspace)?);
    let server = BitfunAppServer::new(app_runtime)
        .with_management(management)
        .with_host_policy(host_policy)
        .with_operation_observer(operation_tracker.clone())
        .with_transport_limits(shared_transport_limits());

    let owner_id = uuid::Uuid::new_v4().to_string();
    let discovery = SharedAppServerDiscovery {
        protocol_version: PROTOCOL_VERSION,
        instance_identity: identity.as_str().to_string(),
        endpoint: listener.local_addr()?.to_string(),
        process_id: std::process::id(),
        token: format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ),
        owner_id,
    };
    write_discovery(&root, &identity, &discovery)?;
    tracing::info!(
        endpoint = %discovery.endpoint,
        "Shared App Server is ready"
    );

    let result = serve_connections(listener, server, discovery.clone(), operation_tracker).await;
    operation_task.abort();
    let _ = operation_task.await;
    let _ = remove_discovery_if_owned(&root, &identity, &discovery);
    crate::shutdown_mcp_servers().await;
    drop(runtime);
    result
}

async fn serve_connections(
    listener: TcpListener,
    server: BitfunAppServer,
    discovery: SharedAppServerDiscovery,
    operation_tracker: std::sync::Arc<SharedOperationTracker>,
) -> Result<()> {
    let mut connections = JoinSet::new();
    loop {
        if connections.is_empty() {
            if can_idle_exit(connections.len(), &operation_tracker) {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (stream, _) = accepted.context("accept Shared App Server connection")?;
                        spawn_connection(&mut connections, stream, server.clone(), discovery.clone());
                    }
                    _ = tokio::time::sleep(IDLE_TIMEOUT) => {
                        if can_idle_exit(connections.len(), &operation_tracker) {
                            break;
                        }
                    },
                }
            } else {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (stream, _) = accepted.context("accept Shared App Server connection")?;
                        spawn_connection(&mut connections, stream, server.clone(), discovery.clone());
                    }
                    _ = tokio::time::sleep(Duration::from_millis(250)) => {}
                }
            }
        } else {
            tokio::select! {
                accepted = listener.accept() => {
                    let (stream, _) = accepted.context("accept Shared App Server connection")?;
                    if connections.len() < MAX_CONNECTIONS {
                        spawn_connection(&mut connections, stream, server.clone(), discovery.clone());
                    } else {
                        tracing::warn!("Shared App Server connection limit reached");
                    }
                }
                joined = connections.join_next() => {
                    if let Some(Err(error)) = joined {
                        tracing::warn!("Shared App Server connection task failed: {error}");
                    }
                }
            }
        }
    }
    connections.abort_all();
    while connections.join_next().await.is_some() {}
    Ok(())
}

fn spawn_connection(
    connections: &mut JoinSet<()>,
    stream: TcpStream,
    server: BitfunAppServer,
    discovery: SharedAppServerDiscovery,
) {
    connections.spawn(async move {
        if let Err(error) = serve_connection(stream, server, &discovery).await {
            tracing::warn!("Shared App Server connection ended: {error:#}");
        }
    });
}

async fn serve_connection(
    stream: TcpStream,
    server: BitfunAppServer,
    discovery: &SharedAppServerDiscovery,
) -> Result<()> {
    stream.set_nodelay(true)?;
    let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(MAX_REQUEST_BYTES));
    let auth_line = tokio::time::timeout(AUTH_TIMEOUT, framed.next())
        .await
        .context("Shared App Server authentication timed out")?
        .ok_or_else(|| anyhow!("Shared App Server connection closed before authentication"))??;
    let auth: AuthRequest =
        serde_json::from_str(&auth_line).context("decode Shared App Server authentication")?;
    let accepted = auth.protocol_version == PROTOCOL_VERSION
        && auth.instance_identity == discovery.instance_identity
        && constant_time_eq(auth.token.as_bytes(), discovery.token.as_bytes());
    framed
        .send(serde_json::to_string(&AuthResponse {
            accepted,
            message: (!accepted).then(|| "Shared App Server authentication failed".to_string()),
        })?)
        .await?;
    if !accepted {
        return Err(anyhow!("Shared App Server authentication failed"));
    }

    let (sink, stream) = framed.split();
    let outgoing = sink
        .sink_map_err(line_error)
        .with(|line: String| async move { bounded_line(line, MAX_RESPONSE_EVENT_BYTES) });
    let incoming = stream.map(|result| result.map_err(line_error));
    let (incoming, disconnected) = observe_disconnect(incoming);
    let serving = server
        .require_session_subscriptions(true)
        .serve(Lines::new(outgoing, incoming));
    tokio::pin!(serving);
    tokio::select! {
        biased;
        result = &mut serving => result.map_err(anyhow::Error::from),
        disconnected = disconnected => match disconnected {
            Ok(()) => Ok(()),
            Err(_) => serving.await.map_err(anyhow::Error::from),
        },
    }
}

fn observe_disconnect<S>(
    stream: S,
) -> (
    impl futures_util::Stream<Item = S::Item> + Send,
    oneshot::Receiver<()>,
)
where
    S: futures_util::Stream + Send + 'static,
    S::Item: Send + 'static,
{
    // agent-client-protocol keeps waiting for the server's foreground event
    // loop after a clean EOF, so notify the Host and park until it cancels it.
    let (disconnected_tx, disconnected_rx) = oneshot::channel();
    let stream = futures_util::stream::unfold(
        (Box::pin(stream), Some(disconnected_tx)),
        |(mut stream, mut disconnected_tx)| async move {
            match stream.as_mut().next().await {
                Some(item) => Some((item, (stream, disconnected_tx))),
                None => {
                    if let Some(disconnected_tx) = disconnected_tx.take() {
                        let _ = disconnected_tx.send(());
                    }
                    std::future::pending().await
                }
            }
        },
    );
    (stream, disconnected_rx)
}

async fn connect_existing(
    root: &Path,
    identity: &RuntimeOwnershipKey,
) -> Result<Option<AppServerClient>> {
    let Some(discovery) = read_discovery(root, identity)? else {
        return Ok(None);
    };
    if discovery.protocol_version != PROTOCOL_VERSION
        || discovery.instance_identity != identity.as_str()
    {
        return Err(anyhow!("Shared App Server discovery is incompatible"));
    }
    let endpoint: SocketAddr = discovery
        .endpoint
        .parse()
        .context("parse Shared App Server endpoint")?;
    if !endpoint.ip().is_loopback() {
        return Err(anyhow!("Shared App Server endpoint is not loopback"));
    }
    let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(endpoint))
        .await
        .context("connect Shared App Server timed out")?
        .context("connect Shared App Server")?;
    stream.set_nodelay(true)?;
    let mut framed = Framed::new(
        stream,
        LinesCodec::new_with_max_length(MAX_RESPONSE_EVENT_BYTES),
    );
    framed
        .send(serde_json::to_string(&AuthRequest {
            protocol_version: PROTOCOL_VERSION,
            instance_identity: identity.as_str().to_string(),
            token: discovery.token,
        })?)
        .await?;
    let response_line = tokio::time::timeout(AUTH_TIMEOUT, framed.next())
        .await
        .context("Shared App Server authentication response timed out")?
        .ok_or_else(|| anyhow!("Shared App Server closed during authentication"))??;
    let response: AuthResponse = serde_json::from_str(&response_line)
        .context("decode Shared App Server authentication response")?;
    if !response.accepted {
        return Err(anyhow!(
            "{}",
            response
                .message
                .unwrap_or_else(|| "Shared App Server authentication failed".to_string())
        ));
    }

    let (sink, stream) = framed.split();
    let outgoing = sink
        .sink_map_err(line_error)
        .with(|line: String| async move { bounded_line(line, MAX_REQUEST_BYTES) });
    let incoming = stream.map(|result| result.map_err(line_error));
    bitfun_app_server_client::connect(Lines::new(outgoing, incoming))
        .await
        .context("connect typed Shared App Server client")
        .map(Some)
}

fn bounded_line(line: String, maximum: usize) -> io::Result<String> {
    if line.len() > maximum {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("App Server frame exceeds {maximum} bytes"),
        ))
    } else {
        Ok(line)
    }
}

fn shared_transport_limits() -> TransportLimits {
    TransportLimits {
        max_request_bytes: MAX_REQUEST_BYTES as u64,
        max_response_bytes: MAX_RESPONSE_EVENT_BYTES as u64,
        max_frame_bytes: MAX_REQUEST_BYTES.min(MAX_RESPONSE_EVENT_BYTES) as u64,
        event_buffer_capacity: 1024,
    }
}

fn line_error(error: LinesCodecError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

async fn prepare_client_environment() -> Result<()> {
    crate::agent::agentic_system::select_agentic_system_profile(
        bitfun_core::product_assembly::DeliveryProfile::Cli,
    )?;
    bitfun_core::service::config::initialize_global_config()
        .await
        .map_err(|error| anyhow!("Failed to initialize Shared App Server configuration: {error}"))
}

fn instance_identity(workspace: &Path) -> Result<RuntimeOwnershipKey> {
    RuntimeOwnershipKey::for_workspace(workspace, CoreRuntimeOwnership::distribution_identity())
        .context("resolve Shared App Server identity")
}

fn shared_root() -> Result<PathBuf> {
    Ok(path_manager()?
        .user_data_dir()
        .join("agent-runtime")
        .join(format!("shared-app-server-v{PROTOCOL_VERSION}")))
}

fn path_manager() -> Result<std::sync::Arc<bitfun_core::infrastructure::PathManager>> {
    bitfun_core::infrastructure::try_get_path_manager_arc()
        .map_err(|error| anyhow!(error.to_string()))
}

fn runtime_owner_present(workspace: &Path) -> Result<bool> {
    CoreRuntimeOwnership::runtime_owner_present(path_manager()?.as_ref(), workspace)
        .map_err(anyhow::Error::from)
}

fn embedded_runtime_owner_present(workspace: &Path) -> Result<bool> {
    CoreRuntimeOwnership::embedded_runtime_owner_present(path_manager()?.as_ref(), workspace)
        .map_err(anyhow::Error::from)
}

fn shared_instance_present(root: &Path, identity: &RuntimeOwnershipKey) -> Result<bool> {
    match SharedInstanceLock::try_acquire(root, identity) {
        Ok(lock) => {
            drop(lock);
            Ok(false)
        }
        Err(InstanceLockError::AlreadyOwned(_)) => Ok(true),
        Err(error) => Err(error).context("inspect Shared App Server instance lock"),
    }
}

fn ensure_private_directory(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("create Shared App Server directory {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn configure_private_file(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = options;
}

fn discovery_path(root: &Path, identity: &RuntimeOwnershipKey) -> PathBuf {
    root.join(format!("{}.json", identity.as_str()))
}

fn read_discovery(
    root: &Path,
    identity: &RuntimeOwnershipKey,
) -> Result<Option<SharedAppServerDiscovery>> {
    let path = discovery_path(root, identity);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("read Shared App Server discovery"),
    };
    if bytes.len() as u64 > MAX_DISCOVERY_BYTES {
        return Err(anyhow!("Shared App Server discovery is too large"));
    }
    let record = serde_json::from_slice::<SharedAppServerDiscovery>(&bytes)
        .context("decode Shared App Server discovery")?;
    Ok(Some(record))
}

fn write_discovery(
    root: &Path,
    identity: &RuntimeOwnershipKey,
    record: &SharedAppServerDiscovery,
) -> Result<()> {
    use std::io::Write as _;

    ensure_private_directory(root)?;
    let bytes = serde_json::to_vec(record)?;
    if bytes.len() as u64 > MAX_DISCOVERY_BYTES {
        return Err(anyhow!("Shared App Server discovery is too large"));
    }
    let path = discovery_path(root, identity);
    let mut temporary = tempfile::NamedTempFile::new_in(root)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(&path)
        .map_err(|error| error.error)
        .context("publish Shared App Server discovery")?;
    Ok(())
}

fn remove_discovery_if_owned(
    root: &Path,
    identity: &RuntimeOwnershipKey,
    expected: &SharedAppServerDiscovery,
) -> Result<()> {
    if read_discovery(root, identity)?.as_ref() == Some(expected) {
        let path = discovery_path(root, identity);
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("remove Shared App Server discovery"),
        }
    }
    Ok(())
}

fn remove_discovery(root: &Path, identity: &RuntimeOwnershipKey) -> Result<()> {
    let path = discovery_path(root, identity);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("remove stale Shared App Server discovery"),
    }
}

struct StartupChild {
    child: Option<Child>,
}

impl StartupChild {
    fn spawn(workspace: &Path, identity: &str) -> Result<Self> {
        let executable = std::env::current_exe().context("resolve BitFun executable")?;
        let mut command = bitfun_services_core::process_manager::create_command(executable);
        command
            .arg("__shared-app-server")
            .arg("--workspace")
            .arg(workspace)
            .arg("--instance-identity")
            .arg(identity)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_detached_process(&mut command);
        let child = command.spawn().context("start Shared App Server process")?;
        Ok(Self { child: Some(child) })
    }

    fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        self.child
            .as_mut()
            .expect("startup child is armed")
            .try_wait()
    }

    fn disarm(mut self) {
        self.child.take();
    }
}

impl Drop for StartupChild {
    fn drop(&mut self) {
        let Some(child) = self.child.as_mut() else {
            return;
        };
        #[cfg(unix)]
        if let Ok(process_id) = i32::try_from(child.id()) {
            let _ = unsafe { libc::kill(-process_id, libc::SIGKILL) };
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn configure_detached_process(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    let _ = command;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_binding(
        project: &Path,
        execution: &Path,
    ) -> bitfun_runtime_ports::AgentSessionWorkspaceBinding {
        bitfun_runtime_ports::AgentSessionWorkspaceBinding {
            workspace_id: None,
            workspace_path: execution.to_string_lossy().to_string(),
            project_workspace_path: Some(project.to_string_lossy().to_string()),
            execution_target: Some(bitfun_runtime_ports::SessionExecutionTarget::local(
                execution.to_string_lossy().to_string(),
            )),
            remote_connection_id: None,
            remote_ssh_host: None,
        }
    }

    #[test]
    fn authentication_comparison_rejects_length_and_value_mismatches() {
        assert!(constant_time_eq(b"same-token", b"same-token"));
        assert!(!constant_time_eq(b"same-token", b"other-token"));
        assert!(!constant_time_eq(b"same-token", b"same-token-longer"));
    }

    #[test]
    fn bounded_lines_reject_oversized_frames() {
        assert!(bounded_line("ok".to_string(), 2).is_ok());
        assert!(bounded_line("too-large".to_string(), 2).is_err());
    }

    #[test]
    fn advertised_transport_limits_match_shared_framing() {
        let limits = shared_transport_limits();
        assert_eq!(limits.max_request_bytes, MAX_REQUEST_BYTES as u64);
        assert_eq!(limits.max_response_bytes, MAX_RESPONSE_EVENT_BYTES as u64);
        assert_eq!(
            limits.max_frame_bytes,
            MAX_REQUEST_BYTES.min(MAX_RESPONSE_EVENT_BYTES) as u64
        );
    }

    #[test]
    fn shared_policy_rejects_other_workspaces_and_remote_execution() {
        let workspace = tempfile::tempdir().expect("workspace");
        let other = tempfile::tempdir().expect("other workspace");
        let policy = SharedHostPolicy::new(workspace.path()).expect("Shared Host policy");

        let outside = serde_json::json!({
            "workspacePath": other.path().to_string_lossy(),
        });
        assert!(matches!(
            policy.authorize_request("agent/createSession", &outside),
            Err(AppServerHostPolicyError {
                kind: bitfun_app_server_protocol::error::AppServerErrorKind::InvalidRequest,
                ..
            })
        ));

        let remote = serde_json::json!({
            "workspacePath": workspace.path().to_string_lossy(),
            "remoteConnectionId": "remote-1",
        });
        assert!(matches!(
            policy.authorize_request("agent/createSession", &remote),
            Err(AppServerHostPolicyError {
                kind: bitfun_app_server_protocol::error::AppServerErrorKind::Unsupported,
                ..
            })
        ));
    }

    #[test]
    fn shared_policy_requires_exact_turn_and_authoritative_session_binding() {
        let workspace = tempfile::tempdir().expect("workspace");
        let policy = SharedHostPolicy::new(workspace.path()).expect("Shared Host policy");
        let missing_turn = serde_json::json!({
            "sessionId": "session-1",
            "workspacePath": workspace.path().to_string_lossy(),
        });
        assert!(policy
            .authorize_request("agent/submitDialogTurn", &missing_turn)
            .is_err());

        let request = serde_json::json!({
            "sessionId": "session-1",
            "turnId": "turn-1",
            "workspacePath": workspace.path().to_string_lossy(),
        });
        policy
            .authorize_preflight("agent/submitDialogTurn", &request)
            .expect("preflight must defer authoritative Session binding checks");
        assert!(policy
            .authorize_request("agent/submitDialogTurn", &request)
            .is_err());

        policy
            .register_session_binding(
                "session-1",
                &local_binding(workspace.path(), workspace.path()),
            )
            .expect("register authoritative Session binding");
        policy
            .authorize_request("agent/submitDialogTurn", &request)
            .expect("bound Session and exact turn must be accepted");
    }

    #[test]
    fn shared_policy_hides_and_rejects_unexposed_git_methods() {
        let workspace = tempfile::tempdir().expect("workspace");
        let policy = SharedHostPolicy::new(workspace.path()).expect("Shared Host policy");
        assert!(!policy.allows_method("git/getStatus"));
        assert!(matches!(
            policy.authorize_request(
                "git/getStatus",
                &serde_json::json!({ "repositoryPath": workspace.path().to_string_lossy() }),
            ),
            Err(AppServerHostPolicyError {
                kind: bitfun_app_server_protocol::error::AppServerErrorKind::Unsupported,
                ..
            })
        ));
    }

    #[test]
    fn active_or_uncertain_operations_prevent_idle_exit() {
        let tracker = SharedOperationTracker::default();
        assert!(can_idle_exit(0, &tracker));
        assert!(!can_idle_exit(1, &tracker));

        tracker.operation_started("session-1", "turn-1", AppServerOperationKind::DialogTurn);
        assert!(!can_idle_exit(0, &tracker));
        tracker.operation_admitted("session-1", "turn-1", AppServerOperationKind::DialogTurn);
        assert!(!can_idle_exit(0, &tracker));
        tracker.terminal("session-1", "turn-1", AppServerOperationKind::DialogTurn);
        assert!(can_idle_exit(0, &tracker));

        tracker.mark_uncertain();
        assert!(!can_idle_exit(0, &tracker));
    }

    #[test]
    fn operation_terminal_events_clear_only_the_matching_kind() {
        let tracker = SharedOperationTracker::default();
        tracker.operation_started("session-1", "turn-1", AppServerOperationKind::DialogTurn);
        tracker.operation_admitted("session-1", "turn-1", AppServerOperationKind::DialogTurn);

        tracker.terminal(
            "session-1",
            "turn-1",
            AppServerOperationKind::ContextCompaction,
        );
        assert!(
            !tracker.is_idle(),
            "compaction terminal must not clear a dialog turn"
        );
        tracker.terminal("session-1", "turn-1", AppServerOperationKind::DialogTurn);
        assert!(tracker.is_idle());

        tracker.operation_started(
            "session-1",
            "turn-2",
            AppServerOperationKind::ContextCompaction,
        );
        tracker.operation_admitted(
            "session-1",
            "turn-2",
            AppServerOperationKind::ContextCompaction,
        );
        tracker.terminal("session-1", "turn-2", AppServerOperationKind::DialogTurn);
        assert!(
            !tracker.is_idle(),
            "dialog terminal must not clear compaction"
        );
        tracker.terminal(
            "session-1",
            "turn-2",
            AppServerOperationKind::ContextCompaction,
        );
        assert!(tracker.is_idle());
    }

    #[test]
    fn operation_identity_includes_session_for_reused_turn_ids() {
        let tracker = SharedOperationTracker::default();
        for session_id in ["session-1", "session-2"] {
            tracker.operation_started(
                session_id,
                "shared-turn-id",
                AppServerOperationKind::DialogTurn,
            );
            tracker.operation_admitted(
                session_id,
                "shared-turn-id",
                AppServerOperationKind::DialogTurn,
            );
        }

        tracker.terminal(
            "session-1",
            "shared-turn-id",
            AppServerOperationKind::DialogTurn,
        );
        assert!(!tracker.is_idle(), "the second Session is still active");
        tracker.terminal(
            "session-2",
            "shared-turn-id",
            AppServerOperationKind::DialogTurn,
        );
        assert!(tracker.is_idle());
    }

    #[tokio::test]
    async fn observed_stream_reports_disconnect_after_eof() {
        let (stream, disconnected) = observe_disconnect(futures_util::stream::iter(["request"]));
        tokio::pin!(stream);

        assert_eq!(stream.next().await, Some("request"));
        tokio::time::timeout(Duration::from_millis(100), stream.next())
            .await
            .expect_err("observed stream should park after reporting EOF");
        tokio::time::timeout(Duration::from_secs(1), disconnected)
            .await
            .expect("EOF should report the disconnected client")
            .expect("disconnect observer should remain alive through EOF");
    }

    #[tokio::test]
    async fn observed_stream_does_not_report_disconnect_while_open() {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let source = futures_util::stream::unfold(receiver, |mut receiver| async move {
            receiver.recv().await.map(|item| (item, receiver))
        });
        let (stream, mut disconnected) = observe_disconnect(source);
        tokio::pin!(stream);

        sender.send("request").expect("source should stay open");
        assert_eq!(stream.next().await, Some("request"));
        assert_eq!(
            disconnected.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        );

        drop(sender);
        tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .expect_err("observed stream should park after reporting EOF");
        disconnected
            .await
            .expect("dropping the source should report the disconnected client");
    }
}

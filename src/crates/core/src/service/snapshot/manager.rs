use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::agentic::tools::registry::ToolRegistry;
use crate::service::remote_ssh::workspace_state::is_remote_path;
use crate::service::snapshot::service::SnapshotService;
use crate::service::snapshot::types::{
    OperationType, SnapshotConfig, SnapshotError, SnapshotResult,
};
use crate::service::workspace_runtime::get_workspace_runtime_service_arc;
use async_trait::async_trait;
use log::{debug, error, info, warn};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock as StdRwLock};
use tokio::sync::RwLock;

/// Snapshot manager
///
/// Manages all components of the snapshot system.
pub struct SnapshotManager {
    snapshot_service: Arc<RwLock<SnapshotService>>,
}

impl SnapshotManager {
    /// Creates a new snapshot manager.
    pub async fn new(
        workspace_dir: PathBuf,
        config: Option<SnapshotConfig>,
    ) -> SnapshotResult<Self> {
        info!(
            "Creating snapshot manager: workspace={}",
            workspace_dir.display()
        );

        let runtime_service = get_workspace_runtime_service_arc();
        let runtime_context = runtime_service
            .ensure_local_workspace_runtime(&workspace_dir)
            .await
            .map_err(|e| SnapshotError::ConfigError(e.to_string()))?
            .context;

        let mut snapshot_service = SnapshotService::new(workspace_dir, runtime_context, config);
        snapshot_service.initialize().await?;
        let snapshot_service = Arc::new(RwLock::new(snapshot_service));
        Ok(Self { snapshot_service })
    }

    /// Records a file change.
    pub async fn record_file_change(
        &self,
        session_id: &str,
        turn_index: usize,
        file_path: PathBuf,
        operation_type: OperationType,
        tool_name: String,
    ) -> SnapshotResult<String> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service
            .record_file_change(session_id, turn_index, file_path, operation_type, tool_name)
            .await
    }

    /// Rolls back a session.
    pub async fn rollback_session(&self, session_id: &str) -> SnapshotResult<Vec<PathBuf>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.rollback_session(session_id).await
    }

    /// Rolls back to a specific turn.
    pub async fn rollback_to_turn(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> SnapshotResult<Vec<PathBuf>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service
            .rollback_to_turn(session_id, turn_index)
            .await
    }

    /// Accepts all changes in a session.
    pub async fn accept_session(&self, session_id: &str) -> SnapshotResult<()> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.accept_session(session_id).await
    }

    /// Accepts changes for a single file.
    pub async fn accept_file(&self, session_id: &str, file_path: &str) -> SnapshotResult<()> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);
        snapshot_service.accept_file(session_id, file_path).await
    }

    /// Rejects changes for a single file by restoring its pre-session state.
    pub async fn reject_file(
        &self,
        session_id: &str,
        file_path: &str,
    ) -> SnapshotResult<Vec<PathBuf>> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);
        snapshot_service.reject_file(session_id, file_path).await
    }

    /// Returns the list of files affected by a session.
    pub async fn get_session_files(&self, session_id: &str) -> SnapshotResult<Vec<PathBuf>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.get_session_files(session_id).await
    }

    /// Returns the list of turns for a session.
    pub async fn get_session_turns(&self, session_id: &str) -> SnapshotResult<Vec<usize>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.get_session_turns(session_id).await
    }

    /// Returns the list of files modified in a turn.
    pub async fn get_turn_files(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> SnapshotResult<Vec<PathBuf>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service
            .get_turn_files(session_id, turn_index)
            .await
    }

    /// Returns the diff content for a file.
    pub async fn get_file_diff(
        &self,
        session_id: &str,
        file_path: &str,
        anchor_operation_id: Option<&str>,
    ) -> SnapshotResult<serde_json::Value> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);
        let (original, modified, anchor_line) = snapshot_service
            .get_file_diff_with_anchor(session_id, file_path, anchor_operation_id)
            .await?;

        Ok(serde_json::json!({
            "file_path": file_path.to_string_lossy(),
            "original_content": original,
            "modified_content": modified,
            "anchor_line": anchor_line,
        }))
    }

    pub async fn get_session_file_diff_stats(
        &self,
        session_id: &str,
        file_path: &str,
    ) -> SnapshotResult<crate::service::snapshot::types::SessionFileDiffStats> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);
        snapshot_service
            .get_session_file_diff_stats(session_id, file_path)
            .await
    }

    pub async fn get_operation_summary(
        &self,
        session_id: &str,
        operation_id: &str,
    ) -> SnapshotResult<serde_json::Value> {
        let snapshot_service = self.snapshot_service.read().await;
        let op = snapshot_service
            .get_operation_summary(session_id, operation_id)
            .await?;
        Ok(serde_json::json!({
            "operation_id": op.operation_id,
            "session_id": op.session_id,
            "turn_index": op.turn_index,
            "seq_in_turn": op.seq_in_turn,
            "file_path": op.file_path.to_string_lossy(),
            "operation_type": format!("{:?}", op.operation_type),
            "tool_name": op.tool_context.tool_name,
            "lines_added": op.diff_summary.lines_added,
            "lines_removed": op.diff_summary.lines_removed,
        }))
    }

    pub async fn get_session(
        &self,
        session_id: &str,
    ) -> SnapshotResult<crate::service::snapshot::types::SessionInfo> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.get_session(session_id).await
    }

    /// Returns session statistics.
    pub async fn get_session_stats(&self, session_id: &str) -> SnapshotResult<serde_json::Value> {
        let snapshot_service = self.snapshot_service.read().await;
        let stats = snapshot_service.get_session_stats(session_id).await?;

        serde_json::to_value(stats).map_err(|e| {
            SnapshotError::ConfigError(format!("Failed to serialize statistics: {}", e))
        })
    }

    /// Returns system statistics.
    pub async fn get_system_stats(&self) -> SnapshotResult<serde_json::Value> {
        let snapshot_service = self.snapshot_service.read().await;
        let stats = snapshot_service.get_system_stats().await?;

        serde_json::to_value(stats).map_err(|e| {
            SnapshotError::ConfigError(format!("Failed to serialize system statistics: {}", e))
        })
    }

    pub async fn list_sessions(&self) -> SnapshotResult<Vec<String>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.list_sessions().await
    }

    /// Tries to acquire a file lock.
    pub async fn try_acquire_file_lock(
        &self,
        session_id: &str,
        file_path: &str,
        tool_name: &str,
    ) -> SnapshotResult<bool> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);
        snapshot_service
            .try_acquire_file_lock(session_id, file_path, tool_name)
            .await
    }

    /// Releases a file lock.
    pub async fn release_file_lock(&self, session_id: &str, file_path: &str) -> SnapshotResult<()> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);
        snapshot_service
            .release_file_lock(session_id, file_path)
            .await
    }

    /// Returns file lock status.
    pub async fn get_file_lock_status(&self, file_path: &str) -> SnapshotResult<serde_json::Value> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);

        let lock_status = snapshot_service.get_file_lock_status(file_path).await?;
        Ok(serde_json::json!({
            "locked": lock_status.is_some(),
            "lock_info": lock_status
        }))
    }

    /// Detects file conflicts.
    pub async fn detect_file_conflict(
        &self,
        session_id: &str,
        file_path: &str,
        tool_name: &str,
    ) -> SnapshotResult<serde_json::Value> {
        let snapshot_service = self.snapshot_service.read().await;
        let file_path = std::path::Path::new(file_path);

        let conflict = snapshot_service
            .detect_file_conflict(session_id, file_path, tool_name)
            .await?;

        Ok(serde_json::json!({
            "has_conflict": conflict.is_some(),
            "conflict_info": conflict
        }))
    }

    /// Checks Git isolation status.
    pub async fn check_git_isolation(&self) -> SnapshotResult<bool> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.check_git_isolation().await
    }

    /// Returns the change history for a file.
    pub async fn get_file_change_history(
        &self,
        file_path: &std::path::Path,
    ) -> SnapshotResult<Vec<crate::service::snapshot::snapshot_core::FileChangeEntry>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.get_file_change_history(file_path).await
    }

    /// Returns the list of all modified files.
    pub async fn get_all_modified_files(&self) -> SnapshotResult<Vec<PathBuf>> {
        let snapshot_service = self.snapshot_service.read().await;
        snapshot_service.get_all_modified_files().await
    }

    /// Returns a reference to the snapshot service (for advanced operations).
    pub fn get_snapshot_service(&self) -> Arc<RwLock<SnapshotService>> {
        self.snapshot_service.clone()
    }
}

fn snapshot_managers() -> &'static StdRwLock<HashMap<PathBuf, Arc<SnapshotManager>>> {
    static SNAPSHOT_MANAGERS: OnceLock<StdRwLock<HashMap<PathBuf, Arc<SnapshotManager>>>> =
        OnceLock::new();
    SNAPSHOT_MANAGERS.get_or_init(|| StdRwLock::new(HashMap::new()))
}

/// Ensures the registry always exposes the same tool implementation that will be
/// executed at runtime. File-modifying tools are wrapped once at registration time
/// so tool definitions, permission checks, and execution all share one source of truth.
pub fn wrap_tool_for_snapshot_tracking(tool: Arc<dyn Tool>) -> Arc<dyn Tool> {
    if WrappedTool::is_file_modification_tool_name(tool.name()) {
        Arc::new(WrappedTool::new(tool))
    } else {
        tool
    }
}

/// Compatibility helper that returns a fresh snapshot-aware tool list.
pub fn get_snapshot_wrapped_tools() -> Vec<Arc<dyn Tool>> {
    ToolRegistry::new().get_all_tools()
}

/// Wrapped tool
///
/// Wraps file-modification tools with snapshot functionality.
struct WrappedTool {
    original_tool: Arc<dyn Tool>,
}

impl WrappedTool {
    fn new(original_tool: Arc<dyn Tool>) -> Self {
        Self { original_tool }
    }

    fn is_file_modification_tool_name(tool_name: &str) -> bool {
        [
            "Write",
            "Edit",
            "Delete",
            "write_file",
            "edit_file",
            "create_file",
            "delete_file",
            "rename_file",
            "move_file",
            "search_replace",
        ]
        .contains(&tool_name)
    }
}

#[async_trait]
impl Tool for WrappedTool {
    fn name(&self) -> &str {
        self.original_tool.name()
    }

    async fn description(&self) -> crate::util::errors::BitFunResult<String> {
        Ok(self.original_tool.description().await?)
    }

    fn input_schema(&self) -> Value {
        self.original_tool.input_schema()
    }

    async fn input_schema_for_model(&self) -> Value {
        self.original_tool.input_schema_for_model().await
    }

    async fn input_schema_for_model_with_context(
        &self,
        context: Option<&crate::agentic::tools::framework::ToolUseContext>,
    ) -> Value {
        self.original_tool
            .input_schema_for_model_with_context(context)
            .await
    }

    fn input_json_schema(&self) -> Option<Value> {
        self.original_tool.input_json_schema()
    }

    fn user_facing_name(&self) -> String {
        self.original_tool.user_facing_name().to_string()
    }

    async fn is_enabled(&self) -> bool {
        self.original_tool.is_enabled().await
    }

    fn is_readonly(&self) -> bool {
        self.original_tool.is_readonly()
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        self.original_tool.is_concurrency_safe(input)
    }

    fn needs_permissions(&self, input: Option<&Value>) -> bool {
        self.original_tool.needs_permissions(input)
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> crate::agentic::tools::framework::ValidationResult {
        let original_validation = self.original_tool.validate_input(input, context).await;

        if !original_validation.result {
            return original_validation;
        }

        original_validation
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        let original_render = self.original_tool.render_result_for_assistant(output);
        format!(
            "{}\n\nModification recorded to snapshot system, can be reviewed and managed in the review panel.",
            original_render
        )
    }

    fn render_tool_use_message(
        &self,
        input: &Value,
        options: &crate::agentic::tools::framework::ToolRenderOptions,
    ) -> String {
        let original_message = self.original_tool.render_tool_use_message(input, options);
        original_message.to_string()
    }

    fn render_tool_use_rejected_message(&self) -> String {
        self.original_tool
            .render_tool_use_rejected_message()
            .to_string()
    }

    fn render_tool_result_message(&self, output: &Value) -> String {
        let original_message = self.original_tool.render_tool_result_message(output);
        format!("{} recorded to snapshot", original_message)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> crate::util::errors::BitFunResult<Vec<ToolResult>> {
        if Self::is_file_modification_tool_name(self.name()) {
            debug!(
                "Intercepting file modification tool: tool_name={}",
                self.name()
            );

            match self.handle_file_modification_internal(input, context).await {
                Ok(results) => {
                    return Ok(results);
                }
                Err(e) => {
                    warn!("Snapshot processing failed, falling back to original tool: tool_name={} error={}", self.name(), e);
                    let error_msg = format!("{}", e);
                    if error_msg.contains("file not found") || error_msg.contains("not exist") {
                        warn!("Possible workspace path mismatch, check snapshot workspace and global workspace consistency");
                    }
                }
            }
        }

        self.original_tool.call(input, context).await
    }
}

impl WrappedTool {
    /// Handles a file-modification tool.
    async fn handle_file_modification_internal(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> crate::util::errors::BitFunResult<Vec<ToolResult>> {
        let session_id = context.session_id.clone().ok_or_else(|| {
            crate::util::errors::BitFunError::Tool(
                "session_id is required in ToolUseContext".to_string(),
            )
        })?;

        let raw_path = match self.extract_file_path_simple(input) {
            Ok(path) => path,
            Err(e) => return Err(crate::util::errors::BitFunError::Tool(e.to_string())),
        };

        let snapshot_workspace = context.workspace_root().map(PathBuf::from).ok_or_else(|| {
            crate::util::errors::BitFunError::Tool(
                "workspace is required in ToolUseContext for snapshot tracking".to_string(),
            )
        })?;

        // Remote workspaces: skip snapshot tracking, just execute the tool directly
        if is_remote_path(snapshot_workspace.to_string_lossy().as_ref()).await {
            debug!(
                "Skipping snapshot for remote workspace: workspace={}",
                snapshot_workspace.display()
            );
            return self.original_tool.call(input, context).await;
        }

        let snapshot_manager = get_or_create_snapshot_manager(snapshot_workspace.clone(), None)
            .await
            .map_err(|e| crate::util::errors::BitFunError::Tool(e.to_string()))?;

        let file_path = if raw_path.is_absolute() {
            raw_path.clone()
        } else {
            snapshot_workspace.join(&raw_path)
        };

        let is_create_tool = matches!(self.name(), "Write" | "write_file" | "create_file");

        // For local workspaces only: verify the file exists before attempting to snapshot
        if !is_remote_path(file_path.to_string_lossy().as_ref()).await
            && !file_path.exists()
            && !is_create_tool
        {
            error!(
                "File not found: file_path={} raw_path={} snapshot_workspace={}",
                file_path.display(),
                raw_path.display(),
                snapshot_workspace.display()
            );

            return Err(crate::util::errors::BitFunError::Tool(format!(
                "File not found: {} (Snapshot workspace: {})",
                file_path.display(),
                snapshot_workspace.display()
            )));
        }

        if is_create_tool && !file_path.exists() {
            debug!("Creating new file: file_path={}", file_path.display());
        }

        let file_existed_before = file_path.exists();
        let operation_type = self.get_operation_type_internal(file_existed_before);
        let turn_index = self.extract_turn_index(context);

        let snapshot_service = snapshot_manager.get_snapshot_service();
        let snapshot_service = snapshot_service.read().await;
        let intercept_started_at = std::time::Instant::now();
        let operation_id = snapshot_service
            .intercept_file_modification(
                &session_id,
                turn_index,
                self.name(),
                input.clone(),
                &file_path,
                operation_type,
                context.tool_call_id.clone(),
            )
            .await
            .map_err(|e| crate::util::errors::BitFunError::Tool(e.to_string()))?;
        let intercept_ms = crate::util::elapsed_ms_u64(intercept_started_at);

        debug!(
            "Recorded file modification operation: operation_id={}",
            operation_id
        );

        let start_time = std::time::Instant::now();
        let results = self.original_tool.call(input, context).await?;
        let tool_call_ms = crate::util::elapsed_ms_u64(start_time);

        let complete_started_at = std::time::Instant::now();
        snapshot_service
            .complete_file_modification(&session_id, &operation_id, tool_call_ms)
            .await
            .map_err(|e| crate::util::errors::BitFunError::Tool(e.to_string()))?;
        let complete_ms = crate::util::elapsed_ms_u64(complete_started_at);
        let total_ms = intercept_ms
            .saturating_add(tool_call_ms)
            .saturating_add(complete_ms);

        debug!(
            "File modification tool completed: tool_name={}, operation_id={}, total_ms={}, intercept_ms={}, tool_call_ms={}, complete_ms={}, file_path={}",
            self.name(),
            operation_id,
            total_ms,
            intercept_ms,
            tool_call_ms,
            complete_ms,
            file_path.display()
        );
        Ok(results)
    }

    /// Extracts the turn index.
    fn extract_turn_index(&self, context: &ToolUseContext) -> usize {
        context
            .custom_data
            .get("turn_index")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(0)
    }

    /// Simplified file path extraction.
    fn extract_file_path_simple(&self, input: &Value) -> SnapshotResult<PathBuf> {
        let possible_fields = ["file_path", "path", "target_file", "filename"];

        for field in &possible_fields {
            if let Some(path_value) = input.get(field) {
                if let Some(path_str) = path_value.as_str() {
                    return Ok(PathBuf::from(path_str));
                }
            }
        }

        Err(SnapshotError::ConfigError(
            "Failed to extract file path from tool input".to_string(),
        ))
    }

    /// Returns the operation type.
    fn get_operation_type_internal(&self, file_existed_before: bool) -> OperationType {
        match self.name() {
            "Write" | "write_file" => {
                if file_existed_before {
                    OperationType::Modify
                } else {
                    OperationType::Create
                }
            }
            "create_file" => OperationType::Create,
            "delete_file" | "Delete" => OperationType::Delete,
            "rename_file" | "move_file" => OperationType::Rename,
            _ => OperationType::Modify,
        }
    }
}

pub async fn get_or_create_snapshot_manager(
    workspace_dir: PathBuf,
    config: Option<SnapshotConfig>,
) -> SnapshotResult<Arc<SnapshotManager>> {
    if let Some(existing) = get_snapshot_manager_for_workspace(&workspace_dir) {
        return Ok(existing);
    }

    let manager = Arc::new(SnapshotManager::new(workspace_dir.clone(), config).await?);
    {
        let mut managers = snapshot_managers().write().map_err(|_| {
            SnapshotError::ConfigError("Snapshot manager store lock poisoned".to_string())
        })?;
        if let Some(existing) = managers.get(&workspace_dir) {
            return Ok(existing.clone());
        }
        managers.insert(workspace_dir, manager.clone());
    }

    Ok(manager)
}

pub fn get_snapshot_manager_for_workspace(workspace_dir: &Path) -> Option<Arc<SnapshotManager>> {
    snapshot_managers()
        .read()
        .ok()
        .and_then(|managers| managers.get(workspace_dir).cloned())
}

pub fn ensure_snapshot_manager_for_workspace(
    workspace_dir: &Path,
) -> SnapshotResult<Arc<SnapshotManager>> {
    get_snapshot_manager_for_workspace(workspace_dir).ok_or_else(|| {
        SnapshotError::ConfigError(format!(
            "Snapshot manager not initialized for workspace: {}",
            workspace_dir.display()
        ))
    })
}

/// Initializes a snapshot manager for the provided workspace.
pub async fn initialize_snapshot_manager_for_workspace(
    workspace_dir: PathBuf,
    config: Option<SnapshotConfig>,
) -> SnapshotResult<()> {
    get_or_create_snapshot_manager(workspace_dir, config).await?;
    info!("Snapshot manager initialized for workspace");
    Ok(())
}

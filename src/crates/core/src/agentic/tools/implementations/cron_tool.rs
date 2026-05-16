use super::util::normalize_path;
use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::workspace_paths::posix_style_path_is_absolute;
use crate::service::{
    cron::{
        CreateCronJobRequest, CronJob, CronJobPayload, CronJobRunStatus, CronSchedule,
        UpdateCronJobRequest,
    },
    get_global_cron_service,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use chrono::{DateTime, Local, SecondsFormat, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

const DEFAULT_JOB_NAME: &str = "Cron job";

/// Cron tool - manage scheduled jobs for agent sessions.
pub struct CronTool;

impl CronTool {
    pub fn new() -> Self {
        Self
    }

    fn validate_session_id(session_id: &str) -> Result<(), String> {
        if session_id.is_empty() {
            return Err("session_id cannot be empty".to_string());
        }
        if session_id == "." || session_id == ".." {
            return Err("session_id cannot be '.' or '..'".to_string());
        }
        if session_id.contains('/') || session_id.contains('\\') {
            return Err("session_id cannot contain path separators".to_string());
        }
        if !session_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            return Err(
                "session_id can only contain ASCII letters, numbers, '-' and '_'".to_string(),
            );
        }
        Ok(())
    }

    fn validate_job_id(job_id: &str) -> Result<(), String> {
        if job_id.trim().is_empty() {
            return Err("job_id cannot be empty".to_string());
        }
        Ok(())
    }

    fn validate_workspace_format(
        workspace: &str,
        context: Option<&ToolUseContext>,
    ) -> Result<(), String> {
        if workspace.trim().is_empty() {
            return Err("workspace cannot be empty".to_string());
        }
        let is_remote = context.map(|c| c.is_remote()).unwrap_or(false);
        if is_remote {
            if !posix_style_path_is_absolute(workspace.trim()) {
                return Err(
                    "workspace must be an absolute POSIX path on the remote host".to_string(),
                );
            }
            return Ok(());
        }
        if !Path::new(workspace.trim()).is_absolute() {
            return Err("workspace must be an absolute path".to_string());
        }
        Ok(())
    }

    fn resolve_workspace(
        &self,
        workspace: &str,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Self::validate_workspace_format(workspace, context).map_err(BitFunError::tool)?;

        if let Some(ctx) = context {
            if ctx.is_remote() {
                return ctx.resolve_workspace_tool_path(workspace.trim());
            }
        }

        let resolved = normalize_path(workspace.trim());
        let path = Path::new(&resolved);
        if !path.exists() {
            return Err(BitFunError::tool(format!(
                "Workspace does not exist: {}",
                resolved
            )));
        }
        if !path.is_dir() {
            return Err(BitFunError::tool(format!(
                "Workspace is not a directory: {}",
                resolved
            )));
        }
        Ok(resolved)
    }

    fn resolve_workspace_from_context(&self, context: &ToolUseContext) -> BitFunResult<String> {
        let workspace = context.workspace_root().ok_or_else(|| {
            BitFunError::tool(
                "workspace is required when the current workspace is unavailable".to_string(),
            )
        })?;
        self.resolve_workspace(workspace.to_string_lossy().as_ref(), Some(context))
    }

    fn resolve_effective_workspace(
        &self,
        workspace: Option<&str>,
        context: &ToolUseContext,
    ) -> BitFunResult<String> {
        match workspace {
            Some(workspace) => self.resolve_workspace(workspace, Some(context)),
            None => self.resolve_workspace_from_context(context),
        }
    }

    fn resolve_effective_session_id(
        &self,
        session_id: Option<&str>,
        context: &ToolUseContext,
    ) -> BitFunResult<String> {
        let resolved = match session_id {
            Some(session_id) => session_id.trim().to_string(),
            None => context
                .session_id
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_string(),
        };

        Self::validate_session_id(&resolved).map_err(BitFunError::tool)?;
        Ok(resolved)
    }

    async fn ensure_session_exists(&self, workspace: &str, session_id: &str) -> BitFunResult<()> {
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let sessions = coordinator.list_sessions(Path::new(workspace)).await?;
        if sessions
            .iter()
            .any(|session| session.session_id == session_id)
        {
            return Ok(());
        }

        Err(BitFunError::NotFound(format!(
            "Session '{}' not found in workspace '{}'",
            session_id, workspace
        )))
    }

    fn normalize_add_name(name: Option<String>) -> String {
        match name {
            Some(name) if !name.trim().is_empty() => name.trim().to_string(),
            _ => DEFAULT_JOB_NAME.to_string(),
        }
    }

    fn normalize_optional_name(name: Option<String>) -> BitFunResult<Option<String>> {
        match name {
            Some(name) if name.trim().is_empty() => Err(BitFunError::tool(
                "patch.name cannot be empty when provided".to_string(),
            )),
            Some(name) => Ok(Some(name.trim().to_string())),
            None => Ok(None),
        }
    }

    fn validate_payload(payload: &str, field_name: &str) -> BitFunResult<()> {
        if payload.trim().is_empty() {
            return Err(BitFunError::tool(format!(
                "{}.payload must not be empty",
                field_name
            )));
        }
        Ok(())
    }

    fn into_service_payload(payload: String) -> CronJobPayload {
        CronJobPayload { text: payload }
    }

    fn parse_iso_timestamp_ms(value: &str, field_name: &str) -> BitFunResult<i64> {
        let parsed = DateTime::parse_from_rfc3339(value).map_err(|error| {
            BitFunError::tool(format!(
                "{} must be a valid ISO-8601 timestamp: {}",
                field_name, error
            ))
        })?;
        Ok(parsed.timestamp_millis())
    }

    fn format_iso_timestamp_local(timestamp_ms: i64, field_name: &str) -> BitFunResult<String> {
        let datetime = Local
            .timestamp_millis_opt(timestamp_ms)
            .single()
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "{} timestamp is out of range: {}",
                    field_name, timestamp_ms
                ))
            })?;
        Ok(datetime.to_rfc3339_opts(SecondsFormat::Secs, false))
    }

    fn every_ms_to_seconds(every_ms: u64) -> u64 {
        every_ms.div_ceil(1_000)
    }

    fn seconds_to_every_ms(seconds: u64, field_name: &str) -> BitFunResult<u64> {
        if seconds == 0 {
            return Err(BitFunError::tool(format!(
                "{}.every must be greater than 0 seconds",
                field_name
            )));
        }

        seconds
            .checked_mul(1_000)
            .ok_or_else(|| BitFunError::tool(format!("{}.every is too large", field_name)))
    }

    fn serialize_job(job: &CronJob) -> BitFunResult<Value> {
        serde_json::to_value(CronToolJobOutput::try_from(job)?)
            .map_err(|err| BitFunError::serialization(err.to_string()))
    }

    fn serialize_jobs(jobs: &[CronJob]) -> BitFunResult<Vec<Value>> {
        jobs.iter().map(Self::serialize_job).collect()
    }

    fn escape_markdown_table_cell(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('|', "\\|")
            .replace('\n', "<br>")
    }

    fn schedule_summary(schedule: &CronSchedule) -> String {
        match schedule {
            CronSchedule::At { at } => format!("at {}", at),
            CronSchedule::Every {
                every_ms,
                anchor_ms,
            } => match anchor_ms {
                Some(anchor_ms) => match Self::format_iso_timestamp_local(*anchor_ms, "anchor") {
                    Ok(anchor) => format!(
                        "every {}s from {}",
                        Self::every_ms_to_seconds(*every_ms),
                        anchor
                    ),
                    Err(_) => format!("every {}s", Self::every_ms_to_seconds(*every_ms)),
                },
                None => format!("every {}s", Self::every_ms_to_seconds(*every_ms)),
            },
            CronSchedule::Cron { expr, tz } => match tz.as_deref() {
                Some(tz) if !tz.trim().is_empty() => format!("cron {} ({})", expr, tz),
                _ => format!("cron {} (local timezone)", expr),
            },
        }
    }

    fn build_list_result_for_assistant(
        &self,
        workspace: &str,
        session_id: &str,
        jobs: &[CronJob],
    ) -> String {
        if jobs.is_empty() {
            return format!(
                "No scheduled jobs found for session '{}' in workspace '{}'.",
                session_id, workspace
            );
        }

        let mut lines = vec![format!(
            "Found {} scheduled job(s) for session '{}' in workspace '{}'.",
            jobs.len(),
            session_id,
            workspace,
        )];
        lines.push(String::new());
        lines.push("| job_id | name | enabled | schedule |".to_string());
        lines.push("| --- | --- | --- | --- |".to_string());
        for job in jobs {
            lines.push(format!(
                "| {} | {} | {} | {} |",
                Self::escape_markdown_table_cell(&job.id),
                Self::escape_markdown_table_cell(&job.name),
                if job.enabled { "true" } else { "false" },
                Self::escape_markdown_table_cell(&Self::schedule_summary(&job.schedule)),
            ));
        }
        lines.join("\n")
    }
}

impl Default for CronTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CronAction {
    GetTime,
    List,
    Add,
    Update,
    Remove,
    Run,
}

#[derive(Debug, Clone, Deserialize)]
struct CronToolJobInput {
    name: Option<String>,
    schedule: CronToolScheduleInput,
    payload: String,
    enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CronToolJobPatchInput {
    name: Option<String>,
    schedule: Option<CronToolScheduleInput>,
    payload: Option<String>,
    enabled: Option<bool>,
}

impl CronToolJobPatchInput {
    fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.schedule.is_none()
            && self.payload.is_none()
            && self.enabled.is_none()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CronToolInput {
    action: CronAction,
    workspace: Option<String>,
    session_id: Option<String>,
    job: Option<CronToolJobInput>,
    patch: Option<CronToolJobPatchInput>,
    job_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum CronToolScheduleInput {
    At { at: String },
    Every { every: u64, anchor: Option<String> },
    Cron { expr: String, tz: Option<String> },
}

impl CronToolScheduleInput {
    fn to_service_schedule(&self, field_name: &str) -> BitFunResult<CronSchedule> {
        match self {
            Self::At { at } => {
                let at = at.trim();
                if at.is_empty() {
                    return Err(BitFunError::tool(format!(
                        "{}.at cannot be empty",
                        field_name
                    )));
                }
                CronTool::parse_iso_timestamp_ms(at, &format!("{}.at", field_name))?;
                Ok(CronSchedule::At { at: at.to_string() })
            }
            Self::Every { every, anchor } => {
                let anchor_ms = match anchor.as_deref() {
                    Some(anchor) if anchor.trim().is_empty() => {
                        return Err(BitFunError::tool(format!(
                            "{}.anchor cannot be empty when provided",
                            field_name
                        )));
                    }
                    Some(anchor) => Some(CronTool::parse_iso_timestamp_ms(
                        anchor.trim(),
                        &format!("{}.anchor", field_name),
                    )?),
                    None => None,
                };

                Ok(CronSchedule::Every {
                    every_ms: CronTool::seconds_to_every_ms(*every, field_name)?,
                    anchor_ms,
                })
            }
            Self::Cron { expr, tz } => {
                let expr = expr.trim();
                if expr.is_empty() {
                    return Err(BitFunError::tool(format!(
                        "{}.expr cannot be empty",
                        field_name
                    )));
                }

                Ok(CronSchedule::Cron {
                    expr: expr.to_string(),
                    tz: tz
                        .as_ref()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty()),
                })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum CronToolScheduleOutput {
    At {
        at: String,
    },
    Every {
        every: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        anchor: Option<String>,
    },
    Cron {
        expr: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tz: Option<String>,
    },
}

impl TryFrom<&CronSchedule> for CronToolScheduleOutput {
    type Error = BitFunError;

    fn try_from(schedule: &CronSchedule) -> BitFunResult<Self> {
        match schedule {
            CronSchedule::At { at } => Ok(Self::At { at: at.clone() }),
            CronSchedule::Every {
                every_ms,
                anchor_ms,
            } => Ok(Self::Every {
                every: CronTool::every_ms_to_seconds(*every_ms),
                anchor: anchor_ms
                    .map(|value| CronTool::format_iso_timestamp_local(value, "anchor"))
                    .transpose()?,
            }),
            CronSchedule::Cron { expr, tz } => Ok(Self::Cron {
                expr: expr.clone(),
                tz: tz.clone(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CronToolJobStateOutput {
    next_run_at_ms: Option<i64>,
    pending_trigger_at_ms: Option<i64>,
    retry_at_ms: Option<i64>,
    last_trigger_at_ms: Option<i64>,
    last_enqueued_at_ms: Option<i64>,
    last_run_started_at_ms: Option<i64>,
    last_run_finished_at_ms: Option<i64>,
    last_duration_ms: Option<u64>,
    last_run_status: Option<CronJobRunStatus>,
    last_error: Option<String>,
    active_turn_id: Option<String>,
    consecutive_failures: u32,
    coalesced_run_count: u32,
}

impl From<&crate::service::cron::CronJobState> for CronToolJobStateOutput {
    fn from(state: &crate::service::cron::CronJobState) -> Self {
        Self {
            next_run_at_ms: state.next_run_at_ms,
            pending_trigger_at_ms: state.pending_trigger_at_ms,
            retry_at_ms: state.retry_at_ms,
            last_trigger_at_ms: state.last_trigger_at_ms,
            last_enqueued_at_ms: state.last_enqueued_at_ms,
            last_run_started_at_ms: state.last_run_started_at_ms,
            last_run_finished_at_ms: state.last_run_finished_at_ms,
            last_duration_ms: state.last_duration_ms,
            last_run_status: state.last_run_status,
            last_error: state.last_error.clone(),
            active_turn_id: state.active_turn_id.clone(),
            consecutive_failures: state.consecutive_failures,
            coalesced_run_count: state.coalesced_run_count,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CronToolJobOutput {
    id: String,
    name: String,
    schedule: CronToolScheduleOutput,
    payload: String,
    enabled: bool,
    session_id: String,
    workspace_path: String,
    created_at_ms: i64,
    config_updated_at_ms: i64,
    updated_at_ms: i64,
    state: CronToolJobStateOutput,
}

impl TryFrom<&CronJob> for CronToolJobOutput {
    type Error = BitFunError;

    fn try_from(job: &CronJob) -> BitFunResult<Self> {
        Ok(Self {
            id: job.id.clone(),
            name: job.name.clone(),
            schedule: CronToolScheduleOutput::try_from(&job.schedule)?,
            payload: job.payload.text.clone(),
            enabled: job.enabled,
            session_id: job.session_id.clone(),
            workspace_path: job.workspace_path.clone(),
            created_at_ms: job.created_at_ms,
            config_updated_at_ms: job.config_updated_at_ms,
            updated_at_ms: job.updated_at_ms,
            state: CronToolJobStateOutput::from(&job.state),
        })
    }
}

#[async_trait]
impl Tool for CronTool {
    fn name(&self) -> &str {
        "Cron"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Manage scheduled jobs for agent sessions.

Defaults:
- "workspace": defaults to the current workspace.
- "session_id": defaults to the current session for "list" and "add".

Actions:
- "get_time": Return the current local time including timezone information.
- "list": List all jobs for the effective workspace + session scope.
- "add": Create a job. Requires "job". When "job.name" is omitted, uses "Cron job".
- "update": Update a job. Requires "job_id" and "patch".
- "remove": Delete a job. Requires "job_id".
- "run": Trigger a job immediately. Requires "job_id".

Job schema for "add":
{
  "name": "string (optional)",
  "schedule": { ... },
  "payload": "string (sent to the target session as a user message)",
  "enabled": true | false
}

Schedule schema:
- One-shot at absolute time:
  { "kind": "at", "at": "2026-03-17T12:00:00+08:00" }
- Recurring interval:
  { "kind": "every", "every": 3600, "anchor": "2026-03-17T12:00:00+08:00" }
  - "every" is in seconds.
  - "anchor" is optional and uses the same ISO-8601 format as "at". Defaults to the current time.
- Cron expression:
  { "kind": "cron", "expr": "0 9 * * 1-5", "tz": "Asia/Shanghai" }
  - "tz" is optional. Defaults to the local timezone.

Patch schema for "update":
- Same fields as "job", but every field is optional."#
            .to_string())
    }

    fn short_description(&self) -> String {
        "Manage scheduled jobs for agent sessions.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Collapsed
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "workspace": {
                    "type": "string",
                    "description": "Optional absolute workspace path. Defaults to the current workspace."
                },
                "session_id": {
                    "type": "string",
                    "description": "Optional target session ID. Defaults to the current session for list/add."
                },
                "action": {
                    "type": "string",
                    "enum": ["get_time", "list", "add", "update", "remove", "run"],
                    "description": "Cron action to perform."
                },
                "job": {
                    "type": "object",
                    "description": "Required for add.",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Optional job name. Defaults to 'Cron job'."
                        },
                        "schedule": {
                            "type": "object",
                            "description": "Required schedule definition. Use { \"kind\": \"at\", \"at\": \"<ISO-8601>\" }, { \"kind\": \"every\", \"every\": <seconds>, \"anchor\": \"<optional ISO-8601>\" }, or { \"kind\": \"cron\", \"expr\": \"<cron-expression>\", \"tz\": \"<optional timezone>\" }. anchor defaults to the current time. tz defaults to the local timezone."
                        },
                        "payload": {
                            "type": "string",
                            "description": "Required execution payload text. It will be sent to the target session as a user message."
                        },
                        "enabled": {
                            "type": "boolean",
                            "description": "Optional enabled flag. Defaults to true."
                        }
                    },
                    "required": ["schedule", "payload"],
                    "additionalProperties": false
                },
                "patch": {
                    "type": "object",
                    "description": "Required for update. Same fields as job, but all optional.",
                    "properties": {
                        "name": {
                            "type": "string"
                        },
                        "schedule": {
                            "type": "object"
                        },
                        "payload": {
                            "type": "string",
                            "description": "Optional updated payload text. It will be sent to the target session as a user message."
                        },
                        "enabled": {
                            "type": "boolean"
                        }
                    },
                    "additionalProperties": false
                },
                "job_id": {
                    "type": "string",
                    "description": "Required for update, remove, and run."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        let Some(input) = input else {
            return false;
        };
        let Some(action) = input.get("action").and_then(|value| value.as_str()) else {
            return false;
        };
        matches!(action, "get_time" | "list")
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: CronToolInput = match serde_json::from_value(input.clone()) {
            Ok(value) => value,
            Err(err) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", err)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        if let Some(workspace) = parsed.workspace.as_deref() {
            if let Err(message) = Self::validate_workspace_format(workspace, context) {
                return ValidationResult {
                    result: false,
                    message: Some(message),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        if let Some(session_id) = parsed.session_id.as_deref() {
            if let Err(message) = Self::validate_session_id(session_id.trim()) {
                return ValidationResult {
                    result: false,
                    message: Some(message),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        match parsed.action {
            CronAction::GetTime => ValidationResult::default(),
            CronAction::List => {
                if parsed.session_id.is_none()
                    && context
                        .and_then(|tool_context| tool_context.session_id.as_deref())
                        .is_none()
                {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "session_id is required for list when the current session is unavailable"
                                .to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                if parsed.workspace.is_none()
                    && context
                        .and_then(|tool_context| tool_context.workspace_root())
                        .is_none()
                {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "workspace is required for list when the current workspace is unavailable"
                                .to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                ValidationResult::default()
            }
            CronAction::Add => {
                let Some(job) = parsed.job.as_ref() else {
                    return ValidationResult {
                        result: false,
                        message: Some("job is required for add".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                };

                if let Err(error) = Self::validate_payload(&job.payload, "job") {
                    return ValidationResult {
                        result: false,
                        message: Some(error.to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                if let Err(error) = job.schedule.to_service_schedule("job.schedule") {
                    return ValidationResult {
                        result: false,
                        message: Some(error.to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if parsed.session_id.is_none()
                    && context
                        .and_then(|tool_context| tool_context.session_id.as_deref())
                        .is_none()
                {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "session_id is required for add when the current session is unavailable"
                                .to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                if parsed.workspace.is_none()
                    && context
                        .and_then(|tool_context| tool_context.workspace_root())
                        .is_none()
                {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "workspace is required for add when the current workspace is unavailable"
                                .to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                ValidationResult::default()
            }
            CronAction::Update => {
                let Some(job_id) = parsed.job_id.as_deref() else {
                    return ValidationResult {
                        result: false,
                        message: Some("job_id is required for update".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                };
                if let Err(message) = Self::validate_job_id(job_id) {
                    return ValidationResult {
                        result: false,
                        message: Some(message),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                let Some(patch) = parsed.patch.as_ref() else {
                    return ValidationResult {
                        result: false,
                        message: Some("patch is required for update".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                };
                if patch.is_empty() {
                    return ValidationResult {
                        result: false,
                        message: Some("patch must include at least one field".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                if let Some(name) = patch.name.as_deref() {
                    if name.trim().is_empty() {
                        return ValidationResult {
                            result: false,
                            message: Some("patch.name cannot be empty when provided".to_string()),
                            error_code: Some(400),
                            meta: None,
                        };
                    }
                }
                if let Some(payload) = patch.payload.as_ref() {
                    if let Err(error) = Self::validate_payload(payload, "patch") {
                        return ValidationResult {
                            result: false,
                            message: Some(error.to_string()),
                            error_code: Some(400),
                            meta: None,
                        };
                    };
                }
                if let Some(schedule) = patch.schedule.as_ref() {
                    if let Err(error) = schedule.to_service_schedule("patch.schedule") {
                        return ValidationResult {
                            result: false,
                            message: Some(error.to_string()),
                            error_code: Some(400),
                            meta: None,
                        };
                    }
                }
                ValidationResult::default()
            }
            CronAction::Remove => {
                let Some(job_id) = parsed.job_id.as_deref() else {
                    return ValidationResult {
                        result: false,
                        message: Some("job_id is required for remove".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                };
                if let Err(message) = Self::validate_job_id(job_id) {
                    return ValidationResult {
                        result: false,
                        message: Some(message),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                ValidationResult::default()
            }
            CronAction::Run => {
                let Some(job_id) = parsed.job_id.as_deref() else {
                    return ValidationResult {
                        result: false,
                        message: Some("job_id is required for run".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                };
                if let Err(message) = Self::validate_job_id(job_id) {
                    return ValidationResult {
                        result: false,
                        message: Some(message),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                ValidationResult::default()
            }
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let action = input
            .get("action")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let job_id = input
            .get("job_id")
            .and_then(|value| value.as_str())
            .unwrap_or("auto");

        match action {
            "get_time" => "Get current ISO-8601 time".to_string(),
            "list" => "List scheduled jobs".to_string(),
            "add" => "Create scheduled job".to_string(),
            "update" => format!("Update scheduled job {}", job_id),
            "remove" => format!("Delete scheduled job {}", job_id),
            "run" => format!("Run scheduled job {}", job_id),
            _ => "Manage scheduled jobs".to_string(),
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: CronToolInput = serde_json::from_value(input.clone())
            .map_err(|err| BitFunError::tool(format!("Invalid input: {}", err)))?;

        match params.action {
            CronAction::GetTime => {
                let now = Local::now();
                let iso = now.to_rfc3339_opts(SecondsFormat::Secs, false);
                let result_for_assistant = format!("Current local time: {}", iso);

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "get_time",
                        "now": iso,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            CronAction::List => {
                let cron_service = get_global_cron_service()
                    .ok_or_else(|| BitFunError::tool("cron service not initialized".to_string()))?;
                let workspace =
                    self.resolve_effective_workspace(params.workspace.as_deref(), context)?;
                let session_id =
                    self.resolve_effective_session_id(params.session_id.as_deref(), context)?;
                let mut jobs = cron_service
                    .list_jobs_filtered(Some(&workspace), Some(&session_id))
                    .await;
                jobs.sort_by(|left, right| {
                    left.created_at_ms
                        .cmp(&right.created_at_ms)
                        .then_with(|| left.id.cmp(&right.id))
                });
                let serialized_jobs = Self::serialize_jobs(&jobs)?;

                let result_for_assistant =
                    self.build_list_result_for_assistant(&workspace, &session_id, &jobs);

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "list",
                        "workspace": workspace,
                        "session_id": session_id,
                        "count": jobs.len(),
                        "jobs": serialized_jobs,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            CronAction::Add => {
                let cron_service = get_global_cron_service()
                    .ok_or_else(|| BitFunError::tool("cron service not initialized".to_string()))?;
                let workspace =
                    self.resolve_effective_workspace(params.workspace.as_deref(), context)?;
                let session_id =
                    self.resolve_effective_session_id(params.session_id.as_deref(), context)?;
                let job = params
                    .job
                    .ok_or_else(|| BitFunError::tool("job is required for add".to_string()))?;

                Self::validate_payload(&job.payload, "job")?;
                self.ensure_session_exists(&workspace, &session_id).await?;

                let created = cron_service
                    .create_job(CreateCronJobRequest {
                        name: Self::normalize_add_name(job.name),
                        schedule: job.schedule.to_service_schedule("job.schedule")?,
                        payload: Self::into_service_payload(job.payload),
                        enabled: job.enabled.unwrap_or(true),
                        session_id: session_id.clone(),
                        workspace_path: workspace.clone(),
                    })
                    .await?;
                let serialized_job = Self::serialize_job(&created)?;
                let result_for_assistant = format!(
                    "Created scheduled job '{}' ({}) for session '{}' in workspace '{}'.",
                    created.name, created.id, created.session_id, created.workspace_path
                );

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "add",
                        "workspace": workspace,
                        "session_id": session_id,
                        "job": serialized_job,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            CronAction::Update => {
                let cron_service = get_global_cron_service()
                    .ok_or_else(|| BitFunError::tool("cron service not initialized".to_string()))?;
                let job_id = params.job_id.ok_or_else(|| {
                    BitFunError::tool("job_id is required for update".to_string())
                })?;
                Self::validate_job_id(&job_id).map_err(BitFunError::tool)?;
                let patch = params
                    .patch
                    .ok_or_else(|| BitFunError::tool("patch is required for update".to_string()))?;
                if patch.is_empty() {
                    return Err(BitFunError::tool(
                        "patch must include at least one field".to_string(),
                    ));
                }
                if let Some(payload) = patch.payload.as_ref() {
                    Self::validate_payload(payload, "patch")?;
                }

                let updated = cron_service
                    .update_job(
                        &job_id,
                        UpdateCronJobRequest {
                            name: Self::normalize_optional_name(patch.name)?,
                            schedule: patch
                                .schedule
                                .as_ref()
                                .map(|value| value.to_service_schedule("patch.schedule"))
                                .transpose()?,
                            payload: patch.payload.map(Self::into_service_payload),
                            enabled: patch.enabled,
                            session_id: None,
                            workspace_path: None,
                        },
                    )
                    .await?;
                let serialized_job = Self::serialize_job(&updated)?;
                let result_for_assistant =
                    format!("Updated scheduled job '{}' ({})", updated.name, updated.id);

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "update",
                        "job_id": job_id,
                        "job": serialized_job,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            CronAction::Remove => {
                let cron_service = get_global_cron_service()
                    .ok_or_else(|| BitFunError::tool("cron service not initialized".to_string()))?;
                let job_id = params.job_id.ok_or_else(|| {
                    BitFunError::tool("job_id is required for remove".to_string())
                })?;
                Self::validate_job_id(&job_id).map_err(BitFunError::tool)?;

                let deleted = cron_service.delete_job(&job_id).await?;
                let result_for_assistant = if deleted {
                    format!("Deleted scheduled job '{}'.", job_id)
                } else {
                    format!("No scheduled job found for '{}'.", job_id)
                };

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "remove",
                        "job_id": job_id,
                        "deleted": deleted,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            CronAction::Run => {
                let cron_service = get_global_cron_service()
                    .ok_or_else(|| BitFunError::tool("cron service not initialized".to_string()))?;
                let job_id = params
                    .job_id
                    .ok_or_else(|| BitFunError::tool("job_id is required for run".to_string()))?;
                Self::validate_job_id(&job_id).map_err(BitFunError::tool)?;

                let updated = cron_service.run_job_now(&job_id).await?;
                let serialized_job = Self::serialize_job(&updated)?;
                let result_for_assistant = format!(
                    "Triggered scheduled job '{}' ({}) for immediate execution.",
                    updated.name, updated.id
                );

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "run",
                        "job_id": job_id,
                        "job": serialized_job,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
        }
    }
}

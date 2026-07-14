/// Exec mode implementation
///
/// Single command execution mode (non-interactive).
/// Observes core events through an independent runtime broadcast subscription.
use anyhow::Result;
use clap::ValueEnum;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bitfun_events::AgenticEvent;
use tokio::time::{sleep, Instant};

use crate::agent::{core_adapter::CoreAgentAdapter, Agent};
use crate::config::CliConfig;
use crate::diagnostics::{emit_exit_diagnostic, ExitContext, ExitKind};
use crate::runtime::CliRuntimeContext;

const TOOL_START_INPUT_PREVIEW_CHARS: usize = 4_000;
const INTERRUPT_EVENT_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ExecOutputFormat {
    Text,
    Json,
    StreamJson,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ExecApprovalMode {
    #[default]
    Reject,
    Auto,
}

impl ExecApprovalMode {
    pub(crate) const fn rejects_confirmation(self) -> bool {
        matches!(self, Self::Reject)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct ExecTokenUsage {
    input_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_tokens: Option<usize>,
    total_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    cached_tokens: Option<usize>,
}

impl ExecTokenUsage {
    fn merge_round(&mut self, round: Self) {
        self.input_tokens = self.input_tokens.saturating_add(round.input_tokens);
        self.output_tokens = self
            .output_tokens
            .zip(round.output_tokens)
            .map(|(current, next)| current.saturating_add(next));
        self.total_tokens = self.total_tokens.saturating_add(round.total_tokens);
        self.cached_tokens = self
            .cached_tokens
            .zip(round.cached_tokens)
            .map(|(current, next)| current.saturating_add(next));
    }

    fn accumulate_event<'a>(
        aggregate: &mut Option<Self>,
        event: &'a AgenticEvent,
        expected_turn_id: &str,
    ) -> Option<&'a str> {
        let AgenticEvent::TokenUsageUpdated {
            turn_id,
            model_id,
            input_tokens,
            output_tokens,
            total_tokens,
            cached_tokens,
            ..
        } = event
        else {
            return None;
        };
        if turn_id != expected_turn_id {
            return None;
        }

        let round = Self {
            input_tokens: *input_tokens,
            output_tokens: *output_tokens,
            total_tokens: *total_tokens,
            cached_tokens: *cached_tokens,
        };
        if let Some(total) = aggregate.as_mut() {
            total.merge_round(round);
        } else {
            *aggregate = Some(round);
        }
        Some(model_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct ExecJsonResult {
    #[serde(rename = "type")]
    kind: &'static str,
    subtype: &'static str,
    is_error: bool,
    result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<ExecTokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    patch: Option<ExecPatchOutput>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct ExecPatchOutput {
    target: String,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    patch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExecTerminalStatus {
    Success,
    Error,
    Cancelled,
}

fn event_turn_id(event: &AgenticEvent) -> Option<&str> {
    match event {
        AgenticEvent::DialogTurnStarted { turn_id, .. }
        | AgenticEvent::DialogTurnCompleted { turn_id, .. }
        | AgenticEvent::DialogTurnCancelled { turn_id, .. }
        | AgenticEvent::DialogTurnFailed { turn_id, .. }
        | AgenticEvent::TokenUsageUpdated { turn_id, .. }
        | AgenticEvent::ContextCompressionStarted { turn_id, .. }
        | AgenticEvent::ContextCompressionCompleted { turn_id, .. }
        | AgenticEvent::ContextCompressionFailed { turn_id, .. }
        | AgenticEvent::ModelRoundStarted { turn_id, .. }
        | AgenticEvent::ModelRoundCompleted { turn_id, .. }
        | AgenticEvent::TextChunk { turn_id, .. }
        | AgenticEvent::ThinkingChunk { turn_id, .. }
        | AgenticEvent::ToolEvent { turn_id, .. }
        | AgenticEvent::DeepReviewQueueStateChanged { turn_id, .. }
        | AgenticEvent::UserSteeringInjected { turn_id, .. } => Some(turn_id),
        _ => None,
    }
}

fn event_belongs_to_exec_turn(event: &AgenticEvent, session_id: &str, turn_id: &str) -> bool {
    if event.session_id() != Some(session_id) {
        return false;
    }
    match event_turn_id(event) {
        Some(event_turn_id) => event_turn_id == turn_id,
        None => true,
    }
}

fn completed_turn_failure(
    success: Option<bool>,
    finish_reason: Option<&str>,
    has_final_response: Option<bool>,
) -> Option<String> {
    if success != Some(false) {
        return None;
    }

    let reason = finish_reason
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unsuccessful_completion");
    Some(match has_final_response {
        Some(false) => format!("Execution completed without a successful final response: {reason}"),
        _ => format!("Execution completed unsuccessfully: {reason}"),
    })
}

impl ExecJsonResult {
    pub(crate) fn success(
        session_id: impl Into<String>,
        turn_id: impl Into<String>,
        result: impl Into<String>,
        usage: Option<ExecTokenUsage>,
    ) -> Self {
        Self::new(
            "success",
            false,
            Some(session_id.into()),
            Some(turn_id.into()),
            result,
            usage,
        )
    }

    fn error(
        session_id: impl Into<String>,
        turn_id: impl Into<String>,
        result: impl Into<String>,
        usage: Option<ExecTokenUsage>,
    ) -> Self {
        Self::new(
            "error",
            true,
            Some(session_id.into()),
            Some(turn_id.into()),
            result,
            usage,
        )
    }

    fn session_error(session_id: impl Into<String>, result: impl Into<String>) -> Self {
        Self::new("error", true, Some(session_id.into()), None, result, None)
    }

    fn preflight_error(result: impl Into<String>) -> Self {
        Self::new("error", true, None, None, result, None)
    }

    fn cancelled(
        session_id: impl Into<String>,
        turn_id: impl Into<String>,
        result: impl Into<String>,
        usage: Option<ExecTokenUsage>,
    ) -> Self {
        Self::new(
            "cancelled",
            true,
            Some(session_id.into()),
            Some(turn_id.into()),
            result,
            usage,
        )
    }

    fn new(
        subtype: &'static str,
        is_error: bool,
        session_id: Option<String>,
        turn_id: Option<String>,
        result: impl Into<String>,
        usage: Option<ExecTokenUsage>,
    ) -> Self {
        Self {
            kind: "result",
            subtype,
            is_error,
            result: result.into(),
            session_id,
            turn_id,
            usage,
            patch: None,
        }
    }

    fn with_patch(mut self, patch: Option<ExecPatchOutput>) -> Self {
        self.patch = patch;
        self
    }
}

pub(crate) fn emit_preflight_json_error(
    output_format: ExecOutputFormat,
    error: &anyhow::Error,
) -> Result<()> {
    if output_format == ExecOutputFormat::Json {
        let result = ExecJsonResult::preflight_error(error.to_string());
        println!("{}", serde_json::to_string_pretty(&result)?);
    }
    Ok(())
}

pub(crate) fn serialize_stream_envelope(
    envelope: &bitfun_events::AgenticEventEnvelope,
) -> Result<String> {
    Ok(serde_json::to_string(envelope)?)
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ExecSessionOptions {
    pub resume: Option<String>,
    pub continue_last: bool,
    pub session_id: Option<String>,
    pub fork_session: bool,
}

pub(crate) struct ExecMode {
    #[allow(dead_code)]
    config: CliConfig,
    message: String,
    agent_type: String,
    agent: Arc<CoreAgentAdapter>,
    _runtime: Arc<CliRuntimeContext>,
    workspace_path: Option<PathBuf>,
    /// None: no patch output, Some("-"): output to stdout, Some(path): save to file
    output_patch: Option<String>,
    output_format: ExecOutputFormat,
    approval_mode: ExecApprovalMode,
    session_options: ExecSessionOptions,
}

impl ExecMode {
    pub(crate) fn new(
        config: CliConfig,
        message: String,
        agent_type: String,
        runtime: Arc<CliRuntimeContext>,
        workspace_path: Option<PathBuf>,
        output_patch: Option<String>,
        output_format: ExecOutputFormat,
        session_options: ExecSessionOptions,
    ) -> Self {
        let approval_mode = match runtime.approval_policy() {
            crate::runtime::approval::CliApprovalPolicy::Auto => ExecApprovalMode::Auto,
            crate::runtime::approval::CliApprovalPolicy::Ask
            | crate::runtime::approval::CliApprovalPolicy::Reject => ExecApprovalMode::Reject,
        };
        let agent = Arc::new(CoreAgentAdapter::new(
            runtime.as_ref(),
            workspace_path.clone(),
        ));

        Self {
            config,
            message,
            agent_type,
            agent,
            _runtime: runtime,
            workspace_path,
            output_patch,
            output_format,
            approval_mode,
            session_options,
        }
    }

    fn exit_context<'a>(
        &'a self,
        session_id: Option<&'a str>,
        turn_id: Option<&'a str>,
    ) -> ExitContext<'a> {
        ExitContext {
            session_id,
            turn_id,
            agent_type: Some(self.agent_type.as_str()),
            workspace: self.workspace_path.as_deref(),
        }
    }

    fn workspace_display(&self) -> String {
        self.workspace_path
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|_| ".".to_string())
            })
    }

    fn redact_large_inline_data(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                if map.remove("data_url").is_some() {
                    map.insert("has_data_url".to_string(), serde_json::json!(true));
                }
                for child in map.values_mut() {
                    Self::redact_large_inline_data(child);
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    Self::redact_large_inline_data(child);
                }
            }
            _ => {}
        }
    }

    fn tool_input_preview(params: &serde_json::Value) -> String {
        let mut redacted = params.clone();
        Self::redact_large_inline_data(&mut redacted);
        let raw =
            serde_json::to_string(&redacted).unwrap_or_else(|_| "<unserializable>".to_string());
        if raw.chars().count() <= TOOL_START_INPUT_PREVIEW_CHARS {
            return raw;
        }

        let preview: String = raw.chars().take(TOOL_START_INPUT_PREVIEW_CHARS).collect();
        format!("{preview}... [truncated]")
    }

    fn print_tool_start_details(&self, tool_name: &str, tool_id: &str, params: &serde_json::Value) {
        let started_at = chrono::Utc::now().to_rfc3339();
        let cwd = self.workspace_display();
        let input_preview = Self::tool_input_preview(params);

        self.print_text(|| {
            eprintln!("\nTool call: {}", tool_name);
            eprintln!("   Started at: {}", started_at);
            eprintln!("   Tool ID: {}", tool_id);
            eprintln!("   CWD: {}", cwd);
            eprintln!("   Input: {}", input_preview);
        });
    }

    fn get_git_diff(&self) -> Option<String> {
        let workspace = self.workspace_path.as_ref()?;
        Self::get_git_diff_for_workspace(workspace, self.output_patch.as_deref())
    }

    fn get_git_diff_for_workspace(
        workspace: &std::path::Path,
        output_target: Option<&str>,
    ) -> Option<String> {
        let repo_root_output = bitfun_core::util::process_manager::create_command("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(workspace)
            .output()
            .ok()?;
        if !repo_root_output.status.success() {
            eprintln!("Warning: Workspace is not a git repository, cannot generate patch");
            return None;
        }
        let repo_root = PathBuf::from(
            String::from_utf8_lossy(&repo_root_output.stdout)
                .trim()
                .to_string(),
        );

        let excluded_output = output_target
            .filter(|target| *target != "-")
            .and_then(|target| {
                let repo_root = std::fs::canonicalize(&repo_root).ok()?;
                let output_path =
                    Self::canonicalize_path_allowing_missing(std::path::Path::new(target))?;
                let relative = output_path.strip_prefix(repo_root).ok()?;
                (!relative.as_os_str().is_empty())
                    .then(|| relative.to_string_lossy().replace('\\', "/"))
            });

        let mut tracked_command = bitfun_core::util::process_manager::create_command("git");
        tracked_command
            .args(["diff", "--binary", "--no-color", "HEAD", "--", "."])
            .current_dir(&repo_root);
        if let Some(relative_path) = excluded_output.as_ref() {
            tracked_command.arg(format!(":(exclude,top,literal){relative_path}"));
        }
        let tracked = tracked_command.output().ok()?;
        if !tracked.status.success() {
            eprintln!("Warning: git diff execution failed");
            return None;
        }

        let untracked = bitfun_core::util::process_manager::create_command("git")
            .args(["ls-files", "--others", "--exclude-standard", "-z"])
            .current_dir(&repo_root)
            .output()
            .ok()?;
        if !untracked.status.success() {
            eprintln!("Warning: git untracked file discovery failed");
            return None;
        }

        let mut patch = String::from_utf8_lossy(&tracked.stdout).to_string();
        for relative_path in untracked.stdout.split(|byte| *byte == 0) {
            if relative_path.is_empty() {
                continue;
            }
            let relative_path = String::from_utf8_lossy(relative_path).to_string();
            if excluded_output.as_deref() == Some(relative_path.as_str()) {
                continue;
            }
            let untracked_patch = bitfun_core::util::process_manager::create_command("git")
                .args([
                    "diff",
                    "--no-index",
                    "--binary",
                    "--no-color",
                    "--",
                    "/dev/null",
                    &relative_path,
                ])
                .current_dir(&repo_root)
                .output()
                .ok()?;
            if !matches!(untracked_patch.status.code(), Some(0 | 1)) {
                eprintln!("Warning: failed to generate patch for untracked file {relative_path}");
                return None;
            }
            if !patch.is_empty() && !patch.ends_with('\n') {
                patch.push('\n');
            }
            patch.push_str(&String::from_utf8_lossy(&untracked_patch.stdout));
        }

        Some(patch)
    }

    fn canonicalize_path_allowing_missing(path: &std::path::Path) -> Option<PathBuf> {
        let absolute = std::path::absolute(path).ok()?;
        let mut existing = absolute.as_path();
        let mut missing = Vec::new();
        while !existing.exists() {
            missing.push(existing.file_name()?.to_os_string());
            existing = existing.parent()?;
        }

        let mut resolved = std::fs::canonicalize(existing).ok()?;
        for component in missing.into_iter().rev() {
            resolved.push(component);
        }
        Some(resolved)
    }

    pub(crate) async fn run(&mut self) -> Result<()> {
        tracing::info!(
            agent_type = %self.agent_type,
            message_len = self.message.len(),
            workspace = ?self.workspace_path,
            "Executing command"
        );

        let session_id = match self.prepare_session().await {
            Ok(session_id) => session_id,
            Err(error) => {
                emit_exit_diagnostic(
                    ExitKind::SessionCreateFailed,
                    &error.to_string(),
                    &self.exit_context(None, None),
                );
                if self.output_format == ExecOutputFormat::Json {
                    let result = ExecJsonResult::preflight_error(error.to_string());
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                return Err(error);
            }
        };
        tracing::info!(session_id = %session_id, "Session ready");
        let mut event_rx = self.agent.event_source().subscribe();

        self.print_text(|| {
            eprintln!("Executing: {}", self.message);
            eprintln!();
            eprintln!("Session: {}", session_id);
            eprintln!("Thinking...");
        });

        let turn_id = match self
            .agent
            .send_message(self.message.clone(), &self.agent_type)
            .await
        {
            Ok(turn_id) => turn_id,
            Err(error) => {
                emit_exit_diagnostic(
                    ExitKind::SendMessageFailed,
                    &error.to_string(),
                    &self.exit_context(Some(&session_id), None),
                );
                if self.output_format == ExecOutputFormat::Json {
                    let result = ExecJsonResult::session_error(&session_id, error.to_string());
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                return Err(error);
            }
        };
        tracing::info!(session_id = %session_id, turn_id = %turn_id, "Message sent");

        // Observe the shared Agentic event stream without consuming other clients' events.
        let mut total_tool_calls = 0usize;
        let mut subagent_parent_turns: HashMap<String, (String, String)> = HashMap::new();
        let mut terminal_outcome: Option<Result<()>> = None;
        let mut terminal_status: Option<ExecTerminalStatus> = None;
        let mut terminal_message: Option<String> = None;
        let mut assistant_text = String::new();
        let mut usage: Option<ExecTokenUsage> = None;

        'event_loop: loop {
            let envelope = tokio::select! {
                result = event_rx.recv() => match result {
                Ok(envelope) => envelope,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    let mut message = format!(
                        "Agentic event stream lost {skipped} events; execution state is no longer reliable"
                    );
                    if let Err(error) = self.agent.cancel_current_turn().await {
                        message.push_str(&format!("; failed to cancel active turn: {error}"));
                    }
                    emit_exit_diagnostic(
                        ExitKind::EventStreamFailed,
                        &message,
                        &self.exit_context(Some(&session_id), Some(&turn_id)),
                    );
                    terminal_status = Some(ExecTerminalStatus::Error);
                    terminal_message = Some(message.clone());
                    terminal_outcome = Some(Err(anyhow::anyhow!(message)));
                    break 'event_loop;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    let mut message =
                        "Agentic event stream closed before execution settled".to_string();
                    if let Err(error) = self.agent.cancel_current_turn().await {
                        message.push_str(&format!("; failed to cancel active turn: {error}"));
                    }
                    emit_exit_diagnostic(
                        ExitKind::EventStreamFailed,
                        &message,
                        &self.exit_context(Some(&session_id), Some(&turn_id)),
                    );
                    terminal_status = Some(ExecTerminalStatus::Error);
                    terminal_message = Some(message.clone());
                    terminal_outcome = Some(Err(anyhow::anyhow!(message)));
                    break 'event_loop;
                }
                },
                signal = tokio::signal::ctrl_c() => {
                    let interrupted = signal.is_ok();
                    let mut message = match signal {
                        Ok(()) => "Execution cancelled by interrupt".to_string(),
                        Err(error) => format!("Failed to listen for execution interrupt: {error}"),
                    };
                    if let Err(error) = self.agent.cancel_current_turn().await {
                        message.push_str(&format!("; failed to cancel active turn: {error}"));
                    }
                    if interrupted {
                        self.print_text(|| eprintln!("\nCancelling execution..."));
                        if let Err(error) = self
                            .drain_interrupted_turn_events(&mut event_rx, &session_id, &turn_id)
                            .await
                        {
                            message.push_str(&format!("; {error}"));
                        }
                    }
                    emit_exit_diagnostic(
                        if interrupted { ExitKind::Cancelled } else { ExitKind::ExecError },
                        &message,
                        &self.exit_context(Some(&session_id), Some(&turn_id)),
                    );
                    terminal_status = Some(if interrupted {
                        ExecTerminalStatus::Cancelled
                    } else {
                        ExecTerminalStatus::Error
                    });
                    terminal_message = Some(message.clone());
                    terminal_outcome = Some(Err(anyhow::anyhow!(message)));
                    break 'event_loop;
                }
            };
            let events = [envelope];

            for envelope in events {
                let event = &envelope.event;

                if let AgenticEvent::SubagentSessionLinked {
                    session_id: subagent_session_id,
                    subagent_dialog_turn_id,
                    parent_session_id,
                    parent_dialog_turn_id,
                    ..
                } = event
                {
                    if parent_session_id == &session_id && parent_dialog_turn_id == &turn_id {
                        subagent_parent_turns.insert(
                            subagent_session_id.clone(),
                            (parent_session_id.clone(), subagent_dialog_turn_id.clone()),
                        );
                        self.emit_stream_envelope(&envelope)?;
                    }
                    continue;
                }

                // Only process events for our session
                if event.session_id() != Some(&session_id) {
                    // Check if this is a subagent event whose parent is in our session
                    if let AgenticEvent::ToolEvent {
                        turn_id: event_turn_id,
                        tool_event,
                        ..
                    } = event
                    {
                        let parent_turn = event.session_id().and_then(|event_session_id| {
                            subagent_parent_turns.get(event_session_id)
                        });
                        if parent_turn.is_some_and(|(parent_session_id, subagent_turn_id)| {
                            parent_session_id == &session_id && subagent_turn_id == event_turn_id
                        }) {
                            self.emit_stream_envelope(&envelope)?;
                            use bitfun_events::ToolEventData;
                            match tool_event {
                                ToolEventData::Started {
                                    tool_name,
                                    tool_id,
                                    params,
                                    ..
                                } => {
                                    self.print_text(|| {
                                        let started_at = chrono::Utc::now().to_rfc3339();
                                        let input_preview = Self::tool_input_preview(params);
                                        eprintln!("   [subagent] {}", tool_name);
                                        eprintln!("      Started at: {}", started_at);
                                        eprintln!("      Tool ID: {}", tool_id);
                                        eprintln!("      CWD: {}", self.workspace_display());
                                        eprintln!("      Input: {}", input_preview);
                                    });
                                }
                                ToolEventData::Completed {
                                    tool_name,
                                    result_for_assistant,
                                    result,
                                    ..
                                } => {
                                    let summary = result_for_assistant
                                        .clone()
                                        .unwrap_or_else(|| result.to_string());
                                    self.print_text(|| {
                                        eprintln!(
                                            "   [subagent] {} completed: {}",
                                            tool_name, summary
                                        )
                                    });
                                }
                                ToolEventData::Failed {
                                    tool_name, error, ..
                                } => {
                                    self.print_text(|| {
                                        eprintln!("   [subagent] {} failed: {}", tool_name, error)
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                    continue;
                }

                if !event_belongs_to_exec_turn(event, &session_id, &turn_id) {
                    continue;
                }

                self.emit_stream_envelope(&envelope)?;

                if let Some(model_id) =
                    ExecTokenUsage::accumulate_event(&mut usage, event, &turn_id)
                {
                    self.record_resolved_model_id(&session_id, model_id).await;
                }

                match event {
                    AgenticEvent::ModelRoundStarted {
                        turn_id: event_turn_id,
                        model_id: Some(model_id),
                        ..
                    }
                    | AgenticEvent::ModelRoundCompleted {
                        turn_id: event_turn_id,
                        model_id: Some(model_id),
                        ..
                    } if event_turn_id == &turn_id => {
                        self.record_resolved_model_id(&session_id, model_id).await;
                    }

                    AgenticEvent::TextChunk {
                        turn_id: event_turn_id,
                        text,
                        ..
                    } if event_turn_id == &turn_id => {
                        assistant_text.push_str(text);
                        self.print_text(|| {
                            print!("{}", text);
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                        });
                    }

                    AgenticEvent::ThinkingChunk {
                        turn_id: event_turn_id,
                        content,
                        ..
                    } if event_turn_id == &turn_id => {
                        self.print_text(|| {
                            eprint!("\x1b[2m{}\x1b[0m", content);
                            std::io::stderr().flush().ok();
                        });
                    }

                    AgenticEvent::ToolEvent {
                        turn_id: event_turn_id,
                        tool_event,
                        ..
                    } if event_turn_id == &turn_id => {
                        use bitfun_events::ToolEventData;
                        match tool_event {
                            ToolEventData::ConfirmationNeeded {
                                tool_id, tool_name, ..
                            } => {
                                if self.approval_mode.rejects_confirmation() {
                                    let mut message = format!(
                                        "Permission rejected for {tool_name}; rerun with --auto to approve tool requests"
                                    );
                                    if let Err(error) =
                                        self.agent.reject_tool(tool_id, message.clone()).await
                                    {
                                        message.push_str(&format!(
                                            "; failed to deliver tool rejection: {error}"
                                        ));
                                    }
                                    if let Err(error) = self.agent.cancel_current_turn().await {
                                        message.push_str(&format!(
                                            "; failed to cancel active turn: {error}"
                                        ));
                                    }
                                    if self.output_format == ExecOutputFormat::StreamJson {
                                        if let Err(error) = self
                                            .drain_interrupted_turn_events(
                                                &mut event_rx,
                                                &session_id,
                                                &turn_id,
                                            )
                                            .await
                                        {
                                            message.push_str(&format!(
                                                "; failed to drain terminal event: {error}"
                                            ));
                                        }
                                    }
                                    self.print_text(|| eprintln!("{message}"));
                                    emit_exit_diagnostic(
                                        ExitKind::PermissionRejected,
                                        &message,
                                        &self.exit_context(Some(&session_id), Some(&turn_id)),
                                    );
                                    terminal_status = Some(ExecTerminalStatus::Error);
                                    terminal_message = Some(message.clone());
                                    terminal_outcome = Some(Err(anyhow::anyhow!(message)));
                                    break;
                                } else {
                                    if let Err(error) = self.agent.confirm_tool(tool_id, None).await
                                    {
                                        let mut message = format!(
                                            "Failed to approve tool request for {tool_name}: {error}"
                                        );
                                        if let Err(cancel_error) =
                                            self.agent.cancel_current_turn().await
                                        {
                                            message.push_str(&format!(
                                                "; failed to cancel active turn: {cancel_error}"
                                            ));
                                        }
                                        if self.output_format == ExecOutputFormat::StreamJson {
                                            if let Err(drain_error) = self
                                                .drain_interrupted_turn_events(
                                                    &mut event_rx,
                                                    &session_id,
                                                    &turn_id,
                                                )
                                                .await
                                            {
                                                message.push_str(&format!(
                                                    "; failed to drain terminal event: {drain_error}"
                                                ));
                                            }
                                        }
                                        emit_exit_diagnostic(
                                            ExitKind::PermissionRejected,
                                            &message,
                                            &self.exit_context(Some(&session_id), Some(&turn_id)),
                                        );
                                        terminal_status = Some(ExecTerminalStatus::Error);
                                        terminal_message = Some(message.clone());
                                        terminal_outcome = Some(Err(anyhow::anyhow!(message)));
                                        break;
                                    }
                                }
                            }
                            ToolEventData::Started {
                                tool_name,
                                tool_id,
                                params,
                                ..
                            } => {
                                self.print_tool_start_details(tool_name, tool_id, params);
                                total_tool_calls += 1;
                            }
                            ToolEventData::Progress { message, .. } => {
                                self.print_text(|| eprintln!("   In progress: {}", message));
                            }
                            ToolEventData::Completed {
                                tool_name,
                                result_for_assistant,
                                result,
                                duration_ms,
                                ..
                            } => {
                                let summary = result_for_assistant
                                    .clone()
                                    .unwrap_or_else(|| result.to_string());
                                self.print_text(|| {
                                    eprintln!(
                                        "   [+] {} ({}ms): {}",
                                        tool_name, duration_ms, summary
                                    )
                                });
                            }
                            ToolEventData::Failed {
                                tool_name, error, ..
                            } => {
                                self.print_text(|| eprintln!("   [x] {}: {}", tool_name, error));
                            }
                            _ => {}
                        }
                    }

                    AgenticEvent::DialogTurnCompleted {
                        turn_id: event_turn_id,
                        success,
                        finish_reason,
                        has_final_response,
                        ..
                    } if event_turn_id == &turn_id => {
                        if let Some(message) = completed_turn_failure(
                            *success,
                            finish_reason.as_deref(),
                            *has_final_response,
                        ) {
                            self.print_text(|| eprintln!("\nExecution failed: {message}"));
                            emit_exit_diagnostic(
                                ExitKind::DialogTurnFailed,
                                &message,
                                &self.exit_context(Some(&session_id), Some(&turn_id)),
                            );
                            terminal_status = Some(ExecTerminalStatus::Error);
                            terminal_message = Some(message.clone());
                            terminal_outcome = Some(Err(anyhow::anyhow!(message)));
                            break;
                        }
                        self.print_text(|| {
                            eprintln!("\n");
                            eprintln!("Execution complete");
                            if total_tool_calls > 0 {
                                eprintln!(
                                    "\nTool call statistics: {} tools invoked",
                                    total_tool_calls
                                );
                            }
                        });
                        terminal_status = Some(ExecTerminalStatus::Success);
                        terminal_outcome = Some(Ok(()));
                        break;
                    }

                    AgenticEvent::DialogTurnFailed {
                        turn_id: event_turn_id,
                        error,
                        ..
                    } if event_turn_id == &turn_id => {
                        self.print_text(|| eprintln!("\nExecution failed: {}", error));
                        emit_exit_diagnostic(
                            ExitKind::DialogTurnFailed,
                            error,
                            &self.exit_context(Some(&session_id), Some(&turn_id)),
                        );
                        terminal_status = Some(ExecTerminalStatus::Error);
                        terminal_message = Some(error.clone());
                        terminal_outcome =
                            Some(Err(anyhow::anyhow!("Execution failed: {}", error)));
                        break;
                    }

                    AgenticEvent::DialogTurnCancelled {
                        turn_id: event_turn_id,
                        ..
                    } if event_turn_id == &turn_id => {
                        self.print_text(|| eprintln!("\nExecution cancelled"));
                        let message = "Execution cancelled".to_string();
                        emit_exit_diagnostic(
                            ExitKind::Cancelled,
                            &message,
                            &self.exit_context(Some(&session_id), Some(&turn_id)),
                        );
                        terminal_status = Some(ExecTerminalStatus::Cancelled);
                        terminal_message = Some(message.clone());
                        terminal_outcome = Some(Err(anyhow::anyhow!(message)));
                        break;
                    }

                    AgenticEvent::SystemError { error, .. } => {
                        self.print_text(|| eprintln!("\nSystem error: {}", error));
                        emit_exit_diagnostic(
                            ExitKind::SystemError,
                            error,
                            &self.exit_context(Some(&session_id), Some(&turn_id)),
                        );
                        terminal_status = Some(ExecTerminalStatus::Error);
                        terminal_message = Some(error.clone());
                        terminal_outcome = Some(Err(anyhow::anyhow!("System error: {}", error)));
                        break;
                    }

                    _ => {}
                }
            }

            if terminal_outcome.is_some() {
                break;
            }
        }

        if let Err(error) = self.wait_for_turn_settlement(&session_id, &turn_id).await {
            let message = match terminal_message.take() {
                Some(existing) => format!("{existing}; {error}"),
                None => error.to_string(),
            };
            emit_exit_diagnostic(
                ExitKind::SettlementTimedOut,
                &message,
                &self.exit_context(Some(&session_id), Some(&turn_id)),
            );
            terminal_status = Some(ExecTerminalStatus::Error);
            terminal_message = Some(message.clone());
            terminal_outcome = Some(Err(anyhow::anyhow!(message)));
        }
        let (patch, patch_error) = self.output_patch_if_needed();
        if let Some(error) = patch_error {
            let message = match terminal_message.take() {
                Some(existing) => format!("{existing}; {error}"),
                None => error.to_string(),
            };
            terminal_status = Some(ExecTerminalStatus::Error);
            terminal_message = Some(message.clone());
            terminal_outcome = Some(Err(anyhow::anyhow!(message)));
        }
        if self.output_format == ExecOutputFormat::Json {
            let result_text = terminal_message.unwrap_or(assistant_text);
            let result = match terminal_status.unwrap_or(ExecTerminalStatus::Error) {
                ExecTerminalStatus::Success => {
                    ExecJsonResult::success(&session_id, &turn_id, result_text, usage)
                }
                ExecTerminalStatus::Error => {
                    ExecJsonResult::error(&session_id, &turn_id, result_text, usage)
                }
                ExecTerminalStatus::Cancelled => {
                    ExecJsonResult::cancelled(&session_id, &turn_id, result_text, usage)
                }
            }
            .with_patch(patch);
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        terminal_outcome
            .unwrap_or_else(|| Err(anyhow::anyhow!("Execution ended without a terminal event")))
    }

    async fn record_resolved_model_id(&self, session_id: &str, model_id: &str) {
        let trimmed = model_id.trim();
        if trimmed.is_empty() || matches!(trimmed, "auto" | "default" | "primary" | "fast") {
            return;
        }

        if let Err(error) = self.agent.update_session_model(session_id, trimmed).await {
            tracing::debug!(
                "Failed to persist resolved CLI model id: session_id={}, model_id={}, error={}",
                session_id,
                trimmed,
                error
            );
        }
    }

    async fn prepare_session(&self) -> Result<String> {
        let resume_id = self.session_options.resume.as_deref();

        let resolved_resume = if self.session_options.continue_last || resume_id == Some("last") {
            let sessions = self.agent.list_sessions().await?;
            Some(
                sessions
                    .first()
                    .map(|session| session.session_id.clone())
                    .ok_or_else(|| anyhow::anyhow!("No history sessions for current project"))?,
            )
        } else {
            resume_id.map(ToString::to_string)
        };

        if self.session_options.fork_session {
            let source_session_id = resolved_resume
                .clone()
                .or_else(|| self.session_options.session_id.clone())
                .ok_or_else(|| {
                    anyhow::anyhow!("--fork-session requires --continue, --resume, or --session")
                })?;
            let result = self
                .agent
                .branch_session_at_latest_turn(&source_session_id)
                .await?;
            self.agent.restore_session(&result.session_id).await?;
            return Ok(result.session_id);
        }

        if let Some(session_id) = resolved_resume.as_deref() {
            self.agent.restore_session(session_id).await?;
            return Ok(session_id.to_string());
        }

        if let Some(session_id) = &self.session_options.session_id {
            return self
                .agent
                .create_session_with_id(session_id.clone(), &self.agent_type)
                .await;
        }

        self.agent.ensure_session(&self.agent_type).await
    }

    fn emit_stream_envelope(&self, envelope: &bitfun_events::AgenticEventEnvelope) -> Result<()> {
        if self.output_format == ExecOutputFormat::StreamJson {
            let stdout = std::io::stdout();
            let mut stdout = stdout.lock();
            writeln!(stdout, "{}", serialize_stream_envelope(envelope)?)?;
            stdout.flush()?;
        }
        Ok(())
    }

    fn print_text(&self, f: impl FnOnce()) {
        if self.output_format == ExecOutputFormat::Text {
            f();
        }
    }

    fn output_patch_if_needed(&self) -> (Option<ExecPatchOutput>, Option<anyhow::Error>) {
        let Some(output_target) = self.output_patch.as_ref() else {
            return (None, None);
        };
        if self.output_format == ExecOutputFormat::StreamJson && output_target == "-" {
            let error = anyhow::anyhow!(
                "--output-patch with --output-format stream-json requires an explicit file path"
            );
            emit_exit_diagnostic(
                ExitKind::PatchUnavailable,
                &error.to_string(),
                &self.exit_context(None, None),
            );
            return (
                Some(ExecPatchOutput {
                    target: output_target.clone(),
                    status: "unavailable",
                    patch: None,
                    bytes: None,
                }),
                Some(error),
            );
        }
        let Some(patch) = self.get_git_diff() else {
            self.print_text(|| eprintln!("Unable to generate patch"));
            let error = anyhow::anyhow!("Unable to generate requested git patch");
            emit_exit_diagnostic(
                ExitKind::PatchUnavailable,
                &error.to_string(),
                &self.exit_context(None, None),
            );
            return (
                Some(ExecPatchOutput {
                    target: output_target.clone(),
                    status: "unavailable",
                    patch: None,
                    bytes: None,
                }),
                Some(error),
            );
        };

        let is_empty = patch.trim().is_empty();
        let status = if is_empty { "empty" } else { "generated" };
        if output_target != "-" {
            if let Err(error) = write_patch_to_path(output_target, &patch) {
                emit_exit_diagnostic(
                    ExitKind::PatchWriteFailed,
                    &error.to_string(),
                    &self.exit_context(None, None),
                );
                eprintln!("Failed to save patch: {error}");
                return (
                    Some(ExecPatchOutput {
                        target: output_target.clone(),
                        status: "write_failed",
                        patch: None,
                        bytes: Some(patch.len()),
                    }),
                    Some(anyhow::anyhow!("Failed to save requested patch: {error}")),
                );
            }
        }

        if self.output_format == ExecOutputFormat::Text {
            if is_empty {
                eprintln!("No file modifications");
            } else if output_target == "-" {
                println!("---PATCH_START---");
                println!("{patch}");
                println!("---PATCH_END---");
            } else {
                eprintln!("Patch saved to: {output_target} ({} bytes)", patch.len());
            }
        }

        (
            Some(ExecPatchOutput {
                target: output_target.clone(),
                status,
                patch: (self.output_format == ExecOutputFormat::Json && output_target == "-")
                    .then_some(patch.clone()),
                bytes: Some(patch.len()),
            }),
            None,
        )
    }

    async fn wait_for_turn_settlement(&self, session_id: &str, turn_id: &str) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);

        loop {
            if !self.agent.is_turn_processing(session_id, turn_id) {
                return Ok(());
            }

            if Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "Timed out waiting for exec turn settlement: session_id={session_id}, turn_id={turn_id}"
                ));
            }

            sleep(Duration::from_millis(50)).await;
        }
    }

    async fn drain_interrupted_turn_events(
        &self,
        event_rx: &mut tokio::sync::broadcast::Receiver<bitfun_events::AgenticEventEnvelope>,
        session_id: &str,
        turn_id: &str,
    ) -> Result<()> {
        let deadline = Instant::now() + INTERRUPT_EVENT_DRAIN_TIMEOUT;
        loop {
            let envelope = tokio::time::timeout_at(deadline, event_rx.recv())
                .await
                .map_err(|_| anyhow::anyhow!("timed out draining the cancelled turn event"))?
                .map_err(|error| {
                    anyhow::anyhow!("failed to drain the cancelled turn event: {error}")
                })?;
            if !event_belongs_to_exec_turn(&envelope.event, session_id, turn_id) {
                continue;
            }
            self.emit_stream_envelope(&envelope)?;
            if matches!(
                envelope.event,
                AgenticEvent::DialogTurnCompleted { .. }
                    | AgenticEvent::DialogTurnCancelled { .. }
                    | AgenticEvent::DialogTurnFailed { .. }
            ) {
                return Ok(());
            }
        }
    }
}

pub(crate) fn write_patch_to_path(output_target: &str, patch: &str) -> std::io::Result<()> {
    use std::path::Path;

    let path = Path::new(output_target);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, patch)
}

#[cfg(test)]
mod patch_tests {
    use std::process::Command;

    use super::{
        completed_turn_failure, event_belongs_to_exec_turn, event_turn_id,
        serialize_stream_envelope, write_patch_to_path, ExecApprovalMode, ExecJsonResult, ExecMode,
        ExecTokenUsage, TOOL_START_INPUT_PREVIEW_CHARS,
    };
    use bitfun_events::{AgenticEvent, AgenticEventEnvelope, AgenticEventPriority};
    use serde_json::json;

    #[test]
    fn write_patch_to_path_creates_nested_parent_directories() {
        let temp = tempfile::tempdir().expect("tempdir");
        let patch_path = temp.path().join("parent/child/out.patch");
        write_patch_to_path(patch_path.to_str().expect("utf8 path"), "diff content")
            .expect("write patch");

        let written = std::fs::read_to_string(&patch_path).expect("read patch");
        assert_eq!(written, "diff content");
    }

    #[test]
    fn write_patch_to_path_creates_an_explicit_empty_patch_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let patch_path = temp.path().join("empty.patch");

        write_patch_to_path(patch_path.to_str().expect("utf8 path"), "")
            .expect("write empty patch");

        assert!(patch_path.is_file());
        assert_eq!(std::fs::read_to_string(patch_path).expect("read patch"), "");
    }

    #[test]
    fn git_patch_includes_staged_and_untracked_files_from_a_repo_subdirectory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path();
        let run_git = |args: &[&str]| {
            let output = Command::new("git")
                .args(args)
                .current_dir(repo)
                .output()
                .expect("run git");
            assert!(
                output.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        };
        run_git(&["init", "--quiet"]);
        run_git(&["config", "user.email", "cli-tests@example.invalid"]);
        run_git(&["config", "user.name", "CLI Tests"]);
        std::fs::write(repo.join("tracked.txt"), "before\n").expect("tracked file");
        run_git(&["add", "tracked.txt"]);
        run_git(&["commit", "--quiet", "-m", "initial"]);

        std::fs::write(repo.join("tracked.txt"), "after\n").expect("modify tracked file");
        run_git(&["add", "tracked.txt"]);
        std::fs::write(repo.join("untracked.txt"), "new\n").expect("untracked file");
        std::fs::create_dir_all(repo.join("nested")).expect("nested directory");

        let patch = ExecMode::get_git_diff_for_workspace(&repo.join("nested"), None)
            .expect("workspace patch");

        assert!(patch.contains("tracked.txt"), "{patch}");
        assert!(patch.contains("untracked.txt"), "{patch}");
        assert!(patch.contains("+after"), "{patch}");
        assert!(patch.contains("+new"), "{patch}");
    }

    #[test]
    fn git_patch_excludes_a_preexisting_output_artifact_inside_the_repository() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path();
        let run_git = |args: &[&str]| {
            let output = Command::new("git")
                .args(args)
                .current_dir(repo)
                .output()
                .expect("run git");
            assert!(
                output.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        };
        run_git(&["init", "--quiet"]);
        run_git(&["config", "user.email", "cli-tests@example.invalid"]);
        run_git(&["config", "user.name", "CLI Tests"]);
        std::fs::write(repo.join("tracked.txt"), "before\n").expect("tracked file");
        run_git(&["add", "tracked.txt"]);
        run_git(&["commit", "--quiet", "-m", "initial"]);

        std::fs::write(repo.join("tracked.txt"), "after\n").expect("modify tracked file");
        let output_artifact = repo.join("result.patch");
        std::fs::write(&output_artifact, "old recursive patch payload\n")
            .expect("preexisting output artifact");

        let patch = ExecMode::get_git_diff_for_workspace(
            repo,
            Some(output_artifact.to_str().expect("utf8 artifact path")),
        )
        .expect("workspace patch");

        assert!(patch.contains("tracked.txt"), "{patch}");
        assert!(!patch.contains("result.patch"), "{patch}");
        assert!(!patch.contains("old recursive patch payload"), "{patch}");
    }

    #[test]
    fn git_patch_excludes_a_tracked_output_artifact_inside_the_repository() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path();
        let run_git = |args: &[&str]| {
            let output = Command::new("git")
                .args(args)
                .current_dir(repo)
                .output()
                .expect("run git");
            assert!(
                output.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        };
        run_git(&["init", "--quiet"]);
        run_git(&["config", "user.email", "cli-tests@example.invalid"]);
        run_git(&["config", "user.name", "CLI Tests"]);
        std::fs::write(repo.join("tracked.txt"), "before\n").expect("tracked file");
        std::fs::write(repo.join("result.patch"), "old patch\n").expect("tracked artifact");
        run_git(&["add", "tracked.txt", "result.patch"]);
        run_git(&["commit", "--quiet", "-m", "initial"]);

        std::fs::write(repo.join("tracked.txt"), "after\n").expect("modify tracked file");
        let output_artifact = repo.join("result.patch");
        std::fs::write(&output_artifact, "new recursive patch payload\n")
            .expect("modify tracked artifact");

        let patch = ExecMode::get_git_diff_for_workspace(
            repo,
            Some(output_artifact.to_str().expect("utf8 artifact path")),
        )
        .expect("workspace patch");

        assert!(patch.contains("tracked.txt"), "{patch}");
        assert!(!patch.contains("result.patch"), "{patch}");
        assert!(!patch.contains("recursive patch payload"), "{patch}");
    }

    #[test]
    fn tool_input_preview_redacts_data_urls() {
        let preview = ExecMode::tool_input_preview(&json!({
            "image": {
                "data_url": "data:image/png;base64,abc",
                "name": "sample"
            }
        }));

        assert!(!preview.contains("data:image/png"));
        assert!(preview.contains("\"has_data_url\":true"));
        assert!(preview.contains("\"name\":\"sample\""));
    }

    #[test]
    fn tool_input_preview_truncates_large_inputs() {
        let preview = ExecMode::tool_input_preview(&json!({
            "content": "x".repeat(TOOL_START_INPUT_PREVIEW_CHARS + 100)
        }));

        assert!(preview.ends_with("... [truncated]"));
        assert!(preview.len() < TOOL_START_INPUT_PREVIEW_CHARS + 100);
    }

    #[test]
    fn json_output_is_one_competitor_aligned_result_object() {
        let result = ExecJsonResult::success(
            "session-1",
            "turn-1",
            "completed work",
            Some(ExecTokenUsage {
                input_tokens: 10,
                output_tokens: Some(5),
                total_tokens: 15,
                cached_tokens: Some(3),
            }),
        );

        let encoded = serde_json::to_string(&result).expect("serialize result");
        let value: serde_json::Value = serde_json::from_str(&encoded).expect("one JSON object");

        assert_eq!(value["type"], "result");
        assert_eq!(value["subtype"], "success");
        assert_eq!(value["is_error"], false);
        assert_eq!(value["result"], "completed work");
        assert_eq!(value["session_id"], "session-1");
        assert_eq!(value["turn_id"], "turn-1");
        assert_eq!(value["usage"]["total_tokens"], 15);
    }

    #[test]
    fn json_usage_accumulates_all_model_round_updates_for_the_turn() {
        let events = [
            AgenticEvent::TokenUsageUpdated {
                session_id: "session-1".to_string(),
                turn_id: "turn-1".to_string(),
                model_id: "model".to_string(),
                input_tokens: 100,
                output_tokens: Some(25),
                total_tokens: 125,
                max_context_tokens: Some(200_000),
                is_subagent: false,
                cached_tokens: Some(40),
                token_details: None,
            },
            AgenticEvent::TokenUsageUpdated {
                session_id: "session-1".to_string(),
                turn_id: "turn-1".to_string(),
                model_id: "model".to_string(),
                input_tokens: 200,
                output_tokens: Some(50),
                total_tokens: 250,
                max_context_tokens: Some(200_000),
                is_subagent: false,
                cached_tokens: Some(80),
                token_details: None,
            },
        ];
        let mut usage = None;

        for event in &events {
            assert_eq!(
                ExecTokenUsage::accumulate_event(&mut usage, event, "turn-1"),
                Some("model")
            );
        }

        let value = serde_json::to_value(ExecJsonResult::success(
            "session-1",
            "turn-1",
            "done",
            usage,
        ))
        .expect("serialize result");
        assert_eq!(value["usage"]["input_tokens"], 300);
        assert_eq!(value["usage"]["output_tokens"], 75);
        assert_eq!(value["usage"]["total_tokens"], 375);
        assert_eq!(value["usage"]["cached_tokens"], 120);
    }

    #[test]
    fn json_usage_omits_optional_totals_when_any_round_does_not_report_them() {
        let events = [
            AgenticEvent::TokenUsageUpdated {
                session_id: "session-1".to_string(),
                turn_id: "turn-1".to_string(),
                model_id: "model".to_string(),
                input_tokens: 100,
                output_tokens: None,
                total_tokens: 100,
                max_context_tokens: None,
                is_subagent: false,
                cached_tokens: Some(20),
                token_details: None,
            },
            AgenticEvent::TokenUsageUpdated {
                session_id: "session-1".to_string(),
                turn_id: "turn-1".to_string(),
                model_id: "model".to_string(),
                input_tokens: 50,
                output_tokens: Some(10),
                total_tokens: 60,
                max_context_tokens: None,
                is_subagent: false,
                cached_tokens: None,
                token_details: None,
            },
        ];
        let mut usage = None;

        for event in &events {
            ExecTokenUsage::accumulate_event(&mut usage, event, "turn-1");
        }

        let value = serde_json::to_value(ExecJsonResult::success(
            "session-1",
            "turn-1",
            "done",
            usage,
        ))
        .expect("serialize result");
        assert_eq!(value["usage"]["input_tokens"], 150);
        assert_eq!(value["usage"]["total_tokens"], 160);
        assert!(value["usage"].get("output_tokens").is_none());
        assert!(value["usage"].get("cached_tokens").is_none());
    }

    #[test]
    fn preflight_json_error_omits_unknown_runtime_ids() {
        let result = ExecJsonResult::preflight_error("invalid arguments");
        let value = serde_json::to_value(result).expect("serialize result");

        assert_eq!(value["subtype"], "error");
        assert_eq!(value["is_error"], true);
        assert!(value.get("session_id").is_none());
        assert!(value.get("turn_id").is_none());
    }

    #[test]
    fn cancelled_json_result_is_an_error_outcome() {
        let result = ExecJsonResult::cancelled("session-1", "turn-1", "cancelled", None);
        let value = serde_json::to_value(result).expect("serialize result");

        assert_eq!(value["subtype"], "cancelled");
        assert_eq!(value["is_error"], true);
    }

    #[test]
    fn stream_json_reuses_the_existing_agentic_envelope() {
        let envelope = AgenticEventEnvelope::new(
            AgenticEvent::SessionStateChanged {
                session_id: "session-1".to_string(),
                new_state: "idle".to_string(),
            },
            AgenticEventPriority::Normal,
        );

        let encoded = serialize_stream_envelope(&envelope).expect("serialize envelope");
        let value: serde_json::Value = serde_json::from_str(&encoded).expect("JSONL record");

        assert_eq!(value["id"], envelope.id);
        assert_eq!(value["event"]["type"], "SessionStateChanged");
        assert!(value.get("schema_version").is_none());
        assert!(value.get("sequence").is_none());
    }

    #[test]
    fn default_exec_policy_rejects_confirmation_events() {
        assert!(ExecApprovalMode::Reject.rejects_confirmation());
        assert!(!ExecApprovalMode::Auto.rejects_confirmation());
    }

    #[test]
    fn unsuccessful_completed_turn_is_an_error_outcome() {
        assert_eq!(
            completed_turn_failure(Some(false), Some("empty_round"), Some(false)).as_deref(),
            Some("Execution completed without a successful final response: empty_round")
        );
        assert!(completed_turn_failure(Some(true), Some("stop"), Some(true)).is_none());
        assert!(completed_turn_failure(None, None, None).is_none());
    }

    #[test]
    fn exec_turn_filter_rejects_other_turn_events_in_the_same_session() {
        let event = AgenticEvent::TextChunk {
            session_id: "session-1".to_string(),
            turn_id: "turn-other".to_string(),
            round_id: "round-1".to_string(),
            attempt_id: None,
            attempt_index: None,
            text: "unrelated".to_string(),
        };

        assert_eq!(event_turn_id(&event), Some("turn-other"));
        assert!(!event_belongs_to_exec_turn(
            &event,
            "session-1",
            "turn-current"
        ));
    }

    #[test]
    fn exec_turn_filter_accepts_session_correlated_system_errors() {
        let event = AgenticEvent::SystemError {
            session_id: Some("session-1".to_string()),
            error: "another turn failed".to_string(),
            recoverable: false,
        };

        assert!(event_belongs_to_exec_turn(
            &event,
            "session-1",
            "turn-current"
        ));
        assert!(!event_belongs_to_exec_turn(
            &event,
            "session-other",
            "turn-current"
        ));
    }
}

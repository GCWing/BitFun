/// Exec mode implementation
///
/// Single command execution mode (non-interactive).
/// Consumes core events directly from EventQueue.
use anyhow::Result;
use clap::ValueEnum;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bitfun_core::agentic::core::SessionState;
use bitfun_events::AgenticEvent;
use tokio::time::{sleep, Instant};

use crate::agent::{agentic_system::AgenticSystem, core_adapter::CoreAgentAdapter, Agent};
use crate::config::CliConfig;
use crate::diagnostics::{emit_exit_diagnostic, ExitContext, ExitKind};

const TOOL_START_INPUT_PREVIEW_CHARS: usize = 4_000;

/// Patch verification gate. Activates when `--output-patch` is set and the
/// detector finds verifiers scoped to the changed files (Go, Cargo, TypeScript,
/// or a parse-only fallback). On verification failure, the agent is re-prompted
/// with the captured output and given another chance to finalize.
///
/// Tuning knobs come from env vars:
/// - `BITFUN_PATCH_VERIFY_TIMEOUT_SEC` — default 900s. Should be wide enough to
///   cover a real cold workspace build; treat exceeding it as "scope too broad",
///   not as "verifier means the patch is broken".
/// - `BITFUN_PATCH_VERIFY_MAX_RETRIES` — default 1. Each retry is a full new
///   agent turn, so two-plus retries can double token spend; default low.
#[derive(Debug, Clone)]
struct VerifyConfig {
    timeout: Duration,
    max_retries: u32,
}

impl VerifyConfig {
    fn from_env() -> Self {
        let timeout = std::env::var("BITFUN_PATCH_VERIFY_TIMEOUT_SEC")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(900));
        let max_retries = std::env::var("BITFUN_PATCH_VERIFY_MAX_RETRIES")
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(1);
        Self {
            timeout,
            max_retries,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum VerifyStatus {
    Passed,
    Failed,
    TimedOut,
    SpawnError,
}

#[derive(Debug, Clone, Serialize)]
struct VerifyOutcome {
    status: VerifyStatus,
    command: String,
    exit_code: Option<i32>,
    duration_ms: u64,
    stderr_tail: String,
    retries_used: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ExecOutputFormat {
    Text,
    Json,
    StreamJson,
}

#[derive(Debug, Clone, Default)]
pub struct ExecSessionOptions {
    pub resume: Option<String>,
    pub continue_last: bool,
    pub session_id: Option<String>,
    pub fork_session: bool,
}

pub struct ExecMode {
    #[allow(dead_code)]
    config: CliConfig,
    message: String,
    agent_type: String,
    agent: Arc<CoreAgentAdapter>,
    workspace_path: Option<PathBuf>,
    initial_diff_base: Option<String>,
    initial_untracked_files: std::collections::BTreeSet<String>,
    /// None: no patch output, Some("-"): output to stdout, Some(path): save to file
    output_patch: Option<String>,
    output_format: ExecOutputFormat,
    session_options: ExecSessionOptions,
}

impl ExecMode {
    pub fn new(
        config: CliConfig,
        message: String,
        agent_type: String,
        agentic_system: &AgenticSystem,
        workspace_path: Option<PathBuf>,
        output_patch: Option<String>,
        output_format: ExecOutputFormat,
        session_options: ExecSessionOptions,
    ) -> Self {
        let (initial_diff_base, initial_untracked_files) = if output_patch.is_some() {
            workspace_path
                .as_deref()
                .map(|workspace| (git_diff_base(workspace), untracked_files(workspace)))
                .unwrap_or_default()
        } else {
            Default::default()
        };
        let agent = Arc::new(CoreAgentAdapter::new(
            agentic_system.coordinator.clone(),
            agentic_system.event_queue.clone(),
            workspace_path.clone(),
        ));

        Self {
            config,
            message,
            agent_type,
            agent,
            workspace_path,
            initial_diff_base,
            initial_untracked_files,
            output_patch,
            output_format,
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
            println!("\nTool call: {}", tool_name);
            println!("   Started at: {}", started_at);
            println!("   Tool ID: {}", tool_id);
            println!("   CWD: {}", cwd);
            println!("   Input: {}", input_preview);
            std::io::stdout().flush().ok();
        });
    }

    fn get_git_diff(&self) -> Option<String> {
        let workspace = self.workspace_path.as_ref()?;
        let patch = collect_worktree_patch(
            workspace,
            self.initial_diff_base.as_deref(),
            &self.initial_untracked_files,
        );
        if patch.is_none() {
            eprintln!("Warning: git diff execution failed");
        }
        patch
    }

    pub async fn run(&mut self) -> Result<()> {
        tracing::info!(
            agent_type = %self.agent_type,
            message_len = self.message.len(),
            workspace = ?self.workspace_path,
            "Executing command"
        );

        let session_id = self.prepare_session().await.map_err(|e| {
            emit_exit_diagnostic(
                ExitKind::SessionCreateFailed,
                &e.to_string(),
                &self.exit_context(None, None),
            );
            e
        })?;
        tracing::info!(session_id = %session_id, "Session ready");
        let event_queue = self.agent.event_queue().clone();

        self.emit(json!({
            "type": "session",
            "session_id": session_id,
            "agent": self.agent_type,
        }))?;
        self.print_text(|| {
            println!("Executing: {}", self.message);
            println!();
            println!("Session: {}", session_id);
            println!("Thinking...");
        });

        let verify_cfg = VerifyConfig::from_env();
        let verify_workspace = self
            .workspace_path
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let verify_enabled = self.output_patch.is_some();
        let mut current_message = self.message.clone();
        let mut retries_used: u32 = 0;
        let mut total_tool_calls = 0usize;
        let mut subagent_parent_sessions: HashMap<String, String> = HashMap::new();

        let final_outcome: Result<()> = 'retry: loop {
        let turn_id = self
            .agent
            .send_message(current_message.clone(), &self.agent_type)
            .await
            .map_err(|e| {
                emit_exit_diagnostic(
                    ExitKind::SendMessageFailed,
                    &e.to_string(),
                    &self.exit_context(Some(&session_id), None),
                );
                e
            })?;
        tracing::info!(session_id = %session_id, turn_id = %turn_id, "Message sent");

        // Per-turn loop state.
        let mut terminal_outcome: Option<Result<()>> = None;
        let mut observed_turn_activity = false;

        loop {
            // Wait for events, but wake periodically so exec mode cannot hang
            // forever if a terminal event is missed after core has gone idle.
            tokio::select! {
                _ = event_queue.wait_for_events() => {}
                _ = sleep(Duration::from_secs(1)) => {}
            }

            let events = event_queue.dequeue_batch(20).await;

            for envelope in events {
                let event = &envelope.event;

                if let AgenticEvent::SubagentSessionLinked {
                    session_id: subagent_session_id,
                    parent_session_id,
                    ..
                } = event
                {
                    subagent_parent_sessions
                        .insert(subagent_session_id.clone(), parent_session_id.clone());
                    continue;
                }

                // Only process events for our session
                if event.session_id() != Some(&session_id) {
                    // Check if this is a subagent event whose parent is in our session
                    if let AgenticEvent::ToolEvent { tool_event, .. } = event {
                        let parent_session_id = event.session_id().and_then(|event_session_id| {
                            subagent_parent_sessions.get(event_session_id)
                        });
                        if parent_session_id.map(String::as_str) == Some(session_id.as_str()) {
                            use bitfun_events::ToolEventData;
                            match tool_event {
                                ToolEventData::Started {
                                    tool_name,
                                    tool_id,
                                    params,
                                    ..
                                } => {
                                    self.emit(json!({
                                        "type": "subagent_tool_start",
                                        "session_id": session_id,
                                        "tool_id": tool_id,
                                        "tool_name": tool_name,
                                        "input": params,
                                    }))?;
                                    self.print_text(|| {
                                        let started_at = chrono::Utc::now().to_rfc3339();
                                        let input_preview = Self::tool_input_preview(params);
                                        println!("   [subagent] {}", tool_name);
                                        println!("      Started at: {}", started_at);
                                        println!("      Tool ID: {}", tool_id);
                                        println!("      CWD: {}", self.workspace_display());
                                        println!("      Input: {}", input_preview);
                                        std::io::stdout().flush().ok();
                                    });
                                }
                                ToolEventData::Completed {
                                    tool_name,
                                    tool_id,
                                    result_for_assistant,
                                    result,
                                    duration_ms,
                                    ..
                                } => {
                                    let summary = result_for_assistant
                                        .clone()
                                        .unwrap_or_else(|| result.to_string());
                                    self.emit(json!({
                                        "type": "subagent_tool_result",
                                        "session_id": session_id,
                                        "tool_id": tool_id,
                                        "tool_name": tool_name,
                                        "duration_ms": duration_ms,
                                        "result": result,
                                        "summary": summary,
                                    }))?;
                                    self.print_text(|| {
                                        println!(
                                            "   [subagent] {} completed: {}",
                                            tool_name, summary
                                        )
                                    });
                                }
                                ToolEventData::Failed {
                                    tool_name,
                                    tool_id,
                                    error,
                                    ..
                                } => {
                                    self.emit(json!({
                                        "type": "subagent_tool_error",
                                        "session_id": session_id,
                                        "tool_id": tool_id,
                                        "tool_name": tool_name,
                                        "error": error,
                                    }))?;
                                    self.print_text(|| {
                                        println!("   [subagent] {} failed: {}", tool_name, error)
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                    continue;
                }

                observed_turn_activity = true;

                match event {
                    AgenticEvent::ModelRoundStarted {
                        model_id: Some(model_id),
                        ..
                    }
                    | AgenticEvent::ModelRoundCompleted {
                        model_id: Some(model_id),
                        ..
                    }
                    | AgenticEvent::TokenUsageUpdated { model_id, .. } => {
                        self.record_resolved_model_id(&session_id, model_id).await;
                    }

                    AgenticEvent::TextChunk { text, .. } => {
                        self.emit(json!({
                            "type": "text",
                            "session_id": session_id,
                            "text": text,
                        }))?;
                        self.print_text(|| {
                            print!("{}", text);
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                        });
                    }

                    AgenticEvent::ThinkingChunk { content, .. } => {
                        self.emit(json!({
                            "type": "thinking",
                            "session_id": session_id,
                            "text": content,
                        }))?;
                        self.print_text(|| {
                            print!("\x1b[2m{}\x1b[0m", content);
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                        });
                    }

                    AgenticEvent::ToolEvent { tool_event, .. } => {
                        use bitfun_events::ToolEventData;
                        match tool_event {
                            ToolEventData::Started {
                                tool_name,
                                tool_id,
                                params,
                                ..
                            } => {
                                self.emit(json!({
                                    "type": "tool_start",
                                    "session_id": session_id,
                                    "tool_id": tool_id,
                                    "tool_name": tool_name,
                                    "input": params,
                                }))?;
                                self.print_tool_start_details(tool_name, tool_id, params);
                                total_tool_calls += 1;
                            }
                            ToolEventData::Progress {
                                tool_name,
                                tool_id,
                                message,
                                percentage,
                            } => {
                                self.emit(json!({
                                    "type": "tool_progress",
                                    "session_id": session_id,
                                    "tool_id": tool_id,
                                    "tool_name": tool_name,
                                    "message": message,
                                    "percentage": percentage,
                                }))?;
                                self.print_text(|| println!("   In progress: {}", message));
                            }
                            ToolEventData::Completed {
                                tool_name,
                                tool_id,
                                result_for_assistant,
                                result,
                                duration_ms,
                                ..
                            } => {
                                let summary = result_for_assistant
                                    .clone()
                                    .unwrap_or_else(|| result.to_string());
                                self.emit(json!({
                                    "type": "tool_result",
                                    "session_id": session_id,
                                    "tool_id": tool_id,
                                    "tool_name": tool_name,
                                    "duration_ms": duration_ms,
                                    "result": result,
                                    "summary": summary,
                                }))?;
                                self.print_text(|| {
                                    println!(
                                        "   [+] {} ({}ms): {}",
                                        tool_name, duration_ms, summary
                                    )
                                });
                            }
                            ToolEventData::Failed {
                                tool_name,
                                tool_id,
                                error,
                                ..
                            } => {
                                self.emit(json!({
                                    "type": "tool_error",
                                    "session_id": session_id,
                                    "tool_id": tool_id,
                                    "tool_name": tool_name,
                                    "error": error,
                                }))?;
                                self.print_text(|| println!("   [x] {}: {}", tool_name, error));
                            }
                            _ => {}
                        }
                    }

                    AgenticEvent::DialogTurnCompleted { .. } => {
                        self.emit(json!({
                            "type": "done",
                            "session_id": session_id,
                            "status": "completed",
                            "tool_calls": total_tool_calls,
                        }))?;
                        self.print_text(|| {
                            println!("\n");
                            println!("Execution complete");
                            if total_tool_calls > 0 {
                                println!(
                                    "\nTool call statistics: {} tools invoked",
                                    total_tool_calls
                                );
                            }
                        });
                        terminal_outcome = Some(Ok(()));
                        break;
                    }

                    AgenticEvent::DialogTurnFailed { error, .. } => {
                        self.emit(json!({
                            "type": "error",
                            "session_id": session_id,
                            "message": error,
                        }))?;
                        self.print_text(|| eprintln!("\nExecution failed: {}", error));
                        emit_exit_diagnostic(
                            ExitKind::DialogTurnFailed,
                            error,
                            &self.exit_context(Some(&session_id), Some(&turn_id)),
                        );
                        terminal_outcome =
                            Some(Err(anyhow::anyhow!("Execution failed: {}", error)));
                        break;
                    }

                    AgenticEvent::DialogTurnCancelled { .. } => {
                        self.emit(json!({
                            "type": "done",
                            "session_id": session_id,
                            "status": "cancelled",
                            "tool_calls": total_tool_calls,
                        }))?;
                        self.print_text(|| println!("\nExecution cancelled"));
                        terminal_outcome = Some(Ok(()));
                        break;
                    }

                    AgenticEvent::SystemError { error, .. } => {
                        self.emit(json!({
                            "type": "error",
                            "session_id": session_id,
                            "message": error,
                        }))?;
                        self.print_text(|| eprintln!("\nSystem error: {}", error));
                        emit_exit_diagnostic(
                            ExitKind::SystemError,
                            error,
                            &self.exit_context(Some(&session_id), Some(&turn_id)),
                        );
                        terminal_outcome = Some(Err(anyhow::anyhow!("System error: {}", error)));
                        break;
                    }

                    _ => {}
                }
            }

            if terminal_outcome.is_some() {
                break;
            }

            if observed_turn_activity {
                match self
                    .agent
                    .coordinator()
                    .get_session_manager()
                    .get_session(&session_id)
                    .map(|session| session.state)
                {
                    Some(SessionState::Idle)
                        if !self.agent.coordinator().has_active_turn(&turn_id) =>
                    {
                        tracing::warn!(
                            "Exec observed idle session without terminal turn event; treating turn as settled: session_id={}, turn_id={}",
                            session_id,
                            turn_id
                        );
                        println!("\n");
                        println!("Execution complete");
                        if total_tool_calls > 0 {
                            println!("\nTool call statistics: {} tools invoked", total_tool_calls);
                        }
                        terminal_outcome = Some(Ok(()));
                        break;
                    }
                    Some(SessionState::Idle) => {}
                    Some(SessionState::Error { error, .. }) => {
                        eprintln!("\nExecution failed: {}", error);
                        emit_exit_diagnostic(
                            ExitKind::DialogTurnFailed,
                            &error,
                            &self.exit_context(Some(&session_id), Some(&turn_id)),
                        );
                        terminal_outcome =
                            Some(Err(anyhow::anyhow!("Execution failed: {}", error)));
                        break;
                    }
                    _ => {}
                }
            }
        }

        self.wait_for_turn_settlement(&session_id, &turn_id).await;

        let outcome = terminal_outcome.unwrap_or(Ok(()));

        // Patch verification gate. Only runs when --output-patch is set,
        // the agent's turn settled cleanly, and the workspace exposes a
        // verifier we can detect. Failure to verify triggers up to
        // verify_cfg.max_retries additional turns, each fed the verifier's
        // output (or a scope-narrowing hint, for timeouts) as a system
        // reminder. Passes / skips fall straight through to emit the patch.
        if verify_enabled && outcome.is_ok() {
            if let Some(command) =
                detect_verify_command(
                    &verify_workspace,
                    self.initial_diff_base.as_deref(),
                    &self.initial_untracked_files,
                )
            {
                let result = self
                    .verify_patch(&command, &verify_cfg, retries_used)
                    .await;
                let passed = result.status == VerifyStatus::Passed;
                if !passed {
                    self.emit(json!({
                        "type": "verify_failed",
                        "session_id": session_id,
                        "attempt": retries_used + 1,
                        "command": &result.command,
                        "status": result.status,
                        "exit_code": result.exit_code,
                        "duration_ms": result.duration_ms,
                        "stderr_tail": result.stderr_tail,
                    }))?;
                    if retries_used < verify_cfg.max_retries {
                        self.print_text(|| {
                            eprintln!(
                                "\nVerification failed (attempt {}, exit {:?}): {} — asking the agent to fix and retry",
                                retries_used + 1,
                                result.exit_code,
                                result.command,
                            );
                        });
                        current_message = build_retry_message(&result);
                        retries_used += 1;
                        continue 'retry;
                    }
                    self.print_text(|| {
                        eprintln!(
                            "\nVerification still failing after {} attempt(s); emitting patch unverified ({})",
                            retries_used + 1,
                            result.command,
                        );
                    });
                }
            }
        }

        break outcome;
        };

        self.output_patch_if_needed();
        final_outcome
    }

    async fn verify_patch(
        &self,
        command: &str,
        cfg: &VerifyConfig,
        retries_used: u32,
    ) -> VerifyOutcome {
        let cwd = self
            .workspace_path
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));

        let mut cmd = if cfg!(windows) {
            let mut c = tokio::process::Command::new("cmd");
            c.arg("/C").arg(command);
            c
        } else {
            let mut c = tokio::process::Command::new("sh");
            c.arg("-c").arg(command);
            c
        };
        cmd.current_dir(&cwd);
        cmd.kill_on_drop(true);

        let start = Instant::now();
        let result = tokio::time::timeout(cfg.timeout, cmd.output()).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let exit_code = output.status.code();
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let combined = if stderr.trim().is_empty() {
                    stdout.to_string()
                } else if stdout.trim().is_empty() {
                    stderr.to_string()
                } else {
                    format!("{}\n--- stdout ---\n{}", stderr, stdout)
                };
                let stderr_tail = tail_chars(&combined, 4000);
                let status = if output.status.success() {
                    VerifyStatus::Passed
                } else {
                    VerifyStatus::Failed
                };
                VerifyOutcome {
                    status,
                    command: command.to_string(),
                    exit_code,
                    duration_ms,
                    stderr_tail,
                    retries_used,
                }
            }
            Ok(Err(io_err)) => VerifyOutcome {
                status: VerifyStatus::SpawnError,
                command: command.to_string(),
                exit_code: None,
                duration_ms,
                stderr_tail: format!("spawn error: {}", io_err),
                retries_used,
            },
            Err(_) => VerifyOutcome {
                status: VerifyStatus::TimedOut,
                command: command.to_string(),
                exit_code: None,
                duration_ms,
                stderr_tail: format!("timed out after {}s", cfg.timeout.as_secs()),
                retries_used,
            },
        }
    }

    async fn record_resolved_model_id(&self, session_id: &str, model_id: &str) {
        let trimmed = model_id.trim();
        if trimmed.is_empty() || matches!(trimmed, "auto" | "default" | "primary" | "fast") {
            return;
        }

        if let Err(error) = self
            .agent
            .coordinator()
            .update_session_model(session_id, trimmed)
            .await
        {
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
        let workspace = self
            .workspace_path
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));

        let resolved_resume = if self.session_options.continue_last || resume_id == Some("last") {
            let sessions = self.agent.coordinator().list_sessions(&workspace).await?;
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
            let (_session, turns) = self
                .agent
                .coordinator()
                .restore_session_view(&workspace, &source_session_id)
                .await?;
            let source_turn_id = turns
                .last()
                .map(|turn| turn.turn_id.clone())
                .ok_or_else(|| anyhow::anyhow!("Session has no persisted turns to fork"))?;
            let path_manager = bitfun_core::infrastructure::try_get_path_manager_arc()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let persistence_manager =
                bitfun_core::agentic::persistence::PersistenceManager::new(path_manager)
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let result = persistence_manager
                .branch_session(
                    &workspace,
                    &bitfun_core::agentic::persistence::session_branch::SessionBranchRequest {
                        source_session_id: source_session_id.clone(),
                        source_turn_id,
                    },
                )
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

    fn emit(&self, value: serde_json::Value) -> Result<()> {
        match self.output_format {
            ExecOutputFormat::Text => {}
            ExecOutputFormat::StreamJson => {
                println!("{}", serde_json::to_string(&value)?);
            }
            ExecOutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&value)?);
            }
        }
        Ok(())
    }

    fn print_text(&self, f: impl FnOnce()) {
        if self.output_format == ExecOutputFormat::Text {
            f();
        }
    }

    fn output_patch_if_needed(&self) {
        if let Some(ref output_target) = self.output_patch {
            if let Some(patch) = self.get_git_diff() {
                let status = if patch.trim().is_empty() {
                    "empty"
                } else {
                    "generated"
                };
                let patch_value = json!({
                    "type": "patch",
                    "target": output_target,
                    "status": status,
                    "patch": if output_target == "-" { Some(patch.as_str()) } else { None },
                    "bytes": patch.len(),
                });

                if self.emit(patch_value).is_err() {
                    eprintln!("Failed to emit patch event");
                }

                if self.output_format != ExecOutputFormat::Text {
                    if output_target != "-" && !patch.trim().is_empty() {
                        if let Err(e) = write_patch_to_path(output_target, &patch) {
                            emit_exit_diagnostic(
                                ExitKind::PatchWriteFailed,
                                &e.to_string(),
                                &self.exit_context(None, None),
                            );
                            eprintln!("Failed to save patch: {}", e);
                        }
                    }
                    return;
                }

                println!("\n--- Generating Patch ---");
                if patch.trim().is_empty() {
                    println!("(No file modifications)");
                } else if output_target == "-" {
                    println!("---PATCH_START---");
                    println!("{}", patch);
                    println!("---PATCH_END---");
                } else {
                    match write_patch_to_path(output_target, &patch) {
                        Ok(_) => {
                            println!("Patch saved to: {}", output_target);
                            println!("({} bytes)", patch.len());
                        }
                        Err(e) => {
                            emit_exit_diagnostic(
                                ExitKind::PatchWriteFailed,
                                &e.to_string(),
                                &self.exit_context(None, None),
                            );
                            eprintln!("Failed to save patch: {}", e);
                            println!("---PATCH_START---");
                            println!("{}", patch);
                            println!("---PATCH_END---");
                        }
                    }
                }
            } else {
                let value = json!({
                    "type": "patch",
                    "target": output_target,
                    "status": "unavailable",
                });
                if self.emit(value).is_err() {
                    eprintln!("Failed to emit patch event");
                }
                self.print_text(|| println!("(Unable to generate patch)"));
            }
        }
    }

    async fn wait_for_turn_settlement(&self, session_id: &str, turn_id: &str) {
        let session_manager = self.agent.coordinator().get_session_manager().clone();
        let deadline = Instant::now() + Duration::from_secs(5);

        loop {
            let Some(session) = session_manager.get_session(session_id) else {
                return;
            };

            let still_processing = matches!(
                &session.state,
                SessionState::Processing { current_turn_id, .. } if current_turn_id == turn_id
            );

            if !still_processing {
                return;
            }

            if Instant::now() >= deadline {
                tracing::warn!(
                    "Timed out waiting for exec turn settlement: session_id={}, turn_id={}",
                    session_id,
                    turn_id
                );
                return;
            }

            sleep(Duration::from_millis(50)).await;
        }
    }
}

fn tail_chars(s: &str, max: usize) -> String {
    let total = s.chars().count();
    if total <= max {
        s.to_string()
    } else {
        s.chars().skip(total - max).collect()
    }
}

fn build_retry_message(outcome: &VerifyOutcome) -> String {
    match outcome.status {
        VerifyStatus::TimedOut => format!(
            "<system-reminder>\n\
The verifier we ran timed out — it did not return a pass/fail signal:\n\
\n\
$ {command}\n\
(timed out after {ms}ms)\n\
\n\
Timeout doesn't mean your changes are broken — the command was too slow to finish in the budget. Either:\n\
  - Run a lighter check yourself: a single test file instead of the full package, `tsc --noEmit` on one sub-tsconfig instead of the root project, parsing just the changed file (`python -c 'import ast; ast.parse(...)'`, `node --check`, `gofmt -e`). If it passes, finalize.\n\
  - Or, if you believe your changes are correct, finalize without further verification.\n\
Do not rerun the same command verbatim. Either pick a concretely lighter check or trust your edits.\n\
</system-reminder>",
            command = outcome.command,
            ms = outcome.duration_ms,
        ),
        VerifyStatus::SpawnError => format!(
            "<system-reminder>\n\
The verifier we tried to run could not start:\n\
\n\
$ {command}\n\
{tail}\n\
\n\
The toolchain may be missing in this environment. Run a verification command you can actually invoke (e.g. just parse changed files) and finalize.\n\
</system-reminder>",
            command = outcome.command,
            tail = outcome.stderr_tail,
        ),
        VerifyStatus::Failed => format!(
            "<system-reminder>\n\
Your previous changes did not pass external verification. The verification command exited with code {code:?}:\n\
\n\
$ {command}\n\
\n\
Last output (truncated to 4000 chars):\n\
{tail}\n\
\n\
This is your next signal — diagnose what is still wrong, fix it, and run the verification command yourself to confirm it exits 0 before declaring the task done. Do not finish until the command succeeds or you have a concrete justification that the remaining failure is unrelated to your change.\n\
</system-reminder>",
            command = outcome.command,
            code = outcome.exit_code,
            tail = outcome.stderr_tail,
        ),
        VerifyStatus::Passed => String::from(
            "<system-reminder>\nInternal note: build_retry_message called on a Passed outcome. This is a harness bug; ignore.\n</system-reminder>",
        ),
    }
}

/// Resolves verifiers scoped to the files that will be emitted in the patch:
///   1. Go files are grouped by their nearest `go.mod` and vetted from that
///      module root.
///   2. Rust files are grouped by their nearest package `Cargo.toml` and checked
///      through that manifest, including test targets.
///   3. TypeScript and JSX files use their nearest `tsconfig.json`.
///   4. Remaining Python, JavaScript, and Go files receive parse-only checks.
/// Commands for mixed-language patches are composed instead of stopping at the
/// first detected language.
/// Returns None if nothing applies — verification silently skips in that case.
///
/// Deliberately never auto-select project-wide Makefile or justfile targets:
/// a target named `test` or `check` is not evidence that it is relevant to the
/// changed code or suitable for the current runtime.
fn detect_verify_command(
    workspace: &std::path::Path,
    diff_base: Option<&str>,
    initial_untracked_files: &std::collections::BTreeSet<String>,
) -> Option<String> {
    let changed = changed_files(workspace, diff_base, initial_untracked_files);
    if changed.is_empty() {
        return None;
    }

    let mut commands = Vec::new();
    commands.extend(scoped_go_commands(workspace, &changed));
    commands.extend(scoped_cargo_commands(workspace, &changed));
    commands.extend(scoped_typescript_commands(workspace, &changed));
    if let Some(command) = build_parse_only_command(workspace, &changed) {
        commands.push(command);
    }

    (!commands.is_empty()).then(|| commands.join(" && "))
}

fn scoped_go_commands(workspace: &std::path::Path, files: &[String]) -> Vec<String> {
    use std::collections::{BTreeMap, BTreeSet};

    let mut packages_by_module: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    let mut manifest_only_modules = BTreeSet::new();
    for file in files {
        let path = workspace.join(file);
        let lower = file.to_ascii_lowercase();
        let Some(module_manifest) = find_nearest_manifest(workspace, &path, "go.mod") else {
            continue;
        };
        let module_dir = module_manifest.parent().unwrap_or(workspace).to_path_buf();
        if lower.ends_with(".go") {
            let package_dir = path.parent().unwrap_or(&module_dir);
            let has_current_go_source = std::fs::read_dir(package_dir)
                .ok()
                .into_iter()
                .flatten()
                .any(|entry| {
                    entry
                        .ok()
                        .and_then(|entry| entry.path().extension().map(|ext| ext == "go"))
                        .unwrap_or(false)
                });
            if !has_current_go_source {
                continue;
            }
            let relative = package_dir.strip_prefix(&module_dir).unwrap_or(package_dir);
            let target = if relative.as_os_str().is_empty() {
                ".".to_string()
            } else {
                format!("./{}", relative.to_string_lossy().replace('\\', "/"))
            };
            packages_by_module
                .entry(module_dir)
                .or_default()
                .insert(target);
        } else if matches!(
            std::path::Path::new(&lower)
                .file_name()
                .and_then(|name| name.to_str()),
            Some("go.mod" | "go.sum")
        ) {
            manifest_only_modules.insert(module_dir);
        }
    }

    let mut commands = Vec::new();
    for (module_dir, packages) in &packages_by_module {
        let targets = packages
            .iter()
            .map(|target| shell_single_quote(target))
            .collect::<Vec<_>>()
            .join(" ");
        let command = format!("go vet -printf=false -composites=false -stdmethods=false {targets}");
        commands.push(command_in_directory(workspace, module_dir, &command));
    }
    for module_dir in manifest_only_modules {
        if !packages_by_module.contains_key(&module_dir) {
            commands.push(command_in_directory(
                workspace,
                &module_dir,
                "go list -m all",
            ));
        }
    }
    commands
}

fn scoped_cargo_commands(workspace: &std::path::Path, files: &[String]) -> Vec<String> {
    use std::collections::{BTreeMap, BTreeSet};

    let mut packages: BTreeMap<PathBuf, (Option<String>, BTreeSet<String>)> = BTreeMap::new();
    for file in files {
        let lower = file.to_ascii_lowercase();
        let is_source = lower.ends_with(".rs");
        let is_manifest = matches!(
            std::path::Path::new(&lower)
                .file_name()
                .and_then(|name| name.to_str()),
            Some("cargo.toml" | "cargo.lock")
        );
        if !is_source && !is_manifest {
            continue;
        }
        let path = workspace.join(file);
        let Some(manifest) = find_nearest_manifest(workspace, &path, "Cargo.toml") else {
            continue;
        };
        let name = read_cargo_package_name(&manifest);
        let integration_target = is_source
            .then(|| cargo_integration_test_target(&manifest, &path))
            .flatten();
        let entry = packages
            .entry(manifest)
            .or_insert_with(|| (name, BTreeSet::new()));
        if let Some(target) = integration_target {
            entry.1.insert(target);
        }
    }

    packages
        .into_iter()
        .flat_map(|(manifest, (package, integration_tests))| {
            let manifest = manifest
                .strip_prefix(workspace)
                .unwrap_or(&manifest)
                .to_string_lossy()
                .replace('\\', "/");
            match package {
                Some(package) => {
                    let manifest_arg = shell_single_quote(&manifest);
                    let package_arg = shell_single_quote(&package);
                    let mut commands = vec![format!(
                        "cargo check --manifest-path {manifest_arg} -p {package_arg} --message-format=short"
                    )];
                    if !integration_tests.is_empty() {
                        let targets = integration_tests
                            .iter()
                            .map(|target| format!("--test {}", shell_single_quote(target)))
                            .collect::<Vec<_>>()
                            .join(" ");
                        commands.push(format!(
                            "cargo check --manifest-path {manifest_arg} -p {package_arg} {targets} --message-format=short"
                        ));
                    }
                    commands
                }
                None => vec![format!(
                    "cargo metadata --no-deps --format-version 1 --manifest-path {}",
                    shell_single_quote(&manifest),
                )],
            }
        })
        .collect()
}

fn cargo_integration_test_target(
    manifest: &std::path::Path,
    source: &std::path::Path,
) -> Option<String> {
    if !source.is_file() {
        return None;
    }
    let relative = source.strip_prefix(manifest.parent()?).ok()?;
    let mut components = relative.components();
    if components.next()?.as_os_str() != "tests" {
        return None;
    }
    let target = components.next()?.as_os_str().to_str()?;
    if components.next().is_some() {
        return None;
    }
    std::path::Path::new(target)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToString::to_string)
}

fn find_nearest_manifest(
    workspace: &std::path::Path,
    file: &std::path::Path,
    manifest_name: &str,
) -> Option<PathBuf> {
    let mut current = file.parent()?;
    loop {
        let manifest = current.join(manifest_name);
        if manifest.is_file() {
            return Some(manifest);
        }
        if current == workspace {
            return None;
        }
        current = current.parent()?;
    }
}

fn scoped_typescript_commands(workspace: &std::path::Path, files: &[String]) -> Vec<String> {
    use std::collections::BTreeSet;

    let mut configs = BTreeSet::new();
    for file in files {
        let lower = file.to_ascii_lowercase();
        let extension_is_supported = [".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"]
            .iter()
            .any(|extension| lower.ends_with(extension));
        let is_config = std::path::Path::new(&lower)
            .file_name()
            .and_then(|name| name.to_str())
            == Some("tsconfig.json");
        if !extension_is_supported && !is_config {
            continue;
        }
        if let Some(config) =
            find_nearest_manifest(workspace, &workspace.join(file), "tsconfig.json")
        {
            configs.insert(config);
        }
    }

    configs
        .into_iter()
        .map(|config| {
            let relative = config
                .strip_prefix(workspace)
                .unwrap_or(&config)
                .to_string_lossy()
                .replace('\\', "/");
            format!(
                "npx --no-install tsc --noEmit -p {}",
                shell_single_quote(&relative)
            )
        })
        .collect()
}

fn command_in_directory(
    workspace: &std::path::Path,
    directory: &std::path::Path,
    command: &str,
) -> String {
    let relative = directory.strip_prefix(workspace).unwrap_or(directory);
    if relative.as_os_str().is_empty() {
        command.to_string()
    } else {
        format!(
            "(cd {} && {})",
            shell_single_quote(&relative.to_string_lossy().replace('\\', "/")),
            command
        )
    }
}

/// Minimal, no-dep parser: returns the `name` declared under the first
/// `[package]` table in `toml_path`, or None. Skips a virtual-workspace
/// root that has only `[workspace]` and no `[package]`.
fn read_cargo_package_name(toml_path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(toml_path).ok()?;
    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('[') {
            in_package = rest.starts_with("package]") || rest == "package]";
            continue;
        }
        if in_package {
            if let Some(rest) = trimmed.strip_prefix("name") {
                let rest = rest.trim_start();
                if let Some(rest) = rest.strip_prefix('=') {
                    let value = rest.trim();
                    // Strip an inline comment after the value.
                    let value = value.split('#').next().unwrap_or(value).trim();
                    let stripped = value.trim_matches(|c: char| c == '"' || c == '\'');
                    if !stripped.is_empty() {
                        return Some(stripped.to_string());
                    }
                }
            }
        }
    }
    None
}

fn changed_files(
    workspace: &std::path::Path,
    diff_base: Option<&str>,
    initial_untracked_files: &std::collections::BTreeSet<String>,
) -> Vec<String> {
    use std::collections::BTreeSet;

    let mut files = BTreeSet::new();
    if let Some(diff_base) = diff_base {
        let diff = bitfun_core::util::process_manager::create_command("git")
            .args([
                "diff",
                diff_base,
                "--name-only",
                "--find-renames",
                "-z",
            ])
            .current_dir(workspace)
            .output();
        if let Ok(output) = diff {
            if output.status.success() {
                files.extend(nul_separated_paths(&output.stdout));
            }
        }
    }

    let untracked = bitfun_core::util::process_manager::create_command("git")
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(workspace)
        .output();
    if let Ok(output) = untracked {
        if output.status.success() {
            files.extend(
                nul_separated_paths(&output.stdout)
                    .into_iter()
                    .filter(|path| !initial_untracked_files.contains(path)),
            );
        }
    }

    files.into_iter().collect()
}

fn nul_separated_paths(output: &[u8]) -> Vec<String> {
    output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).to_string())
        .collect()
}

fn untracked_files(workspace: &std::path::Path) -> std::collections::BTreeSet<String> {
    let output = bitfun_core::util::process_manager::create_command("git")
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(workspace)
        .output();
    match output {
        Ok(output) if output.status.success() => {
            nul_separated_paths(&output.stdout).into_iter().collect()
        }
        _ => std::collections::BTreeSet::new(),
    }
}

fn git_diff_base(workspace: &std::path::Path) -> Option<String> {
    let head = bitfun_core::util::process_manager::create_command("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(workspace)
        .output()
        .ok();
    if let Some(output) = head.filter(|output| output.status.success()) {
        let head = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !head.is_empty() {
            return Some(head);
        }
    }

    // Repositories without an initial commit still need a stable base if the
    // agent creates its first commit. Hashing an empty tree through stdin works
    // for both SHA-1 and SHA-256 repositories without modifying the worktree.
    let empty_tree = bitfun_core::util::process_manager::create_command("git")
        .args(["hash-object", "-t", "tree", "--stdin"])
        .current_dir(workspace)
        .output()
        .ok()?;
    if !empty_tree.status.success() {
        return None;
    }
    let oid = String::from_utf8_lossy(&empty_tree.stdout).trim().to_string();
    (!oid.is_empty()).then_some(oid)
}

fn build_parse_only_command(workspace: &std::path::Path, files: &[String]) -> Option<String> {
    let mut checks: Vec<String> = Vec::new();
    for file in files {
        if !workspace.join(file).is_file() {
            continue;
        }
        let quoted = shell_single_quote(file);
        let lower = file.to_ascii_lowercase();
        if lower.ends_with(".py") {
            checks.push(format!(
                "python3 -c 'import ast,sys; ast.parse(open(sys.argv[1]).read())' {}",
                quoted
            ));
        } else if lower.ends_with(".js") || lower.ends_with(".mjs") || lower.ends_with(".cjs") {
            checks.push(format!("node --check {}", quoted));
        } else if lower.ends_with(".go")
            && find_nearest_manifest(workspace, &workspace.join(file), "go.mod").is_none()
        {
            checks.push(format!("gofmt -e -d {}", quoted));
        }
    }
    if checks.is_empty() {
        None
    } else {
        Some(checks.join(" && "))
    }
}

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn collect_worktree_patch(
    workspace: &std::path::Path,
    diff_base: Option<&str>,
    initial_untracked_files: &std::collections::BTreeSet<String>,
) -> Option<String> {
    // Diff against HEAD so staged and unstaged tracked edits share one artifact.
    // Untracked, non-ignored files are appended as ordinary new-file patches.
    let diff_base = diff_base?;
    let tracked = bitfun_core::util::process_manager::create_command("git")
        .args(["diff", diff_base, "--no-color", "--binary"])
        .current_dir(workspace)
        .output()
        .ok()?;
    if !tracked.status.success() {
        return None;
    }

    let mut patch = String::from_utf8_lossy(&tracked.stdout).to_string();
    let untracked = bitfun_core::util::process_manager::create_command("git")
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(workspace)
        .output()
        .ok()?;
    if !untracked.status.success() {
        return None;
    }
    for path in nul_separated_paths(&untracked.stdout) {
        if initial_untracked_files.contains(&path) {
            continue;
        }
        let output = bitfun_core::util::process_manager::create_command("git")
            .args([
                "diff",
                "--no-index",
                "--no-color",
                "--binary",
                "--",
                "/dev/null",
                &path,
            ])
            .current_dir(workspace)
            .output()
            .ok()?;
        if !matches!(output.status.code(), Some(0 | 1)) {
            return None;
        }
        if !patch.is_empty() && !patch.ends_with('\n') {
            patch.push('\n');
        }
        patch.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    Some(patch)
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
#[path = "exec/patch_tests.rs"]
mod patch_tests;

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
/// detector finds a runnable verifier in the workspace (Makefile/justfile target,
/// go.mod, Cargo.toml, tsconfig.json, package.json script, or a parse-only
/// fallback over changed files). On verification failure, the agent is re-prompted
/// with the captured output and given another chance to finalize.
///
/// Tuning knobs come from env vars:
/// - `BITFUN_PATCH_VERIFY_CMD` — explicit command override (skips detection).
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

        let git_dir = workspace.join(".git");
        if !git_dir.exists() {
            eprintln!("Warning: Workspace is not a git repository, cannot generate patch");
            return None;
        }

        let output = bitfun_core::util::process_manager::create_command("git")
            .args(["diff", "--no-color"])
            .current_dir(workspace)
            .output()
            .ok()?;

        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            eprintln!("Warning: git diff execution failed");
            None
        }
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
            if let Some(command) = detect_verify_command(&verify_workspace) {
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
The verifier we ran for you timed out — it did not return a pass/fail signal:\n\
\n\
$ {command}\n\
(timed out after {ms}ms)\n\
\n\
Timeout means the command was too broad/slow, not that your changes are necessarily broken. Do one of:\n\
  - Narrow the verification scope to the package(s) you actually touched. For Go use `go build ./path/to/changed/pkg/...` not `go build ./...`; for Cargo use `cargo check -p <crate>` not `--workspace`; for tsc add a smaller `-p` project.\n\
  - If you believe your changes are correct, finalize without further verification.\n\
Then re-emit your final answer. Do not just re-run the same broad command.\n\
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

/// Resolves the verification command for `workspace`. Priority:
///   1. `BITFUN_PATCH_VERIFY_CMD` env override (escape hatch for harnesses).
///   2. Project-defined targets: `make`/`just` `check|test|ci`.
///   3. Language manifests: `go.mod` → `go build`, `Cargo.toml` → `cargo check`,
///      `tsconfig.json` → `tsc --noEmit`, `package.json` scripts → `npm/pnpm/yarn`.
///   4. Parse-only fallback over changed files (.py, .js/.mjs/.cjs, .go).
/// Returns None if nothing applies — verification silently skips in that case.
fn detect_verify_command(workspace: &std::path::Path) -> Option<String> {
    if let Ok(cmd) = std::env::var("BITFUN_PATCH_VERIFY_CMD") {
        let trimmed = cmd.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    const TARGETS: &[&str] = &["check", "test", "ci"];

    if let Some(target) = detect_make_or_just_target(workspace, "Makefile", TARGETS) {
        return Some(format!("make {}", target));
    }
    if let Some(target) = detect_make_or_just_target(workspace, "justfile", TARGETS) {
        return Some(format!("just {}", target));
    }
    if let Some(target) = detect_make_or_just_target(workspace, ".justfile", TARGETS) {
        return Some(format!("just {}", target));
    }

    if workspace.join("go.mod").exists() {
        return Some("go build ./...".to_string());
    }
    if workspace.join("Cargo.toml").exists() {
        return Some("cargo check --workspace --message-format=short".to_string());
    }
    if workspace.join("tsconfig.json").exists() {
        return Some("npx --no-install tsc --noEmit -p .".to_string());
    }
    if let Some(cmd) = detect_package_json_command(workspace) {
        return Some(cmd);
    }

    let changed = changed_files(workspace);
    if !changed.is_empty() {
        if let Some(cmd) = build_parse_only_command(&changed) {
            return Some(cmd);
        }
    }

    None
}

fn detect_make_or_just_target(
    workspace: &std::path::Path,
    file: &str,
    candidates: &[&str],
) -> Option<String> {
    let content = std::fs::read_to_string(workspace.join(file)).ok()?;
    candidates.iter().find_map(|target| {
        let head = format!("{}:", target);
        let mid = format!("\n{}:", target);
        if content.starts_with(&head) || content.contains(&mid) {
            Some(target.to_string())
        } else {
            None
        }
    })
}

fn detect_package_json_command(workspace: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(workspace.join("package.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let scripts = json.get("scripts")?.as_object()?;

    let pm = if workspace.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if workspace.join("yarn.lock").exists() {
        "yarn"
    } else {
        "npm"
    };

    for candidate in ["typecheck", "check", "build"] {
        if scripts.contains_key(candidate) {
            return Some(match pm {
                "npm" => format!("npm run {}", candidate),
                other => format!("{} {}", other, candidate),
            });
        }
    }
    None
}

fn changed_files(workspace: &std::path::Path) -> Vec<String> {
    let output = bitfun_core::util::process_manager::create_command("git")
        .args(["diff", "--name-only", "--diff-filter=AM"])
        .current_dir(workspace)
        .output()
        .ok();
    let output = match output {
        Some(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn build_parse_only_command(files: &[String]) -> Option<String> {
    let mut checks: Vec<String> = Vec::new();
    for file in files {
        let quoted = shell_single_quote(file);
        let lower = file.to_ascii_lowercase();
        if lower.ends_with(".py") {
            checks.push(format!(
                "python3 -c 'import ast,sys; ast.parse(open(sys.argv[1]).read())' {}",
                quoted
            ));
        } else if lower.ends_with(".js")
            || lower.ends_with(".mjs")
            || lower.ends_with(".cjs")
            || lower.ends_with(".jsx")
        {
            checks.push(format!("node --check {}", quoted));
        } else if lower.ends_with(".go") {
            checks.push(format!("gofmt -e {} > /dev/null", quoted));
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
    use super::{write_patch_to_path, ExecMode, TOOL_START_INPUT_PREVIEW_CHARS};
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
}

/// Exec mode implementation
///
/// Single command execution mode (non-interactive).
/// Consumes core events directly from EventQueue.
use anyhow::Result;
use clap::ValueEnum;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ExecOutputFormat {
    Text,
    Json,
    StreamJson,
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
    workspace_path: Option<PathBuf>,
    /// None: no patch output, Some("-"): output to stdout, Some(path): save to file
    output_patch: Option<String>,
    output_format: ExecOutputFormat,
    session_options: ExecSessionOptions,
}

impl ExecMode {
    pub(crate) fn new(
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

    pub(crate) async fn run(&mut self) -> Result<()> {
        tracing::info!(
            agent_type = %self.agent_type,
            message_len = self.message.len(),
            workspace = ?self.workspace_path,
            "Executing command"
        );

        let session_id = self.prepare_session().await.inspect_err(|error| {
            emit_exit_diagnostic(
                ExitKind::SessionCreateFailed,
                &error.to_string(),
                &self.exit_context(None, None),
            );
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

        let turn_id = self
            .agent
            .send_message(self.message.clone(), &self.agent_type)
            .await
            .inspect_err(|error| {
                emit_exit_diagnostic(
                    ExitKind::SendMessageFailed,
                    &error.to_string(),
                    &self.exit_context(Some(&session_id), None),
                );
            })?;
        tracing::info!(session_id = %session_id, turn_id = %turn_id, "Message sent");

        // Consume events from EventQueue until turn completes
        let mut total_tool_calls = 0usize;
        let mut subagent_parent_sessions: HashMap<String, String> = HashMap::new();
        let mut terminal_outcome: Option<Result<()>> = None;

        loop {
            // Wait for events (efficient, uses Notify internally)
            event_queue.wait_for_events().await;
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
        }

        self.wait_for_turn_settlement(&session_id, &turn_id).await;
        self.output_patch_if_needed();
        terminal_outcome.unwrap_or(Ok(()))
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

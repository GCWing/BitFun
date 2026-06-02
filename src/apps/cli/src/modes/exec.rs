/// Exec mode implementation
///
/// Single command execution mode (non-interactive).
/// Consumes core events directly from EventQueue.
use anyhow::Result;
use clap::ValueEnum;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use bitfun_core::agentic::core::SessionState;
use bitfun_events::AgenticEvent;
use tokio::time::{sleep, Instant};

use crate::agent::{agentic_system::AgenticSystem, core_adapter::CoreAgentAdapter, Agent};
use crate::config::CliConfig;
use crate::diagnostics::{emit_exit_diagnostic, ExitContext, ExitKind};

const TOOL_START_INPUT_PREVIEW_CHARS: usize = 4_000;

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

#[derive(Debug, Clone, Serialize)]
struct WorkspaceFileSnapshot {
    size: u64,
    modified_unix_ms: Option<u128>,
    hash: Option<String>,
}

#[derive(Debug, Clone)]
struct WorkspaceSnapshot {
    files: HashMap<String, WorkspaceFileSnapshot>,
    truncated: bool,
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

    fn effective_message(&self) -> String {
        self.eval_guided_message()
    }

    fn eval_guided_message(&self) -> String {
        let experience = Self::eval_experience_playbook();

        format!(
            "\
You are running in an evaluation-oriented non-interactive execution environment. The grader
scores concrete filesystem artifacts, process state, command behavior, and test results after
your turn; final prose is not a substitute for deliverables.

Evaluation rules:
- Identify the exact required deliverables from the task text. Before finishing, verify every
  required path exists, is non-empty when appropriate, and matches the expected format.
- Use only task-authorized checks and your own sanity checks. Do not depend on hidden grader
  files or private test directories as an implementation guide.
- For service tasks, leave the service running after your shell exits. Use a durable background
  launch method, save logs/PIDs when useful, and verify with the real protocol/client, not only a
  listening port.
- If a command reports a missing dependency, either install the smallest viable dependency quickly
  or switch implementation strategy. Do not spend the whole budget repeatedly probing the same
  missing tool.
- If work is taking too long, stop broad exploration and create the smallest artifact or service
  that the verifier can check.
- When the deadline is near, stop analysis and write the required deliverables. Prefer a small,
  verifiable artifact over an unfinished perfect solution.
- For small generated files, use the Write tool with inline content or a non-interactive shell
  command that writes the file. If a file-write attempt fails once, immediately switch strategy.
- End only after an explicit audit of deliverables, services, and verification commands.

Evaluation experience memory:
{experience}

Original task:
{}",
            self.message
        )
    }

    fn eval_experience_playbook() -> &'static str {
        "\
- Budget discipline: if the task has a 900-1200s budget, a single 300s command is already risky.
  Prefer quick probes, bounded scripts, and incremental artifacts over long exploratory scans.
- Artifact-first strategy: once you identify required paths such as answer files, CSVs, model files,
  service scripts, or images, create a minimal syntactically valid version early, then improve it.
  Many zero scores come from missing files, not from imperfect files.
- Early checkpoint: before spending the first quarter of the budget, ensure there is already a
  concrete placeholder or first-pass implementation at each required output path, or a running
  service for service tasks. Improve it in place instead of waiting until the end to create it.
- Verification strategy: when the prompt explicitly provides a checker, example command, schema, or
  benchmark, use it. Otherwise build small task-faithful sanity checks from the stated requirements
  and audit file count, schema, formatting, thresholds, and leftover temporary artifacts.
- Passing means the verifier can read the exact interface it expects. Preserve conventional calling
  forms and plain formats: function inputs should match natural examples, numeric sample files should
  be headerless numeric text, coordinate files should use flat lists when requested, and scripts should
  run from /app without extra arguments unless the task says otherwise.
- Service tasks: start the service with nohup/setsid/background shell, write logs and a PID when
  useful, then verify the real endpoint/protocol from a separate command. Do not stop the service
  before final.
- Once all required deliverables exist and one bounded task-faithful check passes, stop. Do not keep
  exploring, rerunning expensive checks, or waiting on logs after a likely-passing artifact exists.
- Avoid open-ended waiting. Replace sleeps, tail loops, brute-force searches, large builds, and model
  training with bounded probes plus a fallback artifact. If a background process is needed, launch it
  durably and leave it running instead of blocking the final answer.
- Cleanup-sensitive tasks: remove build products and scratch files when the verifier expects only
  named deliverables. Extra files can fail otherwise correct solutions.
- Secret/sanitizer tasks: search recursively for every exposed token pattern and compare against
  expected clean references when provided; one missed variant is a failure.
- Sanitizer tasks must block dangerous variants, not just examples. For HTML/JS, remove event handlers,
  scriptable URLs, script/style/object/embed/iframe payloads, malformed casing, and encoded variants.
- Numeric, image, video, and biology tasks: validate against the actual tolerance or schema, not a
  rough plausibility check. Off-by-one frames, tuple-vs-list CSV cells, or small Tm gaps can be fatal.
- Video/OCR/transcription tasks need high similarity, not a plausible summary. Extract the exact text
  or command sequence, normalize line endings, and compare against visible/caption/audio evidence.
- Biosequence assembly tasks are order-sensitive. Verify translated protein/domain order and exact
  primer/sequence constraints, not only approximate lengths or GC content.
- Heavy training/build tasks: look for deterministic shortcuts, preexisting assets, smaller public
  checks, or direct artifact generation before committing most of the budget to full training. If a
  build or training probe does not produce the required artifact quickly, write the best fallback
  artifact instead of starting another long run.
- Password/cracking/search tasks need a staged strategy: inspect metadata and hints first, try small
  dictionaries/rules, save the best candidate artifact early, and avoid unbounded brute force.
- Image/board/geometry tasks need a decisive representation early. Create the expected answer file or
  move once confidence is good enough; repeated visual probing often runs out the clock.
- When stuck or near the deadline, stop asking new broad questions. Write the best current
  deliverable, run one verification/audit pass, and leave concrete files behind."
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

    fn workspace_is_git(&self) -> bool {
        self.workspace_path
            .as_ref()
            .is_some_and(|workspace| workspace.join(".git").exists())
    }

    fn capture_workspace_snapshot(&self) -> Option<WorkspaceSnapshot> {
        if self.output_patch.is_none() || self.workspace_is_git() {
            return None;
        }

        let workspace = self.workspace_path.as_ref()?;
        let mut snapshot = WorkspaceSnapshot {
            files: HashMap::new(),
            truncated: false,
        };
        Self::capture_workspace_snapshot_inner(workspace, workspace, &mut snapshot);
        Some(snapshot)
    }

    fn capture_workspace_snapshot_inner(
        root: &std::path::Path,
        dir: &std::path::Path,
        snapshot: &mut WorkspaceSnapshot,
    ) {
        const MAX_FILES: usize = 20_000;
        if snapshot.files.len() >= MAX_FILES {
            snapshot.truncated = true;
            return;
        }

        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            if snapshot.files.len() >= MAX_FILES {
                snapshot.truncated = true;
                return;
            }

            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if Self::skip_snapshot_entry(&name) {
                continue;
            }

            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.is_dir() {
                Self::capture_workspace_snapshot_inner(root, &path, snapshot);
                continue;
            }
            if !metadata.is_file() {
                continue;
            }

            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            let rel = rel.to_string_lossy().replace('\\', "/");
            let modified_unix_ms = metadata
                .modified()
                .ok()
                .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis());
            let hash = Self::file_hash_if_small(&path, metadata.len());
            snapshot.files.insert(
                rel,
                WorkspaceFileSnapshot {
                    size: metadata.len(),
                    modified_unix_ms,
                    hash,
                },
            );
        }
    }

    fn skip_snapshot_entry(name: &str) -> bool {
        matches!(
            name,
            ".git" | "target" | "node_modules" | ".venv" | "__pycache__" | ".bitfun"
        )
    }

    fn file_hash_if_small(path: &std::path::Path, size: u64) -> Option<String> {
        const MAX_HASH_BYTES: u64 = 10 * 1024 * 1024;
        if size > MAX_HASH_BYTES {
            return None;
        }
        let bytes = fs::read(path).ok()?;
        Some(format!("{:016x}", Self::fnv1a64(&bytes)))
    }

    fn fnv1a64(bytes: &[u8]) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    fn workspace_manifest_since(&self, before: &WorkspaceSnapshot) -> Option<String> {
        let mut after = WorkspaceSnapshot {
            files: HashMap::new(),
            truncated: false,
        };
        let workspace = self.workspace_path.as_ref()?;
        Self::capture_workspace_snapshot_inner(workspace, workspace, &mut after);

        let mut added = Vec::new();
        let mut modified = Vec::new();
        let mut deleted = Vec::new();

        for (path, after_meta) in &after.files {
            match before.files.get(path) {
                None => added.push(json!({ "path": path, "after": after_meta })),
                Some(before_meta)
                    if before_meta.size != after_meta.size
                        || before_meta.hash != after_meta.hash
                        || before_meta.modified_unix_ms != after_meta.modified_unix_ms =>
                {
                    modified.push(json!({
                        "path": path,
                        "before": before_meta,
                        "after": after_meta,
                    }));
                }
                _ => {}
            }
        }

        for path in before.files.keys() {
            if !after.files.contains_key(path) {
                deleted.push(json!({ "path": path }));
            }
        }

        let manifest = json!({
            "type": "workspace-change-manifest",
            "note": "Workspace is not a git repository; this manifest records file-level changes instead of a git patch.",
            "truncated": before.truncated || after.truncated,
            "added": added,
            "modified": modified,
            "deleted": deleted,
        });
        serde_json::to_string_pretty(&manifest).ok()
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
        let workspace_snapshot = self.capture_workspace_snapshot();

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
            .send_message(self.effective_message(), &self.agent_type)
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

        // Consume events from EventQueue until turn completes
        let mut total_tool_calls = 0usize;
        let mut subagent_parent_sessions: HashMap<String, String> = HashMap::new();
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
            self.agent.route_internal_events(&events).await;

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
        self.output_patch_if_needed(workspace_snapshot.as_ref());
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

    fn output_patch_if_needed(&self, workspace_snapshot: Option<&WorkspaceSnapshot>) {
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
            } else if let Some(manifest) =
                workspace_snapshot.and_then(|snapshot| self.workspace_manifest_since(snapshot))
            {
                let status = if manifest.contains("\"added\": []")
                    && manifest.contains("\"modified\": []")
                    && manifest.contains("\"deleted\": []")
                {
                    "empty_manifest"
                } else {
                    "manifest"
                };
                let value = json!({
                    "type": "patch",
                    "target": output_target,
                    "status": status,
                    "patch": if output_target == "-" { Some(manifest.as_str()) } else { None },
                    "bytes": manifest.len(),
                });
                if self.emit(value).is_err() {
                    eprintln!("Failed to emit patch event");
                }

                if self.output_format != ExecOutputFormat::Text {
                    if output_target != "-" && !manifest.trim().is_empty() {
                        if let Err(e) = write_patch_to_path(output_target, &manifest) {
                            emit_exit_diagnostic(
                                ExitKind::PatchWriteFailed,
                                &e.to_string(),
                                &self.exit_context(None, None),
                            );
                            eprintln!("Failed to save patch manifest: {}", e);
                        }
                    }
                    return;
                }

                println!("\n--- Generating Workspace Change Manifest ---");
                if status == "empty_manifest" {
                    println!("(No file modifications)");
                } else if output_target == "-" {
                    println!("---PATCH_START---");
                    println!("{}", manifest);
                    println!("---PATCH_END---");
                } else {
                    match write_patch_to_path(output_target, &manifest) {
                        Ok(_) => {
                            println!("Patch manifest saved to: {}", output_target);
                            println!("({} bytes)", manifest.len());
                        }
                        Err(e) => {
                            emit_exit_diagnostic(
                                ExitKind::PatchWriteFailed,
                                &e.to_string(),
                                &self.exit_context(None, None),
                            );
                            eprintln!("Failed to save patch manifest: {}", e);
                            println!("---PATCH_START---");
                            println!("{}", manifest);
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

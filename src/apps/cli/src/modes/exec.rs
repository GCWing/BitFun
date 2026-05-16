/// Exec mode implementation
///
/// Single command execution mode (non-interactive).
/// Consumes core events directly from EventQueue.

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

use bitfun_events::AgenticEvent;

use crate::config::CliConfig;
use crate::agent::{Agent, core_adapter::CoreAgentAdapter, agentic_system::AgenticSystem};

pub struct ExecMode {
    #[allow(dead_code)]
    config: CliConfig,
    message: String,
    agent_type: String,
    agent: Arc<CoreAgentAdapter>,
    workspace_path: Option<PathBuf>,
    /// None: no patch output, Some("-"): output to stdout, Some(path): save to file
    output_patch: Option<String>,
}

impl ExecMode {
    pub fn new(
        config: CliConfig,
        message: String,
        agent_type: String,
        agentic_system: &AgenticSystem,
        workspace_path: Option<PathBuf>,
        output_patch: Option<String>,
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
        }
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
            "Executing command, Agent: {}, Message: {}",
            self.agent_type,
            self.message
        );

        println!("Executing: {}", self.message);
        println!();

        // Ensure session and send message
        let session_id = self.agent.ensure_session(&self.agent_type).await?;
        let event_queue = self.agent.event_queue().clone();

        println!("Thinking...");

        let _turn_id = self.agent.send_message(self.message.clone(), &self.agent_type).await?;

        // Consume events from EventQueue until turn completes
        let mut total_tool_calls = 0usize;

        loop {
            // Wait for events (efficient, uses Notify internally)
            event_queue.wait_for_events().await;
            let events = event_queue.dequeue_batch(20).await;

            for envelope in events {
                let event = &envelope.event;

                // Only process events for our session
                if event.session_id() != Some(&session_id) {
                    // Check if this is a subagent event whose parent is in our session
                    if let AgenticEvent::ToolEvent { tool_event, subagent_parent_info, .. } = event {
                        if subagent_parent_info
                            .as_ref()
                            .map(|info| info.session_id.as_str())
                            == Some(session_id.as_str())
                        {
                            use bitfun_events::ToolEventData;
                            match tool_event {
                                ToolEventData::Started { tool_name, .. } => {
                                    println!("   [subagent] {}", tool_name);
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
                                    println!("   [subagent] {} ✓ {}", tool_name, summary);
                                }
                                ToolEventData::Failed { tool_name, error, .. } => {
                                    println!("   [subagent] {} ✗ {}", tool_name, error);
                                }
                                _ => {}
                            }
                        }
                    }
                    continue;
                }

                match event {
                    AgenticEvent::TextChunk { text, .. } => {
                        print!("{}", text);
                        use std::io::Write;
                        std::io::stdout().flush().ok();
                    }

                    AgenticEvent::ThinkingChunk { content, .. } => {
                        // Show thinking in exec mode as dimmed text
                        print!("\x1b[2m{}\x1b[0m", content);
                        use std::io::Write;
                        std::io::stdout().flush().ok();
                    }

                    AgenticEvent::ToolEvent { tool_event, .. } => {
                        use bitfun_events::ToolEventData;
                        match tool_event {
                            ToolEventData::Started { tool_name, .. } => {
                                println!("\nTool call: {}", tool_name);
                                total_tool_calls += 1;
                            }
                            ToolEventData::Progress { message, .. } => {
                                println!("   In progress: {}", message);
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
                                println!("   [+] {} ({}ms): {}", tool_name, duration_ms, summary);
                            }
                            ToolEventData::Failed {
                                tool_name, error, ..
                            } => {
                                println!("   [x] {}: {}", tool_name, error);
                            }
                            _ => {}
                        }
                    }

                    AgenticEvent::DialogTurnCompleted { .. } => {
                        println!("\n");
                        println!("Execution complete");
                        if total_tool_calls > 0 {
                            println!(
                                "\nTool call statistics: {} tools invoked",
                                total_tool_calls
                            );
                        }
                        // Break out of the event loop
                        self.output_patch_if_needed();
                        return Ok(());
                    }

                    AgenticEvent::DialogTurnFailed { error, .. } => {
                        eprintln!("\nExecution failed: {}", error);
                        self.output_patch_if_needed();
                        return Err(anyhow::anyhow!("Execution failed: {}", error));
                    }

                    AgenticEvent::DialogTurnCancelled { .. } => {
                        println!("\nExecution cancelled");
                        self.output_patch_if_needed();
                        return Ok(());
                    }

                    AgenticEvent::SystemError { error, .. } => {
                        eprintln!("\nSystem error: {}", error);
                        self.output_patch_if_needed();
                        return Err(anyhow::anyhow!("System error: {}", error));
                    }

                    _ => {}
                }
            }
        }
    }

    fn output_patch_if_needed(&self) {
        if let Some(ref output_target) = self.output_patch {
            println!("\n--- Generating Patch ---");
            if let Some(patch) = self.get_git_diff() {
                if patch.trim().is_empty() {
                    println!("(No file modifications)");
                } else if output_target == "-" {
                    println!("---PATCH_START---");
                    println!("{}", patch);
                    println!("---PATCH_END---");
                } else {
                    match std::fs::write(output_target, &patch) {
                        Ok(_) => {
                            println!("Patch saved to: {}", output_target);
                            println!("({} bytes)", patch.len());
                        }
                        Err(e) => {
                            eprintln!("Failed to save patch: {}", e);
                            println!("---PATCH_START---");
                            println!("{}", patch);
                            println!("---PATCH_END---");
                        }
                    }
                }
            } else {
                println!("(Unable to generate patch)");
            }
        }
    }
}

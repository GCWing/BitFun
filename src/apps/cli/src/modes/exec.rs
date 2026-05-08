use crate::agent::{core_adapter::CoreAgentAdapter, Agent, AgentEvent, AgenticSystem};
use crate::config::CliConfig;
/// Exec mode implementation
///
/// Single command execution mode
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct ExecMode {
    #[allow(dead_code)]
    config: CliConfig,
    message: String,
    agent: Arc<dyn Agent>,
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
        // Use the real CoreAgentAdapter
        let agent = Arc::new(CoreAgentAdapter::new(
            agent_type,
            agentic_system.coordinator.clone(),
            agentic_system.event_queue.clone(),
            workspace_path.clone(),
        )) as Arc<dyn Agent>;

        Self {
            config,
            message,
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
            self.agent.name(),
            self.message
        );

        println!("Executing: {}", self.message);
        println!();

        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let agent = self.agent.clone();
        let message = self.message.clone();

        let handle = tokio::spawn(async move { agent.process_message(message, event_tx).await });

        while let Some(event) = event_rx.recv().await {
            match event {
                AgentEvent::Thinking => {
                    println!("Thinking...");
                }
                AgentEvent::TextChunk(chunk) => {
                    print!("{}", chunk);
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
                AgentEvent::ToolCallStart {
                    tool_name,
                    parameters: _,
                } => {
                    println!("\nTool call: {}", tool_name);
                }
                AgentEvent::ToolCallProgress {
                    tool_name: _,
                    message,
                } => {
                    println!("   In progress: {}", message);
                }
                AgentEvent::ToolCallComplete {
                    tool_name,
                    result,
                    success,
                } => {
                    if success {
                        println!("   [+] {}: {}", tool_name, result);
                    } else {
                        println!("   [x] {}: {}", tool_name, result);
                    }
                }
                AgentEvent::Done => {
                    println!("\n");
                    break;
                }
                AgentEvent::Error(err) => {
                    eprintln!("\nError: {}", err);
                    break;
                }
            }
        }

        let result = handle.await;

        match result {
            Ok(Ok(response)) => {
                if response.success {
                    println!("Execution complete");
                    if !response.tool_calls.is_empty() {
                        println!(
                            "\nTool call statistics: {} tools invoked",
                            response.tool_calls.len()
                        );
                    }
                } else {
                    println!("Execution failed");
                }
            }
            Ok(Err(e)) => {
                eprintln!("Execution failed: {}", e);
                return Err(e);
            }
            Err(e) => {
                eprintln!("Task failed: {}", e);
                return Err(e.into());
            }
        }

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

        Ok(())
    }
}

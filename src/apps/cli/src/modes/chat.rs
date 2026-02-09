/// Chat mode implementation
/// 
/// Interactive chat mode with TUI interface

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::CliConfig;
use crate::session::Session;
use crate::ui::chat::ChatView;
use crate::ui::theme::Theme;
use crate::ui::{init_terminal, restore_terminal};
use crate::agent::{Agent, core_adapter::CoreAgentAdapter, agentic_system::AgenticSystem};
use uuid;

/// Chat mode exit reason
#[derive(Debug, Clone, PartialEq)]
pub enum ChatExitReason {
    /// User exits program
    Quit,
    /// Return to main menu
    BackToMenu,
}

pub struct ChatMode {
    config: CliConfig,
    agent_name: String,
    workspace: Option<String>,
    agent: Arc<dyn Agent>,
}

impl ChatMode {
    pub fn new(
        config: CliConfig, 
        agent_name: String, 
        workspace: Option<String>,
        agentic_system: &AgenticSystem,
    ) -> Self {
        // Use the real CoreAgentAdapter
        let agent = Arc::new(CoreAgentAdapter::new(
            agent_name.clone(),
            agentic_system.coordinator.clone(),
            agentic_system.event_queue.clone(),
        )) as Arc<dyn Agent>;
        
        Self {
            config,
            agent_name,
            workspace,
            agent,
        }
    }

    pub fn run(
        &mut self,
        existing_terminal: Option<Terminal<CrosstermBackend<io::Stdout>>>,
    ) -> Result<ChatExitReason> {
        tracing::info!("Starting Chat mode, Agent: {}", self.agent_name);
        if let Some(ws) = &self.workspace {
            tracing::info!("Workspace: {}", ws);
        }

        let mut terminal = match existing_terminal {
            Some(t) => t,
            None => init_terminal()?,
        };
        let session = Session::new(self.agent_name.clone(), self.workspace.clone());
        
        let theme = match self.config.ui.theme.as_str() {
            "light" => Theme::light(),
            _ => Theme::dark(),
        };
        let mut chat_view = ChatView::new(session, theme);

        let rt_handle = tokio::runtime::Handle::current();
        let (response_tx, mut response_rx) = mpsc::unbounded_channel::<crate::agent::AgentResponse>();
        let (stream_tx, mut stream_rx) = mpsc::unbounded_channel::<crate::agent::AgentEvent>();
        
        let mut pending_response: Option<tokio::task::JoinHandle<Result<()>>> = None;
        let mut current_assistant_message_text = String::new();
        let mut current_tool_map: std::collections::HashMap<String, crate::session::ToolCall> = std::collections::HashMap::new();

        let mut exit_reason = ChatExitReason::Quit;
        let mut should_quit = false;
        
        while !should_quit {
            terminal.draw(|frame| {
                chat_view.render(frame);
            })?;

            while let Ok(event) = stream_rx.try_recv() {
                use crate::agent::AgentEvent;
                use crate::session::{ToolCall, ToolCallStatus};
                
                match event {
                    AgentEvent::TextChunk(chunk) => {
                        current_assistant_message_text.push_str(&chunk);
                        chat_view.session.update_last_message_text_flow(
                            current_assistant_message_text.clone(),
                            true
                        );
                    }
                    
                    AgentEvent::ToolCallStart { tool_name, parameters } => {
                        if !current_assistant_message_text.is_empty() {
                            chat_view.session.update_last_message_text_flow(
                                current_assistant_message_text.clone(),
                                false
                            );
                        }
                        
                        let tool_id = uuid::Uuid::new_v4().to_string();
                        let tool_call = ToolCall {
                            tool_id: Some(tool_id.clone()),
                            tool_name,
                            parameters,
                            result: None,
                            status: ToolCallStatus::Running,
                            progress: Some(0.0),
                            progress_message: None,
                            duration_ms: None,
                        };
                        
                        current_tool_map.insert(tool_id, tool_call.clone());
                        chat_view.session.add_tool_to_last_message(tool_call);
                    }
                    
                    AgentEvent::ToolCallProgress { tool_name, message } => {
                        for (tool_id, tool) in current_tool_map.iter() {
                            if tool.tool_name == tool_name {
                                let tid = tool_id.clone();
                                chat_view.session.update_tool_in_last_message(&tid, |t| {
                                    t.progress_message = Some(message.clone());
                                });
                                break;
                            }
                        }
                    }
                    
                    AgentEvent::ToolCallComplete { tool_name, result, success } => {
                        for (tool_id, tool) in current_tool_map.iter_mut() {
                            if tool.tool_name == tool_name && tool.status == ToolCallStatus::Running {
                                tool.status = if success {
                                    ToolCallStatus::Success
                                } else {
                                    ToolCallStatus::Failed
                                };
                                tool.result = Some(result.clone());
                                tool.progress = Some(1.0);
                                
                                let tid = tool_id.clone();
                                chat_view.session.update_tool_in_last_message(&tid, |t| {
                                    t.status = tool.status.clone();
                                    t.result = Some(result.clone());
                                    t.progress = Some(1.0);
                                });
                                break;
                            }
                        }
                    }
                    
                    AgentEvent::Done => {
                        if !current_assistant_message_text.is_empty() {
                            chat_view.session.update_last_message_text_flow(
                                current_assistant_message_text.clone(),
                                false
                            );
                        }
                    }
                    
                    AgentEvent::Error(err) => {
                        chat_view.set_status(Some(format!("Error: {}", err)));
                    }
                    
                    _ => {}
                }
            }

            if let Ok(_response) = response_rx.try_recv() {
                current_assistant_message_text.clear();
                current_tool_map.clear();
                chat_view.set_loading(false);
                chat_view.set_status(None);
            }
            
            if let Some(handle) = &pending_response {
                if handle.is_finished() {
                    pending_response = None;
                    tracing::debug!("Agent response task completed");
                }
            }

            if crossterm::event::poll(Duration::from_millis(16))? {
                if let Ok(event) = crossterm::event::read() {
                    match event {
                        Event::Key(key) => {
                            if let Some(reason) = self.handle_key_event(
                                key, 
                                &mut chat_view, 
                                &mut pending_response, 
                                &rt_handle, 
                                &response_tx,
                                &stream_tx,
                                &mut current_assistant_message_text,
                                &mut current_tool_map,
                            )? {
                                should_quit = true;
                                exit_reason = reason;
                            }
                        }
                        Event::Resize(_, _) => {}
                        _ => {}
                    }
                }
            }

            if self.config.behavior.auto_save && pending_response.is_none() {
                chat_view.session.save()?;
            }
        }

        restore_terminal(terminal)?;
        chat_view.session.save()?;
        tracing::info!("Session saved");

        Ok(exit_reason)
    }

    fn handle_key_event(
        &self, 
        key: KeyEvent, 
        chat_view: &mut ChatView,
        pending_response: &mut Option<tokio::task::JoinHandle<Result<()>>>,
        rt_handle: &tokio::runtime::Handle,
        response_tx: &mpsc::UnboundedSender<crate::agent::AgentResponse>,
        stream_tx: &mpsc::UnboundedSender<crate::agent::AgentEvent>,
        current_assistant_message_text: &mut String,
        current_tool_map: &mut std::collections::HashMap<String, crate::session::ToolCall>,
    ) -> Result<Option<ChatExitReason>> {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return Ok(None);
        }
        
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                tracing::info!("User requested quit");
                return Ok(Some(ChatExitReason::Quit));
            }

            (KeyCode::Char('m'), KeyModifiers::CONTROL) => {
                tracing::info!("User returning to main menu");
                chat_view.set_status(Some("Returning to main menu...".to_string()));
                return Ok(Some(ChatExitReason::BackToMenu));
            }

            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                chat_view.clear_screen();
            }

            (KeyCode::Enter, _) => {
                if pending_response.is_some() {
                    return Ok(None);
                }

                if let Some(input) = chat_view.send_input() {
                    tracing::info!("User input: {}", input);
                    
                    if input.starts_with('/') {
                        self.handle_command(&input, chat_view)?;
                        return Ok(None);
                    }
                    
                    chat_view.set_loading(true);
                    chat_view.set_status(Some(format!("{} is thinking...", self.agent_name)));
                    chat_view.session.add_message("assistant".to_string(), String::new());
                    
                    current_assistant_message_text.clear();
                    current_tool_map.clear();
                    
                    let agent = Arc::clone(&self.agent);
                    let input_clone = input.clone();
                    let resp_tx = response_tx.clone();
                    let stream_tx_clone = stream_tx.clone();
                    
                    let handle_clone = rt_handle.spawn(async move {
                        match agent.process_message(input_clone, stream_tx_clone.clone()).await {
                            Ok(response) => {
                                tracing::info!("Agent response complete: {} tool calls", response.tool_calls.len());
                                let _ = resp_tx.send(response);
                            }
                            Err(e) => {
                                tracing::error!("Agent processing failed: {}", e);
                                let _ = stream_tx_clone.send(crate::agent::AgentEvent::Error(e.to_string()));
                                let _ = resp_tx.send(crate::agent::AgentResponse {
                                    tool_calls: vec![],
                                    success: false,
                                });
                            }
                        }
                        Ok(())
                    });
                    
                    *pending_response = Some(handle_clone);
                }
            }

            (KeyCode::Backspace, _) => {
                chat_view.handle_backspace();
            }

            (KeyCode::Left, _) => {
                chat_view.move_cursor_left();
            }
            (KeyCode::Right, _) => {
                chat_view.move_cursor_right();
            }

            (KeyCode::Up, _) => {
                if chat_view.browse_mode {
                    chat_view.scroll_up(1);
                } else {
                    chat_view.history_prev();
                }
            }
            (KeyCode::Down, _) => {
                if chat_view.browse_mode {
                    chat_view.scroll_down(1);
                } else {
                    chat_view.history_next();
                }
            }

            (KeyCode::Home, KeyModifiers::CONTROL) => {
                chat_view.scroll_to_top();
                chat_view.set_status(Some("Jumped to conversation top".to_string()));
            }

            (KeyCode::End, KeyModifiers::CONTROL) => {
                chat_view.scroll_to_bottom();
                chat_view.set_status(Some("Jumped to conversation bottom".to_string()));
            }

            (KeyCode::Home, _) => {
                chat_view.cursor = 0;
            }

            (KeyCode::End, _) => {
                chat_view.cursor = chat_view.input.len();
            }

            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                chat_view.input.clear();
                chat_view.cursor = 0;
            }

            (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                chat_view.toggle_browse_mode();
                let status_msg = if chat_view.browse_mode {
                    "Entered browse mode, use ↑↓ or PageUp/PageDown to scroll"
                } else {
                    "Exited browse mode, back to normal input"
                };
                chat_view.set_status(Some(status_msg.to_string()));
            }

            (KeyCode::PageUp, _) => {
                chat_view.scroll_up(10);
            }

            (KeyCode::PageDown, _) => {
                chat_view.scroll_down(10);
            }

            (KeyCode::Esc, _) => {
                if chat_view.browse_mode {
                    chat_view.scroll_to_bottom();
                    chat_view.set_status(Some("Exited browse mode".to_string()));
                } else {
                    tracing::info!("User returning to main menu via Esc");
                    return Ok(Some(ChatExitReason::BackToMenu));
                }
            }

            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                if !c.is_control() && c != '\u{0}' {
                    chat_view.handle_char(c);
                }
            }

            _ => {}
        }

        Ok(None)
    }

    /// Handle shortcut commands
    fn handle_command(&self, command: &str, chat_view: &mut ChatView) -> Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "/help" => {
                chat_view.add_message(
                    "system".to_string(),
                    "Available commands:\n\
                     /help - Show help\n\
                     /clear - Clear conversation\n\
                     /agents - List available agents\n\
                     /switch <agent> - Switch agent\n\
                     /history - Show history\n\
                     /export - Export session".to_string(),
                );
            }
            "/clear" => {
                chat_view.clear_screen();
                chat_view.set_status(Some("Conversation cleared".to_string()));
            }
            "/agents" => {
                chat_view.add_message(
                    "system".to_string(),
                    "Available Agents:\n\
                     • agentic - General purpose agent\n\
                     • code-writer - Code writing expert\n\
                     • test-writer - Test writing expert\n\
                     • docs-writer - Documentation expert\n\
                     • rust-specialist - Rust expert\n\
                     • visual-debugger - Visual debugging expert".to_string(),
                );
            }
            "/switch" => {
                if parts.len() > 1 {
                    chat_view.add_message(
                        "system".to_string(),
                        format!("Warning: Agent switching feature coming soon\nTip: Use `bitfun chat --agent {}` to start a new session", parts[1]),
                    );
                } else {
                    chat_view.add_message(
                        "system".to_string(),
                        "Usage: /switch <agent>".to_string(),
                    );
                }
            }
            "/history" => {
                chat_view.add_message(
                    "system".to_string(),
                    format!("Current session statistics:\n\
                             • Messages: {}\n\
                             • Tool calls: {}\n\
                             • Files modified: {}",
                            chat_view.session.metadata.message_count,
                            chat_view.session.metadata.tool_calls,
                            chat_view.session.metadata.files_modified),
                );
            }
            "/export" => {
                chat_view.add_message(
                    "system".to_string(),
                    format!("Session auto-saved to: ~/.config/bitfun/sessions/{}.json", chat_view.session.id),
                );
            }
            _ => {
                chat_view.add_message(
                    "system".to_string(),
                    format!("Unknown command: {}\nUse /help to see available commands", parts[0]),
                );
            }
        }

        Ok(())
    }
}


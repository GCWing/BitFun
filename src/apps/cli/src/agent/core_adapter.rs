//! Core Agent adapter
//!
//! Adapts bitfun-core's Agentic system to CLI's Agent interface

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::{Agent, AgentEvent, AgentResponse};
use crate::session::{ToolCall, ToolCallStatus};
use bitfun_core::agentic::coordination::{
    ConversationCoordinator, DialogSubmissionPolicy, DialogTriggerSource,
};
use bitfun_core::agentic::core::SessionConfig;
use bitfun_core::agentic::events::EventQueue;
use bitfun_events::{AgenticEvent as CoreEvent, ToolEventData};

/// Core-based Agent implementation
pub struct CoreAgentAdapter {
    name: String,
    agent_type: String,
    coordinator: Arc<ConversationCoordinator>,
    event_queue: Arc<EventQueue>,
    workspace_path: Option<PathBuf>,
    session_id: Option<String>,
}

impl CoreAgentAdapter {
    pub fn new(
        agent_type: String,
        coordinator: Arc<ConversationCoordinator>,
        event_queue: Arc<EventQueue>,
        workspace_path: Option<PathBuf>,
    ) -> Self {
        let name = match agent_type.as_str() {
            "agentic" => "Fang",
            _ => "AI Assistant",
        };

        Self {
            name: name.to_string(),
            agent_type: agent_type.clone(),
            coordinator,
            event_queue,
            workspace_path,
            session_id: None,
        }
    }

    async fn ensure_session(&mut self) -> Result<String> {
        if let Some(session_id) = &self.session_id {
            return Ok(session_id.clone());
        }

        let workspace_path = self
            .workspace_path
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .map(|path| path.to_string_lossy().to_string());

        let session = self
            .coordinator
            .create_session(
                format!(
                    "CLI Session - {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                ),
                self.agent_type.clone(),
                SessionConfig {
                    workspace_path,
                    ..Default::default()
                },
            )
            .await?;

        self.session_id = Some(session.session_id.clone());
        tracing::info!("Created session: {}", session.session_id);

        Ok(session.session_id)
    }
}

#[async_trait::async_trait]
impl Agent for CoreAgentAdapter {
    async fn process_message(
        &self,
        message: String,
        event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> Result<AgentResponse> {
        let mut self_mut = Self {
            name: self.name.clone(),
            agent_type: self.agent_type.clone(),
            coordinator: self.coordinator.clone(),
            event_queue: self.event_queue.clone(),
            workspace_path: self.workspace_path.clone(),
            session_id: self.session_id.clone(),
        };

        let session_id = self_mut.ensure_session().await?;
        tracing::info!("Processing message: {}", message);

        let _ = event_tx.send(AgentEvent::Thinking);
        self.coordinator
            .start_dialog_turn(
                session_id.clone(),
                message.clone(),
                None,
                None,
                self.agent_type.clone(),
                None,
                DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
            )
            .await?;

        let mut accumulated_text = String::new();
        let mut tool_map: std::collections::HashMap<String, ToolCall> =
            std::collections::HashMap::new();

        let event_queue = self.event_queue.clone();
        let session_id_clone = session_id.clone();

        loop {
            let events = event_queue.dequeue_batch(10).await;

            if events.is_empty() {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                continue;
            }

            for envelope in events {
                let event = envelope.event;

                if event.session_id() != Some(&session_id_clone) {
                    continue;
                }

                tracing::debug!("Received event: {:?}", event);

                match event {
                    CoreEvent::TextChunk { text, .. } => {
                        accumulated_text.push_str(&text);
                        let _ = event_tx.send(AgentEvent::TextChunk(text));
                    }

                    CoreEvent::ToolEvent { tool_event, .. } => match tool_event {
                        ToolEventData::EarlyDetected { tool_id, tool_name } => {
                            tool_map.insert(
                                tool_id.clone(),
                                ToolCall {
                                    tool_id: Some(tool_id),
                                    tool_name: tool_name.clone(),
                                    parameters: serde_json::Value::Null,
                                    result: None,
                                    status: ToolCallStatus::EarlyDetected,
                                    progress: None,
                                    progress_message: None,
                                    duration_ms: None,
                                },
                            );
                        }

                        ToolEventData::ParamsPartial {
                            tool_id,
                            tool_name: _,
                            params,
                        } => {
                            if let Some(tool) = tool_map.get_mut(&tool_id) {
                                tool.status = ToolCallStatus::ParamsPartial;
                                tool.progress_message = Some(params);
                            }
                        }

                        ToolEventData::Queued {
                            tool_id,
                            tool_name: _,
                            position,
                        } => {
                            if let Some(tool) = tool_map.get_mut(&tool_id) {
                                tool.status = ToolCallStatus::Queued;
                                tool.progress_message =
                                    Some(format!("Queue position: {}", position));
                            }
                        }

                        ToolEventData::Waiting {
                            tool_id,
                            tool_name: _,
                            dependencies,
                        } => {
                            if let Some(tool) = tool_map.get_mut(&tool_id) {
                                tool.status = ToolCallStatus::Waiting;
                                tool.progress_message =
                                    Some(format!("Waiting for: {:?}", dependencies));
                            }
                        }

                        ToolEventData::Started {
                            tool_id,
                            tool_name,
                            params,
                        } => {
                            tool_map.entry(tool_id.clone()).or_insert_with(|| ToolCall {
                                tool_id: Some(tool_id.clone()),
                                tool_name: tool_name.clone(),
                                parameters: params.clone(),
                                result: None,
                                status: ToolCallStatus::Running,
                                progress: Some(0.0),
                                progress_message: None,
                                duration_ms: None,
                            });

                            let _ = event_tx.send(AgentEvent::ToolCallStart {
                                tool_name,
                                parameters: params,
                            });
                        }

                        ToolEventData::Progress {
                            tool_id,
                            tool_name,
                            message,
                            percentage,
                        } => {
                            if let Some(tool) = tool_map.get_mut(&tool_id) {
                                tool.progress = Some(percentage);
                                tool.progress_message = Some(message.clone());
                            }

                            let _ =
                                event_tx.send(AgentEvent::ToolCallProgress { tool_name, message });
                        }

                        ToolEventData::Streaming {
                            tool_id,
                            tool_name: _,
                            chunks_received,
                        } => {
                            if let Some(tool) = tool_map.get_mut(&tool_id) {
                                tool.status = ToolCallStatus::Streaming;
                                tool.progress_message =
                                    Some(format!("Received {} chunks", chunks_received));
                            }
                        }

                        ToolEventData::ConfirmationNeeded {
                            tool_id,
                            tool_name: _,
                            params: _,
                        } => {
                            if let Some(tool) = tool_map.get_mut(&tool_id) {
                                tool.status = ToolCallStatus::ConfirmationNeeded;
                                tool.progress_message =
                                    Some("Waiting for user confirmation".to_string());
                            }
                        }

                        ToolEventData::Confirmed {
                            tool_id,
                            tool_name: _,
                        } => {
                            if let Some(tool) = tool_map.get_mut(&tool_id) {
                                tool.status = ToolCallStatus::Confirmed;
                            }
                        }

                        ToolEventData::Rejected {
                            tool_id,
                            tool_name: _,
                        } => {
                            if let Some(tool) = tool_map.get_mut(&tool_id) {
                                tool.status = ToolCallStatus::Rejected;
                                tool.result = Some("User rejected execution".to_string());
                            }
                        }

                        ToolEventData::Completed {
                            tool_id,
                            tool_name,
                            result,
                            result_for_assistant: _,
                            duration_ms,
                        } => {
                            let result_str = serde_json::to_string(&result)
                                .unwrap_or_else(|_| "Success".to_string());

                            if let Some(tool) = tool_map.get_mut(&tool_id) {
                                tool.status = ToolCallStatus::Success;
                                tool.result = Some(result_str.clone());
                                tool.progress = Some(1.0);
                                tool.duration_ms = Some(duration_ms);
                            }

                            let _ = event_tx.send(AgentEvent::ToolCallComplete {
                                tool_name,
                                result: result_str,
                                success: true,
                            });
                        }

                        ToolEventData::Failed {
                            tool_id,
                            tool_name,
                            error,
                        } => {
                            if let Some(tool) = tool_map.get_mut(&tool_id) {
                                tool.status = ToolCallStatus::Failed;
                                tool.result = Some(error.clone());
                            }

                            let _ = event_tx.send(AgentEvent::ToolCallComplete {
                                tool_name,
                                result: error,
                                success: false,
                            });
                        }

                        ToolEventData::Cancelled {
                            tool_id,
                            tool_name: _,
                            reason,
                        } => {
                            if let Some(tool) = tool_map.get_mut(&tool_id) {
                                tool.status = ToolCallStatus::Cancelled;
                                tool.result = Some(reason);
                            }
                        }

                        _ => {}
                    },

                    CoreEvent::DialogTurnCompleted { .. } => {
                        tracing::info!("Dialog turn completed");
                        let _ = event_tx.send(AgentEvent::Done);
                        let tool_calls: Vec<ToolCall> = tool_map.into_values().collect();

                        return Ok(AgentResponse {
                            tool_calls,
                            success: true,
                        });
                    }

                    CoreEvent::DialogTurnFailed { error, .. } => {
                        tracing::error!("Execution error: {}", error);
                        let _ = event_tx.send(AgentEvent::Error(error.clone()));
                        let tool_calls: Vec<ToolCall> = tool_map.into_values().collect();

                        return Ok(AgentResponse {
                            tool_calls,
                            success: false,
                        });
                    }

                    CoreEvent::SystemError { error, .. } => {
                        tracing::error!("System error: {}", error);
                        let _ = event_tx.send(AgentEvent::Error(error.clone()));
                        let tool_calls: Vec<ToolCall> = tool_map.into_values().collect();

                        return Ok(AgentResponse {
                            tool_calls,
                            success: false,
                        });
                    }

                    _ => {
                        tracing::debug!("Ignoring event: {:?}", event);
                    }
                }
            }
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

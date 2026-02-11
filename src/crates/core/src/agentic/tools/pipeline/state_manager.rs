//! Tool state manager
//!
//! Manages the status and lifecycle of tool execution tasks

use super::types::ToolTask;
use crate::agentic::core::ToolExecutionState;
use crate::agentic::events::{AgenticEvent, EventPriority, EventQueue, ToolEventData};
use dashmap::DashMap;
use log::debug;
use std::sync::Arc;

/// Tool state manager
pub struct ToolStateManager {
    /// Tool task status (by tool ID)
    tasks: Arc<DashMap<String, ToolTask>>,

    /// Event queue
    event_queue: Arc<EventQueue>,
}

impl ToolStateManager {
    pub fn new(event_queue: Arc<EventQueue>) -> Self {
        Self {
            tasks: Arc::new(DashMap::new()),
            event_queue,
        }
    }

    /// Create task
    pub async fn create_task(&self, task: ToolTask) -> String {
        let tool_id = task.tool_call.tool_id.clone();
        self.tasks.insert(tool_id.clone(), task);
        tool_id
    }

    /// Update task state
    pub async fn update_state(&self, tool_id: &str, new_state: ToolExecutionState) {
        if let Some(mut task) = self.tasks.get_mut(tool_id) {
            let old_state = task.state.clone();
            task.state = new_state.clone();

            // Update timestamp
            match &new_state {
                ToolExecutionState::Running { .. } | ToolExecutionState::Streaming { .. } => {
                    task.started_at = Some(std::time::SystemTime::now());
                }
                ToolExecutionState::Completed { .. }
                | ToolExecutionState::Failed { .. }
                | ToolExecutionState::Cancelled { .. } => {
                    task.completed_at = Some(std::time::SystemTime::now());
                }
                _ => {}
            }

            debug!(
                "Tool state changed: tool_id={}, old_state={:?}, new_state={:?}",
                tool_id,
                format!("{:?}", old_state).split('{').next().unwrap_or(""),
                format!("{:?}", new_state).split('{').next().unwrap_or("")
            );

            // Send state change event
            self.emit_state_change_event(task.clone()).await;
        }
    }

    /// Get task
    pub fn get_task(&self, tool_id: &str) -> Option<ToolTask> {
        self.tasks.get(tool_id).map(|t| t.clone())
    }

    /// Update task arguments
    pub fn update_task_arguments(&self, tool_id: &str, new_arguments: serde_json::Value) {
        if let Some(mut task) = self.tasks.get_mut(tool_id) {
            debug!(
                "Updated tool arguments: tool_id={}, old_args={:?}, new_args={:?}",
                tool_id, task.tool_call.arguments, new_arguments
            );
            task.tool_call.arguments = new_arguments;
        }
    }

    /// Get all tasks of a session
    pub fn get_session_tasks(&self, session_id: &str) -> Vec<ToolTask> {
        self.tasks
            .iter()
            .filter(|entry| entry.value().context.session_id == session_id)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get all tasks of a dialog turn
    pub fn get_dialog_turn_tasks(&self, dialog_turn_id: &str) -> Vec<ToolTask> {
        self.tasks
            .iter()
            .filter(|entry| entry.value().context.dialog_turn_id == dialog_turn_id)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Delete task
    pub fn remove_task(&self, tool_id: &str) {
        self.tasks.remove(tool_id);
    }

    /// Clear all tasks of a session
    pub fn clear_session(&self, session_id: &str) {
        let to_remove: Vec<_> = self
            .tasks
            .iter()
            .filter(|entry| entry.value().context.session_id == session_id)
            .map(|entry| entry.key().clone())
            .collect();

        for tool_id in to_remove {
            self.tasks.remove(&tool_id);
        }

        debug!("Cleared session tool tasks: session_id={}", session_id);
    }

    /// Send state change event (full version)
    async fn emit_state_change_event(&self, task: ToolTask) {
        let tool_event = match &task.state {
            ToolExecutionState::Queued { position } => ToolEventData::Queued {
                tool_id: task.tool_call.tool_id.clone(),
                tool_name: task.tool_call.tool_name.clone(),
                position: *position,
            },

            ToolExecutionState::Waiting { dependencies } => ToolEventData::Waiting {
                tool_id: task.tool_call.tool_id.clone(),
                tool_name: task.tool_call.tool_name.clone(),
                dependencies: dependencies.clone(),
            },

            ToolExecutionState::Running { .. } => ToolEventData::Started {
                tool_id: task.tool_call.tool_id.clone(),
                tool_name: task.tool_call.tool_name.clone(),
                params: task.tool_call.arguments.clone(),
            },

            ToolExecutionState::Streaming {
                chunks_received, ..
            } => ToolEventData::Streaming {
                tool_id: task.tool_call.tool_id.clone(),
                tool_name: task.tool_call.tool_name.clone(),
                chunks_received: *chunks_received,
            },

            ToolExecutionState::AwaitingConfirmation { params, .. } => {
                ToolEventData::ConfirmationNeeded {
                    tool_id: task.tool_call.tool_id.clone(),
                    tool_name: task.tool_call.tool_name.clone(),
                    params: params.clone(),
                }
            }

            ToolExecutionState::Completed {
                result,
                duration_ms,
            } => ToolEventData::Completed {
                tool_id: task.tool_call.tool_id.clone(),
                tool_name: task.tool_call.tool_name.clone(),
                result: result.content(),
                duration_ms: *duration_ms,
            },

            ToolExecutionState::Failed {
                error,
                is_retryable: _,
            } => ToolEventData::Failed {
                tool_id: task.tool_call.tool_id.clone(),
                tool_name: task.tool_call.tool_name.clone(),
                error: error.clone(),
            },

            ToolExecutionState::Cancelled { reason } => ToolEventData::Cancelled {
                tool_id: task.tool_call.tool_id.clone(),
                tool_name: task.tool_call.tool_name.clone(),
                reason: reason.clone(),
            },
        };

        // Determine priority based on tool event type
        let priority = match &task.state {
            // Critical state change: High priority (user needs to see immediately)
            ToolExecutionState::Running { .. }                  // Start execution
            | ToolExecutionState::AwaitingConfirmation { .. }   // Need confirmation
            | ToolExecutionState::Completed { .. }              // Completed
            | ToolExecutionState::Failed { .. }                 // Failed
            => EventPriority::High,
            
            // Cancel event: Critical priority (need immediate feedback)
            ToolExecutionState::Cancelled { .. } => EventPriority::Critical,
            
            // Progress state: Normal priority (avoid blocking critical events)
            ToolExecutionState::Queued { .. }
            | ToolExecutionState::Waiting { .. }
            | ToolExecutionState::Streaming { .. }
            => EventPriority::Normal,
        };

        let event_subagent_parent_info = task.context.subagent_parent_info.map(|info| info.into());
        let event = AgenticEvent::ToolEvent {
            session_id: task.context.session_id,
            turn_id: task.context.dialog_turn_id,
            tool_event,
            subagent_parent_info: event_subagent_parent_info,
        };

        let _ = self.event_queue.enqueue(event, Some(priority)).await;
    }

    /// Get statistics
    pub fn get_stats(&self) -> ToolStats {
        let tasks: Vec<_> = self.tasks.iter().map(|e| e.value().clone()).collect();

        let mut stats = ToolStats::default();
        stats.total = tasks.len();

        for task in tasks {
            match task.state {
                ToolExecutionState::Queued { .. } => stats.queued += 1,
                ToolExecutionState::Waiting { .. } => stats.waiting += 1,
                ToolExecutionState::Running { .. } => stats.running += 1,
                ToolExecutionState::Streaming { .. } => stats.streaming += 1,
                ToolExecutionState::AwaitingConfirmation { .. } => stats.awaiting_confirmation += 1,
                ToolExecutionState::Completed { .. } => stats.completed += 1,
                ToolExecutionState::Failed { .. } => stats.failed += 1,
                ToolExecutionState::Cancelled { .. } => stats.cancelled += 1,
            }
        }

        stats
    }
}

/// Tool statistics
#[derive(Debug, Clone, Default)]
pub struct ToolStats {
    pub total: usize,
    pub queued: usize,
    pub waiting: usize,
    pub running: usize,
    pub streaming: usize,
    pub awaiting_confirmation: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

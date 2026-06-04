use crate::agentic::tools::framework::ToolUseContext;
use crate::infrastructure::events::event_system::{
    get_global_event_system, BackendEvent::ToolExecutionProgress,
};
use crate::util::types::event::ToolExecutionProgressInfo;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const PROGRESS_FLUSH_INTERVAL_MS: u64 = 60;
const PROGRESS_FLUSH_MAX_CHARS: usize = 16 * 1024;
const PROGRESS_CHANNEL_CAPACITY: usize = 256;

pub(super) struct ExecOutputProgressBridge {
    tx: mpsc::Sender<String>,
    task: JoinHandle<()>,
}

impl ExecOutputProgressBridge {
    pub(super) fn start(context: &ToolUseContext, tool_name: &str) -> Option<Self> {
        let tool_use_id = context.tool_call_id.clone()?;
        let (tx, mut rx) = mpsc::channel::<String>(PROGRESS_CHANNEL_CAPACITY);
        let tool_name = tool_name.to_string();
        let task = tokio::spawn(async move {
            let event_system = get_global_event_system();
            let mut pending = String::new();

            loop {
                match tokio::time::timeout(
                    Duration::from_millis(PROGRESS_FLUSH_INTERVAL_MS),
                    rx.recv(),
                )
                .await
                {
                    Ok(Some(chunk)) => {
                        if chunk.is_empty() {
                            continue;
                        }
                        pending.push_str(&chunk);
                        if pending.chars().count() >= PROGRESS_FLUSH_MAX_CHARS {
                            emit_progress(&event_system, &tool_use_id, &tool_name, &mut pending)
                                .await;
                        }
                    }
                    Ok(None) => {
                        emit_progress(&event_system, &tool_use_id, &tool_name, &mut pending).await;
                        break;
                    }
                    Err(_) => {
                        emit_progress(&event_system, &tool_use_id, &tool_name, &mut pending).await;
                    }
                }
            }
        });

        Some(Self { tx, task })
    }

    pub(super) fn sender(&self) -> mpsc::Sender<String> {
        self.tx.clone()
    }

    pub(super) async fn finish(self) {
        drop(self.tx);
        let _ = tokio::time::timeout(Duration::from_millis(500), self.task).await;
    }
}

async fn emit_progress(
    event_system: &std::sync::Arc<crate::infrastructure::events::event_system::BackendEventSystem>,
    tool_use_id: &str,
    tool_name: &str,
    pending: &mut String,
) {
    if pending.is_empty() {
        return;
    }

    let progress_message = std::mem::take(pending);
    let progress_event = ToolExecutionProgress(ToolExecutionProgressInfo {
        tool_use_id: tool_use_id.to_string(),
        tool_name: tool_name.to_string(),
        progress_message,
        percentage: None,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    });

    let _ = event_system.emit(progress_event).await;
}

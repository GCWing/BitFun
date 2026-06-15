/// EventEmitter Trait
///
/// All event sending interfaces for all platforms, core layer sends events through this trait
use async_trait::async_trait;
use log::{debug, info};

/// Event emitter trait
///
/// Core services send events through this trait, without directly depending on specific platforms
#[async_trait]
pub trait EventEmitter: Send + Sync {
    /// Send generic events
    async fn emit(&self, event_name: &str, payload: serde_json::Value) -> anyhow::Result<()>;

    /// Send LSP events
    async fn emit_lsp(
        &self,
        workspace_path: &str,
        event_data: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.emit(
            "lsp-event",
            serde_json::json!({
                "workspace_path": workspace_path,
                "event_data": event_data
            }),
        )
        .await
    }

    /// Send Profile events
    async fn emit_profile(
        &self,
        workspace_path: &str,
        event_data: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.emit(
            "profile-event",
            serde_json::json!({
                "workspace_path": workspace_path,
                "event_data": event_data
            }),
        )
        .await
    }

    /// Send file watch events
    async fn emit_file_watch(&self, path: &str, event_type: &str) -> anyhow::Result<()> {
        self.emit(
            "file-system-changed",
            serde_json::json!({
                "path": path,
                "kind": event_type,
                "timestamp": chrono::Utc::now().timestamp()
            }),
        )
        .await
    }

    /// Send Terminal output events
    async fn emit_terminal(
        &self,
        session_id: &str,
        output: &str,
        stream_type: &str,
    ) -> anyhow::Result<()> {
        self.emit(
            "terminal-output",
            serde_json::json!({
                "session_id": session_id,
                "output": output,
                "stream_type": stream_type
            }),
        )
        .await
    }

    /// Send Snapshot events
    async fn emit_snapshot(
        &self,
        snapshot_id: &str,
        event_data: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.emit(
            "snapshot-event",
            serde_json::json!({
                "snapshot_id": snapshot_id,
                "event_data": event_data
            }),
        )
        .await
    }
}

/// NullEmitter - Do not send any events
#[derive(Debug, Clone, Copy)]
pub struct NullEmitter;

#[async_trait]
impl EventEmitter for NullEmitter {
    async fn emit(&self, event_name: &str, _payload: serde_json::Value) -> anyhow::Result<()> {
        debug!("NullEmitter: ignore event {}", event_name);
        Ok(())
    }
}

/// LoggingEmitter - Only log events, do not actually send
#[derive(Debug, Clone, Copy)]
pub struct LoggingEmitter;

#[async_trait]
impl EventEmitter for LoggingEmitter {
    async fn emit(&self, event_name: &str, payload: serde_json::Value) -> anyhow::Result<()> {
        info!("Event [{}]: {:?}", event_name, payload);
        Ok(())
    }
}

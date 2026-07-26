//! Host capability required by Remote Connect LAN and Ngrok orchestration.
//!
//! Product assembly decides when the embedded relay is needed. Concrete
//! listener, router, static-file, and task lifecycle details belong to the app
//! host implementing this port.

use anyhow::Result;

#[async_trait::async_trait]
pub trait EmbeddedRelayHost: Send + Sync {
    async fn start(&self, port: u16, static_dir: Option<String>) -> Result<()>;

    async fn stop(&self);
}

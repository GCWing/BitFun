//! MCP server process compatibility facade.

use std::sync::Arc;
use std::time::Duration;

use crate::infrastructure::try_get_path_manager_arc;
use crate::service::mcp::protocol::MCPServerInfo;
use crate::service::mcp::server::{MCPConnection, MCPServerConfig, MCPServerStatus, MCPServerType};
use crate::util::errors::BitFunResult;

pub struct MCPServerProcess {
    inner: bitfun_services_integrations::mcp::server::MCPServerProcess,
}

impl MCPServerProcess {
    pub fn new(id: String, name: String, server_type: MCPServerType) -> Self {
        Self {
            inner: bitfun_services_integrations::mcp::server::MCPServerProcess::new(
                id,
                name,
                server_type,
            ),
        }
    }

    pub async fn start(
        &mut self,
        command: &str,
        args: &[String],
        env: &std::collections::HashMap<String, String>,
    ) -> BitFunResult<()> {
        self.inner.start(command, args, env).await?;
        Ok(())
    }

    pub async fn start_remote(&mut self, config: &MCPServerConfig) -> BitFunResult<()> {
        let data_dir = try_get_path_manager_arc()?.user_data_dir();
        self.inner.start_remote(data_dir, config).await?;
        Ok(())
    }

    pub async fn stop(&mut self) -> BitFunResult<()> {
        self.inner.stop().await?;
        Ok(())
    }

    pub async fn restart(
        &mut self,
        command: &str,
        args: &[String],
        env: &std::collections::HashMap<String, String>,
    ) -> BitFunResult<()> {
        self.inner.restart(command, args, env).await?;
        Ok(())
    }

    pub async fn status(&self) -> MCPServerStatus {
        self.inner.status().await
    }

    pub async fn status_message(&self) -> Option<String> {
        self.inner.status_message().await
    }

    pub fn connection(&self) -> Option<Arc<MCPConnection>> {
        self.inner.connection()
    }

    pub fn server_info(&self) -> Option<&MCPServerInfo> {
        self.inner.server_info()
    }

    pub fn id(&self) -> &str {
        self.inner.id()
    }

    pub fn name(&self) -> &str {
        self.inner.name()
    }

    pub fn server_type(&self) -> MCPServerType {
        self.inner.server_type()
    }

    pub fn uptime(&self) -> Option<Duration> {
        self.inner.uptime()
    }
}

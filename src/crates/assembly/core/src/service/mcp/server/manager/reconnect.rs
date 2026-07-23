use super::*;
use bitfun_services_integrations::mcp::server::{
    mcp_reconnect_runtime_decision, MCPReconnectRuntimeDecision,
};

impl MCPServerManager {
    pub(super) fn start_reconnect_monitor_if_needed(&self) {
        if self.reconnect_monitor_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let manager = self.clone();
        tokio::spawn(async move {
            manager.run_reconnect_monitor().await;
        });
        info!("Started MCP reconnect monitor");
    }

    async fn run_reconnect_monitor(self) {
        let mut interval = tokio::time::interval(self.runtime.reconnect_poll_interval());
        loop {
            interval.tick().await;
            if let Err(e) = self.reconnect_once().await {
                warn!("MCP reconnect monitor tick failed: {}", e);
            }
        }
    }

    async fn reconnect_once(&self) -> BitFunResult<()> {
        let has_registered_servers = !self.runtime.is_empty().await;
        let has_pending_reconnects = self.runtime.has_pending_reconnects().await;
        if !has_registered_servers && !has_pending_reconnects {
            return Ok(());
        }

        let configs = self.config_service.load_all_configs().await?;

        for config in configs {
            if !(config.enabled && config.auto_start) {
                self.clear_reconnect_state(&config.id).await;
                continue;
            }

            let status = self
                .get_server_status(&config.id)
                .await
                .unwrap_or(MCPServerStatus::Uninitialized);

            match mcp_reconnect_runtime_decision(&config, status) {
                MCPReconnectRuntimeDecision::Clear => {
                    self.clear_reconnect_state(&config.id).await;
                }
                MCPReconnectRuntimeDecision::Retry => {
                    self.try_reconnect_server(&config.id, &config.name, status)
                        .await;
                }
                MCPReconnectRuntimeDecision::Skip => {}
            }
        }

        Ok(())
    }

    async fn try_reconnect_server(
        &self,
        server_id: &str,
        server_name: &str,
        status: MCPServerStatus,
    ) {
        let Some((attempt_number, next_delay)) =
            self.runtime.next_due_reconnect_attempt(server_id).await
        else {
            return;
        };

        info!(
            "Attempting MCP reconnect: server_name={} server_id={} attempt={} status={:?}",
            server_name, server_id, attempt_number, status
        );

        let _ = self.stop_server(server_id).await;
        match self.start_server(server_id).await {
            Ok(_) => {
                self.clear_reconnect_state(server_id).await;
                info!(
                    "MCP reconnect succeeded: server_name={} server_id={} attempt={}",
                    server_name, server_id, attempt_number
                );
            }
            Err(e) => {
                warn!(
                    "MCP reconnect failed: server_name={} server_id={} attempt={} next_retry_in={}s error={}",
                    server_name,
                    server_id,
                    attempt_number,
                    next_delay.as_secs(),
                    e
                );
            }
        }
    }

    pub(super) async fn clear_reconnect_state(&self, server_id: &str) {
        self.runtime.clear_reconnect_state(server_id).await;
    }
}

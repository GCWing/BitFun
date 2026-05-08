use super::*;

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
        let mut interval = tokio::time::interval(self.reconnect_policy.poll_interval);
        loop {
            interval.tick().await;
            if let Err(e) = self.reconnect_once().await {
                warn!("MCP reconnect monitor tick failed: {}", e);
            }
        }
    }

    async fn reconnect_once(&self) -> BitFunResult<()> {
        let has_registered_servers = !self.registry.get_all_server_ids().await.is_empty();
        let has_pending_reconnects = !self.reconnect_states.read().await.is_empty();
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

            if matches!(
                status,
                MCPServerStatus::Connected | MCPServerStatus::Healthy | MCPServerStatus::Starting
            ) {
                self.clear_reconnect_state(&config.id).await;
                continue;
            }

            if matches!(status, MCPServerStatus::NeedsAuth) {
                self.clear_reconnect_state(&config.id).await;
                continue;
            }

            if !matches!(
                status,
                MCPServerStatus::Reconnecting | MCPServerStatus::Failed
            ) {
                continue;
            }

            self.try_reconnect_server(&config.id, &config.name, status)
                .await;
        }

        Ok(())
    }

    async fn try_reconnect_server(
        &self,
        server_id: &str,
        server_name: &str,
        status: MCPServerStatus,
    ) {
        let now = Instant::now();

        let (attempt_number, next_delay) = {
            let mut reconnect_states = self.reconnect_states.write().await;
            let state = reconnect_states
                .entry(server_id.to_string())
                .or_insert_with(|| ReconnectAttemptState::new(now));

            if now < state.next_retry_at {
                return;
            }

            state.attempts += 1;
            let delay = Self::compute_backoff_delay(
                self.reconnect_policy.base_delay,
                self.reconnect_policy.max_delay,
                state.attempts,
            );
            state.next_retry_at = now + delay;
            (state.attempts, delay)
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

    pub(super) fn compute_backoff_delay(base: Duration, max: Duration, attempt: u32) -> Duration {
        let shift = attempt.saturating_sub(1).min(20);
        let factor = 1u64 << shift;
        let base_ms = base.as_millis() as u64;
        let max_ms = max.as_millis() as u64;
        let delay_ms = base_ms.saturating_mul(factor).min(max_ms);
        Duration::from_millis(delay_ms)
    }

    pub(super) async fn clear_reconnect_state(&self, server_id: &str) {
        let mut reconnect_states = self.reconnect_states.write().await;
        reconnect_states.remove(server_id);
    }
}

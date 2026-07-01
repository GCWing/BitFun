//! Small MCP runtime policy helpers shared by core assembly.

use std::time::Duration;

use super::{MCPServerConfig, MCPServerStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MCPListChangedKind {
    Tools,
    Prompts,
    Resources,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MCPReconnectRuntimeDecision {
    Clear,
    Retry,
    Skip,
}

pub fn detect_mcp_list_changed_kind(method: &str) -> Option<MCPListChangedKind> {
    match method {
        "notifications/tools/list_changed"
        | "notifications/tools/listChanged"
        | "tools/list_changed" => Some(MCPListChangedKind::Tools),
        "notifications/prompts/list_changed"
        | "notifications/prompts/listChanged"
        | "prompts/list_changed" => Some(MCPListChangedKind::Prompts),
        "notifications/resources/list_changed"
        | "notifications/resources/listChanged"
        | "resources/list_changed" => Some(MCPListChangedKind::Resources),
        _ => None,
    }
}

pub fn compute_mcp_backoff_delay(base: Duration, max: Duration, attempt: u32) -> Duration {
    let shift = attempt.saturating_sub(1).min(20);
    let factor = 1u64 << shift;
    let base_ms = base.as_millis() as u64;
    let max_ms = max.as_millis() as u64;
    let delay_ms = base_ms.saturating_mul(factor).min(max_ms);
    Duration::from_millis(delay_ms)
}

pub fn mcp_server_is_running(status: MCPServerStatus) -> bool {
    matches!(
        status,
        MCPServerStatus::Connected | MCPServerStatus::Healthy
    )
}

pub fn mcp_server_is_starting_or_running(status: MCPServerStatus) -> bool {
    matches!(
        status,
        MCPServerStatus::Connected | MCPServerStatus::Healthy | MCPServerStatus::Starting
    )
}

pub fn mcp_should_start_after_config_update(
    config: &MCPServerConfig,
    status: MCPServerStatus,
) -> bool {
    config.enabled
        && config.auto_start
        && matches!(
            status,
            MCPServerStatus::NeedsAuth
                | MCPServerStatus::Failed
                | MCPServerStatus::Reconnecting
                | MCPServerStatus::Stopped
                | MCPServerStatus::Uninitialized
        )
}

pub fn mcp_reconnect_runtime_decision(
    config: &MCPServerConfig,
    status: MCPServerStatus,
) -> MCPReconnectRuntimeDecision {
    if !(config.enabled && config.auto_start) {
        return MCPReconnectRuntimeDecision::Clear;
    }

    if mcp_server_is_starting_or_running(status) || matches!(status, MCPServerStatus::NeedsAuth) {
        return MCPReconnectRuntimeDecision::Clear;
    }

    if matches!(
        status,
        MCPServerStatus::Reconnecting | MCPServerStatus::Failed
    ) {
        return MCPReconnectRuntimeDecision::Retry;
    }

    MCPReconnectRuntimeDecision::Skip
}

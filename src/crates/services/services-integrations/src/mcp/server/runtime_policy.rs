//! Small MCP runtime policy helpers shared by core assembly.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MCPListChangedKind {
    Tools,
    Prompts,
    Resources,
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

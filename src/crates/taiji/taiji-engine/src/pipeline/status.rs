use crate::node::NodeId;
use serde::Serialize;

/// Pipeline 整体状态
#[derive(Debug, Clone, Serialize)]
pub struct PipelineStatus {
    pub state: PipelineState,
    pub nodes: Vec<NodeStatus>,
    pub uptime_secs: u64,
    pub total_ticks: u64,
    pub total_signals: u64,
    pub total_bars: u64,
}

#[derive(Debug, Clone, Serialize)]
pub enum PipelineState {
    Initializing,
    Running,
    Degraded(String),
    Stopped,
}

/// 单节点状态
#[derive(Debug, Clone, Serialize)]
pub struct NodeStatus {
    pub id: NodeId,
    pub name: String,
    pub ready: bool,
    pub last_execution_ms: Option<u64>,
    pub signals_emitted: u64,
    pub errors: u64,
    pub state: NodeState,
}

#[derive(Debug, Clone, Serialize)]
pub enum NodeState {
    Idle,
    Running,
    WarmingUp,
    Error(String),
    Degraded,
}

impl PipelineStatus {
    pub fn new() -> Self {
        Self {
            state: PipelineState::Initializing,
            nodes: Vec::new(),
            uptime_secs: 0,
            total_ticks: 0,
            total_signals: 0,
            total_bars: 0,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }
}

impl Default for PipelineStatus {
    fn default() -> Self {
        Self::new()
    }
}

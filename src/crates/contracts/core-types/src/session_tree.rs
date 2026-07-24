use serde::{Deserialize, Serialize};

/// Session 在对话树中的位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTreePosition {
    /// 父 session ID（None 表示根节点）
    pub parent_session_id: Option<String>,
    /// 创建本 session 的父 tool_call_id
    pub parent_tool_call_id: Option<String>,
    /// 在树中的深度（根=0）
    pub depth: u32,
    /// 父 session 中创建本 session 的 agent_type
    pub parent_agent_type: Option<String>,
}

/// 对话树节点摘要（用于 UI 树形展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTreeNode {
    pub session_id: String,
    pub session_name: String,
    pub agent_type: String,
    pub agent_display_name: String,
    pub depth: u32,
    pub status: SessionTreeNodeStatus,
    pub children: Vec<SessionTreeNode>,
    pub is_acp_external: bool,
    pub external_provider_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionTreeNodeStatus {
    Running,
    Completed,
    Error(String),
    Cancelled,
}

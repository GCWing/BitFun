use serde::{Deserialize, Serialize};

/// Maximum allowed fission depth for subagent delegation trees.
/// Authoritative single source; runtime-ports re-exports this.
pub const MAX_FISSION_DEPTH: u8 = 10;

/// Maximum nesting depth of the session tree (session tree layer limit).
/// Authoritative single source; coordinator initializes `SessionTreeManager::new` with this.
pub const MAX_TREE_DEPTH: u32 = 10;

/// Hard recursion guard for session tree traversal (subtree/build_tree recursion),
/// prevents stack overflow in deep trees. Distinct from the tree layer limit above.
pub const MAX_TREE_RECURSION_DEPTH: u32 = 128;

/// Maximum recursion depth for session tree serialization to prevent stack overflow.
pub const MAX_TREE_SERIALIZE_DEPTH: usize = 256;

/// Position of a session in the conversation tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTreePosition {
    /// Parent session ID (None means root node)
    pub parent_session_id: Option<String>,
    /// tool_call_id of the parent that created this session
    pub parent_tool_call_id: Option<String>,
    /// Depth in the tree (root = 0)
    pub depth: u32,
    /// agent_type of the parent session that created this session
    pub parent_agent_type: Option<String>,
}

/// Conversation tree node summary (for UI tree display)
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

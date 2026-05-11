use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpToolInfo {
    pub server_id: String,
    pub server_name: String,
    pub tool_name: String,
}

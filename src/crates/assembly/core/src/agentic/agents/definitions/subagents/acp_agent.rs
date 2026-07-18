//! ACP bridge agent — an AgentRegistry entry for every configured ACP client.
//!
//! Each ACP client (OpenCode, Claude Code, CodeBuddy, etc.) is represented as a
//! `SubAgent` so it appears in the agent selector and can be targeted by
//! `SessionControl` / `SessionMessage` for legion orchestration.

use crate::agentic::agents::{Agent, UserContextPolicy};
use async_trait::async_trait;

/// A thin Agent wrapper around a single ACP client config.
#[allow(dead_code)]
pub struct AcpAgent {
    agent_id: String,
    display_name: String,
    default_tools: Vec<String>,
}

impl AcpAgent {
    pub fn new(client_id: String, display_name: String) -> Self {
        let agent_id = Self::agent_id_for(&client_id);
        Self {
            // ACP prompt tool is registered in the global tool registry by
            // register_configured_tools() — do NOT add it to default_tools
            // here, or the tool name will appear twice in the model manifest.
            default_tools: vec![
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "LS".to_string(),
            ],
            agent_id,
            display_name,
        }
    }

    /// The agent registry id: `acp__<client_id>`
    pub fn agent_id_for(client_id: &str) -> String {
        format!("acp__{client_id}")
    }
}

#[async_trait]
impl Agent for AcpAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        &self.agent_id
    }

    fn name(&self) -> &str {
        &self.display_name
    }

    fn description(&self) -> &str {
        "ACP agent"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "acp_agent"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

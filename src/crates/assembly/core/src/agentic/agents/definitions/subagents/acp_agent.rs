//! ACP bridge agent — an AgentRegistry entry for every configured ACP client.
//!
//! Each ACP client (OpenCode, Claude Code, CodeBuddy, etc.) is represented as a
//! `SubAgent` so it appears in the agent selector and can be targeted by
//! `SessionControl` / `SessionMessage` for legion orchestration.

use crate::agentic::agents::{
    shared_coding_mode_tool_exposure_overrides, shared_coding_mode_tools,
    shared_coding_mode_user_context_policy, Agent, AgentToolPolicyOverrides, UserContextPolicy,
};
use async_trait::async_trait;
use bitfun_agent_tools::build_acp_external_agent_tool_name;

/// A thin Agent wrapper around a single ACP client config.
#[allow(dead_code)]
pub struct AcpAgent {
    agent_id: String,
    display_name: String,
    default_tools: Vec<String>,
    tool_exposure_overrides: AgentToolPolicyOverrides,
}

impl AcpAgent {
    pub fn new(client_id: String, display_name: String) -> Self {
        let agent_id = Self::agent_id_for(&client_id);
        // R-WF-10: ACP agents are aligned with agentic main-agent semantics.
        // Tool set = shared_coding_mode_tools() (the agentic coding mode set,
        // without SessionControl), so an ACP client can act as a main agent
        // with the same full tool baseline as agentic. Previously this was
        // subagent_default_tools() (shared_coding_mode_tools + SessionControl).
        let mut default_tools = shared_coding_mode_tools();
        // This client's `acp__<client>__prompt` forwarding tool. It is also
        // registered in the global tool registry by register_configured_tools()
        // under the same name; listing it here makes it part of the ACP agent
        // session tool set. When the client is disabled or unconfigured the
        // name is dropped by mode_config_canonicalizer's valid-tools filter,
        // so it never leaks into sessions.
        let forwarding_tool = build_acp_external_agent_tool_name(&client_id);
        if !default_tools.contains(&forwarding_tool) {
            default_tools.push(forwarding_tool);
        }
        Self {
            default_tools,
            tool_exposure_overrides: shared_coding_mode_tool_exposure_overrides(),
            agent_id,
            display_name,
        }
    }

    /// The agent registry id prefix shared by all ACP agents
    pub fn agent_id_prefix() -> &'static str {
        "acp__"
    }

    /// The agent registry id: `acp__<client_id>`
    pub fn agent_id_for(client_id: &str) -> String {
        format!("{}{client_id}", Self::agent_id_prefix())
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
        "External ACP coding agent: run delegated implementation and analysis through the configured ACP client"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "acp_agent"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn tool_exposure_overrides(&self) -> &AgentToolPolicyOverrides {
        &self.tool_exposure_overrides
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        // R-WF-10: align with agentic — shared coding mode user context
        // (workspace context + workspace instructions + project layout +
        // memory summary). Previously this omitted project_layout and
        // memory_summary.
        shared_coding_mode_user_context_policy()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{AcpAgent, Agent};
    use crate::agentic::agents::{
        shared_coding_mode_tools, shared_coding_mode_user_context_policy,
    };

    #[test]
    fn acp_agent_default_tools_match_agentic_plus_forwarding_tool() {
        let agent = AcpAgent::new("test-client".to_string(), "Test Client".to_string());
        let tools = agent.default_tools();

        // R-WF-10: aligned with agentic — shared coding mode tool set
        // (no SessionControl)...
        let mut expected = shared_coding_mode_tools();
        // ...plus this client's forwarding tool, named exactly like the
        // globally registered AcpAgentTool (acp__<client>__prompt).
        expected.push("acp__test-client__prompt".to_string());
        assert_eq!(tools, expected);
    }

    #[test]
    fn acp_agent_forwarding_tool_survives_client_id_sanitization() {
        // Client ids with spaces map to the same sanitized tool name that
        // register_configured_tools uses when registering AcpAgentTool.
        let agent = AcpAgent::new("Claude Code".to_string(), "Claude Code".to_string());
        let tools = agent.default_tools();
        assert!(tools.contains(&"acp__Claude_Code__prompt".to_string()));
    }

    #[test]
    fn acp_agent_does_not_include_session_control() {
        // R-WF-10: ACP agents are main-agent aligned; SessionControl stays
        // exclusive to subagents (subagent_default_tools).
        let agent = AcpAgent::new("test-client".to_string(), "Test Client".to_string());
        let tools = agent.default_tools();
        assert!(!tools.contains(&"SessionControl".to_string()));
    }

    #[test]
    fn acp_agent_user_context_policy_matches_agentic() {
        // R-WF-10: user context policy includes project layout + memory
        // summary, matching the agentic shared coding mode policy.
        let agent = AcpAgent::new("test-client".to_string(), "Test Client".to_string());
        assert_eq!(
            agent.user_context_policy(),
            shared_coding_mode_user_context_policy()
        );
    }
}

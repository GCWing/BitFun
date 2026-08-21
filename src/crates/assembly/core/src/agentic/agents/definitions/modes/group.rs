//! Group Mode — 群聊会话（agent_type="group"）
//!
//! 群聊 = 一个普通会话（群聊 v3 定标）：群 = agent_type="group" 的会话，
//! 成员会话通过 group_room_tools（GroupRoomTool 9 action）互发消息。
//! 本 Mode 实现 Agent trait 使 group 成为后端一等内置类型：
//! - 工具集 = subagent_default_tools()（含群聊 9 工具 + SessionControl/
//!   SessionMessage 会话核心），群主会话据此管理群成员。
//! - 无大模型独立响应语义：群消息由成员主动发送（send_group_message），
//!   群主会话本身不产生自主输出；prompt 模板仅用于兜底 system prompt 构建
//!   （group 会话不得命中 get_embedded_prompt 空键 panic）。

use crate::agentic::agents::{subagent_default_tools, Agent, UserContextPolicy};
use async_trait::async_trait;

pub struct GroupMode {
    default_tools: Vec<String>,
}

impl Default for GroupMode {
    fn default() -> Self {
        Self::new()
    }
}

impl GroupMode {
    pub fn new() -> Self {
        // 共享子代理工具箱（含群聊 9 工具 GROUP_CHAT_TOOL_NAMES +
        // SessionControl/SessionMessage/goal 族/Plan 族/canvas 族）。
        // 群主会话需要会话核心工具来管理成员与转发消息。
        let default_tools = subagent_default_tools();
        Self { default_tools }
    }
}

#[async_trait]
impl Agent for GroupMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "group"
    }

    fn name(&self) -> &str {
        "group"
    }

    fn description(&self) -> &str {
        "Group chat session: a container session that aggregates messages from member sessions through the group chat tools"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "group_mode"
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

#[cfg(test)]
mod tests {
    use super::GroupMode;
    use crate::agentic::agents::Agent;

    #[test]
    fn group_mode_basics() {
        let agent = GroupMode::new();
        assert_eq!(agent.id(), "group");
        assert_eq!(agent.name(), "group");
        assert_eq!(agent.prompt_template_name(None), "group_mode");
        assert!(!agent.is_readonly());
        assert!(agent
            .default_tools()
            .contains(&"send_group_message".to_string()));
        assert!(agent
            .default_tools()
            .contains(&"create_group_chat".to_string()));
        assert!(agent
            .default_tools()
            .contains(&"SessionMessage".to_string()));
    }

    #[test]
    fn group_mode_includes_all_subagent_shared_tools() {
        let tools = GroupMode::new().default_tools();
        let shared = crate::agentic::agents::subagent_default_tools();
        for tool in &shared {
            assert!(
                tools.contains(tool),
                "Group default tools must include shared tool {}",
                tool
            );
        }
    }
}

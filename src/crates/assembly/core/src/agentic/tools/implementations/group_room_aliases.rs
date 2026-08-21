//! GroupRoomTool 9-action 契约名别名注册（R-GC-09，契约 §六）。
//!
//! 背景：框架注册 key 取自 `tool.name()`（tool-contracts framework.rs
//! register_tool_with_static_provider），而 GroupRoomTool 本体 `name()`
//! 固定返回 `"group_room"`（group_room_tools.rs:781-783）。契约 §六 要求
//! 6 处注册点写死 9 个独立工具名（create_group_chat / invite_group_member /
//! remove_group_member / send_group_message / get_group_history /
//! list_group_chats / fork_group_chat / group_member_status /
//! delete_group_chat）。若 9 名都映射到 GroupRoomTool::new() 会在 registry
//! 中互相覆盖（key 相同）。
//!
//! 故以别名包装器逐名注册：每个别名固定一个 GroupRoomAction，`name()` 返回
//! 契约名，执行转发 GroupRoomTool；`is_readonly` 按 action 与
//! `group_room_action_is_readonly`（group_room_tools.rs:165）一致——
//! get_group_history / list_group_chats / group_member_status 只读。

use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolExposure, ToolResult, ToolUseContext,
};
use crate::agentic::tools::implementations::group_room_tools::{
    group_room_action_is_readonly, GroupRoomAction, GroupRoomTool,
};
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use serde_json::{json, Value};

/// 9 + 2 契约工具名（R-GC-09 §六 + R-WF-03 编排扩展：update_group_member_tools/
/// update_group_wiring）。
pub const GROUP_ROOM_ALIAS_TOOL_NAMES: &[&str] = &[
    "create_group_chat",
    "invite_group_member",
    "remove_group_member",
    "send_group_message",
    "get_group_history",
    "list_group_chats",
    "fork_group_chat",
    "group_member_status",
    "delete_group_chat",
    "update_group_member_tools",
    "update_group_wiring",
];

/// 契约名 → 本体 action（9+2 全覆盖）。
pub(crate) fn group_room_alias_action(tool_name: &str) -> Option<GroupRoomAction> {
    match tool_name {
        "create_group_chat" => Some(GroupRoomAction::Create),
        "invite_group_member" => Some(GroupRoomAction::Invite),
        "remove_group_member" => Some(GroupRoomAction::Remove),
        "send_group_message" => Some(GroupRoomAction::Send),
        "get_group_history" => Some(GroupRoomAction::History),
        "list_group_chats" => Some(GroupRoomAction::List),
        "fork_group_chat" => Some(GroupRoomAction::Fork),
        "group_member_status" => Some(GroupRoomAction::MemberStatus),
        "delete_group_chat" => Some(GroupRoomAction::Delete),
        "update_group_member_tools" => Some(GroupRoomAction::UpdateMemberTools),
        "update_group_wiring" => Some(GroupRoomAction::UpdateWiring),
        _ => None,
    }
}

/// action → 契约名（与 `group_room_alias_action` 互逆）。
pub(crate) fn group_room_action_alias_name(action: GroupRoomAction) -> &'static str {
    match action {
        GroupRoomAction::Create => "create_group_chat",
        GroupRoomAction::Invite => "invite_group_member",
        GroupRoomAction::Remove => "remove_group_member",
        GroupRoomAction::Send => "send_group_message",
        GroupRoomAction::History => "get_group_history",
        GroupRoomAction::List => "list_group_chats",
        GroupRoomAction::Fork => "fork_group_chat",
        GroupRoomAction::MemberStatus => "group_member_status",
        GroupRoomAction::Delete => "delete_group_chat",
        GroupRoomAction::UpdateMemberTools => "update_group_member_tools",
        GroupRoomAction::UpdateWiring => "update_group_wiring",
    }
}

/// action → 本体内部 serde 名（body `action` 字段，契约 §二 snake_case）。
fn group_room_action_serde_name(action: GroupRoomAction) -> &'static str {
    match action {
        GroupRoomAction::Create => "create",
        GroupRoomAction::Invite => "invite",
        GroupRoomAction::Remove => "remove",
        GroupRoomAction::Send => "send",
        GroupRoomAction::History => "history",
        GroupRoomAction::List => "list",
        GroupRoomAction::Fork => "fork",
        GroupRoomAction::MemberStatus => "member_status",
        GroupRoomAction::Delete => "delete",
        GroupRoomAction::UpdateMemberTools => "update_member_tools",
        GroupRoomAction::UpdateWiring => "update_wiring",
    }
}

/// 别名包装器：固定一个 action，`name()` = 契约名，执行转发 GroupRoomTool。
pub struct GroupRoomAliasTool {
    action: GroupRoomAction,
    inner: GroupRoomTool,
}

impl GroupRoomAliasTool {
    pub(crate) fn new_for_action(action: GroupRoomAction) -> Self {
        Self {
            action,
            inner: GroupRoomTool::new(),
        }
    }
}

/// 按契约名取别名工具实例（materialization 工厂入口）。
pub(crate) fn group_room_alias_tool_for_name(tool_name: &str) -> Option<GroupRoomAliasTool> {
    group_room_alias_action(tool_name).map(GroupRoomAliasTool::new_for_action)
}

#[async_trait]
impl Tool for GroupRoomAliasTool {
    fn name(&self) -> &str {
        group_room_action_alias_name(self.action)
    }

    fn short_description(&self) -> String {
        format!(
            "Group chat {}: {}.",
            group_room_action_alias_name(self.action),
            match self.action {
                GroupRoomAction::Create =>
                    "create a group room with a name, members, and a dedicated workspace",
                GroupRoomAction::Invite => "invite a member session into a group",
                GroupRoomAction::Remove => "remove a member session from a group",
                GroupRoomAction::Send => "send a group message",
                GroupRoomAction::History => "read group message history",
                GroupRoomAction::List => "list group chats in the workspace",
                GroupRoomAction::Fork => "fork a child group from a turn",
                GroupRoomAction::MemberStatus => "query a member session's state",
                GroupRoomAction::Delete => "delete a group chat",
                GroupRoomAction::UpdateMemberTools =>
                    "update a member session's tool set in a group (orchestration control)",
                GroupRoomAction::UpdateWiring =>
                    "update the group wiring definition (orchestration control)",
            }
        )
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(format!(
            "Group chat action '{}' of the group_room tool family (type-contract v3). {} The `action` field is fixed to \"{}\"; argument semantics are shared with the group_room tool.",
            group_room_action_alias_name(self.action),
            self.short_description(),
            group_room_action_serde_name(self.action),
        ))
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Direct
    }

    fn input_schema(&self) -> Value {
        let mut schema = self.inner.input_schema();
        if let Some(properties) = schema
            .get_mut("properties")
            .and_then(|properties| properties.as_object_mut())
        {
            properties.insert(
                "action".to_string(),
                json!({
                    "type": "string",
                    "const": group_room_action_serde_name(self.action),
                }),
            );
        }
        schema
    }

    /// 按 action 区分只读（契约 §六.5，与 group_room_action_is_readonly 一致）。
    fn is_readonly(&self) -> bool {
        group_room_action_is_readonly(self.action)
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        group_room_action_is_readonly(self.action)
    }

    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        if group_room_action_is_readonly(self.action) {
            return Ok(Vec::new());
        }
        Ok(vec![PermissionIntent::new(
            "custom_tool",
            vec![self.name().to_string()],
        )])
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        // 别名工具固定 action：注入内部 action 名后转发本体执行。
        let mut merged = input.clone();
        if let Some(object) = merged.as_object_mut() {
            object.insert(
                "action".to_string(),
                json!(group_room_action_serde_name(self.action)),
            );
        }
        self.inner.call_impl(&merged, context).await
    }
}

#[cfg(test)]
mod tests {
    use super::{
        group_room_action_alias_name, group_room_action_is_readonly, group_room_alias_action,
        GroupRoomAction,
    };

    #[test]
    fn alias_mapping_round_trips_all_nine_names() {
        for (tool_name, action) in [
            ("create_group_chat", GroupRoomAction::Create),
            ("invite_group_member", GroupRoomAction::Invite),
            ("remove_group_member", GroupRoomAction::Remove),
            ("send_group_message", GroupRoomAction::Send),
            ("get_group_history", GroupRoomAction::History),
            ("list_group_chats", GroupRoomAction::List),
            ("fork_group_chat", GroupRoomAction::Fork),
            ("group_member_status", GroupRoomAction::MemberStatus),
            ("delete_group_chat", GroupRoomAction::Delete),
            (
                "update_group_member_tools",
                GroupRoomAction::UpdateMemberTools,
            ),
            ("update_group_wiring", GroupRoomAction::UpdateWiring),
        ] {
            assert_eq!(group_room_alias_action(tool_name), Some(action));
            assert_eq!(group_room_action_alias_name(action), tool_name);
        }
        assert_eq!(group_room_alias_action("nope"), None);
    }

    #[test]
    fn alias_readonly_matches_action_readonly() {
        for (tool_name, expected) in [
            ("create_group_chat", false),
            ("invite_group_member", false),
            ("remove_group_member", false),
            ("send_group_message", false),
            ("get_group_history", true),
            ("list_group_chats", true),
            ("fork_group_chat", false),
            ("group_member_status", true),
            ("delete_group_chat", false),
            ("update_group_member_tools", false),
            ("update_group_wiring", false),
        ] {
            let action = group_room_alias_action(tool_name).expect(tool_name);
            assert_eq!(
                group_room_action_is_readonly(action),
                expected,
                "tool_name={tool_name}"
            );
        }
    }
}

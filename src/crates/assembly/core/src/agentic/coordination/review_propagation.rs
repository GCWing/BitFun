//! 审核沿对话树传播——基础版
//!
//! 叶子 Agent 完成后，沿 parent_session_id 链向上传播审核结果。

use bitfun_services_core::session::types::{SessionMetadata, SessionRelationshipKind};
use log::{debug, info};

pub struct ReviewPropagationManager;

/// 审核传播动作
pub enum ReviewPropagationAction {
    /// 无需操作
    None,
    /// 建议触发父 session 审核
    ReviewNeeded {
        parent_session_id: String,
        child_session_id: String,
    },
}

impl ReviewPropagationManager {
    /// 叶子 Agent 完成时触发——检查父 session 并决定是否传播审核
    pub fn on_leaf_completed(
        session_id: &str,
        agent_type: &str,
        response_text: &str,
        parent_session_id: Option<&str>,
    ) -> ReviewPropagationAction {
        info!(
            "ReviewPropagation: leaf agent completed session={} agent_type={} text_len={} parent={:?}",
            session_id,
            agent_type,
            response_text.len(),
            parent_session_id,
        );

        match parent_session_id {
            Some(parent_id) if !parent_id.is_empty() => {
                debug!(
                    "ReviewPropagation: review may be needed for parent session={} (child={} completed)",
                    parent_id, session_id
                );
                ReviewPropagationAction::ReviewNeeded {
                    parent_session_id: parent_id.to_string(),
                    child_session_id: session_id.to_string(),
                }
            }
            _ => ReviewPropagationAction::None,
        }
    }

    /// 基于对话树路径构建 commit message 前缀
    /// e.g. "[agentic → Explore → claude-code] fix: ..."
    pub fn build_commit_message(
        sessions: &[SessionMetadata],
        leaf_id: &str,
        summary: &str,
    ) -> String {
        let path = Self::build_tree_path(sessions, leaf_id);
        format!("{} {}", path, summary)
    }

    /// 构建从叶子到根的树路径字符串
    /// e.g. "[agentic → Explore → claude-code]"
    pub fn build_tree_path(sessions: &[SessionMetadata], leaf_id: &str) -> String {
        let mut path = Vec::new();
        let mut current_id = leaf_id.to_string();

        loop {
            let Some(session) = sessions.iter().find(|s| s.session_id == current_id) else {
                break;
            };
            path.push(session.agent_type.clone());

            let Some(ref relationship) = session.relationship else {
                break;
            };
            let Some(ref parent_id) = relationship.parent_session_id else {
                break;
            };
            current_id = parent_id.clone();
        }

        path.reverse();
        format!("[{}]", path.join(" → "))
    }

    /// 汇总所有后代 SubAgent 的产出摘要
    pub fn build_pr_summary(sessions: &[SessionMetadata], root_id: &str) -> String {
        let children: Vec<_> = sessions
            .iter()
            .filter(|s| {
                s.relationship
                    .as_ref()
                    .and_then(|r| r.parent_session_id.as_deref())
                    == Some(root_id)
            })
            .collect();

        if children.is_empty() {
            return String::new();
        }

        let mut summary = String::new();
        for child in &children {
            summary.push_str(&format!(
                "- **{}** (`{}`): {} turns\n",
                child.session_name, child.agent_type, child.turn_count
            ));
            let child_summary = Self::build_pr_summary(sessions, &child.session_id);
            if !child_summary.is_empty() {
                for line in child_summary.lines() {
                    summary.push_str(&format!("  {}\n", line));
                }
            }
        }
        summary
    }

    /// 收集子树中所有 SubAgent session_ids
    pub fn collect_descendant_subagent_ids(
        sessions: &[SessionMetadata],
        root_id: &str,
    ) -> Vec<String> {
        let mut result = Vec::new();
        for session in sessions {
            if let Some(ref relationship) = session.relationship {
                if relationship.kind == Some(SessionRelationshipKind::Subagent) {
                    if let Some(ref parent_id) = relationship.parent_session_id {
                        if parent_id == root_id {
                            result.push(session.session_id.clone());
                            let grandchildren = Self::collect_descendant_subagent_ids(
                                sessions,
                                &session.session_id,
                            );
                            result.extend(grandchildren);
                        }
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_meta(id: &str, agent_type: &str, parent_id: Option<&str>) -> SessionMetadata {
        SessionMetadata {
            session_id: id.to_string(),
            session_name: format!("Session {}", id),
            agent_type: agent_type.to_string(),
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            created_by: None,
            session_kind: bitfun_core_types::SessionKind::Subagent,
            memory_mode: bitfun_services_core::session::types::SessionMemoryMode::Enabled,
            model_name: "model".to_string(),
            created_at: 1,
            last_active_at: 1,
            last_finished_at: None,
            turn_count: 3,
            message_count: 5,
            tool_call_count: 10,
            status: bitfun_services_core::session::types::SessionStatus::Completed,
            terminal_session_id: None,
            snapshot_session_id: None,
            tags: vec![],
            custom_metadata: None,
            relationship: parent_id.map(|pid| {
                bitfun_services_core::session::types::SessionRelationship {
                    kind: Some(SessionRelationshipKind::Subagent),
                    parent_session_id: Some(pid.to_string()),
                    depth: Some(1),
                    ..Default::default()
                }
            }),
            todos: None,
            review_action_state: None,
            deep_review_run_manifest: None,
            review_target_evidence: None,
            deep_review_cache: None,
            workspace_path: None,
            workspace_hostname: None,
            unread_completion: None,
            needs_user_attention: None,
            runtime_state: None,
        }
    }

    #[test]
    fn build_tree_path_three_levels() {
        let sessions = vec![
            make_meta("root", "agentic", None),
            make_meta("child", "Explore", Some("root")),
            make_meta("grandchild", "claude-code", Some("child")),
        ];
        let path = ReviewPropagationManager::build_tree_path(&sessions, "grandchild");
        assert!(path.contains("agentic"));
        assert!(path.contains("Explore"));
        assert!(path.contains("claude-code"));
    }

    #[test]
    fn collect_descendant_subagent_ids_two_levels() {
        let sessions = vec![
            make_meta("root", "agentic", None),
            make_meta("child-a", "Explore", Some("root")),
            make_meta("child-b", "FileFinder", Some("root")),
            make_meta("grandchild", "GeneralPurpose", Some("child-a")),
        ];
        let descendants =
            ReviewPropagationManager::collect_descendant_subagent_ids(&sessions, "root");
        assert_eq!(descendants.len(), 3);
        assert!(descendants.contains(&"child-a".to_string()));
        assert!(descendants.contains(&"child-b".to_string()));
        assert!(descendants.contains(&"grandchild".to_string()));
    }
}

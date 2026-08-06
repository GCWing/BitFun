//! Review propagation along the conversation tree - basic version
//!
//! When a leaf agent completes, review results propagate upward along the parent_session_id chain.

use bitfun_services_core::session::types::{SessionMetadata, SessionRelationshipKind};
use log::{debug, info};

pub struct ReviewPropagationManager;

/// Review propagation action
pub enum ReviewPropagationAction {
    /// No action needed
    None,
    /// Suggest triggering a review of the parent session
    ReviewNeeded {
        parent_session_id: String,
        child_session_id: String,
    },
}

impl ReviewPropagationManager {
    /// Triggered when a leaf agent completes - checks the parent session and decides whether to propagate a review
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

    /// Build a commit message prefix from the conversation tree path
    /// e.g. "[agentic → Explore → claude-code] fix: ..."
    pub fn build_commit_message(
        sessions: &[SessionMetadata],
        leaf_id: &str,
        summary: &str,
    ) -> String {
        let path = Self::build_tree_path(sessions, leaf_id);
        format!("{} {}", path, summary)
    }

    /// Build a tree path string from leaf to root
    /// e.g. "[agentic → Explore → claude-code]"
    pub fn build_tree_path(sessions: &[SessionMetadata], leaf_id: &str) -> String {
        let mut path = Vec::new();
        let mut current_id = leaf_id.to_string();

        while let Some(session) = sessions.iter().find(|s| s.session_id == current_id) {
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

    /// Aggregate output summaries of all descendant SubAgents
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

    /// Collect all SubAgent session_ids in the subtree
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
            is_daemon: false,
            execution_target: None,
            project_workspace_path: None,
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

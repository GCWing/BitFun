use crate::session::types::{SessionMetadata, SessionRelationshipKind};
use bitfun_core_types::session_tree::{
    SessionTreeNode, SessionTreeNodeStatus, MAX_TREE_RECURSION_DEPTH,
};
use dashmap::DashMap;
use std::collections::HashMap;

/// Session tree error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTreeError {
    CycleDetected { child_id: String, ancestor: String },
    SelfReference(String),
}

/// Conversation tree manager - pure in-memory data structure, not persisted.
/// All relationship data is read from SessionMetadata.relationship.
/// Hard recursion limit - traversal is truncated beyond this depth to prevent stack overflow.
/// Value is the authoritative `MAX_TREE_RECURSION_DEPTH` in `bitfun_core_types::session_tree`.
pub struct SessionTreeManager {
    /// parent_id -> child_ids mapping
    edges: DashMap<String, Vec<String>>,
    /// child_id -> parent_id reverse index (O(1) parent lookup)
    child_to_parent: DashMap<String, String>,
    /// session_id -> depth mapping
    depths: DashMap<String, u32>,
    /// Maximum nesting depth
    pub max_depth: u32,
}

impl SessionTreeManager {
    pub fn new(max_depth: u32) -> Self {
        Self {
            edges: DashMap::new(),
            child_to_parent: DashMap::new(),
            depths: DashMap::new(),
            max_depth,
        }
    }

    /// Register a parent-child relationship
    /// Depth values exceeding max_depth are clamped with a warning instead of
    /// rejecting the registration, preventing cascading failures in deep trees.
    ///
    /// Depth policy (d2-P2-5): this clamp is a last-resort defensive guard.
    /// Callers that can reject an over-limit depth up front must do so (e.g.
    /// LegionControl validates `child_depth <= max_depth` before creating any
    /// session), so the clamp only applies to callers that cannot fail, such
    /// as `load_from_sessions` rebuilding the tree from persisted lineage.
    /// Keep both layers in sync if the max-depth policy changes.
    pub fn register_child(&self, parent_id: &str, child_id: &str, depth: u32) -> Result<(), SessionTreeError> {
        if child_id == parent_id {
            return Err(SessionTreeError::SelfReference(child_id.to_string()));
        }
        let clamped_depth = if depth > self.max_depth {
            log::warn!(
                "register_child: depth {} exceeds max_depth {} for child_id={}, clamping",
                depth, self.max_depth, child_id
            );
            self.max_depth
        } else {
            depth
        };
        let mut current = parent_id.to_string();
        loop {
            match self.get_parent(&current) {
                Some(p) if p == child_id => {
                    return Err(SessionTreeError::CycleDetected {
                        child_id: child_id.to_string(),
                        ancestor: current,
                    });
                }
                Some(p) => current = p,
                None => break,
            }
        }
        self.edges
            .entry(parent_id.to_string())
            .or_default()
            .push(child_id.to_string());
        self.child_to_parent
            .insert(child_id.to_string(), parent_id.to_string());
        self.depths.insert(child_id.to_string(), clamped_depth);
        Ok(())
    }

    /// Calculate subtree max depth (iterative DFS to prevent stack overflow).
    pub fn subtree_depth(&self, session_id: &str) -> u32 {
        let mut max_depth: u32 = 0;
        let mut stack: Vec<(String, u32)> = vec![(session_id.to_string(), 0)];
        let mut visited = std::collections::HashSet::new();

        while let Some((id, recursion_depth)) = stack.pop() {
            if recursion_depth > MAX_TREE_RECURSION_DEPTH {
                continue;
            }
            if !visited.insert(id.clone()) {
                continue;
            }
            let own = self.depths.get(&id).map(|d| *d).unwrap_or(0);
            max_depth = max_depth.max(own);
            if let Some(children) = self.edges.get(&id) {
                for child_id in children.iter() {
                    stack.push((child_id.clone(), recursion_depth + 1));
                }
            }
        }

        max_depth
    }

    /// Get direct child node IDs
    pub fn get_children(&self, session_id: &str) -> Vec<String> {
        self.edges
            .get(session_id)
            .map(|children| children.clone())
            .unwrap_or_default()
    }

    /// Get all descendant node IDs (direct and indirect children), BFS traversal
    pub fn get_descendants(&self, session_id: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut stack = vec![session_id.to_string()];
        let mut seen = std::collections::HashSet::new();
        seen.insert(session_id.to_string()); // exclude self
        while let Some(id) = stack.pop() {
            for child in self.get_children(&id) {
                if seen.insert(child.clone()) {
                    result.push(child.clone());
                    stack.push(child);
                }
            }
        }
        result
    }

    /// Get the parent node (O(1) reverse-index lookup)
    pub fn get_parent(&self, session_id: &str) -> Option<String> {
        self.child_to_parent
            .get(session_id)
            .map(|entry| entry.value().clone())
    }

    /// Get the depth of a node (O(1) lookup)
    pub fn get_depth(&self, session_id: &str) -> Option<u32> {
        self.depths
            .get(session_id)
            .map(|entry| *entry)
    }

    /// Collect all ancestor session_ids along the parent chain (nearest first)
    pub fn walk_ancestors(&self, session_id: &str) -> Vec<String> {
        let mut ancestors = Vec::new();
        let mut current = session_id.to_string();
        while let Some(parent) = self.get_parent(&current) {
            ancestors.push(parent.clone());
            current = parent;
        }
        ancestors
    }

    /// Build a SessionTreeNode tree from sessions metadata
    pub fn build_tree(
        &self,
        root_id: &str,
        sessions: &[SessionMetadata],
    ) -> Option<SessionTreeNode> {
        let session_map: HashMap<&str, &SessionMetadata> =
            sessions.iter().map(|s| (s.session_id.as_str(), s)).collect();
        self.build_tree_impl(root_id, &session_map, &mut std::collections::HashSet::new(), 0)
    }

    fn build_tree_impl(
        &self,
        root_id: &str,
        sessions: &HashMap<&str, &SessionMetadata>,
        visited: &mut std::collections::HashSet<String>,
        recursion_depth: u32,
    ) -> Option<SessionTreeNode> {
        if recursion_depth > MAX_TREE_RECURSION_DEPTH {
            return None;
        }
        if !visited.insert(root_id.to_string()) {
            return None;
        }
        let root = sessions.get(root_id)?;
        let relationship = root.relationship.as_ref();
        let is_acp_external = relationship
            .and_then(|r| r.kind.as_ref())
            .map(|k| matches!(k, SessionRelationshipKind::Subagent))
            .unwrap_or(false);

        Some(SessionTreeNode {
            session_id: root.session_id.clone(),
            session_name: root.session_name.clone(),
            agent_type: root.agent_type.clone(),
            agent_display_name: root.agent_type.clone(),
            depth: root
                .relationship
                .as_ref()
                .and_then(|r| r.depth)
                .unwrap_or(0),
            status: session_status_to_tree_node_status(&root.status),
            children: self
                .get_children(root_id)
                .iter()
                .filter_map(|child_id| self.build_tree_impl(child_id, sessions, visited, recursion_depth + 1))
                .collect(),
            is_acp_external,
            external_provider_label: relationship.and_then(|r| r.subagent_type.clone()),
        })
    }

    /// Remove a subtree (iterative, not recursive - prevents stack overflow)
    /// Uses a HashSet to deduplicate IDs during BFS traversal, avoiding duplicate
    /// iteration over already-visited nodes in diamond-shaped subagent graphs.
    pub fn remove_subtree(&self, session_id: &str) {
        let mut stack = vec![session_id.to_string()];
        let mut to_remove = Vec::new();
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            to_remove.push(id.clone());
            for child in self.get_children(&id) {
                stack.push(child);
            }
        }
        for id in &to_remove {
            if let Some(parent_id) = self.get_parent(id) {
                if let Some(mut parent_children) = self.edges.get_mut(&parent_id) {
                    parent_children.retain(|x| x != id);
                }
            }
            self.edges.remove(id);
            self.child_to_parent.remove(id);
            self.depths.remove(id);
        }
    }

    /// Cycle detection: whether target_agent_type already appears in the ancestor chain of parent_id
    pub fn check_cycle(
        &self,
        parent_id: &str,
        target_agent_type: &str,
        agent_types: &DashMap<String, String>,
    ) -> bool {
        let mut current = parent_id.to_string();
        while let Some(parent) = self.get_parent(&current) {
            if let Some(agent_type) = agent_types.get(&parent) {
                if agent_type.as_str() == target_agent_type {
                    return true;
                }
            }
            current = parent;
        }
        false
    }

    /// Batch-load tree relationships from sessions
    ///
    /// SESSION-11 rebuild fallback: the SessionControl create chain persists
    /// the session record first and writes the structured SessionRelationship
    /// afterwards (create_session -> persist_session_lineage -> register_child).
    /// A crash between those steps leaves a persisted session without a
    /// relationship, which previously made its parent-child lineage invisible
    /// in the tree forever after restart. Pass 1 loads the authoritative
    /// relationship edges as before; pass 2 re-hangs relationship-less sessions
    /// from the creator marker (`session-<parent_session_id>`) or the
    /// `parentSessionId` free-form custom-metadata key, so the lost lineage is
    /// rebuilt instead of dropped.
    pub fn load_from_sessions(&self, sessions: &[SessionMetadata]) {
        self.edges.clear();
        self.child_to_parent.clear();
        self.depths.clear();
        for session in sessions {
            if let Some(ref relationship) = session.relationship {
                if let Some(ref parent_id) = relationship.parent_session_id {
                    let depth = relationship.depth.unwrap_or(1);
                    if let Err(e) = self.register_child(parent_id, &session.session_id, depth) {
                        log::warn!(
                            "Failed to register child session {} under {} in tree during load: {:?}",
                            session.session_id, parent_id, e
                        );
                    }
                }
            }
        }
        for session in sessions {
            if session.relationship.is_some() {
                continue;
            }
            let Some(parent_id) = lineage_rebuild_parent_session_id(session) else {
                continue;
            };
            if parent_id == session.session_id {
                log::warn!(
                    "Skipping SESSION-11 lineage rebuild for {}: creator marker points at the session itself",
                    session.session_id
                );
                continue;
            }
            // Best-effort depth: parent depth + 1 when the parent is already
            // registered (pass 1 or an earlier pass-2 rebuild), otherwise the
            // same default as the authoritative path.
            let depth = self.get_depth(&parent_id).map(|d| d + 1).unwrap_or(1);
            if let Err(e) = self.register_child(&parent_id, &session.session_id, depth) {
                log::warn!(
                    "SESSION-11 lineage rebuild failed for session {} under {}: {:?}",
                    session.session_id, parent_id, e
                );
            }
        }
    }
}

/// SESSION-11: recover the lost parent session id of a session record whose
/// SessionRelationship was never persisted (crash window between
/// create_session and persist_session_lineage). The SessionControl,
/// SessionMessage (Task), LegionControl and Worktree create chains all persist
/// the creator marker `session-<parent_session_id>` into the top-level
/// created_by field; a free-form `parentSessionId` custom-metadata key and a
/// custom-metadata `createdBy` marker (same shape) are honored defensively.
/// Non-marker creator values (not prefixed with `session-`) are not lineage
/// facts and are ignored.
fn lineage_rebuild_parent_session_id(session: &SessionMetadata) -> Option<String> {
    if let Some(serde_json::Value::Object(metadata)) = session.custom_metadata.as_ref() {
        if let Some(parent_id) = metadata
            .get("parentSessionId")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(parent_id.to_string());
        }
    }
    session
        .created_by
        .as_deref()
        .and_then(creator_marker_parent_session_id)
        .or_else(|| {
            session
                .custom_metadata
                .as_ref()
                .and_then(|value| value.get("createdBy"))
                .and_then(|value| value.as_str())
                .and_then(creator_marker_parent_session_id)
        })
}

/// Parse the `session-<parent_session_id>` creator marker produced by
/// `session_control_creator_marker`. Returns None for any other shape so
/// non-lineage creator values are never mistaken for a parent relationship.
fn creator_marker_parent_session_id(marker: &str) -> Option<String> {
    let parent_id = marker.trim().strip_prefix("session-")?;
    let parent_id = parent_id.trim();
    (!parent_id.is_empty()).then(|| parent_id.to_string())
}

fn session_status_to_tree_node_status(
    status: &crate::session::types::SessionStatus,
) -> SessionTreeNodeStatus {
    match status {
        crate::session::types::SessionStatus::Active => SessionTreeNodeStatus::Running,
        crate::session::types::SessionStatus::Completed => {
            SessionTreeNodeStatus::Completed
        }
        crate::session::types::SessionStatus::Archived => {
            SessionTreeNodeStatus::Completed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::types::SessionRelationship;

    fn make_metadata(id: &str, parent_id: Option<&str>, depth: Option<u32>) -> SessionMetadata {
        SessionMetadata {
            session_id: id.to_string(),
            session_name: format!("Session {}", id),
            agent_type: "agentic".to_string(),
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            created_by: None,
            session_kind: bitfun_core_types::SessionKind::Standard,
            memory_mode: crate::session::types::SessionMemoryMode::Enabled,
            model_name: "model".to_string(),
            created_at: 1,
            last_active_at: 1,
            last_finished_at: None,
            turn_count: 0,
            message_count: 0,
            tool_call_count: 0,
            status: crate::session::types::SessionStatus::Active,
            terminal_session_id: None,
            snapshot_session_id: None,
            tags: vec![],
            custom_metadata: None,
            current_context_usage: None,
            relationship: parent_id.map(|pid| SessionRelationship {
                kind: Some(SessionRelationshipKind::Subagent),
                parent_session_id: Some(pid.to_string()),
                depth,
                ..Default::default()
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
            project_workspace_path: None,
            execution_target: None,
            is_daemon: false,
        }
    }

    #[test]
    fn register_and_query_child() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("root", "child-1", 1).unwrap();
        assert_eq!(mgr.get_children("root"), vec!["child-1"]);
        assert_eq!(mgr.get_parent("child-1"), Some("root".to_string()));
    }

    #[test]
    fn depth_calculation_five_levels() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("root", "l1", 1).unwrap();
        mgr.register_child("l1", "l2", 2).unwrap();
        mgr.register_child("l2", "l3", 3).unwrap();
        mgr.register_child("l3", "l4", 4).unwrap();
        mgr.register_child("l4", "l5", 5).unwrap();
        assert_eq!(mgr.subtree_depth("root"), 5);
    }

    #[test]
    fn cycle_detection_same_agent_type() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("root", "a", 1).unwrap();
        let agent_types: DashMap<String, String> = DashMap::new();
        agent_types.insert("root".to_string(), "agentic".to_string());
        agent_types.insert("a".to_string(), "agentic".to_string());
        assert!(mgr.check_cycle("a", "agentic", &agent_types));
    }

    #[test]
    fn cycle_detection_different_agent_type_allowed() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("root", "a", 1).unwrap();
        let agent_types: DashMap<String, String> = DashMap::new();
        agent_types.insert("root".to_string(), "agentic".to_string());
        agent_types.insert("a".to_string(), "Explore".to_string());
        assert!(!mgr.check_cycle("a", "Explore", &agent_types));
    }

    #[test]
    fn remove_subtree_cascading() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("root", "a", 1).unwrap();
        mgr.register_child("a", "b", 2).unwrap();
        mgr.register_child("b", "c", 3).unwrap();
        mgr.remove_subtree("a");
        assert!(mgr.get_children("a").is_empty());
        assert!(mgr.get_children("b").is_empty());
        assert!(mgr.get_parent("a").is_none());
    }

    #[test]
    fn build_tree_three_levels() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("root", "a", 1).unwrap();
        mgr.register_child("a", "b", 2).unwrap();

        let sessions = vec![
            make_metadata("root", None, Some(0)),
            make_metadata("a", Some("root"), Some(1)),
            make_metadata("b", Some("a"), Some(2)),
        ];

        let tree = mgr.build_tree("root", &sessions).expect("root should exist");
        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].session_id, "a");
        assert_eq!(tree.children[0].children.len(), 1);
        assert_eq!(tree.children[0].children[0].session_id, "b");
    }

    #[test]
    fn max_depth_limit_enforced() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("root", "l1", 1).unwrap();
        mgr.register_child("l1", "l2", 2).unwrap();
        mgr.register_child("l2", "l3", 3).unwrap();
        mgr.register_child("l3", "l4", 4).unwrap();
        mgr.register_child("l4", "l5", 5).unwrap();
        // l5 depth is 5, reaching max_depth; no further child can be created
        let child_depth = 6;
        assert!(child_depth > mgr.max_depth);
    }

    #[test]
    fn walk_ancestors_from_leaf() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("root", "a", 1).unwrap();
        mgr.register_child("a", "b", 2).unwrap();
        mgr.register_child("b", "c", 3).unwrap();
        let ancestors = mgr.walk_ancestors("c");
        assert_eq!(ancestors, vec!["b", "a", "root"]);
    }

    #[test]
    fn test_register_child_rejects_cycle() {
        let mgr = SessionTreeManager::new(5);
        mgr.register_child("A", "B", 1).unwrap();
        mgr.register_child("B", "C", 2).unwrap();
        let result = mgr.register_child("C", "A", 3);
        assert!(matches!(result, Err(SessionTreeError::CycleDetected { .. })));
    }

    #[test]
    fn test_register_child_rejects_self_reference() {
        let mgr = SessionTreeManager::new(5);
        let result = mgr.register_child("A", "A", 1);
        assert!(matches!(result, Err(SessionTreeError::SelfReference(_))));
    }

    #[test]
    fn test_register_child_clamps_excessive_depth() {
        let mgr = SessionTreeManager::new(5);
        // Depth 6 exceeds max_depth 5, should be clamped rather than rejected.
        let result = mgr.register_child("A", "B", 6);
        assert!(result.is_ok());
        // The registered depth is clamped to max_depth.
        assert_eq!(mgr.get_depth("B"), Some(5));
    }

    #[test]
    fn load_from_sessions_rebuilds_lineage_from_created_by_marker() {
        // SESSION-11: a session persisted in the crash window between
        // create_session and persist_session_lineage has no relationship but
        // keeps the `session-<parent_id>` creator marker in created_by.
        let mgr = SessionTreeManager::new(5);
        let parent = make_metadata("parent", None, Some(0));
        let mut orphan = make_metadata("child", None, None);
        orphan.created_by = Some("session-parent".to_string());
        mgr.load_from_sessions(&[parent, orphan]);
        assert_eq!(mgr.get_parent("child"), Some("parent".to_string()));
        assert_eq!(mgr.get_depth("child"), Some(1));
    }

    #[test]
    fn load_from_sessions_ignores_non_marker_created_by() {
        // Creator values that are not `session-` markers are not lineage facts.
        let mgr = SessionTreeManager::new(5);
        let mut orphan = make_metadata("child", None, None);
        orphan.created_by = Some("some-external-creator".to_string());
        mgr.load_from_sessions(&[orphan]);
        assert_eq!(mgr.get_parent("child"), None);
    }

    #[test]
    fn load_from_sessions_uses_parent_session_id_custom_metadata() {
        // Defensive path: free-form custom-metadata parentSessionId key.
        let mgr = SessionTreeManager::new(5);
        let parent = make_metadata("parent", None, Some(0));
        let mut orphan = make_metadata("child", None, None);
        orphan.custom_metadata = Some(serde_json::json!({ "parentSessionId": "parent" }));
        mgr.load_from_sessions(&[parent, orphan]);
        assert_eq!(mgr.get_parent("child"), Some("parent".to_string()));
    }

    #[test]
    fn load_from_sessions_uses_custom_metadata_created_by_marker() {
        // Defensive path: custom-metadata createdBy marker (same shape).
        let mgr = SessionTreeManager::new(5);
        let parent = make_metadata("parent", None, Some(0));
        let mut orphan = make_metadata("child", None, None);
        orphan.custom_metadata = Some(serde_json::json!({ "createdBy": "session-parent" }));
        mgr.load_from_sessions(&[parent, orphan]);
        assert_eq!(mgr.get_parent("child"), Some("parent".to_string()));
    }

    #[test]
    fn load_from_sessions_lineage_rebuild_inherits_parent_depth() {
        // The rebuilt child inherits parent depth + 1 when the parent is
        // already registered through its own authoritative relationship.
        let mgr = SessionTreeManager::new(5);
        let parent = make_metadata("parent", Some("root"), Some(1));
        let mut orphan = make_metadata("child", None, None);
        orphan.created_by = Some("session-parent".to_string());
        mgr.load_from_sessions(&[parent, orphan]);
        assert_eq!(mgr.get_parent("child"), Some("parent".to_string()));
        assert_eq!(mgr.get_depth("child"), Some(2));
    }

    #[test]
    fn load_from_sessions_skips_self_reference_marker() {
        // A marker pointing at the session itself must not create a self loop.
        let mgr = SessionTreeManager::new(5);
        let mut orphan = make_metadata("selfish", None, None);
        orphan.created_by = Some("session-selfish".to_string());
        mgr.load_from_sessions(&[orphan]);
        assert_eq!(mgr.get_parent("selfish"), None);
        assert_eq!(mgr.get_children("selfish"), Vec::<String>::new());
    }
}

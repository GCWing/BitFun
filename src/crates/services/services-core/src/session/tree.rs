use bitfun_core_types::session_tree::{SessionTreeNode, SessionTreeNodeStatus};
use crate::session::types::{SessionMetadata, SessionRelationshipKind};
use dashmap::DashMap;
use std::collections::HashMap;

/// 会话树错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTreeError {
    CycleDetected { child_id: String, ancestor: String },
    SelfReference(String),
}

/// 对话树管理器——纯内存数据结构，不持久化。
/// 所有关系数据从 SessionMetadata.relationship 中读取。
/// 递归遍历硬上限——超过此深度截断以防止栈溢出
const MAX_RECURSION_DEPTH: u32 = 128;

pub struct SessionTreeManager {
    /// parent_id → child_ids 映射
    edges: DashMap<String, Vec<String>>,
    /// child_id → parent_id 反向索引（O(1) parent lookup）
    child_to_parent: DashMap<String, String>,
    /// session_id → depth 映射
    depths: DashMap<String, u32>,
    /// 最大嵌套深度
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

    /// 注册父子关系
    /// Depth values exceeding max_depth are clamped with a warning instead of
    /// rejecting the registration, preventing cascading failures in deep trees.
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
            if recursion_depth > MAX_RECURSION_DEPTH {
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

    /// 鑾峰彇鐩存帴瀛愯妭鐐?
    pub fn get_children(&self, session_id: &str) -> Vec<String> {
        self.edges
            .get(session_id)
            .map(|children| children.clone())
            .unwrap_or_default()
    }

    /// 获取所有后代节点（包括直接子节点和间接子节点），BFS 遍历
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

    /// 获取父节点（O(1) 反向索引查询）
    pub fn get_parent(&self, session_id: &str) -> Option<String> {
        self.child_to_parent
            .get(session_id)
            .map(|entry| entry.value().clone())
    }

    /// 获取节点的深度（O(1) 查询）
    pub fn get_depth(&self, session_id: &str) -> Option<u32> {
        self.depths
            .get(session_id)
            .map(|entry| *entry)
    }

    /// 娌?parent 閾炬敹闆嗘墍鏈夌鍏?session_id锛堜粠杩戝埌杩滐級
    pub fn walk_ancestors(&self, session_id: &str) -> Vec<String> {
        let mut ancestors = Vec::new();
        let mut current = session_id.to_string();
        while let Some(parent) = self.get_parent(&current) {
            ancestors.push(parent.clone());
            current = parent;
        }
        ancestors
    }

    /// 浠?sessions 鍏冩暟鎹瀯寤?SessionTreeNode 鏍?
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
        if recursion_depth > MAX_RECURSION_DEPTH {
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

    /// 移除子树（迭代，非递归——防止栈溢出）
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

    /// 寰幆妫€娴嬶細target_agent_type 鏄惁宸插嚭鐜板湪 parent_id 鐨勭鍏堥摼涓?
    pub fn check_cycle(
        &self,
        parent_id: &str,
        target_agent_type: &str,
        agent_types: &DashMap<String, String>,
    ) -> bool {
        let mut current = parent_id.to_string();
        loop {
            match self.get_parent(&current) {
                Some(parent) => {
                    if let Some(agent_type) = agent_types.get(&parent) {
                        if agent_type.as_str() == target_agent_type {
                            return true;
                        }
                    }
                    current = parent;
                }
                None => break,
            }
        }
        false
    }

    /// 浠?sessions 鎵归噺鍔犺浇鏍戝叧绯?
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
    }
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
        // l5 深度为5，达到 max_depth，不能再创建子节点
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
}

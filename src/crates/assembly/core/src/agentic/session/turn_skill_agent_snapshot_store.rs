use crate::agentic::skill_agent_snapshot::TurnSkillAgentSnapshot;
use dashmap::DashMap;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Default)]
pub struct TurnSkillAgentSnapshotStore {
    session_snapshots: Arc<DashMap<String, BTreeMap<usize, TurnSkillAgentSnapshot>>>,
}

impl TurnSkillAgentSnapshotStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(&self, session_id: &str) {
        self.session_snapshots
            .entry(session_id.to_string())
            .or_default();
    }

    pub fn get_snapshot(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> Option<TurnSkillAgentSnapshot> {
        self.session_snapshots
            .get(session_id)
            .and_then(|snapshots| snapshots.get(&turn_index).cloned())
    }

    pub fn set_snapshot(
        &self,
        session_id: &str,
        turn_index: usize,
        snapshot: TurnSkillAgentSnapshot,
    ) {
        if let Some(mut snapshots) = self.session_snapshots.get_mut(session_id) {
            snapshots.insert(turn_index, snapshot);
        } else {
            let mut snapshots = BTreeMap::new();
            snapshots.insert(turn_index, snapshot);
            self.session_snapshots
                .insert(session_id.to_string(), snapshots);
        }
    }

    pub fn latest_snapshot_at_or_before(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> Option<(usize, TurnSkillAgentSnapshot)> {
        self.session_snapshots
            .get(session_id)
            .and_then(|snapshots| {
                snapshots
                    .range(..=turn_index)
                    .next_back()
                    .map(|(index, snapshot)| (*index, snapshot.clone()))
            })
    }

    pub fn delete_session(&self, session_id: &str) {
        self.session_snapshots.remove(session_id);
    }

    pub fn remove_from(&self, session_id: &str, turn_index: usize) {
        if let Some(mut snapshots) = self.session_snapshots.get_mut(session_id) {
            snapshots.retain(|index, _| *index < turn_index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TurnSkillAgentSnapshotStore;
    use crate::agentic::skill_agent_snapshot::{SkillSnapshotEntry, TurnSkillAgentSnapshot};

    #[test]
    fn latest_snapshot_at_or_before_returns_nearest_sparse_snapshot() {
        let store = TurnSkillAgentSnapshotStore::new();
        store.create_session("session");
        store.set_snapshot(
            "session",
            0,
            TurnSkillAgentSnapshot {
                skills: vec![SkillSnapshotEntry {
                    name: "skill-a".to_string(),
                    description: "desc-a".to_string(),
                    location: "/a".to_string(),
                }],
                ..Default::default()
            },
        );
        store.set_snapshot(
            "session",
            3,
            TurnSkillAgentSnapshot {
                skills: vec![SkillSnapshotEntry {
                    name: "skill-b".to_string(),
                    description: "desc-b".to_string(),
                    location: "/b".to_string(),
                }],
                ..Default::default()
            },
        );

        let nearest = store
            .latest_snapshot_at_or_before("session", 2)
            .expect("nearest snapshot should exist");
        let latest = store
            .latest_snapshot_at_or_before("session", 4)
            .expect("latest snapshot should exist");

        assert_eq!(nearest.0, 0);
        assert_eq!(nearest.1.skills[0].name, "skill-a");
        assert_eq!(latest.0, 3);
        assert_eq!(latest.1.skills[0].name, "skill-b");
    }
}

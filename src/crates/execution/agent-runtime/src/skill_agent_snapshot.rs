use crate::prompt::ToolListingSections;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSnapshotEntry {
    pub name: String,
    pub description: String,
    pub location: String,
}

impl SkillSnapshotEntry {
    fn to_xml_desc(&self) -> String {
        format!(r#"<skill name="{}">{}</skill>"#, self.name, self.description)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSnapshotEntry {
    pub id: String,
    pub description: String,
    pub default_tools: Vec<String>,
}

impl AgentSnapshotEntry {
    fn to_xml_desc(&self) -> String {
        format!(
            "<agent type=\"{}\">\n<description>\n{}\n</description>\n<tools>{}</tools>\n</agent>",
            self.id,
            self.description,
            self.default_tools.join(", ")
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnSkillAgentSnapshot {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<SkillSnapshotEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subagents: Vec<AgentSnapshotEntry>,
}

impl TurnSkillAgentSnapshot {
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty() && self.subagents.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillAgentDiff {
    pub added_skills: Vec<SkillSnapshotEntry>,
    pub changed_skills: Vec<SkillSnapshotEntry>,
    pub removed_skills: Vec<String>,
    pub added_subagents: Vec<AgentSnapshotEntry>,
    pub changed_subagents: Vec<AgentSnapshotEntry>,
    pub removed_subagents: Vec<String>,
}

impl SkillAgentDiff {
    pub fn is_empty(&self) -> bool {
        self.added_skills.is_empty()
            && self.changed_skills.is_empty()
            && self.removed_skills.is_empty()
            && self.added_subagents.is_empty()
            && self.changed_subagents.is_empty()
            && self.removed_subagents.is_empty()
    }

    pub fn render_skill_listing_update(&self) -> Option<String> {
        if self.added_skills.is_empty()
            && self.changed_skills.is_empty()
            && self.removed_skills.is_empty()
        {
            return None;
        }

        let mut sections = Vec::new();
        if !self.added_skills.is_empty() {
            sections.push(render_titled_skill_entries(
                "Added Skills",
                &self.added_skills,
            ));
        }
        if !self.changed_skills.is_empty() {
            sections.push(render_titled_skill_entries(
                "Changed Skills",
                &self.changed_skills,
            ));
        }
        if !self.removed_skills.is_empty() {
            sections.push(render_removed_name_entries(
                "Removed Skills",
                &self.removed_skills,
            ));
        }

        Some(format!(
            "# Skill Listing Update\n\n{}",
            sections.join("\n\n")
        ))
    }

    pub fn render_agent_listing_update(&self) -> Option<String> {
        if self.added_subagents.is_empty()
            && self.changed_subagents.is_empty()
            && self.removed_subagents.is_empty()
        {
            return None;
        }

        let mut sections = Vec::new();
        if !self.added_subagents.is_empty() {
            sections.push(render_titled_subagent_entries(
                "Added Agents",
                &self.added_subagents,
            ));
        }
        if !self.changed_subagents.is_empty() {
            sections.push(render_titled_subagent_entries(
                "Changed Agents",
                &self.changed_subagents,
            ));
        }
        if !self.removed_subagents.is_empty() {
            sections.push(render_removed_name_entries(
                "Removed Agents",
                &self.removed_subagents,
            ));
        }

        Some(format!(
            "# Agent Listing Update\n\n{}",
            sections.join("\n\n")
        ))
    }
}

pub fn diff_skill_agent_snapshot(
    previous: &TurnSkillAgentSnapshot,
    current: &TurnSkillAgentSnapshot,
) -> SkillAgentDiff {
    let previous_skills = previous
        .skills
        .iter()
        .cloned()
        .map(|entry| (entry.name.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let current_skills = current
        .skills
        .iter()
        .cloned()
        .map(|entry| (entry.name.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let previous_subagents = previous
        .subagents
        .iter()
        .cloned()
        .map(|entry| (entry.id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let current_subagents = current
        .subagents
        .iter()
        .cloned()
        .map(|entry| (entry.id.clone(), entry))
        .collect::<BTreeMap<_, _>>();

    let mut diff = SkillAgentDiff::default();

    for (name, entry) in &current_skills {
        match previous_skills.get(name) {
            None => diff.added_skills.push(entry.clone()),
            Some(previous) if previous != entry => diff.changed_skills.push(entry.clone()),
            Some(_) => {}
        }
    }
    for name in previous_skills.keys() {
        if !current_skills.contains_key(name) {
            diff.removed_skills.push(name.clone());
        }
    }

    for (id, entry) in &current_subagents {
        match previous_subagents.get(id) {
            None => diff.added_subagents.push(entry.clone()),
            Some(previous) if !agent_snapshot_entries_match_for_diff(previous, entry) => {
                diff.changed_subagents.push(entry.clone())
            }
            Some(_) => {}
        }
    }
    for id in previous_subagents.keys() {
        if !current_subagents.contains_key(id) {
            diff.removed_subagents.push(id.clone());
        }
    }

    diff
}

fn agent_snapshot_entries_match_for_diff(
    previous: &AgentSnapshotEntry,
    current: &AgentSnapshotEntry,
) -> bool {
    previous.id == current.id
        && previous.description == current.description
        && sorted_tool_names(&previous.default_tools) == sorted_tool_names(&current.default_tools)
}

fn sorted_tool_names(tool_names: &[String]) -> Vec<&str> {
    let mut normalized = tool_names.iter().map(String::as_str).collect::<Vec<_>>();
    normalized.sort_unstable();
    normalized
}

pub fn render_full_skill_listing_body(skills: &[SkillSnapshotEntry]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    format!(
        "<available_skills>\n{}\n</available_skills>",
        skills
            .iter()
            .map(SkillSnapshotEntry::to_xml_desc)
            .collect::<Vec<_>>()
            .join("\n")
    )
}

pub fn render_full_agent_listing_body(subagents: &[AgentSnapshotEntry]) -> String {
    if subagents.is_empty() {
        return String::new();
    }
    format!(
        "<available_agents>\n{}\n</available_agents>",
        subagents
            .iter()
            .map(AgentSnapshotEntry::to_xml_desc)
            .collect::<Vec<_>>()
            .join("\n")
    )
}

pub fn build_skill_agent_tool_listing_sections_from_snapshot(
    snapshot: &TurnSkillAgentSnapshot,
) -> ToolListingSections {
    ToolListingSections {
        skill_listing: (!snapshot.skills.is_empty())
            .then(|| render_full_skill_listing_body(&snapshot.skills))
            .filter(|body| !body.is_empty()),
        agent_listing: (!snapshot.subagents.is_empty())
            .then(|| render_full_agent_listing_body(&snapshot.subagents))
            .filter(|body| !body.is_empty()),
        collapsed_tool_listing: None,
    }
}

fn render_titled_skill_entries(title: &str, entries: &[SkillSnapshotEntry]) -> String {
    format!(
        "## {}\n\n{}",
        title,
        entries
            .iter()
            .map(SkillSnapshotEntry::to_xml_desc)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn render_titled_subagent_entries(title: &str, entries: &[AgentSnapshotEntry]) -> String {
    format!(
        "## {}\n\n{}",
        title,
        entries
            .iter()
            .map(AgentSnapshotEntry::to_xml_desc)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn render_removed_name_entries(title: &str, names: &[String]) -> String {
    let entries = names
        .iter()
        .map(|name| format!("- {}", name))
        .collect::<Vec<_>>()
        .join("\n");
    format!("## {}\n\n{}", title, entries)
}

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
    use super::{
        diff_skill_agent_snapshot, AgentSnapshotEntry, SkillSnapshotEntry, TurnSkillAgentSnapshot,
        TurnSkillAgentSnapshotStore,
    };

    #[test]
    fn skill_agent_diff_renders_changed_added_and_removed_entries() {
        let previous = TurnSkillAgentSnapshot {
            skills: vec![
                SkillSnapshotEntry {
                    name: "skill-a".to_string(),
                    description: "desc-a".to_string(),
                    location: "C:/skills/skill-a".to_string(),
                },
                SkillSnapshotEntry {
                    name: "skill-b".to_string(),
                    description: "desc-b".to_string(),
                    location: "C:/skills/skill-b".to_string(),
                },
            ],
            subagents: vec![AgentSnapshotEntry {
                id: "agent-a".to_string(),
                description: "desc-a".to_string(),
                default_tools: vec!["Read".to_string()],
            }],
        };
        let current = TurnSkillAgentSnapshot {
            skills: vec![
                SkillSnapshotEntry {
                    name: "skill-a".to_string(),
                    description: "desc-a2".to_string(),
                    location: "C:/skills/skill-a".to_string(),
                },
                SkillSnapshotEntry {
                    name: "skill-c".to_string(),
                    description: "desc-c".to_string(),
                    location: "C:/skills/skill-c".to_string(),
                },
            ],
            subagents: vec![AgentSnapshotEntry {
                id: "agent-a".to_string(),
                description: "desc-a".to_string(),
                default_tools: vec!["Read".to_string(), "Grep".to_string()],
            }],
        };

        let diff = diff_skill_agent_snapshot(&previous, &current);
        let skill_update = diff
            .render_skill_listing_update()
            .expect("skill update should render");
        let agent_update = diff
            .render_agent_listing_update()
            .expect("agent update should render");

        assert!(skill_update.contains("## Changed Skills"));
        assert!(skill_update.contains("## Added Skills"));
        assert!(skill_update.contains("## Removed Skills"));
        assert!(skill_update.contains(r#"<skill name="skill-a">desc-a2</skill>"#));
        assert!(skill_update.contains(r#"<skill name="skill-c">desc-c</skill>"#));
        assert!(!skill_update.contains("C:/skills/skill-a"));
        assert!(!skill_update.contains("C:/skills/skill-c"));
        assert!(skill_update.contains("- skill-b"));
        assert!(agent_update.contains("## Changed Agents"));
        assert!(agent_update.contains("Grep"));
    }

    #[test]
    fn full_skill_listing_renders_inline_name_and_description_without_location() {
        let listing = super::render_full_skill_listing_body(&[SkillSnapshotEntry {
            name: "skill-a".to_string(),
            description: "desc-a".to_string(),
            location: "C:/skills/skill-a".to_string(),
        }]);

        assert!(listing.contains("<available_skills>"));
        assert!(listing.contains(r#"<skill name="skill-a">desc-a</skill>"#));
        assert!(!listing.contains("<location>"));
        assert!(!listing.contains("C:/skills/skill-a"));
    }

    #[test]
    fn skill_agent_diff_ignores_default_tool_reordering_for_agents() {
        let previous = TurnSkillAgentSnapshot {
            subagents: vec![AgentSnapshotEntry {
                id: "agent-a".to_string(),
                description: "desc-a".to_string(),
                default_tools: vec!["Read".to_string(), "Grep".to_string()],
            }],
            ..Default::default()
        };
        let current = TurnSkillAgentSnapshot {
            subagents: vec![AgentSnapshotEntry {
                id: "agent-a".to_string(),
                description: "desc-a".to_string(),
                default_tools: vec!["Grep".to_string(), "Read".to_string()],
            }],
            ..Default::default()
        };

        let diff = diff_skill_agent_snapshot(&previous, &current);

        assert!(diff.changed_subagents.is_empty());
        assert!(diff.is_empty());
    }

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
                    location: "C:/skills/skill-a".to_string(),
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
                    location: "C:/skills/skill-b".to_string(),
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

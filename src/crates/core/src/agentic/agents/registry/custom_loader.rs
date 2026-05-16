use crate::agentic::agents::definitions::custom::{CustomSubagent, CustomSubagentKind};
use crate::agentic::agents::Agent;
use crate::infrastructure::get_path_manager_arc;
use log::error;
use std::path::{Path, PathBuf};

/// Existing subagent directory and its source
#[derive(Debug, Clone)]
pub struct SubagentDirEntry {
    pub path: PathBuf,
    pub kind: CustomSubagentKind,
}

/// Project subagent directory names (relative to workspace root, each item is in [".bitfun", "agents"] format)
const PROJECT_AGENT_SUBDIRS: &[(&str, &str)] = &[
    (".bitfun", "agents"),
    (".claude", "agents"),
    (".cursor", "agents"),
    (".codex", "agents"),
];

/// Custom subagent loader: discovers possible agent paths from project/user directories
pub struct CustomSubagentLoader;

struct CustomSubagentCandidate {
    agent: CustomSubagent,
    root_priority: usize,
    path: PathBuf,
}

impl CustomSubagentLoader {
    /// Returns existing possible paths (directories) and their sources (project/user).
    /// - Project subagents: .bitfun/agents, .claude/agents, .cursor/agents, .codex/agents under workspace
    /// - User subagents: agents under bitfun user config, ~/.claude/agents, ~/.cursor/agents, ~/.codex/agents
    pub fn get_possible_paths(workspace_root: &Path) -> Vec<SubagentDirEntry> {
        let mut entries = Vec::new();

        for (parent, sub) in PROJECT_AGENT_SUBDIRS {
            let p = workspace_root.join(parent).join(sub);
            if p.exists() && p.is_dir() {
                entries.push(SubagentDirEntry {
                    path: p,
                    kind: CustomSubagentKind::Project,
                });
            }
        }

        let pm = get_path_manager_arc();
        let bitfun_agents = pm.user_agents_dir();
        if bitfun_agents.exists() && bitfun_agents.is_dir() {
            entries.push(SubagentDirEntry {
                path: bitfun_agents,
                kind: CustomSubagentKind::User,
            });
        }

        if let Some(home) = dirs::home_dir() {
            for (parent, sub) in PROJECT_AGENT_SUBDIRS {
                if *parent == ".bitfun" {
                    continue;
                }
                let p = home.join(parent).join(sub);
                if p.exists() && p.is_dir() {
                    entries.push(SubagentDirEntry {
                        path: p,
                        kind: CustomSubagentKind::User,
                    });
                }
            }
        }

        entries
    }

    /// Load custom subagents from all possible paths (only .md files).
    /// Agents with the same name are prioritized by path order: earlier paths have higher priority, later ones won't override already loaded agents with the same name.
    pub fn load_custom_subagents(workspace_root: &Path) -> Vec<CustomSubagent> {
        let mut candidates = Vec::new();
        for (root_priority, entry) in Self::get_possible_paths(workspace_root).into_iter().enumerate() {
            for md_path in Self::list_md_files(&entry.path) {
                let path_str = md_path.to_string_lossy();
                match CustomSubagent::from_file(path_str.as_ref(), entry.kind) {
                    Ok(agent) => candidates.push(CustomSubagentCandidate {
                        agent,
                        root_priority,
                        path: md_path,
                    }),
                    Err(e) => {
                        error!(
                            "Failed to load custom subagent from {}: {}",
                            md_path.display(),
                            e
                        );
                    }
                }
            }
        }

        candidates.sort_by(|a, b| {
            a.root_priority
                .cmp(&b.root_priority)
                .then_with(|| a.agent.id().to_lowercase().cmp(&b.agent.id().to_lowercase()))
                .then_with(|| a.agent.id().cmp(b.agent.id()))
                .then_with(|| a.path.cmp(&b.path))
        });

        let mut ordered = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();
        for candidate in candidates {
            let id = candidate.agent.id().to_string();
            if seen_ids.insert(id) {
                ordered.push(candidate.agent);
            }
        }

        ordered
    }

    /// List all .md files in directory (non-recursive)
    fn list_md_files(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(dir) else {
            return out;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() && p.extension().is_some_and(|ext| ext == "md") {
                out.push(p);
            }
        }
        out.sort();
        out
    }
}

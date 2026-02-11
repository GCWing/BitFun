use crate::agentic::agents::Agent;
use crate::infrastructure::get_path_manager_arc;
use log::error;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{CustomSubagent, CustomSubagentKind};

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

impl CustomSubagentLoader {
    /// Returns existing possible paths (directories) and their sources (project/user).
    /// - Project subagents: .bitfun/agents, .claude/agents, .cursor/agents, .codex/agents under workspace
    /// - User subagents: agents under bitfun user config, ~/.claude/agents, ~/.cursor/agents, ~/.codex/agents
    pub fn get_possible_paths(workspace_root: &Path) -> Vec<SubagentDirEntry> {
        let mut entries = Vec::new();

        // Project subagent paths
        for (parent, sub) in PROJECT_AGENT_SUBDIRS {
            let p = workspace_root.join(parent).join(sub);
            if p.exists() && p.is_dir() {
                entries.push(SubagentDirEntry {
                    path: p,
                    kind: CustomSubagentKind::Project,
                });
            }
        }

        // User subagents: agents under bitfun user config
        let pm = get_path_manager_arc();
        let bitfun_agents = pm.user_agents_dir();
        if bitfun_agents.exists() && bitfun_agents.is_dir() {
            entries.push(SubagentDirEntry {
                path: bitfun_agents,
                kind: CustomSubagentKind::User,
            });
        }

        // User subagents: ~/.claude/agents, ~/.cursor/agents, ~/.codex/agents
        if let Some(home) = dirs::home_dir() {
            for (parent, sub) in PROJECT_AGENT_SUBDIRS {
                if *parent == ".bitfun" {
                    continue; // bitfun user path already handled by path_manager
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
        let mut by_id: HashMap<String, CustomSubagent> = HashMap::new();
        for entry in Self::get_possible_paths(workspace_root) {
            for md_path in Self::list_md_files(&entry.path) {
                let path_str = md_path.to_string_lossy();
                match CustomSubagent::from_file(path_str.as_ref(), entry.kind) {
                    Ok(agent) => {
                        by_id.entry(agent.id().to_string()).or_insert(agent);
                    }
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
        by_id.into_values().collect()
    }

    /// List all .md files in directory (non-recursive)
    fn list_md_files(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(dir) else {
            return out;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() && p.extension().map_or(false, |ext| ext == "md") {
                out.push(p);
            }
        }
        out
    }
}

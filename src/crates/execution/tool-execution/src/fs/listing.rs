//! Directory listing policy shared by native and transported workspace filesystems.
//!
//! The provider supplies entries and metadata. Traversal, limits, ordering and
//! rendering must not depend on whether those entries came from local IO or SFTP.

use openbitfun_runtime_ports::{WorkspaceDirEntry, WorkspaceFileSystem, WorkspacePathKind};
use std::collections::{HashMap, VecDeque};
use std::time::SystemTime;

use crate::search::workspace_ignore::WorkspaceIgnoreRules;

const MAX_IGNORE_BYTES: usize = 1024 * 1024;
const EXCLUDED_NAMES: &[&str] = &[
    "node_modules",
    "__pycache__",
    "env",
    "venv",
    "target",
    "build",
    "dist",
    "out",
    "bundle",
    "vendor",
    "tmp",
    "temp",
    "deps",
    "pkg",
    "Pods",
    "Cargo.lock",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceListingEntry {
    pub name: String,
    pub path: String,
    /// Components come from directory entries, never from host path parsing.
    pub components: Vec<String>,
    pub is_dir: bool,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceListing {
    pub entries: Vec<WorkspaceListingEntry>,
    pub truncated: bool,
}

#[derive(Clone)]
struct ScopedIgnore {
    depth: usize,
    rules: WorkspaceIgnoreRules,
}

struct PendingDirectory {
    path: String,
    components: Vec<String>,
    ignores: Vec<ScopedIgnore>,
}

fn compare_entries(left: &WorkspaceDirEntry, right: &WorkspaceDirEntry) -> std::cmp::Ordering {
    right
        .modified
        .cmp(&left.modified)
        .then(left.name.cmp(&right.name))
}

fn ignored(components: &[String], is_dir: bool, rules: &[ScopedIgnore]) -> bool {
    let mut ignored = false;
    for scope in rules {
        if let Some(decision) = scope
            .rules
            .matched(&components[scope.depth..].join("/"), is_dir)
        {
            ignored = decision;
        }
    }
    ignored
}

/// List visible entries breadth-first, choosing newer siblings before older ones.
///
/// An entire directory is enumerated before sorting: taking the first `limit`
/// transport entries would make the result depend on provider batch order and
/// could hide visible entries behind an arbitrary number of hidden entries.
/// Traversal stops after the first additional visible entry proves truncation.
pub async fn list_workspace_directory(
    fs: &dyn WorkspaceFileSystem,
    root: &str,
    limit: usize,
) -> Result<WorkspaceListing, String> {
    if limit == 0 {
        return Err("Directory listing limit must be positive".to_string());
    }
    match fs
        .path_kind_no_follow(root)
        .await
        .map_err(|error| error.to_string())?
    {
        Some(WorkspacePathKind::Directory) => {}
        None => return Err(format!("Directory does not exist: {root}")),
        Some(_) => return Err(format!("Path is not a directory: {root}")),
    }

    let mut pending = VecDeque::from([PendingDirectory {
        path: root.to_string(),
        components: Vec::new(),
        ignores: Vec::new(),
    }]);
    let mut result = Vec::with_capacity(limit.min(4096));
    while let Some(mut directory) = pending.pop_front() {
        let mut entries = fs
            .read_dir(&directory.path)
            .await
            .map_err(|error| format!("Failed to list directory {}: {error}", directory.path))?;
        if let Some(ignore_entry) = entries
            .iter()
            .find(|entry| entry.name == ".gitignore" && !entry.is_dir && !entry.is_symlink)
        {
            let text = fs
                .read_file_text_bounded(&ignore_entry.path, MAX_IGNORE_BYTES)
                .await
                .map_err(|error| format!("Failed to read {}: {error}", ignore_entry.path))?
                .ok_or_else(|| {
                    format!(
                        "Ignore file exceeds {MAX_IGNORE_BYTES} bytes: {}",
                        ignore_entry.path
                    )
                })?;
            directory.ignores.push(ScopedIgnore {
                depth: directory.components.len(),
                rules: WorkspaceIgnoreRules::parse(&text).map_err(|error| {
                    format!("Invalid ignore file {}: {error}", ignore_entry.path)
                })?,
            });
        }
        entries.sort_by(compare_entries);
        for entry in entries {
            if entry.is_symlink
                || entry.name.starts_with('.')
                || EXCLUDED_NAMES.contains(&entry.name.as_str())
            {
                continue;
            }
            let mut components = directory.components.clone();
            components.push(entry.name.clone());
            if ignored(&components, entry.is_dir, &directory.ignores) {
                continue;
            }
            if result.len() == limit {
                return Ok(WorkspaceListing {
                    entries: result,
                    truncated: true,
                });
            }
            if entry.is_dir {
                pending.push_back(PendingDirectory {
                    path: entry.path.clone(),
                    components: components.clone(),
                    ignores: directory.ignores.clone(),
                });
            }
            result.push(WorkspaceListingEntry {
                name: entry.name,
                path: entry.path,
                components,
                is_dir: entry.is_dir,
                modified: entry.modified,
            });
        }
    }
    Ok(WorkspaceListing {
        entries: result,
        truncated: false,
    })
}

fn display_name(name: &str) -> String {
    if name.chars().any(char::is_control) || name.contains('\\') {
        serde_json::to_string(name).expect("strings serialize")
    } else {
        name.to_string()
    }
}

/// Render the same tree for every workspace provider, preserving POSIX names.
pub fn format_workspace_listing(listing: &WorkspaceListing, logical_root: &str) -> String {
    let mut children: HashMap<&[String], Vec<&WorkspaceListingEntry>> = HashMap::new();
    for entry in &listing.entries {
        children
            .entry(&entry.components[..entry.components.len() - 1])
            .or_default()
            .push(entry);
    }
    for entries in children.values_mut() {
        entries.sort_by(|left, right| {
            right
                .modified
                .cmp(&left.modified)
                .then(left.name.cmp(&right.name))
        });
    }
    // An explicit stack avoids recursion on unusually deep workspace trees.
    let mut output = logical_root.to_string();
    let mut stack = Vec::new();
    if let Some(entries) = children.get(&[][..]) {
        for (index, entry) in entries.iter().enumerate().rev() {
            stack.push((*entry, String::new(), index + 1 == entries.len()));
        }
    }
    while let Some((entry, prefix, last)) = stack.pop() {
        output.push('\n');
        output.push_str(&prefix);
        output.push_str(if last { "└── " } else { "├── " });
        output.push_str(&display_name(&entry.name));
        if entry.is_dir {
            output.push('/');
            if let Some(entries) = children.get(entry.components.as_slice()) {
                let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
                for (index, child) in entries.iter().enumerate().rev() {
                    stack.push((*child, child_prefix.clone(), index + 1 == entries.len()));
                }
            }
        }
    }
    output
}

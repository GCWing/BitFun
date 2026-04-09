use crate::util::errors::*;
use ignore::gitignore::Gitignore;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct DirectoryListingEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub modified_time: SystemTime,
}

#[derive(Debug, Clone)]
pub struct FormattedDirectoryListing {
    pub reached_limit: bool,
    pub text: String,
}

#[derive(Debug, Clone)]
struct TreeEntry {
    path: String,
    is_dir: bool,
    modified_time: SystemTime,
}

pub fn list_directory_entries(
    dir_path: &str,
    limit: usize,
) -> BitFunResult<Vec<DirectoryListingEntry>> {
    let path = Path::new(dir_path);
    if !path.exists() {
        return Err(BitFunError::service(format!(
            "Directory does not exist: {}",
            dir_path
        )));
    }

    let mut result = Vec::new();
    let mut queue = VecDeque::new();

    if let Ok(metadata) = fs::symlink_metadata(path) {
        if !metadata.file_type().is_symlink() && metadata.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                for dir_entry in entries.flatten() {
                    let entry_path = dir_entry.path();
                    if let Ok(entry_metadata) = fs::symlink_metadata(&entry_path) {
                        if !entry_metadata.file_type().is_symlink() {
                            queue.push_back(DirectoryListingEntry {
                                path: entry_path,
                                is_dir: entry_metadata.is_dir(),
                                depth: 1,
                                modified_time: entry_metadata
                                    .modified()
                                    .unwrap_or(SystemTime::UNIX_EPOCH),
                            });
                        }
                    }
                }
            }
        }
    }

    let gitignore = load_gitignore(path);

    let special_folders = [
        Path::new("/"),
        Path::new("/home"),
        Path::new("/Users"),
        Path::new("/System"),
        Path::new("/Windows"),
        Path::new("/Program Files"),
        Path::new("/Program Files (x86)"),
    ];

    let excluded_folders = [
        "node_modules",
        "__pycache__",
        "env",
        "venv",
        "target",
        "target/dependency",
        "build",
        "build/dependencies",
        "dist",
        "out",
        "bundle",
        "vendor",
        "tmp",
        "temp",
        "deps",
        "pkg",
        "Pods",
        ".git",
        "Cargo.lock",
    ];

    while !queue.is_empty() && result.len() < limit {
        let current_level_size = queue.len();
        let mut level_complete = true;

        for _ in 0..current_level_size {
            if result.len() >= limit {
                level_complete = false;
                break;
            }

            let Some(entry) = queue.pop_front() else {
                continue;
            };
            let entry_path = &entry.path;

            let is_special = special_folders
                .iter()
                .any(|special| entry_path == *special || entry_path.starts_with(special));

            let folder_name = entry_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");

            let is_excluded = if entry.depth == 0 {
                false
            } else {
                excluded_folders.contains(&folder_name)
                    || (folder_name.starts_with('.') && folder_name != "." && folder_name != "..")
            };

            let is_gitignored = if let Some(ref gitignore) = gitignore {
                gitignore.matched(entry_path, entry.is_dir).is_ignore()
            } else {
                false
            };

            let is_symlink = if let Ok(metadata) = fs::symlink_metadata(entry_path) {
                metadata.file_type().is_symlink()
            } else {
                false
            };

            if !is_excluded && !is_gitignored && !is_symlink {
                result.push(entry.clone());
            }

            if entry.is_dir && !is_special && !is_excluded && !is_gitignored && !is_symlink {
                if let Ok(entries) = fs::read_dir(entry_path) {
                    for dir_entry in entries.flatten() {
                        let path = dir_entry.path();
                        if let Ok(metadata) = fs::symlink_metadata(&path) {
                            if !metadata.file_type().is_symlink() {
                                queue.push_back(DirectoryListingEntry {
                                    path,
                                    is_dir: metadata.is_dir(),
                                    depth: entry.depth + 1,
                                    modified_time: metadata
                                        .modified()
                                        .unwrap_or(SystemTime::UNIX_EPOCH),
                                });
                            }
                        }
                    }
                }
            }
        }

        if !level_complete {
            let excess = result.len().saturating_sub(limit);
            if excess > 0 {
                result.truncate(result.len() - excess);
            }
            break;
        }
    }

    Ok(result)
}

pub fn format_directory_listing(entries: &[DirectoryListingEntry], dir_path: &str) -> String {
    let base_path = Path::new(dir_path);
    let mut result = String::new();
    result.push_str(&format!(
        "{}\n",
        base_path.display().to_string().replace('\\', "/")
    ));

    let mut tree: HashMap<String, Vec<TreeEntry>> = HashMap::new();
    let mut added_dirs: HashSet<String> = HashSet::new();

    for entry in entries {
        if let Ok(rel_path) = entry.path.strip_prefix(base_path) {
            if let Some(rel_str) = rel_path.to_str() {
                let normalized = rel_str.replace('\\', "/");

                if normalized.is_empty() {
                    continue;
                }

                let final_path = if entry.is_dir && !normalized.ends_with('/') {
                    format!("{}/", normalized)
                } else {
                    normalized.clone()
                };

                let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
                for i in 0..parts.len() {
                    let is_final_entry = i == parts.len() - 1 && !entry.is_dir;
                    if is_final_entry {
                        break;
                    }

                    let ancestor_path = format!("{}/", parts[..=i].join("/"));
                    let ancestor_parent = if i == 0 {
                        "/".to_string()
                    } else {
                        format!("{}/", parts[..i].join("/"))
                    };

                    if !added_dirs.contains(&ancestor_path) {
                        added_dirs.insert(ancestor_path.clone());
                        tree.entry(ancestor_parent).or_default().push(TreeEntry {
                            path: ancestor_path,
                            is_dir: true,
                            modified_time: entry.modified_time,
                        });
                    }
                }

                if entry.is_dir && added_dirs.contains(&final_path) {
                    continue;
                }

                let parts_for_parent: Vec<&str> = final_path.split('/').collect();
                let parent = if entry.is_dir {
                    if parts_for_parent.len() > 2 {
                        format!(
                            "{}/",
                            parts_for_parent[..parts_for_parent.len() - 2].join("/")
                        )
                    } else {
                        "/".to_string()
                    }
                } else if parts_for_parent.len() > 1 {
                    format!(
                        "{}/",
                        parts_for_parent[..parts_for_parent.len() - 1].join("/")
                    )
                } else {
                    "/".to_string()
                };

                if entry.is_dir {
                    added_dirs.insert(final_path.clone());
                }

                tree.entry(parent).or_default().push(TreeEntry {
                    path: final_path,
                    is_dir: entry.is_dir,
                    modified_time: entry.modified_time,
                });
            }
        }
    }

    for children in tree.values_mut() {
        children.sort_by(|a, b| match b.modified_time.cmp(&a.modified_time) {
            std::cmp::Ordering::Equal => a.path.cmp(&b.path),
            other => other,
        });
    }

    fn format_tree(
        tree: &HashMap<String, Vec<TreeEntry>>,
        parent: &str,
        prefix: &str,
        result: &mut String,
    ) {
        if let Some(children) = tree.get(parent) {
            let count = children.len();
            for (i, child) in children.iter().enumerate() {
                let is_last = i == count - 1;
                let name = if child.is_dir {
                    let dir_name = child.path[..child.path.len() - 1]
                        .rsplit('/')
                        .next()
                        .unwrap_or("");
                    format!("{}/", dir_name)
                } else {
                    child.path.rsplit('/').next().unwrap_or("").to_string()
                };

                let connector = if is_last { "└── " } else { "├── " };
                result.push_str(&format!("{}{}{}\n", prefix, connector, name));

                if child.is_dir {
                    let child_prefix = if is_last {
                        format!("{}    ", prefix)
                    } else {
                        format!("{}│   ", prefix)
                    };
                    format_tree(tree, &child.path, &child_prefix, result);
                }
            }
        }
    }

    format_tree(&tree, "/", "", &mut result);

    if result.ends_with('\n') {
        result.pop();
    }

    result
}

pub fn get_formatted_directory_listing(
    dir_path: &str,
    limit: usize,
) -> BitFunResult<FormattedDirectoryListing> {
    let entries = list_directory_entries(dir_path, limit)?;
    let reached_limit = entries.len() >= limit;
    let text = format_directory_listing(&entries, dir_path);
    Ok(FormattedDirectoryListing {
        reached_limit,
        text,
    })
}

fn load_gitignore(dir_path: &Path) -> Option<Gitignore> {
    let gitignore_path = dir_path.join(".gitignore");

    if gitignore_path.exists() {
        match Gitignore::new(gitignore_path) {
            (gitignore, None) => Some(gitignore),
            (_, Some(_)) => None,
        }
    } else {
        None
    }
}

use globset::{GlobBuilder, GlobMatcher};
use ignore::gitignore::Gitignore;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub modified_time: SystemTime,
}

// Compiled glob matcher with its dir_only flag
struct CompiledGlob {
    matcher: GlobMatcher,
    dir_only: bool,
}

impl CompiledGlob {
    /// Check if the given path matches this glob pattern
    /// - `rel_path_str`: relative path with forward slashes
    /// - `is_dir`: whether the path is a directory
    fn is_match(&self, rel_path_str: &str, is_dir: bool) -> bool {
        // If pattern ends with '/', only match directories
        if self.dir_only && !is_dir {
            return false;
        }

        if is_dir {
            // For directories, try matching with and without trailing slash
            self.matcher.is_match(rel_path_str)
                || self.matcher.is_match(format!("{}/", rel_path_str))
        } else {
            self.matcher.is_match(rel_path_str)
        }
    }
}

pub fn list_files(
    dir_path: &str,
    limit: usize,
    glob_patterns: Option<Vec<String>>,
) -> Result<Vec<FileEntry>, String> {
    // Validate directory path
    let path = Path::new(dir_path);
    if !path.exists() {
        return Err(format!("Directory does not exist: {}", dir_path));
    }

    let mut result = Vec::new();
    let mut queue = VecDeque::new();

    // Compile glob patterns if provided
    let compiled_globs: Vec<CompiledGlob> = glob_patterns
        .map(|patterns| {
            patterns
                .into_iter()
                .filter_map(|pattern| {
                    let dir_only = pattern.ends_with('/');
                    GlobBuilder::new(&pattern)
                        .literal_separator(true) // '*' and '?' won't match path separators, only '**' will
                        .build()
                        .ok()
                        .map(|g| CompiledGlob {
                            matcher: g.compile_matcher(),
                            dir_only,
                        })
                })
                .collect()
        })
        .unwrap_or_default();

    // Initialize with the root directory
    if let Ok(metadata) = fs::symlink_metadata(path) {
        // Don't start if the root directory itself is a symbolic link
        if !metadata.file_type().is_symlink() && metadata.is_dir() {
            // Add root directory contents to queue but don't add root itself to result
            if let Ok(entries) = fs::read_dir(path) {
                for dir_entry in entries.flatten() {
                    let entry_path = dir_entry.path();
                    if let Ok(entry_metadata) = fs::symlink_metadata(&entry_path) {
                        // Skip symbolic links completely
                        if !entry_metadata.file_type().is_symlink() {
                            let is_dir = entry_metadata.is_dir();
                            queue.push_back(FileEntry {
                                path: entry_path,
                                is_dir,
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

    // Load .gitignore if it exists
    let gitignore = load_gitignore(path);

    // Special folders that should not be expanded
    let special_folders = [
        Path::new("/"),
        Path::new("/home"),
        Path::new("/Users"),
        Path::new("/System"),
        Path::new("/Windows"),
        Path::new("/Program Files"),
        Path::new("/Program Files (x86)"),
    ];

    // Folders to exclude
    let excluded_folders = vec![
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

        // Process the current level
        for _ in 0..current_level_size {
            if result.len() >= limit {
                level_complete = false;
                break;
            }

            let Some(entry) = queue.pop_front() else {
                continue;
            };
            let entry_path = &entry.path;

            // Check if this is a special folder that should not be expanded
            let is_special = special_folders
                .iter()
                .any(|special| entry_path == *special || entry_path.starts_with(special));

            // Check if this folder should be excluded
            let folder_name = entry_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");

            let is_excluded = if entry.depth == 0 {
                // Never exclude the root directory
                false
            } else {
                excluded_folders.contains(&folder_name)
                    || (folder_name.starts_with('.') && folder_name != "." && folder_name != "..")
            };

            // Check .gitignore
            let is_gitignored = if let Some(ref gitignore) = gitignore {
                gitignore.matched(entry_path, entry.is_dir).is_ignore()
            } else {
                false
            };

            // Check if the entry is a symbolic link
            let is_symlink = if let Ok(metadata) = fs::symlink_metadata(entry_path) {
                metadata.file_type().is_symlink()
            } else {
                false
            };

            // Add to result if not excluded, not gitignored, and not a symbolic link
            if !is_excluded && !is_gitignored && !is_symlink {
                // Check glob pattern match (relative to dir_path)
                let matches_glob = if compiled_globs.is_empty() {
                    true // No glob patterns means match everything
                } else if let Ok(rel_path) = entry.path.strip_prefix(path) {
                    // Convert to forward slashes for consistent matching
                    let rel_path_str = rel_path.to_string_lossy().replace('\\', "/");
                    // Match if any pattern matches (OR logic)
                    compiled_globs
                        .iter()
                        .any(|glob| glob.is_match(&rel_path_str, entry.is_dir))
                } else {
                    false
                };

                if matches_glob {
                    result.push(entry.clone());
                }
            }

            // Expand directories if they should be expanded (but not symbolic links)
            if entry.is_dir && !is_special && !is_excluded && !is_gitignored && !is_symlink {
                if let Ok(entries) = fs::read_dir(entry_path) {
                    for dir_entry in entries.flatten() {
                        let path = dir_entry.path();
                        if let Ok(metadata) = fs::symlink_metadata(&path) {
                            // Skip symbolic links completely
                            if !metadata.file_type().is_symlink() {
                                let is_dir = metadata.is_dir();
                                queue.push_back(FileEntry {
                                    path,
                                    is_dir,
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

        // If we hit the limit and the current level is not complete,
        // remove only the entries that exceeded the limit from this level
        if !level_complete {
            let excess = result.len() - limit;
            if excess > 0 {
                result.truncate(result.len() - excess);
            }
            break;
        }
    }

    Ok(result)
}

// Tree node entry with path and modified time for sorting
#[derive(Debug, Clone)]
struct TreeEntry {
    path: String, // relative path (with trailing slash for directories)
    is_dir: bool,
    modified_time: SystemTime,
}

pub fn format_files_list(files_list: Vec<FileEntry>, dir_path: &str) -> String {
    let base_path = Path::new(dir_path);
    let mut result = String::new();

    // Add the root path as the first line
    result.push_str(&format!(
        "{}\n",
        base_path.display().to_string().replace('\\', "/")
    ));

    // Parse paths into a tree structure
    let mut tree: HashMap<String, Vec<TreeEntry>> = HashMap::new();

    // Track which directory entries have been added to avoid duplicates
    let mut added_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in files_list {
        if let Ok(rel_path) = entry.path.strip_prefix(base_path) {
            if let Some(rel_str) = rel_path.to_str() {
                let normalized = rel_str.replace('\\', "/");

                if normalized.is_empty() {
                    continue;
                }

                // Add trailing slash for directories
                let final_path = if entry.is_dir && !normalized.ends_with('/') {
                    format!("{}/", normalized)
                } else {
                    normalized.clone()
                };

                // First, ensure all ancestor directories are added to the tree
                let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
                for i in 0..parts.len() {
                    let is_final_entry = i == parts.len() - 1 && !entry.is_dir;
                    if is_final_entry {
                        // This is the actual file, not a directory ancestor
                        break;
                    }

                    let ancestor_path = format!("{}/", parts[..=i].join("/"));
                    let ancestor_parent = if i == 0 {
                        "/".to_string()
                    } else {
                        format!("{}/", parts[..i].join("/"))
                    };

                    // Only add if not already added
                    if !added_dirs.contains(&ancestor_path) {
                        added_dirs.insert(ancestor_path.clone());
                        tree.entry(ancestor_parent).or_default().push(TreeEntry {
                            path: ancestor_path,
                            is_dir: true,
                            modified_time: entry.modified_time, // Use the file's time for the directory
                        });
                    }
                }

                // Now add the actual entry (file or directory)
                // For directories, skip if already added as ancestor
                if entry.is_dir && added_dirs.contains(&final_path) {
                    continue;
                }

                // Determine parent directory
                let parts_for_parent: Vec<&str> = final_path.split('/').collect();
                let parent = if entry.is_dir {
                    // For directories, parts_for_parent ends with empty string
                    if parts_for_parent.len() > 2 {
                        format!(
                            "{}/",
                            parts_for_parent[..parts_for_parent.len() - 2].join("/")
                        )
                    } else {
                        "/".to_string()
                    }
                } else {
                    // For files
                    if parts_for_parent.len() > 1 {
                        format!(
                            "{}/",
                            parts_for_parent[..parts_for_parent.len() - 1].join("/")
                        )
                    } else {
                        "/".to_string()
                    }
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

    // Sort all entries in the tree: first by modified time (newest first), then by name
    for children in tree.values_mut() {
        children.sort_by(|a, b| {
            // First compare by modified time (descending - newest first)
            match b.modified_time.cmp(&a.modified_time) {
                std::cmp::Ordering::Equal => {
                    // If modified times are equal, sort by name (ascending)
                    a.path.cmp(&b.path)
                }
                other => other,
            }
        });
    }

    // Build the formatted output recursively with tree-style prefixes
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

                // Extract the file/directory name (last component)
                let name = if child.is_dir {
                    let dir_name = child.path[..child.path.len() - 1]
                        .rsplit('/')
                        .next()
                        .unwrap_or("");
                    format!("{}/", dir_name)
                } else {
                    child.path.rsplit('/').next().unwrap_or("").to_string()
                };

                // Choose the appropriate connector
                let connector = if is_last { "└── " } else { "├── " };

                // Add the line with prefix and connector
                result.push_str(&format!("{}{}{}\n", prefix, connector, name));

                // If it's a directory, recurse with updated prefix
                if child.is_dir {
                    // For children, add either "│   " or "    " depending on whether current is last
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

    // Start with root level (empty parent string)
    format_tree(&tree, "/", "", &mut result);

    // Remove trailing newline
    if result.ends_with('\n') {
        result.pop();
    }

    result
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

pub fn get_formatted_files_list(
    dir_path: &str,
    limit: usize,
    glob_patterns: Option<Vec<String>>,
) -> Result<(bool, String), String> {
    let files_list = list_files(dir_path, limit, glob_patterns)?;
    let files_count = files_list.len();
    let formatted_files_list = format_files_list(files_list, dir_path);
    Ok((files_count >= limit, formatted_files_list))
}

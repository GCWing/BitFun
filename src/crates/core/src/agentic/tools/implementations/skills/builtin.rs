//! Built-in skills shipped with BitFun.
//!
//! These skills are embedded into the `bitfun-core` binary and installed into the user skills
//! directory on demand and kept in sync with bundled versions.

use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::BitFunResult;
use include_dir::{include_dir, Dir};
use log::{debug, error};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::fs;

static BUILTIN_SKILLS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/builtin_skills");
static BUILTIN_SKILL_DIR_NAMES: OnceLock<HashSet<String>> = OnceLock::new();

fn collect_builtin_skill_dir_names() -> HashSet<String> {
    BUILTIN_SKILLS_DIR
        .dirs()
        .filter_map(|dir| {
            let rel = dir.path();
            if rel.components().count() != 1 {
                return None;
            }

            rel.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
        })
        .collect()
}

pub fn builtin_skill_dir_names() -> &'static HashSet<String> {
    BUILTIN_SKILL_DIR_NAMES.get_or_init(collect_builtin_skill_dir_names)
}

pub fn is_builtin_skill_dir_name(dir_name: &str) -> bool {
    builtin_skill_dir_names().contains(dir_name)
}

pub fn builtin_skill_group_key(dir_name: &str) -> Option<&'static str> {
    match dir_name {
        "docx" | "pdf" | "pptx" | "xlsx" => Some("office"),
        "find-skills" | "writing-skills" => Some("meta"),
        "agent-browser" => Some("computer-use"),
        _ if dir_name.starts_with("gstack-") => Some("team"),
        _ => None,
    }
}

pub fn is_team_skill(dir_name: &str) -> bool {
    builtin_skill_group_key(dir_name) == Some("team")
}

pub async fn ensure_builtin_skills_installed() -> BitFunResult<()> {
    let pm = get_path_manager_arc();
    let dest_root = pm.user_skills_dir();

    // Create user skills directory if needed.
    if let Err(e) = fs::create_dir_all(&dest_root).await {
        error!(
            "Failed to create user skills directory: path={}, error={}",
            dest_root.display(),
            e
        );
        return Err(e.into());
    }

    let mut installed = 0usize;
    let mut updated = 0usize;
    for skill_dir in BUILTIN_SKILLS_DIR.dirs() {
        let rel = skill_dir.path();
        if rel.components().count() != 1 {
            continue;
        }

        let stats = sync_dir(skill_dir, &dest_root).await?;
        installed += stats.installed;
        updated += stats.updated;
    }

    if installed > 0 || updated > 0 {
        debug!(
            "Built-in skills synchronized: installed={}, updated={}, dest_root={}",
            installed,
            updated,
            dest_root.display()
        );
    }

    Ok(())
}

#[derive(Default)]
struct SyncStats {
    installed: usize,
    updated: usize,
}

async fn sync_dir(dir: &Dir<'_>, dest_root: &Path) -> BitFunResult<SyncStats> {
    let mut files: Vec<&include_dir::File<'_>> = Vec::new();
    collect_files(dir, &mut files);

    let mut stats = SyncStats::default();
    for file in files.into_iter() {
        let dest_path = safe_join(dest_root, file.path())?;
        let desired = desired_file_content(file, &dest_path).await?;

        if let Ok(current) = fs::read(&dest_path).await {
            if current == desired {
                continue;
            }
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let existed = dest_path.exists();
        fs::write(&dest_path, desired).await?;
        if existed {
            stats.updated += 1;
        } else {
            stats.installed += 1;
        }
    }

    Ok(stats)
}

fn collect_files<'a>(dir: &'a Dir<'a>, out: &mut Vec<&'a include_dir::File<'a>>) {
    for file in dir.files() {
        out.push(file);
    }

    for sub in dir.dirs() {
        collect_files(sub, out);
    }
}

fn safe_join(root: &Path, relative: &Path) -> BitFunResult<PathBuf> {
    if relative.is_absolute() {
        return Err(crate::util::errors::BitFunError::validation(format!(
            "Unexpected absolute path in built-in skills: {}",
            relative.display()
        )));
    }

    // Prevent `..` traversal even though include_dir should only contain clean relative paths.
    for c in relative.components() {
        if matches!(c, std::path::Component::ParentDir) {
            return Err(crate::util::errors::BitFunError::validation(format!(
                "Unexpected parent dir component in built-in skills path: {}",
                relative.display()
            )));
        }
    }

    Ok(root.join(relative))
}

async fn desired_file_content(
    file: &include_dir::File<'_>,
    _dest_path: &Path,
) -> BitFunResult<Vec<u8>> {
    Ok(file.contents().to_vec())
}

#[cfg(test)]
mod tests {
    use super::builtin_skill_group_key;

    #[test]
    fn builtin_skill_groups_match_expected_sets() {
        assert_eq!(builtin_skill_group_key("docx"), Some("office"));
        assert_eq!(builtin_skill_group_key("pdf"), Some("office"));
        assert_eq!(builtin_skill_group_key("pptx"), Some("office"));
        assert_eq!(builtin_skill_group_key("xlsx"), Some("office"));
        assert_eq!(builtin_skill_group_key("find-skills"), Some("meta"));
        assert_eq!(builtin_skill_group_key("writing-skills"), Some("meta"));
        assert_eq!(
            builtin_skill_group_key("agent-browser"),
            Some("computer-use")
        );
        assert_eq!(builtin_skill_group_key("unknown-skill"), None);
        assert_eq!(builtin_skill_group_key("gstack-review"), Some("team"));
        assert_eq!(builtin_skill_group_key("gstack-ship"), Some("team"));
        assert_eq!(builtin_skill_group_key("gstack-qa"), Some("team"));
        assert_eq!(builtin_skill_group_key("gstack-cso"), Some("team"));
    }
}

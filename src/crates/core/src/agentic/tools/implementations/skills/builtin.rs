//! Built-in skills shipped with BitFun.
//!
//! These skills are embedded into the `bitfun-core` binary and installed into the user skills
//! directory on demand and kept in sync with bundled versions.

use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::BitFunResult;
use crate::util::front_matter_markdown::FrontMatterMarkdown;
use include_dir::{include_dir, Dir};
use log::{debug, error};
use serde_yaml::Value;
use std::path::{Path, PathBuf};
use tokio::fs;

static BUILTIN_SKILLS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/builtin_skills");

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
    dest_path: &Path,
) -> BitFunResult<Vec<u8>> {
    let source = file.contents();
    if !is_skill_markdown(file.path()) {
        return Ok(source.to_vec());
    }

    let source_text = match std::str::from_utf8(source) {
        Ok(v) => v,
        Err(_) => return Ok(source.to_vec()),
    };

    let enabled = if let Ok(existing) = fs::read_to_string(dest_path).await {
        // Preserve user-selected state when file already exists.
        extract_enabled_flag(&existing).unwrap_or(true)
    } else {
        // On first install, respect bundled default (if present), otherwise enable by default.
        extract_enabled_flag(source_text).unwrap_or(true)
    };

    let merged = merge_skill_markdown_enabled(source_text, enabled)?;
    Ok(merged.into_bytes())
}

fn is_skill_markdown(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.eq_ignore_ascii_case("SKILL.md"))
        .unwrap_or(false)
}

fn extract_enabled_flag(markdown: &str) -> Option<bool> {
    let (metadata, _) = FrontMatterMarkdown::load_str(markdown).ok()?;
    metadata.get("enabled").and_then(|v| v.as_bool())
}

fn merge_skill_markdown_enabled(markdown: &str, enabled: bool) -> BitFunResult<String> {
    let (mut metadata, body) = FrontMatterMarkdown::load_str(markdown)
        .map_err(|e| crate::util::errors::BitFunError::tool(format!("Invalid SKILL.md: {}", e)))?;

    let map = metadata.as_mapping_mut().ok_or_else(|| {
        crate::util::errors::BitFunError::tool(
            "Invalid SKILL.md: metadata is not a mapping".to_string(),
        )
    })?;

    if enabled {
        map.remove(&Value::String("enabled".to_string()));
    } else {
        map.insert(Value::String("enabled".to_string()), Value::Bool(false));
    }

    let yaml = serde_yaml::to_string(&metadata).map_err(|e| {
        crate::util::errors::BitFunError::tool(format!("Failed to serialize SKILL.md: {}", e))
    })?;
    Ok(format!(
        "---\n{}\n---\n\n{}",
        yaml.trim_end(),
        body.trim_start()
    ))
}

//! Built-in skills shipped with BitFun.
//!
//! These skills are embedded into the `bitfun-core` binary and installed into the user skills
//! directory on demand (without overwriting user-installed skills).

use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::BitFunResult;
use include_dir::{include_dir, Dir};
use log::{debug, error};
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
    for skill_dir in BUILTIN_SKILLS_DIR.dirs() {
        let rel = skill_dir.path();
        if rel.components().count() != 1 {
            continue;
        }

        let dest_skill_dir = dest_root.join(rel);
        if dest_skill_dir.exists() {
            continue;
        }

        install_dir(skill_dir, &dest_root).await?;
        installed += 1;
    }

    if installed > 0 {
        debug!(
            "Built-in skills installed: count={}, dest_root={}",
            installed,
            dest_root.display()
        );
    }

    Ok(())
}

async fn install_dir(dir: &Dir<'_>, dest_root: &Path) -> BitFunResult<()> {
    let mut files: Vec<&include_dir::File<'_>> = Vec::new();
    collect_files(dir, &mut files);

    for file in files.into_iter() {
        let dest_path = safe_join(dest_root, file.path())?;
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&dest_path, file.contents()).await?;
    }

    Ok(())
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

use std::path::PathBuf;

use crate::diagnostics::ExitKind;

use super::lifecycle::{ExecMode, ExecOutputFormat, ExecPatchOutput};

impl ExecMode {
    fn get_git_diff(&self) -> Option<String> {
        let workspace = self.workspace_path.as_ref()?;
        Self::get_git_diff_for_workspace(workspace, self.output_patch.as_deref())
    }

    pub(super) fn get_git_diff_for_workspace(
        workspace: &std::path::Path,
        output_target: Option<&str>,
    ) -> Option<String> {
        let repo_root_output = bitfun_core::util::process_manager::create_command("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(workspace)
            .output()
            .ok()?;
        if !repo_root_output.status.success() {
            eprintln!("Warning: Workspace is not a git repository, cannot generate patch");
            return None;
        }
        let repo_root = PathBuf::from(
            String::from_utf8_lossy(&repo_root_output.stdout)
                .trim()
                .to_string(),
        );

        let excluded_output = output_target
            .filter(|target| *target != "-")
            .and_then(|target| {
                let repo_root = std::fs::canonicalize(&repo_root).ok()?;
                let output_path =
                    Self::canonicalize_path_allowing_missing(std::path::Path::new(target))?;
                let relative = output_path.strip_prefix(repo_root).ok()?;
                (!relative.as_os_str().is_empty())
                    .then(|| relative.to_string_lossy().replace('\\', "/"))
            });

        let mut tracked_command = bitfun_core::util::process_manager::create_command("git");
        tracked_command
            .args(["diff", "--binary", "--no-color", "HEAD", "--", "."])
            .current_dir(&repo_root);
        if let Some(relative_path) = excluded_output.as_ref() {
            tracked_command.arg(format!(":(exclude,top,literal){relative_path}"));
        }
        let tracked = tracked_command.output().ok()?;
        if !tracked.status.success() {
            eprintln!("Warning: git diff execution failed");
            return None;
        }

        let untracked = bitfun_core::util::process_manager::create_command("git")
            .args(["ls-files", "--others", "--exclude-standard", "-z"])
            .current_dir(&repo_root)
            .output()
            .ok()?;
        if !untracked.status.success() {
            eprintln!("Warning: git untracked file discovery failed");
            return None;
        }

        let mut patch = String::from_utf8_lossy(&tracked.stdout).to_string();
        for relative_path in untracked.stdout.split(|byte| *byte == 0) {
            if relative_path.is_empty() {
                continue;
            }
            let relative_path = String::from_utf8_lossy(relative_path).to_string();
            if excluded_output.as_deref() == Some(relative_path.as_str()) {
                continue;
            }
            let untracked_patch = bitfun_core::util::process_manager::create_command("git")
                .args([
                    "diff",
                    "--no-index",
                    "--binary",
                    "--no-color",
                    "--",
                    "/dev/null",
                    &relative_path,
                ])
                .current_dir(&repo_root)
                .output()
                .ok()?;
            if !matches!(untracked_patch.status.code(), Some(0 | 1)) {
                eprintln!("Warning: failed to generate patch for untracked file {relative_path}");
                return None;
            }
            if !patch.is_empty() && !patch.ends_with('\n') {
                patch.push('\n');
            }
            patch.push_str(&String::from_utf8_lossy(&untracked_patch.stdout));
        }

        Some(patch)
    }

    fn canonicalize_path_allowing_missing(path: &std::path::Path) -> Option<PathBuf> {
        let absolute = std::path::absolute(path).ok()?;
        let mut existing = absolute.as_path();
        let mut missing = Vec::new();
        while !existing.exists() {
            missing.push(existing.file_name()?.to_os_string());
            existing = existing.parent()?;
        }

        let mut resolved = std::fs::canonicalize(existing).ok()?;
        for component in missing.into_iter().rev() {
            resolved.push(component);
        }
        Some(resolved)
    }

    pub(super) fn output_patch_if_needed(
        &self,
    ) -> (Option<ExecPatchOutput>, Option<(ExitKind, anyhow::Error)>) {
        let Some(output_target) = self.output_patch.as_ref() else {
            return (None, None);
        };
        if self.output_format == ExecOutputFormat::StreamJson && output_target == "-" {
            let error = anyhow::anyhow!(
                "--output-patch with --output-format stream-json requires an explicit file path"
            );
            return (
                Some(ExecPatchOutput {
                    target: output_target.clone(),
                    status: "unavailable",
                    patch: None,
                    bytes: None,
                }),
                Some((ExitKind::PatchUnavailable, error)),
            );
        }
        let Some(patch) = self.get_git_diff() else {
            self.print_text(|| eprintln!("Unable to generate patch"));
            let error = anyhow::anyhow!("Unable to generate requested git patch");
            return (
                Some(ExecPatchOutput {
                    target: output_target.clone(),
                    status: "unavailable",
                    patch: None,
                    bytes: None,
                }),
                Some((ExitKind::PatchUnavailable, error)),
            );
        };

        let is_empty = patch.trim().is_empty();
        let status = if is_empty { "empty" } else { "generated" };
        if output_target != "-" {
            if let Err(error) = write_patch_to_path(output_target, &patch) {
                eprintln!("Failed to save patch: {error}");
                return (
                    Some(ExecPatchOutput {
                        target: output_target.clone(),
                        status: "write_failed",
                        patch: None,
                        bytes: Some(patch.len()),
                    }),
                    Some((
                        ExitKind::PatchWriteFailed,
                        anyhow::anyhow!("Failed to save requested patch: {error}"),
                    )),
                );
            }
        }

        if self.output_format == ExecOutputFormat::Text {
            if is_empty {
                eprintln!("No file modifications");
            } else if output_target == "-" {
                println!("---PATCH_START---");
                println!("{patch}");
                println!("---PATCH_END---");
            } else {
                eprintln!("Patch saved to: {output_target} ({} bytes)", patch.len());
            }
        }

        (
            Some(ExecPatchOutput {
                target: output_target.clone(),
                status,
                patch: (self.output_format == ExecOutputFormat::Json && output_target == "-")
                    .then_some(patch.clone()),
                bytes: Some(patch.len()),
            }),
            None,
        )
    }
}

pub(super) fn write_patch_to_path(output_target: &str, patch: &str) -> std::io::Result<()> {
    use std::path::Path;

    let path = Path::new(output_target);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, patch)
}

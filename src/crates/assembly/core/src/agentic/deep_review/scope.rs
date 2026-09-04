use crate::agentic::tools::framework::ToolUseContext;
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use openbitfun_agent_runtime::deep_review::{
    FocusedReviewAssignment, FocusedReviewPathAccess, ReviewTargetEvidence,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn focused_scope(
    context: &ToolUseContext,
) -> OpenBitFunResult<Option<(FocusedReviewAssignment, ReviewTargetEvidence)>> {
    let Some(raw_manifest) = context.custom_data.get("deep_review_run_manifest") else {
        return Ok(None);
    };
    let parsed;
    let manifest = if let Some(serialized) = raw_manifest.as_str() {
        parsed = serde_json::from_str::<Value>(serialized).map_err(|_| {
            OpenBitFunError::tool("Focused Review manifest is malformed".to_string())
        })?;
        &parsed
    } else {
        raw_manifest
    };
    let Some(assignment) = FocusedReviewAssignment::from_manifest(manifest)
        .map_err(|violation| OpenBitFunError::tool(violation.to_tool_error_message()))?
    else {
        return Ok(None);
    };
    let evidence = ReviewTargetEvidence::from_manifest(manifest)
        .map_err(|error| OpenBitFunError::tool(error.to_string()))?
        .ok_or_else(|| {
            OpenBitFunError::tool("Focused Review target evidence is missing".to_string())
        })?;
    Ok(Some((assignment, evidence)))
}

pub fn ensure_focused_review_path_allowed(
    context: &ToolUseContext,
    path: &str,
) -> OpenBitFunResult<()> {
    let Some((assignment, evidence)) = focused_scope(context)? else {
        return Ok(());
    };
    ensure_path_allowed(&assignment, &evidence, path)
}

pub fn ensure_focused_review_resolved_path_allowed(
    context: &ToolUseContext,
    resolved_path: &str,
) -> OpenBitFunResult<()> {
    let Some((assignment, evidence)) = focused_scope(context)? else {
        return Ok(());
    };
    if context.is_remote() {
        return Err(OpenBitFunError::tool(
            "Focused Review file access is unavailable for remote workspaces because target scope cannot be guaranteed"
                .to_string(),
        ));
    }
    let root = context.workspace_root().ok_or_else(|| {
        OpenBitFunError::tool("Focused Review file access requires a workspace root".to_string())
    })?;
    let resolved_path = Path::new(resolved_path);
    ensure_focused_local_path_syntax_safe(resolved_path)?;
    let path = local_workspace_relative_path(resolved_path, root).ok_or_else(|| {
        OpenBitFunError::tool(
            "Focused Review file access is limited to the current workspace".to_string(),
        )
    })?;
    ensure_focused_relative_path_syntax_safe(&path)?;
    if tool_runtime::fs::path_has_multiple_hard_links(resolved_path).map_err(|_| {
        OpenBitFunError::tool("Focused Review could not verify the local file identity".to_string())
    })? {
        return Err(OpenBitFunError::tool(
            "Focused Review file access cannot use a hard-link alias".to_string(),
        ));
    }
    ensure_local_path_allowed(&assignment, &evidence, &path)?;

    if let (Ok(canonical_root), Ok(canonical_path)) = (
        std::fs::canonicalize(root),
        std::fs::canonicalize(resolved_path),
    ) {
        let canonical_relative = local_workspace_relative_path(&canonical_path, &canonical_root)
            .ok_or_else(|| {
                OpenBitFunError::tool(
                    "Focused Review file access cannot follow links outside the current workspace"
                        .to_string(),
                )
            })?;
        ensure_local_path_allowed(&assignment, &evidence, &canonical_relative)?;
        if !workspace_relative_path_eq(&path, &canonical_relative) {
            return Err(OpenBitFunError::tool(
                "Focused Review file access cannot use a linked path alias".to_string(),
            ));
        }
    }
    Ok(())
}

fn ensure_focused_local_path_syntax_safe(path: &Path) -> OpenBitFunResult<()> {
    #[cfg(windows)]
    {
        let normalized = path.to_string_lossy().replace('/', "\\");
        if normalized.starts_with(r"\\?\") || normalized.starts_with(r"\\.\") {
            return Err(OpenBitFunError::tool(
                "Focused Review file access cannot use a Windows device path".to_string(),
            ));
        }
    }
    #[cfg(not(windows))]
    let _ = path;
    Ok(())
}

fn ensure_focused_relative_path_syntax_safe(path: &str) -> OpenBitFunResult<()> {
    #[cfg(windows)]
    if path.split('/').any(|component| component.contains(':')) {
        return Err(OpenBitFunError::tool(
            "Focused Review file access cannot use a Windows alternate data stream".to_string(),
        ));
    }
    #[cfg(not(windows))]
    let _ = path;
    Ok(())
}

fn ensure_path_allowed(
    assignment: &FocusedReviewAssignment,
    evidence: &ReviewTargetEvidence,
    path: &str,
) -> OpenBitFunResult<()> {
    ensure_access_allowed(assignment.path_access_with_evidence(evidence, path), path)
}

fn ensure_local_path_allowed(
    assignment: &FocusedReviewAssignment,
    evidence: &ReviewTargetEvidence,
    path: &str,
) -> OpenBitFunResult<()> {
    ensure_access_allowed(
        assignment.path_access_with_local_evidence(evidence, path),
        path,
    )
}

fn ensure_access_allowed(access: FocusedReviewPathAccess, path: &str) -> OpenBitFunResult<()> {
    if access == FocusedReviewPathAccess::UnassignedChange {
        return Err(OpenBitFunError::tool(format!(
            "Focused Review scope excludes changed file '{path}'; inspect only assigned changes or unchanged dependencies needed as evidence"
        )));
    }
    Ok(())
}

fn local_workspace_relative_path(path: &Path, root: &Path) -> Option<String> {
    if let Ok(relative) = path.strip_prefix(root) {
        return Some(relative.to_string_lossy().replace('\\', "/"));
    }

    #[cfg(windows)]
    {
        let path = path.to_string_lossy().replace('\\', "/");
        let root = root
            .to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string();
        if path.eq_ignore_ascii_case(&root) {
            return Some(String::new());
        }
        let prefix = format!("{root}/");
        if path
            .get(..prefix.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&prefix))
        {
            return path.get(prefix.len()..).map(str::to_string);
        }
    }

    None
}

fn workspace_relative_path_eq(left: &str, right: &str) -> bool {
    if cfg!(windows) {
        left.to_lowercase() == right.to_lowercase()
    } else {
        left == right
    }
}

pub fn focused_review_excluded_changed_paths(
    context: &ToolUseContext,
) -> OpenBitFunResult<Option<Vec<PathBuf>>> {
    let Some((assignment, evidence)) = focused_scope(context)? else {
        return Ok(None);
    };
    let root = context.workspace_root().ok_or_else(|| {
        OpenBitFunError::tool("Focused Review grep requires a local workspace root".to_string())
    })?;
    let excluded = evidence
        .files()
        .iter()
        .filter(|file| {
            !assignment
                .allowed_changed_paths()
                .iter()
                .any(|allowed| allowed == file.path())
        })
        .map(|file| root.join(file.path()))
        .collect();
    Ok(Some(excluded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::WorkspaceBinding;
    use serde_json::json;

    #[test]
    fn focused_scope_blocks_other_changes_but_allows_dependencies() {
        let mut context = ToolUseContext::for_tool_listing(None, None);
        context.custom_data.insert(
            "deep_review_run_manifest".to_string(),
            json!({
                "evidencePack": {
                    "reviewTarget": {
                        "version": 1,
                        "source": "git_range",
                        "fingerprint": "target-12345678",
                        "baseRevision": "1111111111111111111111111111111111111111",
                        "headRevision": "2222222222222222222222222222222222222222",
                        "completeness": "complete",
                        "workspaceBinding": "matching_clean",
                        "files": [
                            { "path": "src/assigned.rs", "status": "modified", "completeness": "complete" },
                            { "path": "src/other.rs", "status": "modified", "completeness": "complete" }
                        ],
                        "diffRefs": [],
                        "limitations": []
                    }
                },
                "focusedAssignment": {
                    "questionId": "focus-1",
                    "question": "Is the assigned boundary safe?",
                    "independentValue": "The primary review needs separate evidence.",
                    "targetFingerprint": "target-12345678",
                    "allowedChangedPaths": ["src/assigned.rs"],
                    "expectedEvidence": "A concrete call path.",
                    "capabilityKey": "builtin::review-worker",
                    "capabilityFingerprint": "capability-12345678"
                }
            }),
        );

        assert!(ensure_focused_review_path_allowed(&context, "src/assigned.rs").is_ok());
        assert!(ensure_focused_review_path_allowed(&context, "src/dependency.rs").is_ok());
        assert!(ensure_focused_review_path_allowed(&context, "src/other.rs").is_err());
    }

    #[test]
    fn focused_scope_rejects_resolved_paths_outside_the_workspace() {
        let temp = tempfile::tempdir().expect("temporary workspace");
        let root = temp.path().join("project");
        std::fs::create_dir_all(root.join("src")).expect("source directory");
        std::fs::write(root.join("src/assigned.rs"), "assigned").expect("assigned file");
        std::fs::write(root.join("src/other.rs"), "other").expect("other file");
        std::fs::hard_link(root.join("src/other.rs"), root.join("alias.rs"))
            .expect("hard-link alias");
        let mut context =
            ToolUseContext::for_tool_listing(Some(WorkspaceBinding::new(None, root.clone())), None);
        context.custom_data.insert(
            "deep_review_run_manifest".to_string(),
            json!({
                "evidencePack": {
                    "reviewTarget": {
                        "version": 1,
                        "source": "git_range",
                        "fingerprint": "target-12345678",
                        "baseRevision": "1111111111111111111111111111111111111111",
                        "headRevision": "2222222222222222222222222222222222222222",
                        "completeness": "complete",
                        "workspaceBinding": "matching_clean",
                        "files": [
                            { "path": "src/assigned.rs", "status": "modified", "completeness": "complete" },
                            { "path": "src/other.rs", "status": "modified", "completeness": "complete" }
                        ],
                        "diffRefs": [],
                        "limitations": []
                    }
                },
                "focusedAssignment": {
                    "questionId": "focus-1",
                    "question": "Is the assigned boundary safe?",
                    "independentValue": "The primary review needs separate evidence.",
                    "targetFingerprint": "target-12345678",
                    "allowedChangedPaths": ["src/assigned.rs"],
                    "expectedEvidence": "A concrete call path.",
                    "capabilityKey": "builtin::review-worker",
                    "capabilityFingerprint": "capability-12345678"
                }
            }),
        );

        assert!(
            ensure_focused_review_resolved_path_allowed(&context, &root.to_string_lossy(),).is_ok()
        );
        assert!(ensure_focused_review_resolved_path_allowed(
            &context,
            &root.join("src/assigned.rs").to_string_lossy(),
        )
        .is_ok());
        assert!(ensure_focused_review_resolved_path_allowed(
            &context,
            &temp.path().join("elsewhere/secret.rs").to_string_lossy(),
        )
        .is_err());
        assert!(ensure_focused_review_resolved_path_allowed(
            &context,
            &root.join("alias.rs").to_string_lossy(),
        )
        .is_err());
        #[cfg(windows)]
        assert!(ensure_focused_review_resolved_path_allowed(
            &context,
            &root.join("SRC/OTHER.RS").to_string_lossy(),
        )
        .is_err());
        #[cfg(windows)]
        assert!(ensure_focused_review_resolved_path_allowed(
            &context,
            &root.join("src/other.rs::$DATA").to_string_lossy(),
        )
        .is_err());
        #[cfg(windows)]
        assert!(ensure_focused_review_resolved_path_allowed(
            &context,
            &format!(r"\\?\{}\src\assigned.rs", root.to_string_lossy()),
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn focused_scope_rejects_symlink_aliases_to_unassigned_changes() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temporary workspace");
        let root = temp.path();
        std::fs::create_dir(root.join("src")).expect("source directory");
        std::fs::write(root.join("src/other.rs"), "secret").expect("changed file");
        symlink(root.join("src/other.rs"), root.join("alias.rs")).expect("symlink");

        let mut context = ToolUseContext::for_tool_listing(
            Some(WorkspaceBinding::new(None, root.to_path_buf())),
            None,
        );
        context.custom_data.insert(
            "deep_review_run_manifest".to_string(),
            json!({
                "evidencePack": { "reviewTarget": {
                    "version": 1,
                    "source": "git_range",
                    "fingerprint": "target-12345678",
                    "baseRevision": "1111111111111111111111111111111111111111",
                    "headRevision": "2222222222222222222222222222222222222222",
                    "completeness": "complete",
                    "workspaceBinding": "matching_clean",
                    "files": [
                        { "path": "src/assigned.rs", "status": "modified", "completeness": "complete" },
                        { "path": "src/other.rs", "status": "modified", "completeness": "complete" }
                    ],
                    "diffRefs": [],
                    "limitations": []
                }},
                "focusedAssignment": {
                    "questionId": "focus-1",
                    "question": "Is the assigned boundary safe?",
                    "independentValue": "The primary review needs separate evidence.",
                    "targetFingerprint": "target-12345678",
                    "allowedChangedPaths": ["src/assigned.rs"],
                    "expectedEvidence": "A concrete call path.",
                    "capabilityKey": "builtin::review-worker",
                    "capabilityFingerprint": "capability-12345678"
                }
            }),
        );

        assert!(ensure_focused_review_resolved_path_allowed(
            &context,
            &root.join("alias.rs").to_string_lossy(),
        )
        .is_err());
    }
}

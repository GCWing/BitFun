use crate::agentic::tools::framework::{PermissionIntent, ToolPathResolution, ToolUseContext};
use crate::agentic::tools::restrictions::{
    canonicalize_local_path_best_effort, is_local_path_within_root,
};
use crate::util::errors::{BitFunError, BitFunResult};
use std::collections::HashSet;
use std::path::Path;

pub(crate) fn file_permission_intents<'a>(
    action: &str,
    paths: impl IntoIterator<Item = &'a str>,
    context: &ToolUseContext,
) -> BitFunResult<Vec<PermissionIntent>> {
    let mut resources = Vec::new();
    let mut external_directories = Vec::new();
    let mut seen_resources = HashSet::new();
    let mut seen_external_directories = HashSet::new();

    for path in paths {
        let resolved = context.resolve_tool_path(path)?;
        let resource = normalized_permission_resource(&resolved)?;
        if seen_resources.insert(resource.clone()) {
            resources.push(resource);
        }

        if let Some(directory) = external_directory_resource(context, &resolved)? {
            if seen_external_directories.insert(directory.clone()) {
                external_directories.push(directory);
            }
        }
    }

    if resources.is_empty() {
        return Err(BitFunError::validation(
            "File permission intent requires at least one resource".to_string(),
        ));
    }

    let mut intents = vec![PermissionIntent::new(action, resources)];
    if !external_directories.is_empty() {
        intents.push(PermissionIntent::new(
            "external_directory",
            external_directories,
        ));
    }
    Ok(intents)
}

fn normalized_permission_resource(resolved: &ToolPathResolution) -> BitFunResult<String> {
    if resolved.uses_remote_workspace_backend() || resolved.runtime_scope.is_some() {
        return Ok(resolved.resolved_path.replace('\\', "/"));
    }

    Ok(
        canonicalize_local_path_best_effort(Path::new(&resolved.resolved_path))?
            .to_string_lossy()
            .replace('\\', "/"),
    )
}

fn external_directory_resource(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> BitFunResult<Option<String>> {
    if resolved.uses_remote_workspace_backend() || resolved.runtime_scope.is_some() {
        return Ok(None);
    }

    let workspace_root = context.workspace_root().ok_or_else(|| {
        BitFunError::validation("A workspace is required for file permissions".to_string())
    })?;
    let path = Path::new(&resolved.resolved_path);
    if is_local_path_within_root(path, workspace_root)? {
        return Ok(None);
    }

    let directory = if path.is_dir() {
        path
    } else {
        path.parent().ok_or_else(|| {
            BitFunError::validation(format!(
                "External path '{}' has no parent directory",
                path.display()
            ))
        })?
    };
    Ok(Some(
        canonicalize_local_path_best_effort(directory)?
            .to_string_lossy()
            .replace('\\', "/"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::framework::Tool;
    use crate::agentic::tools::implementations::{
        DeleteFileTool, FileEditTool, FileReadTool, FileWriteTool,
    };
    use crate::agentic::WorkspaceBinding;
    use serde_json::{json, Value};
    use std::fs;

    #[test]
    fn local_external_file_adds_external_directory_intent() {
        let temp = tempfile::tempdir().expect("temp dir");
        let workspace = temp.path().join("workspace");
        let external = temp.path().join("external");
        fs::create_dir_all(&workspace).expect("workspace dir");
        fs::create_dir_all(&external).expect("external dir");
        let external_file = external.join("outside.txt");
        fs::write(&external_file, "outside").expect("external file");
        let context =
            ToolUseContext::for_tool_listing(Some(WorkspaceBinding::new(None, workspace)), None);

        let intents =
            file_permission_intents("read", [external_file.to_string_lossy().as_ref()], &context)
                .expect("permission intents");

        assert_eq!(intents.len(), 2);
        assert_eq!(intents[0].action, "read");
        assert_eq!(intents[1].action, "external_directory");
        assert_eq!(intents[1].resources.len(), 1);
        assert_eq!(
            intents[1].resources[0],
            canonicalize_local_path_best_effort(&external)
                .expect("canonical external dir")
                .to_string_lossy()
                .replace('\\', "/")
        );
    }

    #[test]
    fn workspace_file_does_not_add_external_directory_intent() {
        let temp = tempfile::tempdir().expect("temp dir");
        let workspace = temp.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace dir");
        let workspace_file = workspace.join("inside.txt");
        fs::write(&workspace_file, "inside").expect("workspace file");
        let context =
            ToolUseContext::for_tool_listing(Some(WorkspaceBinding::new(None, workspace)), None);

        let intents = file_permission_intents(
            "read",
            [workspace_file.to_string_lossy().as_ref()],
            &context,
        )
        .expect("permission intents");

        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].action, "read");
    }

    #[test]
    fn multi_file_edit_keeps_patch_targets_in_one_atomic_intent() {
        let temp = tempfile::tempdir().expect("temp dir");
        let workspace = temp.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace dir");
        let first = workspace.join("first.txt");
        let second = workspace.join("second.txt");
        fs::write(&first, "first").expect("first file");
        fs::write(&second, "second").expect("second file");
        let context =
            ToolUseContext::for_tool_listing(Some(WorkspaceBinding::new(None, workspace)), None);

        let intents = file_permission_intents(
            "edit",
            [
                first.to_string_lossy().as_ref(),
                second.to_string_lossy().as_ref(),
            ],
            &context,
        )
        .expect("multi-file edit intent");

        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].action, "edit");
        assert_eq!(intents[0].resources.len(), 2);
        assert_eq!(intents[0].save_resources, intents[0].resources);
    }

    #[test]
    fn migrated_file_tools_emit_read_and_edit_intents() {
        let temp = tempfile::tempdir().expect("temp dir");
        let workspace = temp.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace dir");
        let file = workspace.join("file.txt");
        fs::write(&file, "old").expect("workspace file");
        let mut context =
            ToolUseContext::for_tool_listing(Some(WorkspaceBinding::new(None, workspace)), None);
        context.tool_call_id = Some("tool-call-123".to_string());
        let file_path = file.to_string_lossy();

        let read = FileReadTool::new();
        let write = FileWriteTool::new();
        let edit = FileEditTool::new();
        let delete = DeleteFileTool::new();
        let cases: Vec<(&dyn Tool, Value, &str)> = vec![
            (&read, json!({ "file_path": file_path.as_ref() }), "read"),
            (
                &write,
                json!({ "payload": format!("+++ {}\nnew", file_path) }),
                "edit",
            ),
            (
                &edit,
                json!({
                    "file_path": file_path.as_ref(),
                    "old_string": "old",
                    "new_string": "new"
                }),
                "edit",
            ),
            (&delete, json!({ "path": file_path.as_ref() }), "edit"),
        ];

        for (tool, input, expected_action) in cases {
            let intents = tool
                .permission_intents(&input, &context)
                .expect("file tool permission intent");
            assert_eq!(intents.len(), 1, "{}", tool.name());
            assert_eq!(intents[0].action, expected_action, "{}", tool.name());
            assert_eq!(intents[0].resources.len(), 1, "{}", tool.name());
        }

        let fallback = FileWriteTool::new()
            .permission_intents(&json!({ "payload": "new file" }), &context)
            .expect("fallback write intent");
        assert!(fallback[0].resources[0]
            .replace('\\', "/")
            .ends_with("/.bitfun/tmp/write_toolcall123.tmp"));
    }
}

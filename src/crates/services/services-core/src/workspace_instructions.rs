use std::path::Path;
use tokio::fs;

pub const WORKSPACE_INSTRUCTION_FILE_NAMES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInstructionFile {
    pub name: String,
    pub content: String,
}

pub async fn read_workspace_instruction_files(
    workspace_root: &Path,
) -> Result<Vec<WorkspaceInstructionFile>, String> {
    let mut files = Vec::new();

    for file_name in WORKSPACE_INSTRUCTION_FILE_NAMES {
        let path = workspace_root.join(file_name);
        if !path.exists() || !path.is_file() {
            continue;
        }

        let content = fs::read_to_string(&path).await.map_err(|e| {
            format!(
                "Failed to read workspace instruction file {}: {}",
                path.display(),
                e
            )
        })?;

        if content.trim().is_empty() {
            continue;
        }

        files.push(WorkspaceInstructionFile {
            name: file_name.to_string(),
            content,
        });
    }

    Ok(files)
}

use std::path::Path;
use tokio::fs;

pub const WORKSPACE_INSTRUCTION_FILE_NAMES: [&str; 3] =
    ["AGENTS.override.md", "AGENTS.md", "CLAUDE.md"];

const WORKSPACE_INSTRUCTION_FILE_GROUPS: [&[&str]; 2] =
    [&["AGENTS.override.md", "AGENTS.md"], &["CLAUDE.md"]];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInstructionFile {
    pub name: String,
    pub content: String,
}

pub async fn read_workspace_instruction_files(
    workspace_root: &Path,
) -> Result<Vec<WorkspaceInstructionFile>, String> {
    let mut files = Vec::new();

    for candidates in WORKSPACE_INSTRUCTION_FILE_GROUPS {
        for file_name in candidates {
            let path = workspace_root.join(file_name);
            if !path.is_file() {
                continue;
            }

            let content = fs::read_to_string(&path).await.map_err(|e| {
                format!(
                    "Failed to read workspace instruction file {}: {}",
                    path.display(),
                    e
                )
            })?;

            if !content.trim().is_empty() {
                files.push(WorkspaceInstructionFile {
                    name: (*file_name).to_string(),
                    content,
                });
            }
            break;
        }
    }

    Ok(files)
}

#[cfg(feature = "workspace-runtime")]
pub async fn read_workspace_instruction_files_with_fs(
    fs: &dyn bitfun_runtime_ports::WorkspaceFileSystem,
    workspace_root: &str,
) -> Result<Vec<WorkspaceInstructionFile>, String> {
    let mut files = Vec::new();

    for candidates in WORKSPACE_INSTRUCTION_FILE_GROUPS {
        for file_name in candidates {
            let path = join_workspace_path(workspace_root, file_name);
            let is_file = fs.is_file(&path).await.map_err(|error| {
                format!("Failed to inspect workspace instruction file {path}: {error}")
            })?;
            if !is_file {
                continue;
            }

            let content = fs.read_file_text(&path).await.map_err(|error| {
                format!("Failed to read workspace instruction file {path}: {error}")
            })?;
            if !content.trim().is_empty() {
                files.push(WorkspaceInstructionFile {
                    name: (*file_name).to_string(),
                    content,
                });
            }
            break;
        }
    }

    Ok(files)
}

#[cfg(feature = "workspace-runtime")]
fn join_workspace_path(workspace_root: &str, file_name: &str) -> String {
    let root = workspace_root.trim_end_matches(['/', '\\']);
    let separator = if root.contains('\\') && !root.contains('/') {
        '\\'
    } else {
        '/'
    };
    format!("{root}{separator}{file_name}")
}

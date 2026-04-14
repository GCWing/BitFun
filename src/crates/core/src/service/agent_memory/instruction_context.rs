use crate::util::errors::*;
use std::path::Path;
use tokio::fs;

const WORKSPACE_INSTRUCTION_FILE_NAMES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

#[derive(Debug)]
struct WorkspaceInstructionFile {
    name: String,
    content: String,
}

async fn load_workspace_instruction_files(
    workspace_root: &Path,
) -> BitFunResult<Vec<WorkspaceInstructionFile>> {
    let mut files = Vec::new();

    for file_name in WORKSPACE_INSTRUCTION_FILE_NAMES {
        let path = workspace_root.join(file_name);
        if !path.exists() || !path.is_file() {
            continue;
        }

        let content = fs::read_to_string(&path).await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to read workspace instruction file {}: {}",
                path.display(),
                e
            ))
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

fn render_workspace_instruction_files_section(
    files: &[WorkspaceInstructionFile],
) -> Option<String> {
    if files.is_empty() {
        return None;
    }

    let mut rendered =
        String::from("As you answer the user's questions, you can use the following context:\n\n");

    for file in files {
        rendered.push_str(&format!(
            "<document name=\"{}\">\n{}\n</document>\n\n",
            file.name,
            file.content.trim()
        ));
    }

    Some(rendered.trim_end().to_string())
}

pub(crate) async fn build_workspace_instruction_files_context(
    workspace_root: &Path,
) -> BitFunResult<Option<String>> {
    let instruction_files = load_workspace_instruction_files(workspace_root).await?;
    Ok(render_workspace_instruction_files_section(
        &instruction_files,
    ))
}

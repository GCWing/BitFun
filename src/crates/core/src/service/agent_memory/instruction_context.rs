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
        String::from("## Codebase and user instructions\n\nBe sure to adhere to these instructions. IMPORTANT: These instructions OVERRIDE any default behavior and you MUST follow them exactly as written.\n");

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

#[cfg(test)]
mod tests {
    use super::build_workspace_instruction_files_context;
    use std::path::PathBuf;
    use tokio::fs;

    #[tokio::test]
    async fn workspace_instructions_load_agents_md() {
        let workspace = unique_temp_workspace("instructions-root");
        fs::create_dir_all(&workspace)
            .await
            .expect("create workspace");
        fs::write(
            workspace.join("AGENTS.md"),
            "# Root instructions\n\nFollow these rules.",
        )
        .await
        .expect("write AGENTS");

        let context = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("context should build")
            .expect("context should exist");

        assert!(context.contains("<document name=\"AGENTS.md\">"));
        assert!(context.contains("Follow these rules."));

        let _ = fs::remove_dir_all(&workspace).await;
    }

    #[tokio::test]
    async fn workspace_instructions_skips_missing_agents_md() {
        let workspace = unique_temp_workspace("instructions-empty");

        let context = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("context should build");

        assert!(context.is_none(), "empty workspace should produce no context");

        let _ = fs::remove_dir_all(&workspace).await;
    }

    fn unique_temp_workspace(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("bitfun-{}-{}", name, uuid::Uuid::new_v4()))
    }
}

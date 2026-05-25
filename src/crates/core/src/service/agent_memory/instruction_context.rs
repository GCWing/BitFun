use crate::util::errors::*;
use std::path::Path;
use tokio::fs;

const WORKSPACE_INSTRUCTION_FILE_NAMES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];
const AGENT_CONTEXT_DIRS: [&str; 0] = [];
const MAX_AGENT_CONTEXT_FILES_PER_DIR: usize = 20;
const MAX_AGENT_CONTEXT_FILE_BYTES: usize = 12_000;

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

    for context_dir in AGENT_CONTEXT_DIRS {
        files.extend(load_agent_context_files(workspace_root, context_dir).await?);
    }

    Ok(files)
}

async fn load_agent_context_files(
    workspace_root: &Path,
    context_dir: &str,
) -> BitFunResult<Vec<WorkspaceInstructionFile>> {
    let dir = workspace_root.join(context_dir);
    if !dir.exists() || !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(&dir).await.map_err(|e| {
        BitFunError::service(format!(
            "Failed to read workspace agent context directory {}: {}",
            dir.display(),
            e
        ))
    })?;
    let mut paths = Vec::new();

    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        BitFunError::service(format!(
            "Failed to read workspace agent context entry in {}: {}",
            dir.display(),
            e
        ))
    })? {
        let path = entry.path();
        if path.is_file()
            && path.extension().and_then(|ext| ext.to_str()) == Some("md")
            && !is_agent_context_readme(&path)
        {
            paths.push(path);
        }
    }

    paths.sort();

    let omitted_paths = if paths.len() > MAX_AGENT_CONTEXT_FILES_PER_DIR {
        paths[MAX_AGENT_CONTEXT_FILES_PER_DIR..].to_vec()
    } else {
        Vec::new()
    };
    paths.truncate(MAX_AGENT_CONTEXT_FILES_PER_DIR);

    let mut files = Vec::new();
    for path in paths {
        let raw_content = fs::read_to_string(&path).await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to read workspace agent context file {}: {}",
                path.display(),
                e
            ))
        })?;
        let content = truncate_agent_context_file(raw_content);

        if content.trim().is_empty() {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("context.md");
        files.push(WorkspaceInstructionFile {
            name: format!("{}/{}", context_dir, file_name),
            content,
        });
    }

    if !omitted_paths.is_empty() {
        files.push(WorkspaceInstructionFile {
            name: format!("{}/__context_budget__.md", context_dir),
            content: render_agent_context_omission_marker(context_dir, &omitted_paths),
        });
    }

    Ok(files)
}

fn is_agent_context_readme(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.eq_ignore_ascii_case("README.md"))
        .unwrap_or(false)
}

fn render_agent_context_omission_marker(
    context_dir: &str,
    omitted_paths: &[std::path::PathBuf],
) -> String {
    let omitted_files = omitted_paths
        .iter()
        .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "BitFun context budget loaded the first {} Markdown files from `{}` and omitted {} additional file(s). Use file tools to inspect omitted files if they may affect the task.\n\nOmitted files: {}",
        MAX_AGENT_CONTEXT_FILES_PER_DIR,
        context_dir,
        omitted_paths.len(),
        omitted_files
    )
}

fn truncate_agent_context_file(content: String) -> String {
    if content.len() <= MAX_AGENT_CONTEXT_FILE_BYTES {
        return content;
    }

    let truncated =
        crate::util::truncate_at_char_boundary(&content, MAX_AGENT_CONTEXT_FILE_BYTES);
    format!(
        "{}\n\n[Context file truncated to {} bytes by BitFun context budget.]",
        truncated.trim_end(),
        MAX_AGENT_CONTEXT_FILE_BYTES
    )
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

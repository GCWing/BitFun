use crate::util::errors::*;
use std::path::Path;
use tokio::fs;

const WORKSPACE_INSTRUCTION_FILE_NAMES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];
const AGENT_CONTEXT_DIRS: [&str; 3] = [".agent/rules", ".agent/knowledge", ".agent/changes"];
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
    async fn workspace_instruction_context_includes_agent_context_files() {
        let workspace = unique_temp_workspace("agent-context");
        let rules_dir = workspace.join(".agent").join("rules");
        let knowledge_dir = workspace.join(".agent").join("knowledge");
        let changes_dir = workspace.join(".agent").join("changes");
        fs::create_dir_all(&rules_dir)
            .await
            .expect("create rules dir");
        fs::create_dir_all(&knowledge_dir)
            .await
            .expect("create knowledge dir");
        fs::create_dir_all(&changes_dir)
            .await
            .expect("create changes dir");
        fs::write(
            workspace.join("AGENTS.md"),
            "# Root instructions\n\nUse repo rules.",
        )
        .await
        .expect("write AGENTS");
        fs::write(
            rules_dir.join("architecture.md"),
            "# Architecture\n\nKeep core portable.",
        )
        .await
        .expect("write architecture rule");
        fs::write(
            rules_dir.join("security.md"),
            "# Security\n\nDo not commit secrets.",
        )
        .await
        .expect("write security rule");
        fs::write(
            knowledge_dir.join("domain.md"),
            "# Domain\n\nWorkspace means project root.",
        )
        .await
        .expect("write domain knowledge");
        fs::write(
            changes_dir.join("current-task.md"),
            "# Change\n\nKeep this task documentation-first.",
        )
        .await
        .expect("write change note");

        let context = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("context should build")
            .expect("context should exist");

        assert!(context.contains("<document name=\"AGENTS.md\">"));
        assert!(context.contains("<document name=\".agent/rules/architecture.md\">"));
        assert!(context.contains("Keep core portable."));
        assert!(context.contains("<document name=\".agent/rules/security.md\">"));
        assert!(context.contains("Do not commit secrets."));
        assert!(context.contains("<document name=\".agent/knowledge/domain.md\">"));
        assert!(context.contains("Workspace means project root."));
        assert!(context.contains("<document name=\".agent/changes/current-task.md\">"));
        assert!(context.contains("Keep this task documentation-first."));

        let _ = fs::remove_dir_all(&workspace).await;
    }

    #[tokio::test]
    async fn workspace_instruction_context_limits_agent_context_file_count() {
        let workspace = unique_temp_workspace("agent-context-count");
        let knowledge_dir = workspace.join(".agent").join("knowledge");
        fs::create_dir_all(&knowledge_dir)
            .await
            .expect("create knowledge dir");

        for index in 0..25 {
            fs::write(
                knowledge_dir.join(format!("{:02}.md", index)),
                format!("# Note {}\n\ncontent {}", index, index),
            )
            .await
            .expect("write knowledge note");
        }

        let context = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("context should build")
            .expect("context should exist");

        assert!(context.contains("<document name=\".agent/knowledge/00.md\">"));
        assert!(context.contains("<document name=\".agent/knowledge/19.md\">"));
        assert!(!context.contains("<document name=\".agent/knowledge/20.md\">"));
        assert!(!context.contains("<document name=\".agent/knowledge/24.md\">"));
        assert!(context.contains("<document name=\".agent/knowledge/__context_budget__.md\">"));
        assert!(context.contains("omitted 5 additional file(s)"));
        assert!(context.contains("Omitted files: 20.md, 21.md, 22.md, 23.md, 24.md"));

        let _ = fs::remove_dir_all(&workspace).await;
    }

    #[tokio::test]
    async fn workspace_instruction_context_marks_omitted_agent_context_files() {
        let workspace = unique_temp_workspace("agent-context-marker");
        let changes_dir = workspace.join(".agent").join("changes");
        fs::create_dir_all(&changes_dir)
            .await
            .expect("create changes dir");

        for index in 0..22 {
            fs::write(
                changes_dir.join(format!("{:02}.md", index)),
                format!("# Change {}\n\ncontent {}", index, index),
            )
            .await
            .expect("write change note");
        }

        let context = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("context should build")
            .expect("context should exist");

        assert!(context.contains("<document name=\".agent/changes/19.md\">"));
        assert!(!context.contains("<document name=\".agent/changes/20.md\">"));
        assert!(context.contains("<document name=\".agent/changes/__context_budget__.md\">"));
        assert!(context.contains("loaded the first 20 Markdown files from `.agent/changes`"));
        assert!(context.contains("Omitted files: 20.md, 21.md"));

        let _ = fs::remove_dir_all(&workspace).await;
    }

    #[tokio::test]
    async fn workspace_instruction_context_skips_agent_context_readmes() {
        let workspace = unique_temp_workspace("agent-context-readme");
        let knowledge_dir = workspace.join(".agent").join("knowledge");
        fs::create_dir_all(&knowledge_dir)
            .await
            .expect("create knowledge dir");
        fs::write(
            knowledge_dir.join("README.md"),
            "# Knowledge README\n\nHuman guidance only.",
        )
        .await
        .expect("write README");

        for index in 0..20 {
            fs::write(
                knowledge_dir.join(format!("{:02}.md", index)),
                format!("# Note {}\n\ncontent {}", index, index),
            )
            .await
            .expect("write knowledge note");
        }

        let context = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("context should build")
            .expect("context should exist");

        assert!(!context.contains("<document name=\".agent/knowledge/README.md\">"));
        assert!(!context.contains("Human guidance only."));
        assert!(context.contains("<document name=\".agent/knowledge/00.md\">"));
        assert!(context.contains("<document name=\".agent/knowledge/19.md\">"));
        assert!(!context.contains("<document name=\".agent/knowledge/__context_budget__.md\">"));

        let _ = fs::remove_dir_all(&workspace).await;
    }

    #[tokio::test]
    async fn workspace_instruction_context_truncates_large_agent_context_files() {
        let workspace = unique_temp_workspace("agent-context-truncate");
        let changes_dir = workspace.join(".agent").join("changes");
        fs::create_dir_all(&changes_dir)
            .await
            .expect("create changes dir");

        let large_content = format!("{}{}", "a".repeat(11_999), "测");
        fs::write(changes_dir.join("large.md"), large_content)
            .await
            .expect("write large change note");

        let context = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("context should build")
            .expect("context should exist");

        assert!(context.contains("<document name=\".agent/changes/large.md\">"));
        assert!(context.contains("[Context file truncated to 12000 bytes by BitFun context budget.]"));
        assert!(context.is_char_boundary(context.len()));

        let _ = fs::remove_dir_all(&workspace).await;
    }

    fn unique_temp_workspace(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("bitfun-{}-{}", name, uuid::Uuid::new_v4()))
    }
}

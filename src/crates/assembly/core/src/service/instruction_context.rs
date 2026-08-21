use crate::util::errors::*;
use bitfun_runtime_ports::WorkspaceFileSystem;
use bitfun_services_core::local_instructions::LocalInstructionFile;
use bitfun_services_core::workspace_instructions::WorkspaceInstructionFile;
use std::path::Path;

const MAX_RENDERED_INSTRUCTION_FILES: usize = 256;
const MAX_RENDERED_INSTRUCTION_BYTES: usize = 2 * 1024 * 1024;

pub(crate) struct InstructionContextBuild {
    pub(crate) content: Option<String>,
    pub(crate) cacheable: bool,
}

async fn load_user_instruction_files(workspace_root: &Path) -> (Vec<LocalInstructionFile>, bool) {
    #[cfg(feature = "external-sources")]
    {
        // Runtime master switch (ai.external_instruction_sources): when off,
        // external user instruction files (~/.claude/CLAUDE.md + rules/,
        // OpenCode AGENTS.md, Codex AGENTS.md) are not read at all.
        if !crate::service::config::external_instruction_sources_enabled() {
            return (Vec::new(), true);
        }
        let files =
            crate::instruction_sources::load_local_user_instruction_files(workspace_root).await;
        (files.files, files.cacheable)
    }
    #[cfg(not(feature = "external-sources"))]
    {
        let _ = workspace_root;
        (Vec::new(), true)
    }
}

async fn load_user_conditional_instruction_files() -> Vec<LocalInstructionFile> {
    #[cfg(feature = "external-sources")]
    {
        // Same runtime gate as `load_user_instruction_files`: when the master
        // switch is off, conditional user rules are not read either.
        if !crate::service::config::external_instruction_sources_enabled() {
            return Vec::new();
        }
        crate::instruction_sources::load_local_user_conditional_instruction_sources().await
    }
    #[cfg(not(feature = "external-sources"))]
    {
        Vec::new()
    }
}

pub(crate) async fn build_workspace_instruction_files_context(
    workspace_root: &Path,
) -> BitFunResult<Option<String>> {
    Ok(
        build_workspace_instruction_files_context_detailed(workspace_root)
            .await?
            .content,
    )
}

/// Gate for the workspace instruction files master switch
/// (`ai.workspace_instruction_files`). When off, no workspace instruction file
/// content (project AGENTS.md / CLAUDE.md / opencode config references) is
/// rendered into the User Context.
fn workspace_instruction_files_enabled() -> bool {
    crate::service::config::workspace_instruction_files_enabled()
}

pub(crate) async fn build_workspace_instruction_files_context_detailed(
    workspace_root: &Path,
) -> BitFunResult<InstructionContextBuild> {
    if !workspace_instruction_files_enabled() {
        return Ok(InstructionContextBuild {
            content: None,
            cacheable: true,
        });
    }
    let (user_instruction_files, user_instruction_files_cacheable) =
        load_user_instruction_files(workspace_root).await;
    let workspace_instruction_files =
        bitfun_services_core::workspace_instructions::read_workspace_instruction_files(
            workspace_root,
        )
        .await
        .map_err(BitFunError::service)?;
    Ok(compose_local_instruction_sources(
        workspace_root,
        user_instruction_files,
        user_instruction_files_cacheable,
        workspace_instruction_files,
    ))
}

pub(crate) async fn build_local_workspace_instruction_files_context_with_fs_detailed(
    workspace_root: &Path,
    fs: &dyn WorkspaceFileSystem,
    workspace_root_path: &str,
) -> BitFunResult<InstructionContextBuild> {
    if !workspace_instruction_files_enabled() {
        return Ok(InstructionContextBuild {
            content: None,
            cacheable: true,
        });
    }
    let (user_instruction_files, user_instruction_files_cacheable) =
        load_user_instruction_files(workspace_root).await;
    let workspace_instruction_files =
        bitfun_services_core::workspace_instructions::read_workspace_instruction_files_with_fs(
            fs,
            workspace_root_path,
        )
        .await
        .map_err(BitFunError::service)?;
    Ok(compose_local_instruction_sources(
        workspace_root,
        user_instruction_files,
        user_instruction_files_cacheable,
        workspace_instruction_files,
    ))
}

fn compose_local_instruction_sources(
    workspace_root: &Path,
    user_instruction_files: Vec<LocalInstructionFile>,
    user_instruction_files_cacheable: bool,
    workspace_instruction_files: Vec<WorkspaceInstructionFile>,
) -> InstructionContextBuild {
    let instruction_files = merge_local_instruction_sources(
        workspace_root,
        user_instruction_files,
        workspace_instruction_files,
    );
    InstructionContextBuild {
        content: render_workspace_instruction_files_section(&instruction_files),
        cacheable: user_instruction_files_cacheable,
    }
}

fn merge_local_instruction_sources(
    workspace_root: &Path,
    user_instruction_files: Vec<LocalInstructionFile>,
    mut workspace_instruction_files: Vec<WorkspaceInstructionFile>,
) -> Vec<WorkspaceInstructionFile> {
    bitfun_services_core::workspace_instructions::retain_distinct_local_workspace_instruction_files(
        workspace_root,
        user_instruction_files
            .iter()
            .map(|file| file.canonical_path.clone()),
        &mut workspace_instruction_files,
    );
    let mut instruction_files = user_instruction_files
        .into_iter()
        .map(|file| WorkspaceInstructionFile {
            name: file.name,
            content: file.content,
            path_patterns: file.path_patterns,
        })
        .collect::<Vec<_>>();
    instruction_files.extend(workspace_instruction_files);
    instruction_files
}

pub(crate) async fn load_local_conditional_instruction_files(
    workspace_root: &Path,
) -> BitFunResult<Vec<WorkspaceInstructionFile>> {
    let user_instruction_files = load_user_conditional_instruction_files().await;
    let workspace_instruction_files =
        bitfun_services_core::workspace_instructions::read_workspace_conditional_instruction_sources(
            workspace_root,
        )
        .await
        .map_err(BitFunError::service)?;
    Ok(merge_local_instruction_sources(
        workspace_root,
        user_instruction_files,
        workspace_instruction_files,
    ))
}

pub(crate) async fn load_local_conditional_instruction_files_with_fs(
    workspace_root: &Path,
    fs: &dyn WorkspaceFileSystem,
    workspace_root_path: &str,
) -> BitFunResult<Vec<WorkspaceInstructionFile>> {
    let user_instruction_files = load_user_conditional_instruction_files().await;
    let workspace_instruction_files =
        bitfun_services_core::workspace_instructions::read_workspace_conditional_instruction_sources_with_fs(
            fs,
            workspace_root_path,
        )
        .await
        .map_err(BitFunError::service)?;
    Ok(merge_local_instruction_sources(
        workspace_root,
        user_instruction_files,
        workspace_instruction_files,
    ))
}

pub(crate) async fn load_workspace_conditional_instruction_files_with_fs(
    fs: &dyn WorkspaceFileSystem,
    workspace_root: &str,
) -> BitFunResult<Vec<WorkspaceInstructionFile>> {
    bitfun_services_core::workspace_instructions::read_workspace_conditional_instruction_sources_with_fs(
        fs,
        workspace_root,
    )
    .await
    .map_err(BitFunError::service)
}

pub(crate) async fn build_workspace_instruction_files_context_with_fs(
    fs: &dyn WorkspaceFileSystem,
    workspace_root: &str,
) -> BitFunResult<Option<String>> {
    if !workspace_instruction_files_enabled() {
        return Ok(None);
    }
    let instruction_files =
        bitfun_services_core::workspace_instructions::read_workspace_instruction_files_with_fs(
            fs,
            workspace_root,
        )
        .await
        .map_err(BitFunError::service)?;
    Ok(render_workspace_instruction_files_section(
        &instruction_files,
    ))
}

fn render_workspace_instruction_files_section(
    files: &[WorkspaceInstructionFile],
) -> Option<String> {
    render_instruction_documents(
        "## Codebase and user instructions\n\nBe sure to adhere to these instructions. IMPORTANT: These instructions OVERRIDE any default behavior and you MUST follow them exactly as written.\n",
        files.iter().filter(|file| file.path_patterns.is_empty()),
    )
    .0
}

pub(crate) fn render_instruction_documents<'a>(
    header: &str,
    files: impl IntoIterator<Item = &'a WorkspaceInstructionFile>,
) -> (Option<String>, Vec<String>) {
    let mut rendered = String::from(header);
    let mut rendered_names = Vec::new();

    for file in files.into_iter().take(MAX_RENDERED_INSTRUCTION_FILES) {
        let escaped_name = escape_document_name(&file.name);
        let document = format!(
            "<document name=\"{}\">\n{}\n</document>\n\n",
            escaped_name,
            file.content.trim()
        );
        if rendered.len().saturating_add(document.len()) > MAX_RENDERED_INSTRUCTION_BYTES {
            break;
        }
        rendered.push_str(&document);
        rendered_names.push(file.name.clone());
    }

    (
        (!rendered_names.is_empty()).then(|| rendered.trim_end().to_string()),
        rendered_names,
    )
}

fn escape_document_name(name: &str) -> String {
    name.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "external-sources")]
    use super::{
        build_workspace_instruction_files_context,
        build_workspace_instruction_files_context_detailed,
        build_workspace_instruction_files_context_with_fs,
        load_local_conditional_instruction_files,
    };
    use super::{render_workspace_instruction_files_section, WorkspaceInstructionFile};
    #[cfg(feature = "external-sources")]
    use crate::instruction_sources::test_support::{
        lock_environment, EnvironmentGuard, InstructionSwitches,
    };
    #[cfg(feature = "external-sources")]
    use bitfun_services_core::workspace::LocalWorkspaceFs;

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn local_user_instructions_precede_workspace_instructions_by_ecosystem_priority() {
        let _environment = lock_environment();
        // Enable both instruction master switches for this test; the
        // InstructionSwitches guard restores the previous values on drop.
        let _switches = InstructionSwitches::enable_all();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config directory");
        std::fs::create_dir_all(&codex).expect("Codex config directory");
        std::fs::create_dir_all(&claude).expect("Claude config directory");
        std::fs::create_dir_all(&workspace).expect("workspace directory");
        std::fs::write(xdg.join("opencode/AGENTS.md"), "OpenCode user\n")
            .expect("OpenCode instructions");
        std::fs::write(codex.join("AGENTS.md"), "Codex user\n").expect("Codex instructions");
        std::fs::write(claude.join("CLAUDE.md"), "Claude user\n").expect("Claude instructions");
        std::fs::write(workspace.join("AGENTS.md"), "Workspace project\n")
            .expect("workspace instructions");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);

        let rendered = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("instruction context")
            .expect("rendered instructions");

        let positions = [
            rendered
                .find("OpenCode user")
                .expect("OpenCode instructions"),
            rendered.find("Codex user").expect("Codex instructions"),
            rendered.find("Claude user").expect("Claude instructions"),
            rendered
                .find("Workspace project")
                .expect("workspace instructions"),
        ];
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn conditional_instructions_keep_user_then_workspace_precedence() {
        let _environment = lock_environment();
        // Enable both instruction master switches for this test; the
        // InstructionSwitches guard restores the previous values on drop.
        let _switches = InstructionSwitches::enable_all();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config directory");
        std::fs::create_dir_all(&codex).expect("Codex config directory");
        std::fs::create_dir_all(claude.join("rules")).expect("Claude rules directory");
        std::fs::create_dir_all(workspace.join(".claude/rules"))
            .expect("workspace rules directory");
        std::fs::write(
            claude.join("rules/user.md"),
            "---\npaths:\n  - src/**/*.rs\n---\nUser rule\n",
        )
        .expect("user rule");
        std::fs::write(
            workspace.join(".claude/rules/project.md"),
            "---\npaths:\n  - src/**/*.rs\n---\nProject rule\n",
        )
        .expect("project rule");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);

        let files = load_local_conditional_instruction_files(&workspace)
            .await
            .expect("conditional instructions");

        assert_eq!(
            files
                .iter()
                .map(|file| file.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "$CLAUDE_CONFIG_DIR/rules/user.md",
                ".claude/rules/project.md"
            ]
        );
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn invalid_user_rule_does_not_hide_project_conditional_instructions() {
        let _environment = lock_environment();
        // Enable both instruction master switches for this test; the
        // InstructionSwitches guard restores the previous values on drop.
        let _switches = InstructionSwitches::enable_all();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config directory");
        std::fs::create_dir_all(&codex).expect("Codex config directory");
        std::fs::create_dir_all(claude.join("rules")).expect("Claude rules directory");
        std::fs::create_dir_all(workspace.join(".claude/rules"))
            .expect("workspace rules directory");
        std::fs::write(
            claude.join("rules/oversized.md"),
            vec![
                b'x';
                bitfun_services_core::local_instructions::MAX_LOCAL_INSTRUCTION_FILE_BYTES + 1
            ],
        )
        .expect("oversized user rule");
        std::fs::write(
            workspace.join(".claude/rules/project.md"),
            "---\npaths:\n  - src/**/*.rs\n---\nProject rule\n",
        )
        .expect("project rule");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);

        let files = load_local_conditional_instruction_files(&workspace)
            .await
            .expect("project conditional instructions");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, ".claude/rules/project.md");
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn opencode_global_config_resolves_relative_instructions_in_the_local_workspace() {
        let _environment = lock_environment();
        // Enable both instruction master switches for this test; the
        // InstructionSwitches guard restores the previous values on drop.
        let _switches = InstructionSwitches::enable_all();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config directory");
        std::fs::create_dir_all(&codex).expect("Codex config directory");
        std::fs::create_dir_all(&claude).expect("Claude config directory");
        std::fs::create_dir_all(workspace.join("shared")).expect("workspace shared directory");
        std::fs::write(xdg.join("opencode/AGENTS.md"), "OpenCode user\n")
            .expect("OpenCode instructions");
        std::fs::write(
            xdg.join("opencode/opencode.json"),
            r#"{"instructions":["shared/*.md"]}"#,
        )
        .expect("OpenCode config");
        std::fs::write(
            workspace.join("shared/team.md"),
            "Shared workspace policy\n",
        )
        .expect("configured workspace instructions");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);

        let rendered = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("instruction context")
            .expect("rendered instructions");

        assert!(rendered.contains("Shared workspace policy"));
        assert!(rendered.contains("<document name=\"&lt;workspace&gt;/shared/team.md\">"));
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn invalid_user_source_does_not_hide_workspace_instructions() {
        let _environment = lock_environment();
        // Enable both instruction master switches for this test; the
        // InstructionSwitches guard restores the previous values on drop.
        let _switches = InstructionSwitches::enable_all();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config directory");
        std::fs::create_dir_all(&codex).expect("Codex config directory");
        std::fs::create_dir_all(&claude).expect("Claude config directory");
        std::fs::create_dir_all(&workspace).expect("workspace directory");
        std::fs::write(xdg.join("opencode/AGENTS.md"), "OpenCode user\n")
            .expect("OpenCode instructions");
        std::fs::write(xdg.join("opencode/opencode.json"), "{ invalid json")
            .expect("invalid OpenCode config");
        std::fs::write(workspace.join("AGENTS.md"), "Workspace survives\n")
            .expect("workspace instructions");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);

        let build = build_workspace_instruction_files_context_detailed(&workspace)
            .await
            .expect("workspace instructions survive user source failure");
        let rendered = build.content.expect("rendered instructions");

        assert!(rendered.contains("Workspace survives"));
        assert!(!build.cacheable);
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn a_user_configured_workspace_file_is_not_rendered_again_as_a_project_source() {
        let _environment = lock_environment();
        // Enable both instruction master switches for this test; the
        // InstructionSwitches guard restores the previous values on drop.
        let _switches = InstructionSwitches::enable_all();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config directory");
        std::fs::create_dir_all(&codex).expect("Codex config directory");
        std::fs::create_dir_all(&claude).expect("Claude config directory");
        std::fs::create_dir_all(&workspace).expect("workspace directory");
        std::fs::write(
            xdg.join("opencode/opencode.json"),
            r#"{"instructions":["AGENTS.md"]}"#,
        )
        .expect("OpenCode config");
        std::fs::write(workspace.join("AGENTS.md"), "One physical source\n")
            .expect("workspace instructions");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);

        let rendered = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("instruction context")
            .expect("rendered instructions");

        assert_eq!(rendered.matches("One physical source").count(), 1);
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn port_backed_workspace_never_falls_back_to_local_user_sources() {
        let _environment = lock_environment();
        // Enable both instruction master switches for this test; the
        // InstructionSwitches guard restores the previous values on drop.
        let _switches = InstructionSwitches::enable_all();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config directory");
        std::fs::create_dir_all(&codex).expect("Codex config directory");
        std::fs::create_dir_all(&claude).expect("Claude config directory");
        std::fs::create_dir_all(&workspace).expect("workspace directory");
        std::fs::write(
            xdg.join("opencode/AGENTS.md"),
            "Local user must stay absent\n",
        )
        .expect("OpenCode instructions");
        std::fs::write(workspace.join("AGENTS.md"), "Port project instructions\n")
            .expect("workspace instructions");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);
        let root = workspace.to_string_lossy();

        let rendered = build_workspace_instruction_files_context_with_fs(&LocalWorkspaceFs, &root)
            .await
            .expect("port-backed instruction context")
            .expect("rendered instructions");

        assert!(rendered.contains("Port project instructions"));
        assert!(!rendered.contains("Local user must stay absent"));
    }

    #[test]
    fn rendered_document_names_escape_markup_characters() {
        let rendered = render_workspace_instruction_files_section(&[WorkspaceInstructionFile {
            name: "<configured-path>/team\"&.md".to_string(),
            content: "Team instructions".to_string(),
            path_patterns: Vec::new(),
        }])
        .expect("rendered instructions");

        assert!(rendered.contains("<document name=\"&lt;configured-path&gt;/team&quot;&amp;.md\">"));
        assert!(!rendered.contains("name=\"<configured-path>"));
    }

    #[test]
    fn rendered_instruction_context_keeps_one_shared_content_budget() {
        let files = (0..3)
            .map(|index| WorkspaceInstructionFile {
                name: format!("source-{index}.md"),
                content: format!("source-{index}\n{}", "x".repeat(700 * 1024)),
                path_patterns: Vec::new(),
            })
            .collect::<Vec<_>>();

        let rendered =
            render_workspace_instruction_files_section(&files).expect("rendered instructions");

        assert!(rendered.len() <= 2 * 1024 * 1024);
        assert!(rendered.contains("source-0\n"));
        assert!(rendered.contains("source-1\n"));
        assert!(!rendered.contains("source-2\n"));
    }

    #[test]
    fn rendered_instruction_context_keeps_one_shared_file_budget() {
        let files = (0..257)
            .map(|index| WorkspaceInstructionFile {
                name: format!("source-{index}.md"),
                content: format!("instruction {index}"),
                path_patterns: Vec::new(),
            })
            .collect::<Vec<_>>();

        let rendered =
            render_workspace_instruction_files_section(&files).expect("rendered instructions");

        assert_eq!(rendered.matches("<document name=").count(), 256);
        assert!(!rendered.contains("instruction 256"));
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn disabled_external_instruction_sources_skip_all_external_user_files() {
        let _environment = lock_environment();
        // Workspace instructions on, external user sources off — exactly what
        // this test asserts. The guard restores the previous values on drop.
        let _switches = InstructionSwitches::set(Some(true), Some(false));
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config directory");
        std::fs::create_dir_all(&codex).expect("Codex config directory");
        std::fs::create_dir_all(claude.join("rules")).expect("Claude rules directory");
        std::fs::create_dir_all(&workspace).expect("workspace directory");
        std::fs::write(xdg.join("opencode/AGENTS.md"), "OpenCode user\n")
            .expect("OpenCode instructions");
        std::fs::write(codex.join("AGENTS.md"), "Codex user\n").expect("Codex instructions");
        std::fs::write(claude.join("CLAUDE.md"), "Claude user\n").expect("Claude instructions");
        std::fs::write(
            claude.join("rules/rust.md"),
            "---\npaths:\n  - src/**/*.rs\n---\nUser rule\n",
        )
        .expect("user rule");
        std::fs::write(workspace.join("AGENTS.md"), "Workspace project\n")
            .expect("workspace instructions");
        std::fs::create_dir_all(workspace.join(".claude/rules")).expect("workspace rules");
        std::fs::write(
            workspace.join(".claude/rules/project.md"),
            "---\npaths:\n  - src/**/*.rs\n---\nProject rule\n",
        )
        .expect("project rule");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);

        let build = build_workspace_instruction_files_context_detailed(&workspace)
            .await
            .expect("workspace instructions survive external switch off");
        let rendered = build.content.expect("rendered instructions");
        assert!(!rendered.contains("OpenCode user"));
        assert!(!rendered.contains("Codex user"));
        assert!(!rendered.contains("Claude user"));
        assert!(!rendered.contains("User rule"));
        assert!(rendered.contains("Workspace project"));

        let files = load_local_conditional_instruction_files(&workspace)
            .await
            .expect("workspace conditional instructions survive external switch off");
        assert_eq!(
            files
                .iter()
                .map(|file| file.name.as_str())
                .collect::<Vec<_>>(),
            vec![".claude/rules/project.md"]
        );
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn enabled_external_instruction_sources_still_load_user_files_by_default() {
        let _environment = lock_environment();
        // Enable both instruction master switches for this test; the
        // InstructionSwitches guard restores the previous values on drop.
        let _switches = InstructionSwitches::enable_all();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config directory");
        std::fs::create_dir_all(&codex).expect("Codex config directory");
        std::fs::create_dir_all(&claude).expect("Claude config directory");
        std::fs::create_dir_all(&workspace).expect("workspace directory");
        std::fs::write(xdg.join("opencode/AGENTS.md"), "OpenCode user\n")
            .expect("OpenCode instructions");
        std::fs::write(codex.join("AGENTS.md"), "Codex user\n").expect("Codex instructions");
        std::fs::write(claude.join("CLAUDE.md"), "Claude user\n").expect("Claude instructions");
        std::fs::write(workspace.join("AGENTS.md"), "Workspace project\n")
            .expect("workspace instructions");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);

        let rendered = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("instruction context")
            .expect("rendered instructions");

        assert!(rendered.contains("OpenCode user"));
        assert!(rendered.contains("Codex user"));
        assert!(rendered.contains("Claude user"));
        assert!(rendered.contains("Workspace project"));
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn stable_switch_state_keeps_rendered_context_byte_identical_across_builds() {
        // Cache-prefix protection (DeepSeek prefix cache): with the same
        // workspace, same switch state, and unchanged files, two builds must
        // render byte-identical content in a stable order — a drift here would
        // invalidate the provider-side prompt prefix cache.
        let _environment = lock_environment();
        // Enable both instruction master switches for this test; the
        // InstructionSwitches guard restores the previous values on drop.
        let _switches = InstructionSwitches::enable_all();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config directory");
        std::fs::create_dir_all(&codex).expect("Codex config directory");
        std::fs::create_dir_all(&claude).expect("Claude config directory");
        std::fs::create_dir_all(&workspace).expect("workspace directory");
        std::fs::write(xdg.join("opencode/AGENTS.md"), "OpenCode user\n")
            .expect("OpenCode instructions");
        std::fs::write(codex.join("AGENTS.md"), "Codex user\n").expect("Codex instructions");
        std::fs::write(claude.join("CLAUDE.md"), "Claude user\n").expect("Claude instructions");
        std::fs::write(workspace.join("AGENTS.md"), "Workspace project\n")
            .expect("workspace instructions");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);

        // Same stable switch state for both builds (guard already enables both).
        let first = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("first instruction context")
            .expect("first rendered instructions");
        let second = build_workspace_instruction_files_context(&workspace)
            .await
            .expect("second instruction context")
            .expect("second rendered instructions");

        assert_eq!(
            first, second,
            "byte-identical prefix across repeated builds"
        );

        // Source order must stay stable: opencode → codex → claude → workspace.
        let positions = [
            first.find("OpenCode user").expect("OpenCode position"),
            first.find("Codex user").expect("Codex position"),
            first.find("Claude user").expect("Claude position"),
            first.find("Workspace project").expect("workspace position"),
        ];
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    }
}

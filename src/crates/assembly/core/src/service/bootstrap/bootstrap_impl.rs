use crate::util::errors::*;
use log::{debug, warn};
use std::path::Path;
use tokio::fs;

const BOOTSTRAP_FILE_NAME: &str = "BOOTSTRAP.md";
const SOUL_FILE_NAME: &str = "SOUL.md";
const USER_FILE_NAME: &str = "USER.md";
const IDENTITY_FILE_NAME: &str = "IDENTITY.md";
const BOOTSTRAP_TEMPLATE: &str = include_str!("templates/BOOTSTRAP.md");
const SOUL_TEMPLATE: &str = include_str!("templates/SOUL.md");
const USER_TEMPLATE: &str = include_str!("templates/USER.md");
const IDENTITY_TEMPLATE: &str = include_str!("templates/IDENTITY.md");
#[cfg(feature = "agent-runtime")]
const PERSONA_FILE_NAMES: [&str; 4] = [
    BOOTSTRAP_FILE_NAME,
    SOUL_FILE_NAME,
    USER_FILE_NAME,
    IDENTITY_FILE_NAME,
];

fn normalize_line_endings(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

async fn ensure_markdown_placeholder(path: &Path, content: &str) -> BitFunResult<bool> {
    if path.exists() {
        return Ok(false);
    }

    let normalized_content = normalize_line_endings(content);
    fs::write(path, normalized_content)
        .await
        .map_err(|e| BitFunError::service(format!("Failed to create {}: {}", path.display(), e)))?;

    Ok(true)
}

fn gitignore_already_ignores_bitfun(content: &str) -> bool {
    content.lines().any(|line| {
        let entry = line.trim();
        !entry.starts_with('#')
            && matches!(entry, ".bitfun" | ".bitfun/" | "/.bitfun" | "/.bitfun/")
    })
}

pub(crate) async fn ensure_workspace_gitignore_ignores_bitfun(
    workspace_root: &Path,
) -> BitFunResult<bool> {
    let gitignore_path = workspace_root.join(".gitignore");
    let bitfun_entry = ".bitfun/";

    let content = match fs::read_to_string(&gitignore_path).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            debug!(
                "Skipped workspace .gitignore update because file is missing: path={}",
                gitignore_path.display()
            );
            return Ok(false);
        }
        Err(error) => {
            return Err(BitFunError::service(format!(
                "Failed to read {}: {}",
                gitignore_path.display(),
                error
            )));
        }
    };

    if gitignore_already_ignores_bitfun(&content) {
        return Ok(false);
    }

    let line_ending = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut updated = content;
    if !updated.is_empty() && !updated.ends_with('\n') && !updated.ends_with('\r') {
        updated.push_str(line_ending);
    }
    updated.push_str(bitfun_entry);
    updated.push_str(line_ending);

    fs::write(&gitignore_path, updated).await.map_err(|e| {
        BitFunError::service(format!(
            "Failed to update {} for .bitfun: {}",
            gitignore_path.display(),
            e
        ))
    })?;

    debug!(
        "Added workspace .gitignore entry for .bitfun: path={}",
        gitignore_path.display()
    );

    Ok(true)
}

async fn ensure_workspace_gitignore_ignores_bitfun_best_effort(workspace_root: &Path) -> bool {
    match ensure_workspace_gitignore_ignores_bitfun(workspace_root).await {
        Ok(updated) => updated,
        Err(e) => {
            warn!(
                "Failed to ensure workspace .gitignore ignores .bitfun: workspace={}, error={}",
                workspace_root.display(),
                e
            );
            false
        }
    }
}

pub(crate) async fn initialize_workspace_persona_files(workspace_root: &Path) -> BitFunResult<()> {
    let gitignore_updated =
        ensure_workspace_gitignore_ignores_bitfun_best_effort(workspace_root).await;
    let bootstrap_path = workspace_root.join(BOOTSTRAP_FILE_NAME);
    let soul_path = workspace_root.join(SOUL_FILE_NAME);
    let user_path = workspace_root.join(USER_FILE_NAME);
    let identity_path = workspace_root.join(IDENTITY_FILE_NAME);

    let created_bootstrap =
        ensure_markdown_placeholder(&bootstrap_path, BOOTSTRAP_TEMPLATE).await?;
    let created_soul = ensure_markdown_placeholder(&soul_path, SOUL_TEMPLATE).await?;
    let created_user = ensure_markdown_placeholder(&user_path, USER_TEMPLATE).await?;
    let created_identity = ensure_markdown_placeholder(&identity_path, IDENTITY_TEMPLATE).await?;

    debug!(
        "Initialized workspace persona files: path={}, gitignore_updated={}, created_bootstrap={}, created_soul={}, created_user={}, created_identity={}",
        workspace_root.display(),
        gitignore_updated,
        created_bootstrap,
        created_soul,
        created_user,
        created_identity
    );

    Ok(())
}

/// R-WF-07：为成员 Claw 物化三身份文件（SOUL/USER/IDENTITY，无 BOOTSTRAP）。
///
/// 成员身份由工作流节点直接实例化（node.role → IDENTITY、node.prompt/gate →
/// SOUL、直属上级 → USER），不走引导对话：
/// - 不创建 `BOOTSTRAP.md`（引导临时文件，bootstrap 完成即删——成员身份
///   在建群时已物化，无需引导阶段）；
/// - 已存在 `BOOTSTRAP.md`（例如残留/中途失败的引导）→ 直接删除；
/// - 已有同名身份文件绝不覆盖（`ensure_markdown_placeholder` 语义），
///   保证重复建群/幂等物化不丢已确立身份。
pub(crate) async fn initialize_member_persona_files(
    workspace_root: &Path,
    role: &str,
    prompt: &str,
    gate: bool,
    superior: &str,
) -> BitFunResult<()> {
    ensure_workspace_gitignore_ignores_bitfun_best_effort(workspace_root).await;

    let role = role.trim();
    let prompt = prompt.trim();
    let superior = superior.trim();
    let gate_text = if gate { "true" } else { "false" };

    let identity_content = if role.is_empty() {
        IDENTITY_TEMPLATE.to_string()
    } else {
        format!(
            "---\nname: {role}\ncreature: legion member\nvibe: focused\nemoji: \n---\n\n# IDENTITY.md - Who Am I?\n\n## Role\n\nI am the `{role}` member of this team.\n"
        )
    };
    let soul_content = if prompt.is_empty() {
        SOUL_TEMPLATE.to_string()
    } else {
        format!(
            "# SOUL.md - Who You Are\n\n## Mission\n\n{prompt}\n\n## Gate\n\nThis member is gated (gate={gate_text}) and must respect the group workflow gate before acting.\n\n## Core Truths\n\n- Execute your assigned role precisely.\n- Follow your direct superior's direction.\n- Report results honestly and completely.\n"
        )
    };
    let user_content = format!(
        "# USER.md - About Your Human\n\nYour direct superior in the group is `{superior}`.\n\n## Context\n\nYou are a member of a group workflow. Follow the direct superior above and collaborate with your peers.\n"
    );

    let identity_created =
        ensure_markdown_placeholder(&workspace_root.join(IDENTITY_FILE_NAME), &identity_content)
            .await?;
    let soul_created =
        ensure_markdown_placeholder(&workspace_root.join(SOUL_FILE_NAME), &soul_content).await?;
    let user_created =
        ensure_markdown_placeholder(&workspace_root.join(USER_FILE_NAME), &user_content).await?;

    // BOOTSTRAP.md = 引导临时文件：成员身份已直接物化，bootstrap 完成即删。
    // 存在残留（中途失败的引导/旧引导）→ 删除；正常物化本就不创建。
    let bootstrap_path = workspace_root.join(BOOTSTRAP_FILE_NAME);
    let bootstrap_removed = if bootstrap_path.exists() {
        match fs::remove_file(&bootstrap_path).await {
            Ok(()) => true,
            Err(e) => {
                return Err(BitFunError::service(format!(
                    "Failed to remove stale BOOTSTRAP.md at {}: {}",
                    bootstrap_path.display(),
                    e
                )));
            }
        }
    } else {
        false
    };

    debug!(
        "Initialized member persona files: path={}, role={}, gate={}, superior={}, identity_created={}, soul_created={}, user_created={}, bootstrap_removed={}",
        workspace_root.display(),
        role,
        gate_text,
        superior,
        identity_created,
        soul_created,
        user_created,
        bootstrap_removed
    );

    Ok(())
}

#[cfg(feature = "agent-runtime")]
pub(crate) fn is_workspace_bootstrap_pending(workspace_root: &Path) -> bool {
    workspace_root.join(BOOTSTRAP_FILE_NAME).exists()
}

#[cfg(feature = "agent-runtime")]
pub(crate) async fn ensure_workspace_persona_files_for_prompt(
    workspace_root: &Path,
) -> BitFunResult<()> {
    let gitignore_updated =
        ensure_workspace_gitignore_ignores_bitfun_best_effort(workspace_root).await;
    let bootstrap_path = workspace_root.join(BOOTSTRAP_FILE_NAME);
    let soul_path = workspace_root.join(SOUL_FILE_NAME);
    let user_path = workspace_root.join(USER_FILE_NAME);
    let identity_path = workspace_root.join(IDENTITY_FILE_NAME);

    let bootstrap_exists = bootstrap_path.exists();
    let user_exists = user_path.exists();
    let identity_exists = identity_path.exists();

    let (created_bootstrap, created_soul, created_user, created_identity) = if !bootstrap_exists {
        // Rule 1: when USER + IDENTITY already exist, do not create BOOTSTRAP.
        // Only ensure SOUL exists.
        if user_exists && identity_exists {
            (
                false,
                ensure_markdown_placeholder(&soul_path, SOUL_TEMPLATE).await?,
                false,
                false,
            )
        } else {
            // Rule 2: when USER or IDENTITY is missing, backfill all missing files.
            (
                ensure_markdown_placeholder(&bootstrap_path, BOOTSTRAP_TEMPLATE).await?,
                ensure_markdown_placeholder(&soul_path, SOUL_TEMPLATE).await?,
                ensure_markdown_placeholder(&user_path, USER_TEMPLATE).await?,
                ensure_markdown_placeholder(&identity_path, IDENTITY_TEMPLATE).await?,
            )
        }
    } else {
        // BOOTSTRAP already exists: keep persona set complete.
        (
            false,
            ensure_markdown_placeholder(&soul_path, SOUL_TEMPLATE).await?,
            ensure_markdown_placeholder(&user_path, USER_TEMPLATE).await?,
            ensure_markdown_placeholder(&identity_path, IDENTITY_TEMPLATE).await?,
        )
    };

    debug!(
        "Ensured workspace persona files for prompt: path={}, gitignore_updated={}, bootstrap_exists={}, user_exists={}, identity_exists={}, created_bootstrap={}, created_soul={}, created_user={}, created_identity={}",
        workspace_root.display(),
        gitignore_updated,
        bootstrap_exists,
        user_exists,
        identity_exists,
        created_bootstrap,
        created_soul,
        created_user,
        created_identity
    );

    Ok(())
}

pub async fn reset_workspace_persona_files_to_default(workspace_root: &Path) -> BitFunResult<()> {
    let persona_templates = [
        (BOOTSTRAP_FILE_NAME, BOOTSTRAP_TEMPLATE),
        (SOUL_FILE_NAME, SOUL_TEMPLATE),
        (USER_FILE_NAME, USER_TEMPLATE),
        (IDENTITY_FILE_NAME, IDENTITY_TEMPLATE),
    ];

    for (file_name, template) in persona_templates {
        let file_path = workspace_root.join(file_name);
        let normalized_content = normalize_line_endings(template);
        fs::write(&file_path, normalized_content)
            .await
            .map_err(|e| {
                BitFunError::service(format!(
                    "Failed to reset persona file '{}': {}",
                    file_path.display(),
                    e
                ))
            })?;
    }

    debug!(
        "Reset workspace persona files to defaults: path={}",
        workspace_root.display()
    );

    Ok(())
}

#[cfg(feature = "agent-runtime")]
pub(crate) async fn build_workspace_persona_prompt(
    workspace_root: &Path,
) -> BitFunResult<Option<String>> {
    ensure_workspace_persona_files_for_prompt(workspace_root).await?;

    let mut documents = Vec::new();
    for file_name in PERSONA_FILE_NAMES {
        let file_path = workspace_root.join(file_name);
        if !file_path.exists() {
            continue;
        }

        match fs::read_to_string(&file_path).await {
            Ok(content) => documents.push((file_name, normalize_line_endings(&content))),
            Err(e) => {
                warn!(
                    "Failed to read persona file: path={} error={}",
                    file_path.display(),
                    e
                );
            }
        }
    }

    if documents.is_empty() {
        return Ok(None);
    }

    let bootstrap_detected = documents
        .iter()
        .any(|(file_name, _)| *file_name == BOOTSTRAP_FILE_NAME);

    let mut prompt = String::from("<persona>\n");
    for (file_name, content) in documents {
        prompt.push_str(&format!(
            "<persona_file name=\"{}\" description=\"{}\">\n{}\n</persona_file>\n",
            file_name,
            persona_file_description(file_name),
            content
        ));
    }
    prompt.push_str("</persona>");

    let bootstrap_notice = if bootstrap_detected {
        r#"

## Bootstrap Required

`BOOTSTRAP.md` has been detected. Treat this as an unfinished bootstrap state.

Before continuing with normal work, you MUST:
1. Complete or verify the bootstrap instructions in `BOOTSTRAP.md`.
2. Update `IDENTITY.md`, `USER.md`, and `SOUL.md` with any confirmed information.
3. Delete `BOOTSTRAP.md` in the same session as soon as bootstrap is complete.

Additional rules:
- If `IDENTITY.md`, `USER.md`, and `SOUL.md` already contain enough information, treat `BOOTSTRAP.md` as stale bootstrap residue and delete it immediately.
- Bootstrap is only considered complete when `BOOTSTRAP.md` no longer exists.
- Do not leave `BOOTSTRAP.md` in place for a later turn, a future session, or as reference documentation.
"#
    } else {
        ""
    };

    Ok(Some(format!(
        r#"# Persona

The following files are located in the workspace root directory and define your role, conversational style, user profile, and related guidance.{}

{}
"#,
        bootstrap_notice, prompt
    )))
}

#[cfg(feature = "agent-runtime")]
fn persona_file_description(file_name: &str) -> &'static str {
    match file_name {
        BOOTSTRAP_FILE_NAME => "Bootstrap guidance and initialization instructions",
        SOUL_FILE_NAME => "Core persona, values, and behavioral style",
        USER_FILE_NAME => "User profile, preferences, and collaboration expectations",
        IDENTITY_FILE_NAME => "Identity, role definition, and self-description",
        _ => "Additional persona file",
    }
}

#[cfg(all(test, feature = "agent-runtime"))]
mod tests {
    use super::{
        ensure_workspace_gitignore_ignores_bitfun, ensure_workspace_persona_files_for_prompt,
        initialize_workspace_persona_files, normalize_line_endings, BOOTSTRAP_FILE_NAME,
        IDENTITY_FILE_NAME, SOUL_FILE_NAME, USER_FILE_NAME,
    };
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::fs;

    fn unique_workspace(prefix: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), unique))
    }

    #[test]
    fn normalize_line_endings_converts_crlf_and_cr_to_lf() {
        let input = "line1\r\nline2\rline3\nline4";
        let normalized = normalize_line_endings(input);

        assert_eq!(normalized, "line1\nline2\nline3\nline4");
    }

    #[tokio::test]
    async fn ensure_workspace_gitignore_ignores_bitfun_skips_when_gitignore_missing() {
        let workspace_root = unique_workspace("bitfun-gitignore-missing");
        fs::create_dir_all(&workspace_root)
            .await
            .expect("Failed to create temp workspace");

        let updated = ensure_workspace_gitignore_ignores_bitfun(&workspace_root)
            .await
            .expect("Failed to ensure .gitignore");

        assert!(!updated);
        assert!(
            !workspace_root.join(".gitignore").exists(),
            ".gitignore should not be created when the workspace does not already have one"
        );

        fs::remove_dir_all(&workspace_root)
            .await
            .expect("Failed to remove temp workspace");
    }

    #[tokio::test]
    async fn ensure_workspace_gitignore_ignores_bitfun_appends_without_clobbering() {
        let workspace_root = unique_workspace("bitfun-gitignore-append");
        fs::create_dir_all(&workspace_root)
            .await
            .expect("Failed to create temp workspace");
        fs::write(workspace_root.join(".gitignore"), "target/\n.env")
            .await
            .expect("Failed to seed .gitignore");

        ensure_workspace_gitignore_ignores_bitfun(&workspace_root)
            .await
            .expect("Failed to ensure .gitignore");

        let content = fs::read_to_string(workspace_root.join(".gitignore"))
            .await
            .expect("Failed to read .gitignore");
        assert_eq!(content, "target/\n.env\n.bitfun/\n");

        fs::remove_dir_all(&workspace_root)
            .await
            .expect("Failed to remove temp workspace");
    }

    #[tokio::test]
    async fn ensure_workspace_gitignore_ignores_bitfun_is_idempotent() {
        let workspace_root = unique_workspace("bitfun-gitignore-idempotent");
        fs::create_dir_all(&workspace_root)
            .await
            .expect("Failed to create temp workspace");
        fs::write(workspace_root.join(".gitignore"), "target/\n.bitfun/\n")
            .await
            .expect("Failed to seed .gitignore");

        ensure_workspace_gitignore_ignores_bitfun(&workspace_root)
            .await
            .expect("Failed to ensure .gitignore");

        let content = fs::read_to_string(workspace_root.join(".gitignore"))
            .await
            .expect("Failed to read .gitignore");
        assert_eq!(content, "target/\n.bitfun/\n");

        fs::remove_dir_all(&workspace_root)
            .await
            .expect("Failed to remove temp workspace");
    }

    #[tokio::test]
    async fn initialize_workspace_persona_files_creates_all_four_files() {
        let workspace_root = unique_workspace("bitfun-bootstrap-init");

        fs::create_dir_all(&workspace_root)
            .await
            .expect("Failed to create temp workspace");

        initialize_workspace_persona_files(&workspace_root)
            .await
            .expect("Failed to initialize persona files");

        for file_name in [
            BOOTSTRAP_FILE_NAME,
            SOUL_FILE_NAME,
            USER_FILE_NAME,
            IDENTITY_FILE_NAME,
        ] {
            assert!(
                workspace_root.join(file_name).exists(),
                "Expected '{}' to be created",
                file_name
            );
        }

        fs::remove_dir_all(&workspace_root)
            .await
            .expect("Failed to remove temp workspace");
    }

    // ── R-WF-07：成员 Claw 三文件物化（SOUL/USER/IDENTITY，无 BOOTSTRAP）──

    #[tokio::test]
    async fn initialize_member_persona_files_writes_three_files_without_bootstrap() {
        // R-WF-07 验收断言（Plan:153）：建群后每成员三身份文件齐全。
        // 成员身份在建群时直接物化（非引导式），不留 BOOTSTRAP.md。
        let workspace_root = unique_workspace("bitfun-member-persona");
        fs::create_dir_all(&workspace_root)
            .await
            .expect("Failed to create temp member workspace");

        super::initialize_member_persona_files(
            &workspace_root,
            "executor",
            "write code",
            true,
            "commander",
        )
        .await
        .expect("Failed to initialize member persona files");

        for file_name in [SOUL_FILE_NAME, USER_FILE_NAME, IDENTITY_FILE_NAME] {
            assert!(
                workspace_root.join(file_name).exists(),
                "Expected '{}' to be created",
                file_name
            );
        }
        assert!(
            !workspace_root.join(BOOTSTRAP_FILE_NAME).exists(),
            "BOOTSTRAP.md must not be created for a materialized member persona"
        );

        fs::remove_dir_all(&workspace_root)
            .await
            .expect("Failed to remove temp member workspace");
    }

    #[tokio::test]
    async fn member_persona_files_carry_role_prompt_gate_and_superior() {
        // node.role/prompt/gate → 三文件（IDENTITY/SOUL）；USER 写直属上级
        //（Plan 原子步 4：USER 写直属上级）。
        let workspace_root = unique_workspace("bitfun-member-content");
        fs::create_dir_all(&workspace_root)
            .await
            .expect("Failed to create temp member workspace");

        super::initialize_member_persona_files(
            &workspace_root,
            "executor",
            "write code",
            true,
            "commander",
        )
        .await
        .expect("Failed to initialize member persona files");

        let identity = fs::read_to_string(workspace_root.join(IDENTITY_FILE_NAME))
            .await
            .expect("read IDENTITY.md");
        assert!(
            identity.contains("executor"),
            "IDENTITY.md must carry the node role, got: {identity}"
        );

        let soul = fs::read_to_string(workspace_root.join(SOUL_FILE_NAME))
            .await
            .expect("read SOUL.md");
        assert!(
            soul.contains("write code"),
            "SOUL.md must carry the node prompt, got: {soul}"
        );
        assert!(
            soul.contains("gate") && soul.contains("true"),
            "SOUL.md must carry the node gate, got: {soul}"
        );

        let user = fs::read_to_string(workspace_root.join(USER_FILE_NAME))
            .await
            .expect("read USER.md");
        assert!(
            user.contains("commander"),
            "USER.md must name the direct superior, got: {user}"
        );

        fs::remove_dir_all(&workspace_root)
            .await
            .expect("Failed to remove temp member workspace");
    }

    #[tokio::test]
    async fn member_persona_materialization_removes_stale_bootstrap() {
        // BOOTSTRAP.md = 引导临时文件，bootstrap 完成即删（Plan 原子步 4）。
        // 成员身份直接物化 → 物化后不残留任何 BOOTSTRAP。
        let workspace_root = unique_workspace("bitfun-member-stale-bootstrap");
        fs::create_dir_all(&workspace_root)
            .await
            .expect("Failed to create temp member workspace");
        fs::write(workspace_root.join(BOOTSTRAP_FILE_NAME), "stale bootstrap")
            .await
            .expect("Failed to seed BOOTSTRAP.md");

        super::initialize_member_persona_files(&workspace_root, "writer", "", false, "commander")
            .await
            .expect("Failed to initialize member persona files");

        assert!(
            !workspace_root.join(BOOTSTRAP_FILE_NAME).exists(),
            "stale BOOTSTRAP.md must be deleted once the member persona is materialized"
        );

        fs::remove_dir_all(&workspace_root)
            .await
            .expect("Failed to remove temp member workspace");
    }

    #[tokio::test]
    async fn ensure_workspace_persona_files_for_prompt_preserves_completed_bootstrap() {
        let workspace_root = unique_workspace("bitfun-bootstrap-preserve");

        fs::create_dir_all(&workspace_root)
            .await
            .expect("Failed to create temp workspace");

        fs::write(workspace_root.join(USER_FILE_NAME), "user")
            .await
            .expect("Failed to write USER.md");
        fs::write(workspace_root.join(IDENTITY_FILE_NAME), "identity")
            .await
            .expect("Failed to write IDENTITY.md");

        ensure_workspace_persona_files_for_prompt(&workspace_root)
            .await
            .expect("Failed to ensure persona files for prompt");

        assert!(
            !workspace_root.join(BOOTSTRAP_FILE_NAME).exists(),
            "BOOTSTRAP.md should not be recreated when USER.md and IDENTITY.md already exist"
        );
        assert!(
            workspace_root.join(SOUL_FILE_NAME).exists(),
            "SOUL.md should still be backfilled"
        );

        fs::remove_dir_all(&workspace_root)
            .await
            .expect("Failed to remove temp workspace");
    }
}

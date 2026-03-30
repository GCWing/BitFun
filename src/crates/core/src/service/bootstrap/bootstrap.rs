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

pub(crate) async fn initialize_workspace_persona_files(workspace_root: &Path) -> BitFunResult<()> {
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
        "Initialized workspace persona files: path={}, created_bootstrap={}, created_soul={}, created_user={}, created_identity={}",
        workspace_root.display(),
        created_bootstrap,
        created_soul,
        created_user,
        created_identity
    );

    Ok(())
}

pub(crate) fn is_workspace_bootstrap_pending(workspace_root: &Path) -> bool {
    workspace_root.join(BOOTSTRAP_FILE_NAME).exists()
}

pub(crate) async fn ensure_workspace_persona_files_for_prompt(
    workspace_root: &Path,
) -> BitFunResult<()> {
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
        "Ensured workspace persona files for prompt: path={}, bootstrap_exists={}, user_exists={}, identity_exists={}, created_bootstrap={}, created_soul={}, created_user={}, created_identity={}",
        workspace_root.display(),
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

fn persona_file_description(file_name: &str) -> &'static str {
    match file_name {
        BOOTSTRAP_FILE_NAME => "Bootstrap guidance and initialization instructions",
        SOUL_FILE_NAME => "Core persona, values, and behavioral style",
        USER_FILE_NAME => "User profile, preferences, and collaboration expectations",
        IDENTITY_FILE_NAME => "Identity, role definition, and self-description",
        _ => "Additional persona file",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_workspace_persona_files_for_prompt, initialize_workspace_persona_files,
        normalize_line_endings, BOOTSTRAP_FILE_NAME, IDENTITY_FILE_NAME, SOUL_FILE_NAME,
        USER_FILE_NAME,
    };
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::fs;

    #[test]
    fn normalize_line_endings_converts_crlf_and_cr_to_lf() {
        let input = "line1\r\nline2\rline3\nline4";
        let normalized = normalize_line_endings(input);

        assert_eq!(normalized, "line1\nline2\nline3\nline4");
    }

    #[tokio::test]
    async fn initialize_workspace_persona_files_creates_all_four_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before unix epoch")
            .as_nanos();
        let workspace_root = std::env::temp_dir().join(format!(
            "bitfun-bootstrap-init-{}-{}",
            std::process::id(),
            unique
        ));

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

    #[tokio::test]
    async fn ensure_workspace_persona_files_for_prompt_preserves_completed_bootstrap() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before unix epoch")
            .as_nanos();
        let workspace_root = std::env::temp_dir().join(format!(
            "bitfun-bootstrap-preserve-{}-{}",
            std::process::id(),
            unique
        ));

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

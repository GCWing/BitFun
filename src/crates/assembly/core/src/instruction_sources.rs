//! External-source composition for local user instruction adapters.

use openbitfun_claude_code_adapter::{
    load_claude_code_user_instructions, ClaudeCodeInstructionSourceOptions,
};
use openbitfun_codex_adapter::{load_codex_user_instructions, CodexInstructionSourceOptions};
use openbitfun_opencode_adapter::{
    load_opencode_user_instructions, OpenCodeInstructionSourceOptions,
};
use openbitfun_services_core::local_instructions::{LocalInstructionFile, LocalInstructionFiles};
use std::path::Path;

pub(crate) struct LocalUserInstructionFiles {
    pub(crate) files: Vec<LocalInstructionFile>,
    pub(crate) cacheable: bool,
}

pub(crate) async fn load_local_user_instruction_files(
    workspace_root: &Path,
) -> LocalUserInstructionFiles {
    let mut sources = load_local_user_instruction_sources(workspace_root).await;
    sources.files.retain(|file| file.path_patterns.is_empty());
    sources
}

pub(crate) async fn load_local_user_instruction_sources(
    workspace_root: &Path,
) -> LocalUserInstructionFiles {
    let workspace_root = workspace_root.to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        let mut files = Vec::new();
        let mut cacheable = true;
        let mut opencode_options = OpenCodeInstructionSourceOptions::from_environment();
        opencode_options.workspace_root = Some(workspace_root);
        match load_opencode_user_instructions(&opencode_options) {
            Ok(source_files) => files.extend(source_files),
            Err(_) => {
                cacheable = false;
                log::warn!(
                    "Failed to load OpenCode user instructions; retrying on the next message"
                );
            }
        }
        match load_codex_user_instructions(&CodexInstructionSourceOptions::from_environment()) {
            Ok(source_files) => files.extend(source_files),
            Err(_) => {
                cacheable = false;
                log::warn!("Failed to load Codex user instructions; retrying on the next message");
            }
        }
        match load_claude_code_user_instructions(
            &ClaudeCodeInstructionSourceOptions::from_environment(),
        ) {
            Ok(source_files) => files.extend(source_files),
            Err(_) => {
                cacheable = false;
                log::warn!(
                    "Failed to load Claude Code user instructions; retrying on the next message"
                );
            }
        }
        deduplicate_user_instruction_files(&mut files);
        LocalUserInstructionFiles { files, cacheable }
    })
    .await;
    match result {
        Ok(files) => files,
        Err(_) => {
            log::warn!("Failed to join local instruction discovery; retrying on the next message");
            LocalUserInstructionFiles {
                files: Vec::new(),
                cacheable: false,
            }
        }
    }
}

pub(crate) async fn load_local_user_conditional_instruction_sources() -> Vec<LocalInstructionFile> {
    match tokio::task::spawn_blocking(|| {
        load_claude_code_user_instructions(&ClaudeCodeInstructionSourceOptions::from_environment())
            .map(|files| {
                files
                    .into_iter()
                    .filter(|file| !file.path_patterns.is_empty())
                    .collect()
            })
    })
    .await
    {
        Ok(Ok(files)) => files,
        Ok(Err(error)) => {
            log::warn!(
                "Failed to load Claude Code conditional instructions; retrying after a later matching read: {error}"
            );
            Vec::new()
        }
        Err(error) => {
            log::warn!(
                "Failed to join Claude Code conditional instruction discovery; retrying after a later matching read: {error}"
            );
            Vec::new()
        }
    }
}

fn deduplicate_user_instruction_files(files: &mut Vec<LocalInstructionFile>) {
    let mut bounded = LocalInstructionFiles::default();
    bounded.extend(std::mem::take(files));
    *files = bounded.into_files();
}

#[cfg(test)]
mod tests {
    use super::deduplicate_user_instruction_files;
    use openbitfun_services_core::local_instructions::LocalInstructionFile;
    use std::path::PathBuf;

    #[test]
    fn merged_user_sources_keep_first_identity_and_enforce_the_shared_file_budget() {
        let mut files = (0..257)
            .map(|index| LocalInstructionFile {
                canonical_path: PathBuf::from(format!("source-{index}.md")),
                name: format!("source-{index}.md"),
                content: format!("instruction {index}"),
                path_patterns: Vec::new(),
            })
            .collect::<Vec<_>>();
        files.insert(
            1,
            LocalInstructionFile {
                canonical_path: PathBuf::from("source-0.md"),
                name: "duplicate.md".to_string(),
                content: "duplicate must lose".to_string(),
                path_patterns: Vec::new(),
            },
        );

        deduplicate_user_instruction_files(&mut files);

        assert_eq!(files.len(), 256);
        assert_eq!(files[0].name, "source-0.md");
        assert!(!files.iter().any(|file| file.name == "duplicate.md"));
    }

    #[test]
    fn merged_user_sources_enforce_the_shared_total_byte_budget() {
        let mut files = (0..3)
            .map(|index| LocalInstructionFile {
                canonical_path: PathBuf::from(format!("large-{index}.md")),
                name: format!("large-{index}.md"),
                content: "x".repeat(1024 * 1024),
                path_patterns: Vec::new(),
            })
            .collect::<Vec<_>>();

        deduplicate_user_instruction_files(&mut files);

        assert_eq!(files.len(), 2);
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::ffi::OsString;
    use std::path::Path;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    static ENVIRONMENT: OnceLock<Mutex<()>> = OnceLock::new();

    pub(crate) fn lock_environment() -> MutexGuard<'static, ()> {
        ENVIRONMENT
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("instruction environment lock")
    }

    pub(crate) struct EnvironmentGuard {
        values: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvironmentGuard {
        pub(crate) fn set(values: &[(&'static str, &Path)]) -> Self {
            let previous = values
                .iter()
                .map(|(name, value)| {
                    let previous = std::env::var_os(name);
                    std::env::set_var(name, value);
                    (*name, previous)
                })
                .collect();
            Self { values: previous }
        }
    }

    impl Drop for EnvironmentGuard {
        fn drop(&mut self) {
            for (name, value) in self.values.drain(..) {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

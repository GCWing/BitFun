//! External-source composition for local user instruction adapters.

use bitfun_claude_code_adapter::{
    load_claude_code_user_instructions, ClaudeCodeInstructionSourceOptions,
};
use bitfun_codex_adapter::{load_codex_user_instructions, CodexInstructionSourceOptions};
use bitfun_opencode_adapter::{load_opencode_user_instructions, OpenCodeInstructionSourceOptions};
use bitfun_services_core::local_instructions::{LocalInstructionFile, LocalInstructionFiles};
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
    use bitfun_services_core::local_instructions::LocalInstructionFile;
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

    /// Test fixture for the two AtomicBool instruction master switches.
    ///
    /// These switches are process-level global caches
    /// (`set_workspace_instruction_files_enabled` /
    /// `set_external_instruction_sources_enabled`). Mutating them directly in
    /// tests leaks state across tests: a test that flips a switch without
    /// restoring it can silently change the behavior of later tests that
    /// implicitly depend on the default. This guard records the previous value
    /// of each switch, applies the requested values, and restores the previous
    /// values on drop — making every test self-contained regardless of the
    /// order it runs in.
    pub(crate) struct InstructionSwitches {
        previous_workspace: bool,
        previous_external: bool,
    }

    impl InstructionSwitches {
        /// Set both instruction master switches for the duration of the test.
        ///
        /// Pass `Option::None` to leave that switch untouched.
        pub(crate) fn set(
            workspace_instruction_files: Option<bool>,
            external_instruction_sources: Option<bool>,
        ) -> Self {
            let previous_workspace = crate::service::config::workspace_instruction_files_enabled();
            let previous_external = crate::service::config::external_instruction_sources_enabled();
            if let Some(enabled) = workspace_instruction_files {
                crate::service::config::set_workspace_instruction_files_enabled(enabled);
            }
            if let Some(enabled) = external_instruction_sources {
                crate::service::config::set_external_instruction_sources_enabled(enabled);
            }
            Self {
                previous_workspace,
                previous_external,
            }
        }

        /// Enable both instruction master switches (the common test baseline).
        pub(crate) fn enable_all() -> Self {
            Self::set(Some(true), Some(true))
        }
    }

    impl Drop for InstructionSwitches {
        fn drop(&mut self) {
            crate::service::config::set_workspace_instruction_files_enabled(
                self.previous_workspace,
            );
            crate::service::config::set_external_instruction_sources_enabled(
                self.previous_external,
            );
        }
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

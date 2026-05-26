//! MiniApp host-routing string helpers.

use std::path::Path;

const HOST_NAMESPACES: &[&str] = &["fs", "shell", "os", "net"];
const DEFAULT_SHELL_EXEC_TIMEOUT_MS: u64 = 30_000;
const SHELL_EXEC_DEFAULT_ENV: [(&str, &str); 2] = [("GIT_TERMINAL_PROMPT", "0"), ("LC_ALL", "C")];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsAccessMode {
    Read,
    Write,
    Unchecked,
}

impl FsAccessMode {
    pub fn policy_key(self) -> Option<&'static str> {
        match self {
            FsAccessMode::Read => Some("read"),
            FsAccessMode::Write => Some("write"),
            FsAccessMode::Unchecked => None,
        }
    }
}

/// Returns true when `method` belongs to a namespace served by the host directly.
///
/// `storage.*` is intentionally excluded: it is routed through MiniApp storage
/// from the command layer so it can share locking with the rest of the app.
pub fn is_host_primitive(method: &str) -> bool {
    split_host_method(method)
        .map(|(ns, _)| HOST_NAMESPACES.contains(&ns))
        .unwrap_or(false)
}

pub fn split_host_method(method: &str) -> Option<(&str, &str)> {
    method.split_once('.')
}

pub fn fs_method_access_mode(name: &str) -> FsAccessMode {
    match name {
        "writeFile" | "mkdir" | "rm" | "appendFile" | "rename" | "copyFile" => FsAccessMode::Write,
        "access" => FsAccessMode::Unchecked,
        _ => FsAccessMode::Read,
    }
}

pub fn fs_policy_scopes(policy: &serde_json::Value, mode: FsAccessMode) -> Vec<String> {
    let Some(key) = mode.policy_key() else {
        return Vec::new();
    };
    policy
        .get("fs")
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_array())
        .map(|scopes| {
            scopes
                .iter()
                .filter_map(|scope| scope.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub fn fs_resolved_path_allowed<I>(resolved_target: &Path, resolved_scope_roots: I) -> bool
where
    I: IntoIterator<Item = std::path::PathBuf>,
{
    resolved_scope_roots
        .into_iter()
        .any(|scope| resolved_target.starts_with(scope))
}

pub fn command_basename_for_allowlist(command: &str) -> String {
    let file_name = command
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(command);
    Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name)
        .to_lowercase()
}

pub fn command_basename_allowed(allowlist: &[String], basename: &str) -> bool {
    allowlist.is_empty()
        || allowlist
            .iter()
            .any(|allowed| allowed.to_lowercase() == basename)
}

pub fn host_allowed_by_allowlist(allowlist: &[String], host: &str) -> bool {
    allowlist.is_empty()
        || allowlist.iter().any(|allowed| {
            allowed == "*" || host == allowed || host.ends_with(&format!(".{}", allowed))
        })
}

pub fn shell_exec_first_token<'a>(argv: Option<&'a [String]>, command: &'a str) -> &'a str {
    match argv {
        Some(args) => args.first().map(String::as_str).unwrap_or(""),
        None => command.split_whitespace().next().unwrap_or(""),
    }
}

pub fn shell_exec_input_is_empty(argv: Option<&[String]>, command: &str) -> bool {
    argv.map(|args| args.is_empty()).unwrap_or(true) && command.trim().is_empty()
}

pub fn shell_exec_cwd(
    explicit_cwd: Option<&str>,
    workspace_dir: Option<&Path>,
    app_data_dir: &Path,
) -> std::path::PathBuf {
    explicit_cwd
        .map(std::path::PathBuf::from)
        .or_else(|| workspace_dir.map(Path::to_path_buf))
        .unwrap_or_else(|| app_data_dir.to_path_buf())
}

pub fn shell_exec_timeout_ms(explicit_timeout_ms: Option<u64>) -> u64 {
    explicit_timeout_ms.unwrap_or(DEFAULT_SHELL_EXEC_TIMEOUT_MS)
}

pub fn shell_exec_default_env() -> [(&'static str, &'static str); 2] {
    SHELL_EXEC_DEFAULT_ENV
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_host_method_matches_existing_namespace_contract() {
        assert_eq!(split_host_method("fs.readFile"), Some(("fs", "readFile")));
        assert_eq!(split_host_method("storage.get"), Some(("storage", "get")));
        assert_eq!(split_host_method("invalid"), None);
    }

    #[test]
    fn fs_method_access_mode_preserves_access_bypass_and_default_read_contract() {
        assert_eq!(fs_method_access_mode("readFile"), FsAccessMode::Read);
        assert_eq!(fs_method_access_mode("writeFile"), FsAccessMode::Write);
        assert_eq!(fs_method_access_mode("copyFile"), FsAccessMode::Write);
        assert_eq!(fs_method_access_mode("access"), FsAccessMode::Unchecked);
        assert_eq!(fs_method_access_mode("unknownMethod"), FsAccessMode::Read);
        assert_eq!(FsAccessMode::Read.policy_key(), Some("read"));
        assert_eq!(FsAccessMode::Write.policy_key(), Some("write"));
        assert_eq!(FsAccessMode::Unchecked.policy_key(), None);
    }

    #[test]
    fn fs_policy_scopes_and_resolved_prefix_check_preserve_path_boundary() {
        let policy = serde_json::json!({
            "fs": {
                "read": ["/workspace", "/tmp/granted"],
                "write": ["/workspace/out"]
            }
        });

        assert_eq!(
            fs_policy_scopes(&policy, FsAccessMode::Read),
            vec!["/workspace".to_string(), "/tmp/granted".to_string()]
        );
        assert!(fs_policy_scopes(&policy, FsAccessMode::Unchecked).is_empty());
        assert!(fs_resolved_path_allowed(
            Path::new("/workspace/src/main.rs"),
            [std::path::PathBuf::from("/workspace")]
        ));
        assert!(!fs_resolved_path_allowed(
            Path::new("/workspaced/src/main.rs"),
            [std::path::PathBuf::from("/workspace")]
        ));
    }

    #[test]
    fn shell_exec_first_token_prefers_argv_over_shell_command_text() {
        let argv = vec![
            r"C:\Program Files\Git\cmd\git.exe".to_string(),
            "status".to_string(),
        ];

        assert_eq!(
            shell_exec_first_token(Some(&argv), "node ignored.js"),
            r"C:\Program Files\Git\cmd\git.exe"
        );
        assert_eq!(shell_exec_first_token(None, " git status "), "git");
        assert_eq!(shell_exec_first_token(Some(&[]), "git status"), "");
    }

    #[test]
    fn shell_exec_plan_helpers_preserve_defaults_and_precedence() {
        let argv = vec!["git".to_string()];
        assert!(shell_exec_input_is_empty(Some(&[]), ""));
        assert!(!shell_exec_input_is_empty(Some(&argv), ""));
        assert!(!shell_exec_input_is_empty(None, " git status "));
        assert_eq!(
            shell_exec_cwd(
                Some("/explicit"),
                Some(Path::new("/workspace")),
                Path::new("/appdata")
            ),
            std::path::PathBuf::from("/explicit")
        );
        assert_eq!(
            shell_exec_cwd(None, Some(Path::new("/workspace")), Path::new("/appdata")),
            std::path::PathBuf::from("/workspace")
        );
        assert_eq!(
            shell_exec_cwd(None, None, Path::new("/appdata")),
            std::path::PathBuf::from("/appdata")
        );
        assert_eq!(shell_exec_timeout_ms(None), 30_000);
        assert_eq!(shell_exec_timeout_ms(Some(8_000)), 8_000);
        assert_eq!(
            shell_exec_default_env(),
            [("GIT_TERMINAL_PROMPT", "0"), ("LC_ALL", "C")]
        );
    }
}

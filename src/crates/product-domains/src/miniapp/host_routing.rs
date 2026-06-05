//! MiniApp host-routing string helpers.

use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiniAppHostPlanErrorKind {
    Parse,
    Validation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppHostPlanError {
    kind: MiniAppHostPlanErrorKind,
    message: String,
}

impl MiniAppHostPlanError {
    pub fn parse(message: impl Into<String>) -> Self {
        Self {
            kind: MiniAppHostPlanErrorKind::Parse,
            message: message.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            kind: MiniAppHostPlanErrorKind::Validation,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> MiniAppHostPlanErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for MiniAppHostPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MiniAppHostPlanError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppFsHostPathCheck {
    pub path: PathBuf,
    pub mode: FsAccessMode,
    pub denied_prefix: &'static str,
}

impl MiniAppFsHostPathCheck {
    pub fn denied_message(&self) -> String {
        format!(
            "{} not allowed: {}",
            self.denied_prefix,
            self.path.display()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MiniAppFsHostCallPlan {
    ReadFile {
        path: PathBuf,
        encoding_base64: bool,
    },
    WriteFile {
        path: PathBuf,
        data: String,
    },
    ReadDir {
        path: PathBuf,
    },
    Stat {
        path: PathBuf,
    },
    Mkdir {
        path: PathBuf,
        recursive: bool,
    },
    Rm {
        path: PathBuf,
        recursive: bool,
        force: bool,
    },
    CopyFile {
        src: PathBuf,
        dst: PathBuf,
    },
    Rename {
        old_path: PathBuf,
        new_path: PathBuf,
    },
    AppendFile {
        path: PathBuf,
        data: String,
    },
    Access {
        path: PathBuf,
    },
}

impl MiniAppFsHostCallPlan {
    pub fn path_checks(&self) -> Vec<MiniAppFsHostPathCheck> {
        match self {
            Self::ReadFile { path, .. } | Self::ReadDir { path } | Self::Stat { path } => {
                vec![MiniAppFsHostPathCheck {
                    path: path.clone(),
                    mode: FsAccessMode::Read,
                    denied_prefix: "Path",
                }]
            }
            Self::WriteFile { path, .. }
            | Self::Mkdir { path, .. }
            | Self::Rm { path, .. }
            | Self::AppendFile { path, .. } => vec![MiniAppFsHostPathCheck {
                path: path.clone(),
                mode: FsAccessMode::Write,
                denied_prefix: "Path",
            }],
            Self::CopyFile { src, dst } => vec![
                MiniAppFsHostPathCheck {
                    path: src.clone(),
                    mode: FsAccessMode::Read,
                    denied_prefix: "src",
                },
                MiniAppFsHostPathCheck {
                    path: dst.clone(),
                    mode: FsAccessMode::Write,
                    denied_prefix: "dst",
                },
            ],
            Self::Rename { old_path, new_path } => vec![
                MiniAppFsHostPathCheck {
                    path: old_path.clone(),
                    mode: FsAccessMode::Write,
                    denied_prefix: "oldPath",
                },
                MiniAppFsHostPathCheck {
                    path: new_path.clone(),
                    mode: FsAccessMode::Write,
                    denied_prefix: "newPath",
                },
            ],
            Self::Access { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppShellHostCallPlan {
    pub argv: Option<Vec<String>>,
    pub command: String,
    pub first_token: String,
    pub cwd: PathBuf,
    pub timeout_ms: u64,
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

pub fn plan_fs_host_call(
    name: &str,
    params: &serde_json::Value,
) -> Result<MiniAppFsHostCallPlan, MiniAppHostPlanError> {
    let path_param = fs_host_path_param(params);

    match name {
        "readFile" => Ok(MiniAppFsHostCallPlan::ReadFile {
            path: require_path(path_param)?,
            encoding_base64: params
                .get("encoding")
                .and_then(|v| v.as_str())
                .is_some_and(|encoding| encoding == "base64"),
        }),
        "writeFile" => Ok(MiniAppFsHostCallPlan::WriteFile {
            path: require_path(path_param)?,
            data: params
                .get("data")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        }),
        "readdir" => Ok(MiniAppFsHostCallPlan::ReadDir {
            path: require_path(path_param)?,
        }),
        "stat" => Ok(MiniAppFsHostCallPlan::Stat {
            path: require_path(path_param)?,
        }),
        "mkdir" => Ok(MiniAppFsHostCallPlan::Mkdir {
            path: require_path(path_param)?,
            recursive: params
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }),
        "rm" => Ok(MiniAppFsHostCallPlan::Rm {
            path: require_path(path_param)?,
            recursive: params
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            force: params
                .get("force")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }),
        "copyFile" => Ok(MiniAppFsHostCallPlan::CopyFile {
            src: require_named_path(params, "src")?,
            dst: require_named_path(params, "dst")?,
        }),
        "rename" => Ok(MiniAppFsHostCallPlan::Rename {
            old_path: require_named_path(params, "oldPath")?,
            new_path: require_named_path(params, "newPath")?,
        }),
        "appendFile" => Ok(MiniAppFsHostCallPlan::AppendFile {
            path: require_path(path_param)?,
            data: params
                .get("data")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        }),
        "access" => Ok(MiniAppFsHostCallPlan::Access {
            path: require_path(path_param)?,
        }),
        other => Err(MiniAppHostPlanError::validation(format!(
            "unknown fs method: {}",
            other
        ))),
    }
}

pub fn plan_fs_legacy_path_check(
    name: &str,
    params: &serde_json::Value,
) -> Option<MiniAppFsHostPathCheck> {
    let mode = fs_method_access_mode(name);
    mode.policy_key()?;
    fs_host_path_param(params).map(|path| MiniAppFsHostPathCheck {
        path,
        mode,
        denied_prefix: "Path",
    })
}

fn fs_host_path_param(params: &serde_json::Value) -> Option<PathBuf> {
    params
        .get("path")
        .or_else(|| params.get("p"))
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
}

fn require_path(path: Option<PathBuf>) -> Result<PathBuf, MiniAppHostPlanError> {
    path.ok_or_else(|| MiniAppHostPlanError::parse("missing path"))
}

fn require_named_path(
    params: &serde_json::Value,
    key: &str,
) -> Result<PathBuf, MiniAppHostPlanError> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| MiniAppHostPlanError::parse(format!("missing param: {}", key)))
}

pub fn plan_shell_host_call(
    name: &str,
    params: &serde_json::Value,
    workspace_dir: Option<&Path>,
    app_data_dir: &Path,
) -> Result<MiniAppShellHostCallPlan, MiniAppHostPlanError> {
    if name != "exec" {
        return Err(MiniAppHostPlanError::validation(format!(
            "unknown shell method: {}",
            name
        )));
    }

    let argv: Option<Vec<String>> = params.get("args").and_then(|v| v.as_array()).map(|a| {
        a.iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect()
    });
    let command = params
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if shell_exec_input_is_empty(argv.as_deref(), &command) {
        return Err(MiniAppHostPlanError::parse("empty command"));
    }

    Ok(MiniAppShellHostCallPlan {
        first_token: shell_exec_first_token(argv.as_deref(), &command).to_string(),
        cwd: shell_exec_cwd(
            params.get("cwd").and_then(|v| v.as_str()),
            workspace_dir,
            app_data_dir,
        ),
        timeout_ms: shell_exec_timeout_ms(params.get("timeout").and_then(|v| v.as_u64())),
        argv,
        command,
    })
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

    #[test]
    fn miniapp_host_fs_call_plans_preserve_existing_path_and_permission_contract() {
        let read = plan_fs_host_call(
            "readFile",
            &serde_json::json!({ "path": "/workspace/read.txt", "encoding": "base64" }),
        )
        .expect("readFile should plan");

        assert_eq!(
            read,
            MiniAppFsHostCallPlan::ReadFile {
                path: std::path::PathBuf::from("/workspace/read.txt"),
                encoding_base64: true,
            }
        );
        assert_eq!(
            read.path_checks(),
            vec![MiniAppFsHostPathCheck {
                path: std::path::PathBuf::from("/workspace/read.txt"),
                mode: FsAccessMode::Read,
                denied_prefix: "Path",
            }]
        );

        let copy = plan_fs_host_call(
            "copyFile",
            &serde_json::json!({ "src": "/workspace/src.txt", "dst": "/workspace/dst.txt" }),
        )
        .expect("copyFile should plan");
        assert_eq!(
            copy.path_checks(),
            vec![
                MiniAppFsHostPathCheck {
                    path: std::path::PathBuf::from("/workspace/src.txt"),
                    mode: FsAccessMode::Read,
                    denied_prefix: "src",
                },
                MiniAppFsHostPathCheck {
                    path: std::path::PathBuf::from("/workspace/dst.txt"),
                    mode: FsAccessMode::Write,
                    denied_prefix: "dst",
                },
            ]
        );
    }

    #[test]
    fn miniapp_host_shell_call_plans_preserve_existing_input_and_default_contract() {
        let plan = plan_shell_host_call(
            "exec",
            &serde_json::json!({
                "args": ["git", "status"],
                "command": "ignored",
                "cwd": "/workspace",
                "timeout": 8000,
            }),
            Some(Path::new("/fallback-workspace")),
            Path::new("/appdata"),
        )
        .expect("shell.exec should plan");

        assert_eq!(
            plan.argv,
            Some(vec!["git".to_string(), "status".to_string()])
        );
        assert_eq!(plan.command, "ignored");
        assert_eq!(plan.first_token, "git");
        assert_eq!(plan.cwd, std::path::PathBuf::from("/workspace"));
        assert_eq!(plan.timeout_ms, 8000);
    }
}

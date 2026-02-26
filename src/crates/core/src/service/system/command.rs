//! System command utilities
//!
//! Provides command detection and execution.

use crate::util::process_manager;
use log::error;
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::{
    collections::HashSet,
    process::Command,
    sync::OnceLock,
};

/// Command check result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CheckCommandResult {
    /// Whether the command exists
    pub exists: bool,
    /// Full path to the command (if it exists)
    pub path: Option<String>,
}

/// Command execution result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommandOutput {
    /// Exit code
    pub exit_code: i32,
    /// Stdout
    pub stdout: String,
    /// Stderr
    pub stderr: String,
    /// Whether the command succeeded (`exit_code == 0`)
    pub success: bool,
}

/// System command error
#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Command not found: {0}")]
    CommandNotFound(String),
}

/// Platform-specific PATH entries that are commonly used but may not be present in GUI app
/// environments (e.g. macOS apps launched from Finder).
pub fn platform_path_entries() -> Vec<PathBuf> {
    platform_path_entries_impl()
}

#[cfg(target_os = "macos")]
fn platform_path_entries_impl() -> Vec<PathBuf> {
    let candidates = [
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
        "/opt/local/bin",
        "/opt/local/sbin",
    ];

    let mut entries: Vec<PathBuf> = candidates.iter().map(PathBuf::from).collect();
    entries.extend(homebrew_node_opt_bin_entries());
    entries.extend(login_shell_path_entries());

    dedup_existing_dirs(entries)
}

#[cfg(not(target_os = "macos"))]
fn platform_path_entries_impl() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(target_os = "macos")]
static LOGIN_SHELL_PATH_ENTRIES: OnceLock<Vec<PathBuf>> = OnceLock::new();

#[cfg(target_os = "macos")]
fn login_shell_path_entries() -> Vec<PathBuf> {
    LOGIN_SHELL_PATH_ENTRIES
        .get_or_init(resolve_login_shell_path_entries)
        .clone()
}

#[cfg(target_os = "macos")]
fn resolve_login_shell_path_entries() -> Vec<PathBuf> {
    let mut shell_candidates = Vec::new();
    if let Ok(shell) = std::env::var("SHELL") {
        let shell = shell.trim();
        if !shell.is_empty() {
            shell_candidates.push(shell.to_string());
        }
    }
    shell_candidates.push("/bin/zsh".to_string());
    shell_candidates.push("/bin/bash".to_string());

    let mut seen = HashSet::new();
    for shell in shell_candidates {
        if !seen.insert(shell.clone()) {
            continue;
        }
        if let Some(path_value) = read_path_from_login_shell(&shell) {
            let entries: Vec<PathBuf> = std::env::split_paths(&path_value)
                .filter(|p| p.is_dir())
                .collect();
            if !entries.is_empty() {
                return dedup_existing_dirs(entries);
            }
        }
    }

    Vec::new()
}

#[cfg(target_os = "macos")]
fn homebrew_node_opt_bin_entries() -> Vec<PathBuf> {
    let opt_roots = ["/opt/homebrew/opt", "/usr/local/opt"];
    let mut entries = Vec::new();

    for root in opt_roots {
        let root_path = PathBuf::from(root);
        if !root_path.is_dir() {
            continue;
        }

        // Include common fixed paths first.
        let node_bin = root_path.join("node").join("bin");
        if node_bin.is_dir() {
            entries.push(node_bin);
        }

        let read_dir = match std::fs::read_dir(&root_path) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Also include versioned formulas like node@20/node@22.
        for entry in read_dir.flatten() {
            let entry_path = entry.path();
            // Homebrew formula entries under opt are often symlinks; follow links when checking.
            if !entry_path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("node@") {
                continue;
            }

            let bin_dir = entry_path.join("bin");
            if bin_dir.is_dir() {
                entries.push(bin_dir);
            }
        }
    }

    dedup_existing_dirs(entries)
}

#[cfg(target_os = "macos")]
fn read_path_from_login_shell(shell: &str) -> Option<String> {
    let output = Command::new(shell)
        .arg("-lc")
        .arg("printf '%s' \"$PATH\"")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let path_value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path_value.is_empty() {
        None
    } else {
        Some(path_value)
    }
}

#[cfg(target_os = "macos")]
fn dedup_existing_dirs(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        if !path.is_dir() {
            continue;
        }
        let key = path.to_string_lossy().to_string();
        if seen.insert(key) {
            deduped.push(path);
        }
    }
    deduped
}

/// Checks whether a command exists.
///
/// Uses the `which` crate for cross-platform command detection.
///
/// # Parameters
/// - `cmd`: Command name (e.g. "git", "npm", "cargo")
///
/// # Returns
/// - `CheckCommandResult`: Contains existence and full path
///
/// # Example
/// ```rust
/// let result = check_command("git");
/// if result.exists {
///     if let Some(path) = result.path.as_deref() {
///         println!("Git path: {}", path);
///     }
/// }
/// ```
pub fn check_command(cmd: &str) -> CheckCommandResult {
    match which::which(cmd) {
        Ok(path) => CheckCommandResult {
            exists: true,
            path: Some(path.to_string_lossy().to_string()),
        },
        Err(_) => {
            // On macOS, GUI apps (e.g. Tauri release builds launched from Finder) often do not
            // inherit the interactive shell PATH, so common package manager dirs may be missing.
            // Try again with platform PATH extras to improve command discovery.
            #[cfg(target_os = "macos")]
            {
                let mut merged = Vec::new();
                if let Some(existing) = std::env::var_os("PATH") {
                    merged.extend(std::env::split_paths(&existing));
                }
                merged.extend(platform_path_entries());

                if let Ok(joined) = std::env::join_paths(merged) {
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    if let Ok(path) =
                        which::which_in(cmd, Some(joined), cwd)
                    {
                        return CheckCommandResult {
                            exists: true,
                            path: Some(path.to_string_lossy().to_string()),
                        };
                    }
                }
            }

            CheckCommandResult {
                exists: false,
                path: None,
            }
        }
    }
}

/// Checks multiple commands in batch.
///
/// # Parameters
/// - `commands`: List of command names
///
/// # Returns
/// - `Vec<(String, CheckCommandResult)>`: List of command names and results
pub fn check_commands(commands: &[&str]) -> Vec<(String, CheckCommandResult)> {
    commands
        .iter()
        .map(|cmd| (cmd.to_string(), check_command(cmd)))
        .collect()
}

/// Runs a system command.
///
/// # Parameters
/// - `cmd`: Command name
/// - `args`: Command arguments
/// - `cwd`: Working directory (optional)
/// - `env`: Environment variables (optional)
///
/// # Returns
/// - `Result<CommandOutput, SystemError>`: Command output or error
pub async fn run_command(
    cmd: &str,
    args: &[String],
    cwd: Option<&str>,
    env: Option<&[(String, String)]>,
) -> Result<CommandOutput, SystemError> {
    let mut command = process_manager::create_tokio_command(cmd);

    command.args(args);

    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    if let Some(env_vars) = env {
        for (key, value) in env_vars {
            command.env(key, value);
        }
    }

    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let output = command.output().await.map_err(|e| {
        error!("Command execution failed: command={}, error={}", cmd, e);
        SystemError::ExecutionFailed(e.to_string())
    })?;

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();

    Ok(CommandOutput {
        exit_code,
        stdout,
        stderr,
        success,
    })
}

/// Runs a system command (simplified version, without environment variables).
pub async fn run_command_simple(
    cmd: &str,
    args: &[String],
    cwd: Option<&str>,
) -> Result<CommandOutput, SystemError> {
    run_command(cmd, args, cwd, None).await
}

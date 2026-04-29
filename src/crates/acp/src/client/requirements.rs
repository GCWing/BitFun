use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use bitfun_core::util::errors::{BitFunError, BitFunResult};
use tokio::process::Command;

use super::config::{AcpClientConfig, AcpRequirementProbeItem};

const REQUIREMENT_PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const ADAPTER_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const CLI_INSTALL_TIMEOUT: Duration = Duration::from_secs(600);

pub(crate) struct AcpRequirementSpec<'a> {
    pub(crate) tool_command: &'a str,
    pub(crate) install_package: Option<&'a str>,
    pub(crate) adapter: Option<AcpAdapterSpec<'a>>,
}

pub(crate) struct AcpAdapterSpec<'a> {
    pub(crate) package: &'a str,
    pub(crate) bin: &'a str,
}

pub(crate) fn acp_requirement_spec<'a>(
    client_id: &'a str,
    config: Option<&'a AcpClientConfig>,
) -> AcpRequirementSpec<'a> {
    match client_id {
        "claude-code" => AcpRequirementSpec {
            tool_command: "claude",
            install_package: Some("@anthropic-ai/claude-code"),
            adapter: Some(AcpAdapterSpec {
                package: "@zed-industries/claude-code-acp",
                bin: "claude-code-acp",
            }),
        },
        "codex" => AcpRequirementSpec {
            tool_command: "codex",
            install_package: Some("@openai/codex"),
            adapter: Some(AcpAdapterSpec {
                package: "@zed-industries/codex-acp",
                bin: "codex-acp",
            }),
        },
        "opencode" => AcpRequirementSpec {
            tool_command: "opencode",
            install_package: Some("opencode-ai"),
            adapter: None,
        },
        _ => AcpRequirementSpec {
            tool_command: config
                .map(|config| config.command.as_str())
                .unwrap_or(client_id),
            install_package: None,
            adapter: None,
        },
    }
}

pub(crate) async fn probe_executable(command: &str) -> AcpRequirementProbeItem {
    let path = find_executable(command);
    let mut item = AcpRequirementProbeItem {
        name: command.to_string(),
        installed: path.is_some(),
        version: None,
        path: path.as_ref().map(|path| path.to_string_lossy().to_string()),
        error: None,
    };

    if let Some(path) = path {
        match run_command_with_timeout(path.as_os_str(), ["--version"], REQUIREMENT_PROBE_TIMEOUT)
            .await
        {
            Ok(output) if output.status.success() => {
                item.version = parse_version_text(&output.stdout)
                    .or_else(|| parse_version_text(&output.stderr));
            }
            Ok(output) => {
                item.error = Some(command_error_summary(&output.stderr, &output.stdout));
            }
            Err(error) => {
                item.error = Some(error);
            }
        }
    }

    item
}

pub(crate) async fn probe_npm_adapter(package: &str, bin: &str) -> AcpRequirementProbeItem {
    let npm_path = find_executable("npm");
    let mut item = AcpRequirementProbeItem {
        name: package.to_string(),
        installed: false,
        version: None,
        path: None,
        error: None,
    };
    let Some(npm_path) = npm_path else {
        item.error = Some("npm is not available on PATH".to_string());
        return item;
    };

    let global_args = ["ls", "-g", "--json", "--depth=0", package];
    match run_command_with_timeout(npm_path.as_os_str(), global_args, REQUIREMENT_PROBE_TIMEOUT)
        .await
    {
        Ok(output) if output.status.success() => {
            if let Some(version) = npm_ls_package_version(&output.stdout, package) {
                item.installed = true;
                item.version = Some(version);
                item.path = Some("npm global".to_string());
                return item;
            }
        }
        Ok(output) => {
            item.error = Some(command_error_summary(&output.stderr, &output.stdout));
        }
        Err(error) => {
            item.error = Some(error);
        }
    }

    let offline_args = vec![
        "exec".to_string(),
        "--offline".to_string(),
        "--yes".to_string(),
        format!("--package={package}"),
        "--".to_string(),
        bin.to_string(),
        "--help".to_string(),
    ];
    match run_command_with_timeout(
        npm_path.as_os_str(),
        offline_args.iter().map(String::as_str),
        REQUIREMENT_PROBE_TIMEOUT,
    )
    .await
    {
        Ok(output) if output.status.success() => {
            item.installed = true;
            item.path = Some("npm offline cache".to_string());
            item.error = None;
        }
        Ok(output) => {
            item.error = Some(command_error_summary(&output.stderr, &output.stdout));
        }
        Err(error) => {
            item.error = Some(error);
        }
    }

    if find_executable("npx").is_some() {
        item.installed = true;
        item.path = Some("npx auto-install".to_string());
        item.error = None;
    }

    item
}

pub(crate) async fn predownload_npm_adapter(package: &str, bin: &str) -> BitFunResult<()> {
    let npm_path = find_executable("npm")
        .ok_or_else(|| BitFunError::service("npm is not available on PATH".to_string()))?;
    let args = vec![
        "exec".to_string(),
        "--yes".to_string(),
        format!("--package={package}"),
        "--".to_string(),
        bin.to_string(),
        "--help".to_string(),
    ];

    match run_command_with_timeout(
        npm_path.as_os_str(),
        args.iter().map(String::as_str),
        ADAPTER_DOWNLOAD_TIMEOUT,
    )
    .await
    {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(BitFunError::service(format!(
            "Failed to predownload ACP adapter '{}': {}",
            package,
            command_error_summary(&output.stderr, &output.stdout)
        ))),
        Err(error) => Err(BitFunError::service(format!(
            "Failed to predownload ACP adapter '{}': {}",
            package, error
        ))),
    }
}

pub(crate) async fn install_npm_cli_package(package: &str) -> BitFunResult<()> {
    let npm_path = find_executable("npm")
        .ok_or_else(|| BitFunError::service("npm is not available on PATH".to_string()))?;
    let args = ["install", "-g", package];

    match run_command_with_timeout(npm_path.as_os_str(), args, CLI_INSTALL_TIMEOUT).await {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(BitFunError::service(format!(
            "Failed to install ACP agent CLI '{}': {}",
            package,
            command_error_summary(&output.stderr, &output.stdout)
        ))),
        Err(error) => Err(BitFunError::service(format!(
            "Failed to install ACP agent CLI '{}': {}",
            package, error
        ))),
    }
}

pub(crate) fn resolve_configured_command(
    command: &str,
    extra_env: &HashMap<String, String>,
) -> PathBuf {
    let configured_path = configured_path_value(extra_env);
    find_executable_with_path(command, configured_path.as_deref())
        .unwrap_or_else(|| PathBuf::from(command))
}

pub(crate) fn apply_command_environment(
    command: &mut Command,
    extra_env: Option<&HashMap<String, String>>,
) {
    let configured_path = extra_env.and_then(configured_path_value);
    let search_path = joined_command_search_path(configured_path.as_deref());
    if !search_path.is_empty() {
        command.env("PATH", search_path);
    }

    if let Some(extra_env) = extra_env {
        for (key, value) in extra_env {
            if !key.eq_ignore_ascii_case("PATH") {
                command.env(key, value);
            }
        }
    }
}

async fn run_command_with_timeout<I, S>(
    program: &OsStr,
    args: I,
    timeout: Duration,
) -> Result<std::process::Output, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(program);
    command.args(args);
    apply_command_environment(&mut command, None);
    match tokio::time::timeout(timeout, command.output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err("Timed out while checking command".to_string()),
    }
}

fn npm_ls_package_version(stdout: &[u8], package: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(stdout).ok()?;
    value
        .get("dependencies")?
        .get(package)?
        .get("version")?
        .as_str()
        .map(ToString::to_string)
}

fn parse_version_text(output: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(output);
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn command_error_summary(stderr: &[u8], stdout: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    if !stderr.is_empty() {
        return truncate_error(stderr);
    }
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    if !stdout.is_empty() {
        return truncate_error(stdout);
    }
    "Command exited unsuccessfully".to_string()
}

fn truncate_error(value: String) -> String {
    const MAX_LEN: usize = 240;
    if value.chars().count() <= MAX_LEN {
        return value;
    }
    format!("{}...", value.chars().take(MAX_LEN).collect::<String>())
}

fn find_executable(command: &str) -> Option<PathBuf> {
    find_executable_with_path(command, None)
}

fn find_executable_with_path(command: &str, configured_path: Option<&OsStr>) -> Option<PathBuf> {
    let command_path = PathBuf::from(command);
    if command_path.components().count() > 1 {
        return executable_file(&command_path).then_some(command_path);
    }

    for directory in command_search_paths(configured_path) {
        for candidate in executable_candidates(&directory, command) {
            if executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn configured_path_value(extra_env: &HashMap<String, String>) -> Option<OsString> {
    extra_env
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("PATH"))
        .map(|(_, value)| OsString::from(value))
}

fn joined_command_search_path(configured_path: Option<&OsStr>) -> OsString {
    let paths = command_search_paths(configured_path);
    if paths.is_empty() {
        return OsString::new();
    }
    env::join_paths(paths).unwrap_or_else(|_| env::var_os("PATH").unwrap_or_default())
}

fn command_search_paths(configured_path: Option<&OsStr>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    if let Some(configured_path) = configured_path {
        push_split_paths(&mut paths, &mut seen, configured_path);
    }
    if let Some(env_path) = env::var_os("PATH") {
        push_split_paths(&mut paths, &mut seen, &env_path);
    }

    push_user_bin_paths(&mut paths, &mut seen);
    push_system_bin_paths(&mut paths, &mut seen);
    paths
}

fn push_split_paths(paths: &mut Vec<PathBuf>, seen: &mut HashSet<OsString>, value: &OsStr) {
    for directory in env::split_paths(value) {
        push_search_path(paths, seen, directory);
    }
}

fn push_user_bin_paths(paths: &mut Vec<PathBuf>, seen: &mut HashSet<OsString>) {
    let Some(home) = env::var_os("HOME") else {
        return;
    };
    let home = PathBuf::from(home);
    push_existing_search_path(paths, seen, home.join(".local/bin"));
    push_existing_search_path(paths, seen, home.join(".cargo/bin"));
    push_existing_search_path(paths, seen, home.join(".npm-global/bin"));
}

fn push_system_bin_paths(paths: &mut Vec<PathBuf>, seen: &mut HashSet<OsString>) {
    #[cfg(target_os = "macos")]
    {
        for prefix in ["/opt/homebrew", "/usr/local"] {
            push_existing_search_path(paths, seen, PathBuf::from(format!("{prefix}/bin")));
            push_existing_search_path(paths, seen, PathBuf::from(format!("{prefix}/sbin")));
            for node in ["node", "node@18", "node@20", "node@22", "node@24"] {
                push_existing_search_path(
                    paths,
                    seen,
                    PathBuf::from(format!("{prefix}/opt/{node}/bin")),
                );
            }
        }
    }
}

fn push_existing_search_path(
    paths: &mut Vec<PathBuf>,
    seen: &mut HashSet<OsString>,
    path: PathBuf,
) {
    if path.is_dir() {
        push_search_path(paths, seen, path);
    }
}

fn push_search_path(paths: &mut Vec<PathBuf>, seen: &mut HashSet<OsString>, path: PathBuf) {
    if path.as_os_str().is_empty() {
        return;
    }

    let key = search_path_key(&path);
    if seen.insert(key) {
        paths.push(path);
    }
}

fn search_path_key(path: &Path) -> OsString {
    #[cfg(windows)]
    {
        OsString::from(path.to_string_lossy().to_ascii_lowercase())
    }
    #[cfg(not(windows))]
    {
        path.as_os_str().to_os_string()
    }
}

fn executable_candidates(directory: &Path, command: &str) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let command_path = PathBuf::from(command);
        if command_path.extension().is_some() {
            return vec![directory.join(command)];
        }
        let extensions = env::var_os("PATHEXT").unwrap_or_else(|| OsString::from(".EXE;.BAT;.CMD"));
        extensions
            .to_string_lossy()
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(|extension| directory.join(format!("{command}{extension}")))
            .collect()
    }

    #[cfg(not(windows))]
    {
        vec![directory.join(command)]
    }
}

fn executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_search_paths_keep_configured_path_first() {
        let configured_paths = env::join_paths([
            PathBuf::from("/tmp/bitfun-acp-first"),
            PathBuf::from("/tmp/bitfun-acp-second"),
        ])
        .expect("test paths should be joinable");

        let paths = command_search_paths(Some(&configured_paths));

        assert_eq!(paths.first(), Some(&PathBuf::from("/tmp/bitfun-acp-first")));
        assert_eq!(paths.get(1), Some(&PathBuf::from("/tmp/bitfun-acp-second")));
    }

    #[test]
    fn find_executable_uses_configured_path() {
        let test_dir = env::temp_dir().join(format!("bitfun-acp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&test_dir).expect("test dir should be created");

        #[cfg(windows)]
        let file_name = "bitfun-test-tool.EXE";
        #[cfg(not(windows))]
        let file_name = "bitfun-test-tool";

        let executable = test_dir.join(file_name);
        std::fs::write(&executable, b"").expect("test executable should be written");

        let found = find_executable_with_path("bitfun-test-tool", Some(test_dir.as_os_str()));

        let _ = std::fs::remove_dir_all(&test_dir);
        assert_eq!(found, Some(executable));
    }
}

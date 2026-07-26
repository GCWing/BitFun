use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::Instant;

use super::patch::nul_separated_paths;

pub(super) fn needs_change_baseline(
    output_patch: Option<&str>,
    verify_final_changes: bool,
) -> bool {
    output_patch.is_some() || verify_final_changes
}

#[derive(Debug, Clone)]
pub(super) struct VerifyConfig {
    pub(super) timeout: Duration,
    pub(super) max_retries: u32,
}

impl VerifyConfig {
    pub(super) fn from_env() -> Self {
        let timeout = std::env::var("BITFUN_PATCH_VERIFY_TIMEOUT_SEC")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(900));
        let max_retries = std::env::var("BITFUN_PATCH_VERIFY_MAX_RETRIES")
            .ok()
            .and_then(|value| value.trim().parse::<u32>().ok())
            .unwrap_or(1);
        Self {
            timeout,
            max_retries,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum VerifyStatus {
    Passed,
    Failed,
    TimedOut,
    SpawnError,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct VerifyOutcome {
    pub(super) status: VerifyStatus,
    pub(super) command: String,
    pub(super) exit_code: Option<i32>,
    pub(super) duration_ms: u64,
    pub(super) output_tail: String,
    pub(super) retries_used: u32,
}

pub(super) async fn run_verifier(
    workspace: &Path,
    command: &str,
    config: &VerifyConfig,
    retries_used: u32,
) -> VerifyOutcome {
    let mut process = if cfg!(windows) {
        let mut process = tokio::process::Command::new("cmd");
        process.arg("/C").arg(command);
        process
    } else {
        let mut process = tokio::process::Command::new("sh");
        process.arg("-c").arg(command);
        process
    };
    process.current_dir(workspace);
    process.kill_on_drop(true);

    let started = Instant::now();
    let result = tokio::time::timeout(config.timeout, process.output()).await;
    let duration_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(Ok(output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let combined = if stderr.trim().is_empty() {
                stdout.to_string()
            } else if stdout.trim().is_empty() {
                stderr.to_string()
            } else {
                format!("{stderr}\n--- stdout ---\n{stdout}")
            };
            VerifyOutcome {
                status: if output.status.success() {
                    VerifyStatus::Passed
                } else {
                    VerifyStatus::Failed
                },
                command: command.to_string(),
                exit_code: output.status.code(),
                duration_ms,
                output_tail: tail_chars(&combined, 4_000),
                retries_used,
            }
        }
        Ok(Err(error)) => VerifyOutcome {
            status: VerifyStatus::SpawnError,
            command: command.to_string(),
            exit_code: None,
            duration_ms,
            output_tail: format!("spawn error: {error}"),
            retries_used,
        },
        Err(_) => VerifyOutcome {
            status: VerifyStatus::TimedOut,
            command: command.to_string(),
            exit_code: None,
            duration_ms,
            output_tail: format!("timed out after {}s", config.timeout.as_secs()),
            retries_used,
        },
    }
}

fn tail_chars(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        value.to_string()
    } else {
        value.chars().skip(count - max_chars).collect()
    }
}

pub(super) fn build_retry_message(outcome: &VerifyOutcome) -> String {
    match outcome.status {
        VerifyStatus::TimedOut => format!(
            "<system-reminder>\n\
The verifier we ran timed out and did not return a pass/fail signal:\n\n\
$ {command}\n\
(timed out after {duration_ms}ms)\n\n\
Do not rerun the same command verbatim. Run a concretely lighter, scoped check, \
or finalize with a concrete justification if the changes are correct.\n\
</system-reminder>",
            command = outcome.command,
            duration_ms = outcome.duration_ms,
        ),
        VerifyStatus::SpawnError => format!(
            "<system-reminder>\n\
The verifier could not start:\n\n\
$ {command}\n\
{output}\n\n\
Use an available, scoped verification command and finalize.\n\
</system-reminder>",
            command = outcome.command,
            output = outcome.output_tail,
        ),
        VerifyStatus::Failed => format!(
            "<system-reminder>\n\
Your changes did not pass external verification (exit {exit_code:?}):\n\n\
$ {command}\n\n\
Last output (truncated to 4000 characters):\n\
{output}\n\n\
Diagnose the remaining failure, fix it, and run a relevant scoped check before \
finalizing. If the failure is unrelated, provide concrete evidence.\n\
</system-reminder>",
            exit_code = outcome.exit_code,
            command = outcome.command,
            output = outcome.output_tail,
        ),
        VerifyStatus::Passed => {
            "<system-reminder>Verification passed; no retry is required.</system-reminder>"
                .to_string()
        }
    }
}

/// Select checks only for files changed since exec started. Mixed-language
/// changes compose their scoped checks; broad Make/just targets are never
/// inferred merely from their names.
pub(super) fn detect_verify_command(
    workspace: &Path,
    diff_base: Option<&str>,
    initial_untracked_files: &BTreeSet<String>,
) -> Option<String> {
    let changed = changed_files(workspace, diff_base, initial_untracked_files);
    if changed.is_empty() {
        return None;
    }

    let mut commands = Vec::new();
    commands.extend(scoped_go_commands(workspace, &changed));
    commands.extend(scoped_cargo_commands(workspace, &changed));
    commands.extend(scoped_typescript_commands(workspace, &changed));
    if let Some(command) = build_parse_only_command(workspace, &changed) {
        commands.push(command);
    }
    (!commands.is_empty()).then(|| commands.join(" && "))
}

pub(super) fn changed_files(
    workspace: &Path,
    diff_base: Option<&str>,
    initial_untracked_files: &BTreeSet<String>,
) -> Vec<String> {
    let mut files = BTreeSet::new();
    if let Some(diff_base) = diff_base {
        if let Ok(output) = bitfun_core::util::process_manager::create_command("git")
            .args(["diff", diff_base, "--name-only", "--find-renames", "-z"])
            .current_dir(workspace)
            .output()
        {
            if output.status.success() {
                files.extend(nul_separated_paths(&output.stdout));
            }
        }
    }
    if let Ok(output) = bitfun_core::util::process_manager::create_command("git")
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(workspace)
        .output()
    {
        if output.status.success() {
            files.extend(
                nul_separated_paths(&output.stdout)
                    .into_iter()
                    .filter(|path| !initial_untracked_files.contains(path)),
            );
        }
    }
    files.into_iter().collect()
}

fn scoped_go_commands(workspace: &Path, files: &[String]) -> Vec<String> {
    let mut packages_by_module: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    let mut manifest_only_modules = BTreeSet::new();
    for file in files {
        let path = workspace.join(file);
        let lower = file.to_ascii_lowercase();
        let Some(manifest) = find_nearest_manifest(workspace, &path, "go.mod") else {
            continue;
        };
        let module_dir = manifest.parent().unwrap_or(workspace).to_path_buf();
        if lower.ends_with(".go") {
            let package_dir = path.parent().unwrap_or(&module_dir);
            let has_go_source = std::fs::read_dir(package_dir)
                .ok()
                .into_iter()
                .flatten()
                .any(|entry| {
                    entry
                        .ok()
                        .and_then(|entry| entry.path().extension().map(|ext| ext == "go"))
                        .unwrap_or(false)
                });
            if !has_go_source {
                continue;
            }
            let relative = package_dir.strip_prefix(&module_dir).unwrap_or(package_dir);
            let target = if relative.as_os_str().is_empty() {
                ".".to_string()
            } else {
                format!("./{}", relative.to_string_lossy().replace('\\', "/"))
            };
            packages_by_module
                .entry(module_dir)
                .or_default()
                .insert(target);
        } else if matches!(
            Path::new(&lower).file_name().and_then(|name| name.to_str()),
            Some("go.mod" | "go.sum")
        ) {
            manifest_only_modules.insert(module_dir);
        }
    }

    let mut commands = Vec::new();
    for (module_dir, packages) in &packages_by_module {
        let targets = packages
            .iter()
            .map(|target| shell_single_quote(target))
            .collect::<Vec<_>>()
            .join(" ");
        commands.push(command_in_directory(
            workspace,
            module_dir,
            &format!("go vet -printf=false -composites=false -stdmethods=false {targets}"),
        ));
    }
    for module_dir in manifest_only_modules {
        if !packages_by_module.contains_key(&module_dir) {
            commands.push(command_in_directory(
                workspace,
                &module_dir,
                "go list -m all",
            ));
        }
    }
    commands
}

fn scoped_cargo_commands(workspace: &Path, files: &[String]) -> Vec<String> {
    let mut packages: BTreeMap<PathBuf, (Option<String>, BTreeSet<String>)> = BTreeMap::new();
    for file in files {
        let lower = file.to_ascii_lowercase();
        let is_source = lower.ends_with(".rs");
        let is_manifest = matches!(
            Path::new(&lower).file_name().and_then(|name| name.to_str()),
            Some("cargo.toml" | "cargo.lock")
        );
        if !is_source && !is_manifest {
            continue;
        }
        let path = workspace.join(file);
        let Some(manifest) = find_nearest_manifest(workspace, &path, "Cargo.toml") else {
            continue;
        };
        let package = read_cargo_package_name(&manifest);
        let integration_test = is_source
            .then(|| cargo_integration_test_target(&manifest, &path))
            .flatten();
        let entry = packages
            .entry(manifest)
            .or_insert_with(|| (package, BTreeSet::new()));
        if let Some(target) = integration_test {
            entry.1.insert(target);
        }
    }

    packages
        .into_iter()
        .flat_map(|(manifest, (package, integration_tests))| {
            let manifest = manifest
                .strip_prefix(workspace)
                .unwrap_or(&manifest)
                .to_string_lossy()
                .replace('\\', "/");
            match package {
                Some(package) => {
                    let manifest = shell_single_quote(&manifest);
                    let package = shell_single_quote(&package);
                    let mut commands = vec![format!(
                        "cargo check --manifest-path {manifest} -p {package} --message-format=short"
                    )];
                    if !integration_tests.is_empty() {
                        let targets = integration_tests
                            .iter()
                            .map(|target| format!("--test {}", shell_single_quote(target)))
                            .collect::<Vec<_>>()
                            .join(" ");
                        commands.push(format!(
                            "cargo check --manifest-path {manifest} -p {package} {targets} --message-format=short"
                        ));
                    }
                    commands
                }
                None => vec![format!(
                    "cargo metadata --no-deps --format-version 1 --manifest-path {}",
                    shell_single_quote(&manifest)
                )],
            }
        })
        .collect()
}

fn scoped_typescript_commands(workspace: &Path, files: &[String]) -> Vec<String> {
    let mut configs = BTreeSet::new();
    for file in files {
        let lower = file.to_ascii_lowercase();
        let is_source = [".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"]
            .iter()
            .any(|extension| lower.ends_with(extension));
        let is_config =
            Path::new(&lower).file_name().and_then(|name| name.to_str()) == Some("tsconfig.json");
        if !is_source && !is_config {
            continue;
        }
        if let Some(config) =
            find_nearest_manifest(workspace, &workspace.join(file), "tsconfig.json")
        {
            configs.insert(config);
        }
    }
    configs
        .into_iter()
        .map(|config| {
            let relative = config
                .strip_prefix(workspace)
                .unwrap_or(&config)
                .to_string_lossy()
                .replace('\\', "/");
            format!(
                "npx --no-install tsc --noEmit -p {}",
                shell_single_quote(&relative)
            )
        })
        .collect()
}

fn find_nearest_manifest(workspace: &Path, file: &Path, name: &str) -> Option<PathBuf> {
    let mut current = file.parent()?;
    loop {
        let manifest = current.join(name);
        if manifest.is_file() {
            return Some(manifest);
        }
        if current == workspace {
            return None;
        }
        current = current.parent()?;
    }
}

fn cargo_integration_test_target(manifest: &Path, source: &Path) -> Option<String> {
    if !source.is_file() {
        return None;
    }
    let relative = source.strip_prefix(manifest.parent()?).ok()?;
    let mut components = relative.components();
    if components.next()?.as_os_str() != "tests" {
        return None;
    }
    let target = components.next()?.as_os_str().to_str()?;
    if components.next().is_some() {
        return None;
    }
    Path::new(target)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToString::to_string)
}

fn read_cargo_package_name(manifest: &Path) -> Option<String> {
    let content = std::fs::read_to_string(manifest).ok()?;
    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('[') {
            in_package = rest.starts_with("package]");
            continue;
        }
        if in_package {
            if let Some(rest) = trimmed.strip_prefix("name") {
                if let Some(rest) = rest.trim_start().strip_prefix('=') {
                    let value = rest.split('#').next().unwrap_or(rest).trim();
                    let value = value.trim_matches(|character| matches!(character, '"' | '\''));
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }
    None
}

fn build_parse_only_command(workspace: &Path, files: &[String]) -> Option<String> {
    let mut checks = Vec::new();
    for file in files {
        if !workspace.join(file).is_file() {
            continue;
        }
        let quoted = shell_single_quote(file);
        let lower = file.to_ascii_lowercase();
        if lower.ends_with(".py") {
            checks.push(format!(
                "python3 -c 'import ast,sys; ast.parse(open(sys.argv[1]).read())' {quoted}"
            ));
        } else if lower.ends_with(".js") || lower.ends_with(".mjs") || lower.ends_with(".cjs") {
            checks.push(format!("node --check {quoted}"));
        } else if lower.ends_with(".go")
            && find_nearest_manifest(workspace, &workspace.join(file), "go.mod").is_none()
        {
            checks.push(format!("gofmt -e -d {quoted}"));
        }
    }
    (!checks.is_empty()).then(|| checks.join(" && "))
}

fn command_in_directory(workspace: &Path, directory: &Path, command: &str) -> String {
    let relative = directory.strip_prefix(workspace).unwrap_or(directory);
    if relative.as_os_str().is_empty() {
        command.to_string()
    } else {
        format!(
            "(cd {} && {command})",
            shell_single_quote(&relative.to_string_lossy().replace('\\', "/"))
        )
    }
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditorCommand {
    pub(super) program: PathBuf,
    pub(super) args: Vec<OsString>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(super) enum EditorConfigError {
    #[error(
        "No external editor is configured. Set VISUAL or EDITOR to an installed editor command that waits for the file to close"
    )]
    Missing,
    #[error("{variable} is not valid Unicode")]
    InvalidEncoding { variable: &'static str },
    #[error("{variable} contains invalid or unclosed quoting")]
    InvalidSyntax { variable: &'static str },
    #[error("Editor command not found: {program}. Check VISUAL or EDITOR and include the editor's wait flag when required")]
    CommandNotFound { program: String },
}

pub(super) fn resolve_editor_command() -> Result<EditorCommand, EditorConfigError> {
    let visual = read_editor_variable("VISUAL")?;
    if visual
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return resolve_editor_command_from(visual.as_deref(), None, |program| {
            let result = bitfun_services_core::system::check_command(program);
            result.path.map(PathBuf::from)
        });
    }
    let editor = read_editor_variable("EDITOR")?;
    resolve_editor_command_from(visual.as_deref(), editor.as_deref(), |program| {
        let result = bitfun_services_core::system::check_command(program);
        result.path.map(PathBuf::from)
    })
}

fn read_editor_variable(name: &'static str) -> Result<Option<String>, EditorConfigError> {
    match std::env::var_os(name) {
        None => Ok(None),
        Some(value) => value
            .into_string()
            .map(Some)
            .map_err(|_| EditorConfigError::InvalidEncoding { variable: name }),
    }
}

fn resolve_editor_command_from(
    visual: Option<&str>,
    editor: Option<&str>,
    mut find_program: impl FnMut(&str) -> Option<PathBuf>,
) -> Result<EditorCommand, EditorConfigError> {
    let (variable, configured) = match visual.filter(|value| !value.trim().is_empty()) {
        Some(value) => ("VISUAL", value),
        None => match editor.filter(|value| !value.trim().is_empty()) {
            Some(value) => ("EDITOR", value),
            None => return Err(EditorConfigError::Missing),
        },
    };
    let mut parts = split_editor_command(configured)
        .ok_or(EditorConfigError::InvalidSyntax { variable })?
        .into_iter();
    let program = parts
        .next()
        .filter(|program| !program.is_empty())
        .ok_or(EditorConfigError::InvalidSyntax { variable })?;
    let resolved = find_program(&program).ok_or_else(|| EditorConfigError::CommandNotFound {
        program: program.clone(),
    })?;
    Ok(EditorCommand {
        program: resolved,
        args: parts.map(OsString::from).collect(),
    })
}

#[cfg(not(windows))]
fn split_editor_command(value: &str) -> Option<Vec<String>> {
    shlex::split(value)
}

#[cfg(windows)]
fn split_editor_command(value: &str) -> Option<Vec<String>> {
    let parts = winsplit::split(value);
    if parts.is_empty() || has_unclosed_windows_quote(value) {
        None
    } else {
        Some(parts)
    }
}

#[cfg(windows)]
fn has_unclosed_windows_quote(value: &str) -> bool {
    let mut quoted = false;
    let mut backslashes = 0usize;
    for character in value.chars() {
        if character == '\\' {
            backslashes += 1;
            continue;
        }
        if character == '"' && backslashes % 2 == 0 {
            quoted = !quoted;
        }
        backslashes = 0;
    }
    quoted
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ExternalEditOutcome {
    Changed(String),
    Unchanged,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExternalEditResult {
    pub(super) outcome: ExternalEditOutcome,
    pub(super) cleanup_warning: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum EditorRunError {
    #[error("Could not create the temporary Markdown file: {0}")]
    TempCreate(#[source] std::io::Error),
    #[error("Could not write the temporary Markdown file: {0}")]
    TempWrite(#[source] std::io::Error),
    #[error("Could not start the external editor: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("The external editor exited unsuccessfully (exit code: {code:?})")]
    FailedExit { code: Option<i32> },
    #[error("Could not read the edited Markdown file as UTF-8: {0}")]
    TempRead(#[source] std::io::Error),
    #[cfg(windows)]
    #[error(
        "The configured batch editor contains a quote, percent sign, newline, or non-Unicode path that cmd.exe cannot pass safely; use an executable editor shim or a simpler path"
    )]
    UnsafeBatchValue,
}

pub(super) fn run_external_editor(
    command: &EditorCommand,
    seed: &str,
    cwd: Option<&std::path::Path>,
) -> Result<ExternalEditResult, EditorRunError> {
    let mut temp_file = tempfile::Builder::new()
        .prefix("bitfun-editor-")
        .suffix(".md")
        .tempfile()
        .map_err(EditorRunError::TempCreate)?;
    temp_file
        .write_all(seed.as_bytes())
        .and_then(|_| temp_file.flush())
        .map_err(EditorRunError::TempWrite)?;

    // Close the original handle before spawning. Editors on Windows commonly
    // replace the file, which fails if the creator still owns an open handle.
    let temp_path = temp_file.into_temp_path();
    let path = temp_path.to_path_buf();
    let mut process = editor_process(command, &path)?;
    process
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(cwd) = cwd.filter(|path| path.is_dir()) {
        process.current_dir(cwd);
    }
    let operation = (|| {
        let status = process.status().map_err(EditorRunError::Spawn)?;
        if !status.success() {
            return Err(EditorRunError::FailedExit {
                code: status.code(),
            });
        }
        let edited = std::fs::read_to_string(&path).map_err(EditorRunError::TempRead)?;
        Ok(classify_external_edit(
            seed,
            &normalize_editor_text(&edited),
        ))
    })();
    let cleanup_warning = temp_path.close().err().map(|error| {
        format!(
            "Could not remove temporary editor file {}: {error}",
            path.display()
        )
    });
    match operation {
        Ok(outcome) => Ok(ExternalEditResult {
            outcome,
            cleanup_warning,
        }),
        Err(error) => {
            if let Some(warning) = cleanup_warning {
                tracing::warn!("{}", warning);
            }
            Err(error)
        }
    }
}

#[cfg(not(windows))]
fn editor_process(
    command: &EditorCommand,
    path: &std::path::Path,
) -> Result<Command, EditorRunError> {
    let mut process = bitfun_core::util::process_manager::create_command(&command.program);
    process.args(&command.args).arg(path);
    Ok(process)
}

#[cfg(windows)]
fn editor_process(
    command: &EditorCommand,
    path: &std::path::Path,
) -> Result<Command, EditorRunError> {
    use std::os::windows::process::CommandExt;

    let is_batch = command
        .program
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        });
    if is_batch {
        let mut values = Vec::with_capacity(command.args.len() + 2);
        values.push(quote_windows_batch_value(command.program.as_os_str())?);
        for argument in &command.args {
            values.push(quote_windows_batch_value(argument)?);
        }
        values.push(quote_windows_batch_value(path.as_os_str())?);
        let command_line = values.join(" ");
        let mut process = bitfun_core::util::process_manager::create_command("cmd.exe");
        process.args(["/d", "/v:off", "/s", "/c"]);
        // cmd.exe requires an extra outer quote pair when the command itself
        // begins with a quoted executable path. raw_arg is intentional here:
        // Rust's argv quoting does not escape cmd metacharacters.
        process.raw_arg(format!("\"{command_line}\""));
        Ok(process)
    } else {
        let mut process = bitfun_core::util::process_manager::create_command(&command.program);
        process.args(&command.args).arg(path);
        Ok(process)
    }
}

#[cfg(windows)]
fn quote_windows_batch_value(value: &OsStr) -> Result<String, EditorRunError> {
    let value = value.to_str().ok_or(EditorRunError::UnsafeBatchValue)?;
    if value
        .chars()
        .any(|character| matches!(character, '"' | '%' | '\r' | '\n'))
    {
        return Err(EditorRunError::UnsafeBatchValue);
    }
    Ok(format!("\"{value}\""))
}

fn classify_external_edit(seed: &str, edited: &str) -> ExternalEditOutcome {
    if edited.trim().is_empty() {
        ExternalEditOutcome::Empty
    } else if edited == seed {
        ExternalEditOutcome::Unchanged
    } else {
        ExternalEditOutcome::Changed(edited.to_string())
    }
}

fn normalize_editor_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_takes_precedence_and_preserves_quoted_arguments() {
        let command = resolve_editor_command_from(
            Some("editor-bin --wait \"profile with spaces\""),
            Some("fallback-bin"),
            |program| Some(std::path::PathBuf::from(program)),
        )
        .expect("valid VISUAL");

        assert_eq!(command.program, std::path::PathBuf::from("editor-bin"));
        assert_eq!(command.args, ["--wait", "profile with spaces"]);
    }

    #[test]
    fn blank_visual_falls_back_to_editor() {
        let command = resolve_editor_command_from(Some("  "), Some("fallback-bin -w"), |program| {
            Some(std::path::PathBuf::from(program))
        })
        .expect("valid EDITOR");

        assert_eq!(command.program, std::path::PathBuf::from("fallback-bin"));
        assert_eq!(command.args, ["-w"]);
    }

    #[test]
    fn missing_or_unclosed_editor_configuration_is_rejected() {
        assert!(matches!(
            resolve_editor_command_from(None, None, |_| None),
            Err(EditorConfigError::Missing)
        ));
        assert!(matches!(
            resolve_editor_command_from(Some("editor-bin \"unfinished"), None, |_| None),
            Err(EditorConfigError::InvalidSyntax { variable: "VISUAL" })
        ));
    }

    #[test]
    fn missing_editor_program_is_reported_before_terminal_suspend() {
        assert!(matches!(
            resolve_editor_command_from(Some("missing-editor --wait"), None, |_| None),
            Err(EditorConfigError::CommandNotFound { program }) if program == "missing-editor"
        ));
    }

    #[test]
    fn edited_content_distinguishes_changed_unchanged_and_empty() {
        assert_eq!(
            classify_external_edit("draft", "draft"),
            ExternalEditOutcome::Unchanged
        );
        assert_eq!(
            classify_external_edit("draft", ""),
            ExternalEditOutcome::Empty
        );
        assert_eq!(
            classify_external_edit("draft", "updated\n"),
            ExternalEditOutcome::Changed("updated\n".to_string())
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_batch_editor_receives_the_temporary_file_path() {
        let root = tempfile::tempdir().unwrap();
        let helper = root.path().join("editor.cmd");
        std::fs::write(
            &helper,
            "@echo off\r\n>\"%~1\" echo EDITOR_UPDATED_DRAFT\r\n",
        )
        .unwrap();
        let command = EditorCommand {
            program: helper,
            args: Vec::new(),
        };

        let result = run_external_editor(&command, "", Some(root.path())).unwrap();

        assert_eq!(
            result.outcome,
            ExternalEditOutcome::Changed("EDITOR_UPDATED_DRAFT\n".to_string())
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_batch_editor_handles_cmd_metacharacters_in_its_path() {
        let root = tempfile::Builder::new()
            .prefix("editor&safe")
            .tempdir()
            .unwrap();
        let helper = root.path().join("editor.cmd");
        std::fs::write(
            &helper,
            "@echo off\r\n>\"%~1\" echo META_PATH_UPDATED_DRAFT\r\n",
        )
        .unwrap();
        let command = EditorCommand {
            program: helper,
            args: Vec::new(),
        };

        let result = run_external_editor(&command, "", Some(root.path())).unwrap();

        assert_eq!(
            result.outcome,
            ExternalEditOutcome::Changed("META_PATH_UPDATED_DRAFT\n".to_string())
        );
    }
}

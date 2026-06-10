use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellCommandRisk {
    ReadOnly,
    Stateful,
    Mutating,
}

const READ_ONLY_COMMANDS: &[&str] = &[
    "basename", "cat", "cut", "dirname", "egrep", "fgrep", "file", "find", "grep", "head",
    "ls", "md5sum", "pwd", "rg", "sha1sum", "sha256sum", "sha512sum", "sort", "stat",
    "tail", "uniq", "wc", "xxd",
];

const STATEFUL_COMMANDS: &[&str] = &[
    "bash", "cargo", "cc", "clang", "cmake", "g++", "gcc", "git", "go", "java", "make",
    "ninja", "node", "npm", "pip", "pip3", "pnpm", "pytest", "python", "python3", "rustc",
    "sh", "sqlite3", "uv", "yarn",
];

const MUTATING_COMMANDS: &[&str] = &[
    "apt", "apt-get", "chmod", "chown", "cp", "dd", "install", "ln", "mkdir", "mv", "rm",
    "rmdir", "sed", "tee", "touch", "truncate", "unzip",
];

pub(crate) fn exec_command_is_concurrency_safe(command: &str) -> bool {
    classify_command(command) == ShellCommandRisk::ReadOnly
}

pub(crate) fn denial_for_command(command: &str) -> Option<String> {
    if opens_original_app_sqlite_db_with_invalid_wal(command) {
        return Some(
            "Refused: opening a SQLite database whose sibling WAL has an invalid header can \
             cause SQLite to delete or truncate that WAL. Copy the database, WAL, and SHM files \
             to /tmp/work first, then inspect the copy."
                .to_string(),
        );
    }

    None
}

fn classify_command(command: &str) -> ShellCommandRisk {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return ShellCommandRisk::Mutating;
    }

    let lowered = trimmed.to_ascii_lowercase();
    if contains_mutating_shell_syntax(&lowered) {
        return ShellCommandRisk::Mutating;
    }
    if lowered.contains(" -exec ") || lowered.contains(" -delete") {
        return ShellCommandRisk::Mutating;
    }

    let mut saw_stateful = false;
    for segment in command_segments(trimmed) {
        let Some(command_name) = first_command_name(segment) else {
            continue;
        };
        if MUTATING_COMMANDS.contains(&command_name.as_str()) {
            return ShellCommandRisk::Mutating;
        }
        if STATEFUL_COMMANDS.contains(&command_name.as_str()) {
            saw_stateful = true;
            continue;
        }
        if !READ_ONLY_COMMANDS.contains(&command_name.as_str()) {
            saw_stateful = true;
        }
    }

    if saw_stateful {
        ShellCommandRisk::Stateful
    } else {
        ShellCommandRisk::ReadOnly
    }
}

fn contains_mutating_shell_syntax(command: &str) -> bool {
    command.contains(">")
        || command.contains("<<")
        || command.contains("$(")
        || command.contains('`')
}

fn command_segments(command: &str) -> impl Iterator<Item = &str> {
    command
        .split(['\n', ';', '|'])
        .flat_map(|part| part.split("&&"))
        .flat_map(|part| part.split("||"))
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

fn first_command_name(segment: &str) -> Option<String> {
    let mut tokens = segment.split_whitespace().peekable();
    while let Some(token) = tokens.next() {
        let token = token.trim_matches(|c: char| matches!(c, '(' | ')' | '\'' | '"'));
        if token.is_empty() {
            continue;
        }
        if is_env_assignment(token) {
            continue;
        }
        if matches!(token, "env" | "command") {
            continue;
        }
        if token == "timeout" {
            while let Some(next) = tokens.peek() {
                if next.starts_with('-') || next.chars().all(|c| c.is_ascii_digit() || c == '.') {
                    tokens.next();
                    continue;
                }
                break;
            }
            continue;
        }
        let name = Path::new(token)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(token)
            .to_ascii_lowercase();
        return Some(name);
    }
    None
}

fn is_env_assignment(token: &str) -> bool {
    let Some((key, _value)) = token.split_once('=') else {
        return false;
    };
    !key.is_empty()
        && key
            .chars()
            .all(|c| c == '_' || c.is_ascii_alphanumeric())
        && key
            .chars()
            .next()
            .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
}

fn opens_original_app_sqlite_db_with_invalid_wal(command: &str) -> bool {
    if !opens_sqlite_database(command) {
        return false;
    }

    extract_original_app_db_paths(command)
        .iter()
        .any(|path| sqlite_wal_has_invalid_header(path))
}

fn opens_sqlite_database(command: &str) -> bool {
    let lowered = command.to_ascii_lowercase();
    lowered.contains("sqlite3")
        || (lowered.contains("import sqlite3")
            && (lowered.contains(".connect") || lowered.contains("connect(")))
}

fn extract_original_app_db_paths(command: &str) -> Vec<PathBuf> {
    command
        .split(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '\'' | '"' | '`' | ';' | ',' | ')' | '(' | '[' | ']' | '{' | '}' | '<' | '>'
                )
        })
        .filter_map(|raw| {
            let token = raw.trim_matches(|c: char| matches!(c, ':' | '.' | ',' | ';'));
            if !token.starts_with("/app/") {
                return None;
            }
            let lowered = token.to_ascii_lowercase();
            if lowered.ends_with("-wal") || lowered.ends_with("-shm") {
                return None;
            }
            if lowered.ends_with(".db")
                || lowered.ends_with(".sqlite")
                || lowered.ends_with(".sqlite3")
            {
                return Some(PathBuf::from(token));
            }
            None
        })
        .collect()
}

fn sqlite_wal_has_invalid_header(db_path: &Path) -> bool {
    let wal_path = PathBuf::from(format!("{}-wal", db_path.display()));
    let Ok(bytes) = std::fs::read(wal_path) else {
        return false;
    };
    if bytes.len() < 4 {
        return false;
    }
    !matches!(
        &bytes[..4],
        [0x37, 0x7f, 0x06, 0x82] | [0x37, 0x7f, 0x06, 0x83]
    )
}

#[cfg(test)]
mod tests {
    use super::{
        classify_command, denial_for_command, exec_command_is_concurrency_safe,
        extract_original_app_db_paths, ShellCommandRisk,
    };

    #[test]
    fn classifies_plain_readonly_pipelines_as_readonly() {
        assert_eq!(
            classify_command("xxd /app/main.db-wal | head -50"),
            ShellCommandRisk::ReadOnly
        );
        assert_eq!(
            classify_command("ls -la /app && find /app -maxdepth 1 -type f"),
            ShellCommandRisk::ReadOnly
        );
    }

    #[test]
    fn classifies_stateful_and_mutating_commands_conservatively() {
        assert_eq!(
            classify_command("python3 -c 'print(1)'"),
            ShellCommandRisk::Stateful
        );
        assert_eq!(
            classify_command("git status --short"),
            ShellCommandRisk::Stateful
        );
        assert_eq!(
            classify_command("cat input.txt > output.txt"),
            ShellCommandRisk::Mutating
        );
        assert_eq!(
            classify_command("find . -name '*.tmp' -delete"),
            ShellCommandRisk::Mutating
        );
        assert!(!exec_command_is_concurrency_safe("python3 -c 'print(1)'"));
        assert!(!exec_command_is_concurrency_safe("cat input.txt > output.txt"));
        assert!(exec_command_is_concurrency_safe("xxd /app/main.db-wal | head -50"));
    }

    #[test]
    fn extracts_original_app_db_paths_from_python_sqlite_commands() {
        let paths = extract_original_app_db_paths(
            "python3 -c \"import sqlite3; sqlite3.connect('/app/main.db')\"",
        );

        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].to_string_lossy(), "/app/main.db");
    }

    #[test]
    fn sqlite_denial_requires_matching_invalid_wal() {
        assert!(denial_for_command(
            "python3 -c \"import sqlite3; sqlite3.connect('/tmp/work/main.db')\"",
        )
        .is_none());
        assert!(denial_for_command(
            "python3 -c \"import sqlite3; sqlite3.connect('/app/main.db')\"",
        )
        .is_none());
    }
}

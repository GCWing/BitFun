use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellCommandRisk {
    ReadOnly,
    Stateful,
    Mutating,
}

const READ_ONLY_COMMANDS: &[&str] = &[
    "basename",
    "cat",
    "cut",
    "dirname",
    "egrep",
    "fgrep",
    "file",
    "find",
    "grep",
    "head",
    "ls",
    "md5sum",
    "pwd",
    "rg",
    "sha1sum",
    "sha256sum",
    "sha512sum",
    "sort",
    "stat",
    "tail",
    "uniq",
    "wc",
    "xxd",
];

const STATEFUL_COMMANDS: &[&str] = &[
    "bash", "cargo", "cc", "clang", "cmake", "g++", "gcc", "git", "go", "java", "make", "ninja",
    "node", "npm", "pip", "pip3", "pnpm", "pytest", "python", "python3", "rustc", "sh", "sqlite3",
    "uv", "yarn",
];

const MUTATING_COMMANDS: &[&str] = &[
    "apt", "apt-get", "chmod", "chown", "cp", "dd", "install", "ln", "mkdir", "mv", "rm", "rmdir",
    "sed", "tee", "touch", "truncate", "unzip",
];

const BLOCKED_EVALUATION_CODE_SOURCE_DOMAINS: &[&str] = &[
    "github.com",
    "githubusercontent.com",
    "githubassets.com",
    "git.io",
    "sourcegraph.com",
];

const NETWORK_GIT_SUBCOMMANDS: &[&str] =
    &["clone", "fetch", "pull", "push", "ls-remote", "submodule"];

const REVISION_READING_GIT_SUBCOMMANDS: &[&str] = &[
    "archive",
    "blame",
    "cat-file",
    "checkout",
    "diff",
    "format-patch",
    "grep",
    "log",
    "range-diff",
    "reflog",
    "restore",
    "rev-list",
    "show",
    "switch",
    "whatchanged",
];

pub(crate) fn exec_command_is_concurrency_safe(command: &str) -> bool {
    classify_command(command) == ShellCommandRisk::ReadOnly
}

pub(crate) fn denial_for_command(command: &str) -> Option<String> {
    if contains_blocked_evaluation_code_source_domain(command) {
        return Some(
            "Refused in this evaluation: shell commands cannot access GitHub or source-code mirrors such as Sourcegraph. Inspect only the local repository and task materials."
                .to_string(),
        );
    }

    if let Some(message) = evaluation_git_history_denial(command) {
        return Some(message);
    }

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

fn contains_blocked_evaluation_code_source_domain(command: &str) -> bool {
    command
        .to_ascii_lowercase()
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
        })
        .filter(|token| !token.is_empty())
        .any(|token| {
            BLOCKED_EVALUATION_CODE_SOURCE_DOMAINS.iter().any(|domain| {
                token == *domain
                    || token
                        .strip_suffix(domain)
                        .is_some_and(|prefix| prefix.ends_with('.'))
            })
        })
}

fn evaluation_git_history_denial(command: &str) -> Option<String> {
    for segment in command_segments(command) {
        if contains_command_named(segment, &["gh", "hub"]) {
            return Some(
                "Refused in this evaluation: GitHub command-line clients are disabled because they can retrieve an upstream solution. Use only the repository already present in the workspace."
                    .to_string(),
            );
        }

        let Some((subcommand, arguments)) = git_subcommand_and_arguments(segment) else {
            continue;
        };

        if NETWORK_GIT_SUBCOMMANDS.contains(&subcommand.as_str())
            || (subcommand == "remote"
                && arguments
                    .first()
                    .is_some_and(|argument| argument == "update"))
            || (subcommand == "lfs"
                && arguments
                    .first()
                    .is_some_and(|argument| matches!(argument.as_str(), "fetch" | "pull")))
        {
            return Some(
                "Refused in this evaluation: Git network operations are disabled because they can retrieve an upstream solution. Use only the repository already present in the workspace."
                    .to_string(),
            );
        }

        if REVISION_READING_GIT_SUBCOMMANDS.contains(&subcommand.as_str())
            && arguments
                .iter()
                .take_while(|argument| argument.as_str() != "--")
                .any(|argument| is_forbidden_evaluation_revision(argument))
        {
            return Some(
                "Refused in this evaluation: remote-tracking refs, reflogs, and explicit commit hashes may expose the benchmark target patch. Inspect the current checkout and working-tree diff instead."
                    .to_string(),
            );
        }
    }
    None
}

fn contains_command_named(segment: &str, names: &[&str]) -> bool {
    segment.split_whitespace().any(|token| {
        let token = token.trim_matches(|character: char| {
            matches!(character, '(' | ')' | '[' | ']' | '{' | '}' | '\'' | '"')
        });
        Path::new(token)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                names
                    .iter()
                    .any(|candidate| name.eq_ignore_ascii_case(candidate))
            })
    })
}

fn git_subcommand_and_arguments(segment: &str) -> Option<(String, Vec<String>)> {
    let tokens = segment
        .split_whitespace()
        .map(|token| {
            token
                .trim_matches(|character: char| {
                    matches!(character, '(' | ')' | '[' | ']' | '{' | '}' | '\'' | '"')
                })
                .to_string()
        })
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let git_index = tokens.iter().position(|token| {
        Path::new(token)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("git"))
    })?;

    let mut index = git_index + 1;
    while index < tokens.len() {
        let token = &tokens[index];
        if matches!(
            token.as_str(),
            "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace"
        ) {
            index += 2;
            continue;
        }
        if token.starts_with('-') {
            index += 1;
            continue;
        }

        return Some((
            token.to_ascii_lowercase(),
            tokens[index + 1..]
                .iter()
                .map(|argument| argument.to_ascii_lowercase())
                .collect(),
        ));
    }
    None
}

fn is_forbidden_evaluation_revision(argument: &str) -> bool {
    let argument = argument.trim_matches(|character: char| {
        matches!(
            character,
            '\'' | '"' | '(' | ')' | '[' | ']' | '{' | '}' | ','
        )
    });
    argument == "--all"
        || argument == "--reflog"
        || argument == "fetch_head"
        || argument == "orig_head"
        || argument.contains("origin/")
        || argument.contains("upstream/")
        || argument.contains("refs/remotes/")
        || argument.contains("remotes/")
        || argument.contains("@{u")
        || contains_explicit_object_id(argument)
}

fn contains_explicit_object_id(argument: &str) -> bool {
    argument
        .split(|character: char| !character.is_ascii_hexdigit())
        .any(|part| (7..=40).contains(&part.len()))
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
        && key.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
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
        classify_command, contains_blocked_evaluation_code_source_domain, denial_for_command,
        exec_command_is_concurrency_safe, extract_original_app_db_paths, ShellCommandRisk,
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
        assert!(!exec_command_is_concurrency_safe(
            "cat input.txt > output.txt"
        ));
        assert!(exec_command_is_concurrency_safe(
            "xxd /app/main.db-wal | head -50"
        ));
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

    #[test]
    fn evaluation_blocks_direct_and_proxied_github_access() {
        for command in [
            "curl -L https://codeload.github.com/org/repo/tar.gz/main",
            "git clone https://github.com/org/repo.git",
            "wget https://raw.githubusercontent.com/org/repo/main/src/lib.rs",
            "curl https://sourcegraph.com/github.com/org/repo/-/raw/src/lib.rs",
            "git clone git@github.com:org/repo.git",
        ] {
            assert!(denial_for_command(command)
                .as_deref()
                .is_some_and(|message| message.contains("source-code mirrors")));
        }

        assert!(!contains_blocked_evaluation_code_source_domain(
            "curl https://notgithub.com/example"
        ));
        assert!(denial_for_command("curl https://notgithub.com/example").is_none());
    }

    #[test]
    fn evaluation_blocks_git_network_and_hidden_solution_revisions() {
        for command in [
            "git fetch origin",
            "git pull --ff-only",
            "git push origin HEAD",
            "git ls-remote upstream",
            "gh api repos/org/repo/commits/main",
            "git show origin/main",
            "git show 878c25b",
            "git diff HEAD...upstream/main",
            "git log --all --oneline",
            "git cat-file -p fd18df1",
            "git -C /app restore --source=7073d18b src/server.go",
        ] {
            assert!(
                denial_for_command(command).is_some(),
                "should block: {command}"
            );
        }
    }

    #[test]
    fn evaluation_keeps_normal_workspace_git_inspection_available() {
        for command in [
            "git status --short",
            "git diff",
            "git diff --cached",
            "git diff HEAD -- src/lib.rs",
            "git log -5 HEAD",
            "git show HEAD:src/lib.rs",
        ] {
            assert!(
                denial_for_command(command).is_none(),
                "should allow: {command}"
            );
        }
    }
}

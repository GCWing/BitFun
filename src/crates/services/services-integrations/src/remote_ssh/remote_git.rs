//! Shared builder for git commands executed on remote SSH workspaces.
//!
//! Every host-side feature that shells out to `git` inside a remote SSH
//! workspace (desktop git commands, review-platform repository probing, ...)
//! must build the command line through this module so quoting and pager
//! behavior stay consistent.

/// Quotes a value for a POSIX shell command line.
pub fn shell_quote_posix(value: &str) -> String {
    if !value.is_empty()
        && value.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_' | ':' | '=' | '@')
        })
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

/// Builds a `git -C <repository_path> --no-pager <args...>` command line for
/// execution through an SSH channel.
pub fn build_remote_git_command<I, S>(repository_path: &str, args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut parts = vec![
        "git".to_string(),
        "-C".to_string(),
        shell_quote_posix(repository_path),
        "--no-pager".to_string(),
    ];
    parts.extend(args.into_iter().map(|arg| shell_quote_posix(arg.as_ref())));
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_values_stay_unquoted() {
        assert_eq!(shell_quote_posix("/home/user/repo"), "/home/user/repo");
        assert_eq!(shell_quote_posix("rev-parse"), "rev-parse");
    }

    #[test]
    fn special_values_are_single_quoted() {
        assert_eq!(shell_quote_posix("a b"), "'a b'");
        assert_eq!(shell_quote_posix("it's"), "'it'\\''s'");
        assert_eq!(shell_quote_posix(""), "''");
    }

    #[test]
    fn builds_git_command_with_no_pager() {
        assert_eq!(
            build_remote_git_command("/srv/my repo", ["remote", "-v"]),
            "git -C '/srv/my repo' --no-pager remote -v"
        );
    }
}

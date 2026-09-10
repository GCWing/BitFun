//! Repository ownership trust.
//!
//! Git (since 2.35.2) and libgit2 refuse to operate on a repository whose
//! directory owner differs from the current user unless the path is listed in
//! the protected `safe.directory` configuration. That guard is correct, but it
//! surfaces as an opaque command failure: read-only product flows such as
//! Review then look indistinguishable from "this is not a repository".
//!
//! This module keeps the ownership decision explicit and platform-neutral:
//!
//! - classify Git/libgit2 ownership rejections into
//!   [`GitError::RepositoryUntrusted`] so every surface can branch on the code
//!   instead of parsing prose,
//! - report the current trust state without mutating anything, and
//! - apply the trust decision (write `safe.directory`) only when a caller has
//!   an explicit user confirmation, then verify that Git actually accepts the
//!   repository afterwards.
//!
//! Trust is never granted implicitly: nothing in this module is invoked as a
//! fallback of a failed operation.

use super::utils::execute_git_hardened_command_with_env;
use super::GitError;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const REPOSITORY_UNTRUSTED_ERROR_PREFIX: &str =
    crate::repository_trust::REPOSITORY_UNTRUSTED_ERROR_PREFIX;

pub fn is_untrusted_repository_message(message: &str) -> bool {
    crate::repository_trust::is_untrusted_repository_message(message)
}

pub fn normalize_trust_path(raw: &str) -> Option<String> {
    crate::repository_trust::normalize_trust_path(raw)
}

pub fn untrusted_repository_path_from_message(message: &str) -> Option<String> {
    crate::repository_trust::untrusted_repository_path_from_message(message)
}

pub fn untrusted_repository_error_message(repository_path: &str) -> String {
    crate::repository_trust::untrusted_repository_error_message(repository_path)
}

// Repository-level phrasings only. A bare "does not exist" also matches Git
// talking about an object, a ref or a pathspec inside a repository that is
// plainly there ("path 'x' does not exist in 'HEAD'"), which would turn a
// corrupt or unusual failure into "no repository here". A path that genuinely
// is not there is established by looking, not by prose.
const MISSING_REPOSITORY_MARKERS: [&str; 3] = [
    "not a git repository",
    "could not find repository",
    "repository does not exist",
];

/// Whether the given repository path is usable by Git for the current user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitTrustState {
    /// Git accepts the repository as-is.
    Trusted,
    /// The repository exists but Git rejects it on ownership grounds.
    TrustRequired,
    /// No repository was found at the path.
    NotARepository,
}

/// Read-only trust probe result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTrustReport {
    pub state: GitTrustState,
    /// Path Git validates ownership against, when it is known.
    pub repository_path: Option<String>,
    /// Git's own diagnostic, preserved for surfaces that show manual steps.
    pub detail: Option<String>,
    /// Command the user can run to resolve this outside the product.
    pub manual_command: Option<String>,
}

/// Result of an explicitly confirmed trust decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTrustOutcome {
    /// Trust state observed after the decision was applied and verified.
    pub state: GitTrustState,
    pub repository_path: Option<String>,
    /// Whether Git already accepted the repository before this call.
    pub already_trusted: bool,
    /// `safe.directory` entries this call added, in the order they were tried.
    pub added_entries: Vec<String>,
    pub detail: Option<String>,
    pub manual_command: Option<String>,
}

fn contains_any(message: &str, markers: &[&str]) -> bool {
    let lowered = message.to_lowercase();
    markers.iter().any(|marker| lowered.contains(marker))
}

/// Whether a Git/libgit2 diagnostic says no repository is there.
///
/// What is neither this nor [`is_untrusted_repository_message`] is a probe that
/// failed (an unreadable `.git`, a denied path, a broken remote wrapper).
/// Callers must not fold that remainder into "not a repository": that reports a
/// failure to look as an answer, and sends the user to "initialize a
/// repository" for a problem initializing cannot fix.
pub fn is_missing_repository_message(message: &str) -> bool {
    contains_any(message, &MISSING_REPOSITORY_MARKERS)
}

/// The command a user can run themselves when the product cannot apply the
/// decision (remote workspace, peer host, restricted configuration).
///
/// This string exists to be copied into a shell verbatim, so the path cannot go
/// in bare: it can come from another machine, and any `$(...)`, backtick, quote
/// or backslash in it would be the shell's to interpret. It also cannot go in
/// quoted unconditionally — the user's shell is not knowable from here, and
/// single quotes are not quoting characters in `cmd.exe`, which would take them
/// as part of the path and write a `safe.directory` value that never matches.
///
/// So it follows Git's own rule (`sq_quote_buf_pretty` in `quote.c`) for the
/// common case and then goes one step further, because Git only ever quotes for
/// a POSIX shell and this string is most often pasted into a Windows one.
///
/// A path made only of characters no shell can read as syntax is emitted bare —
/// the ordinary Windows and POSIX path, and why Git's own dubious ownership
/// advice prints unquoted. A path that merely needs *grouping* — a space, an
/// apostrophe, a comma, `^` — is double quoted, which `sh`, PowerShell and
/// `cmd.exe` all read the same way, and which is the answer for
/// `C:/Users/John Doe/repo`. Only a path carrying a character that stays live
/// inside double quotes falls back to POSIX single quoting with the `'\''`
/// break-out; that form is correct in POSIX shells alone, but such a path is one
/// we must not hand over unquoted, and a `cmd.exe` user who has one has to
/// re-quote it themselves.
///
/// Two deliberate differences from Git's bare set. `^` and `,` come out: `^` is
/// `cmd.exe`'s escape character, and `,` is PowerShell's array operator, which
/// turns a bare `/srv/a,b` into two arguments rejoined with a space — a
/// `safe.directory` entry that silently never matches. Non-ASCII goes in: Git
/// walks bytes under the C locale, so it quotes every path with a Chinese or
/// Japanese folder name in it, and no shell's syntax lives outside ASCII.
pub fn manual_trust_command(repository_path: &str) -> String {
    format!(
        "git config --global --add safe.directory {}",
        shell_quote_pretty(repository_path)
    )
}

/// ASCII punctuation Git treats as needing no quoting, minus `^` and `,` (see
/// above). None of the rest is syntax in `sh`, PowerShell or `cmd.exe`.
const SHELL_SAFE_PUNCTUATION: &str = "+-./:=@_";

/// Characters that keep their meaning *inside* double quotes in at least one of
/// the three shells: `"` ends the string, `$` and `` ` `` expand in `sh` and
/// PowerShell, `\` escapes in `sh`, `%` and `!` expand in `cmd.exe`.
const DOUBLE_QUOTE_UNSAFE: &[char] = &['"', '$', '`', '\\', '%', '!'];

fn shell_quote_pretty(value: &str) -> String {
    // The non-ASCII allowance is for letters, not for everything outside ASCII:
    // U+00A0, U+2028 and U+3000 are whitespace or line breaks that a shell would
    // split the argument on, and Unicode control characters are invisible in the
    // string we ask the user to paste. Those take the quoted path.
    let bare = !value.is_empty()
        && value.chars().all(|c| {
            (!c.is_ascii() && !c.is_control() && !c.is_whitespace())
                || c.is_ascii_alphanumeric()
                || SHELL_SAFE_PUNCTUATION.contains(c)
        });
    if bare {
        return value.to_string();
    }
    if !value.contains(DOUBLE_QUOTE_UNSAFE) && !value.chars().any(char::is_control) {
        return format!("\"{value}\"");
    }
    shell_single_quote(value)
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

fn untrusted_error(repository_path: Option<String>, detail: impl Into<String>) -> GitError {
    GitError::RepositoryUntrusted {
        repository_path: repository_path.unwrap_or_default(),
        detail: detail.into(),
    }
}

/// Classifies a failed Git CLI invocation, keeping ownership rejections typed.
pub fn classify_command_failure(repo_path: &str, message: String) -> GitError {
    if is_untrusted_repository_message(&message) {
        let repository_path = untrusted_repository_path_from_message(&message)
            .or_else(|| normalize_trust_path(repo_path));
        return untrusted_error(repository_path, message);
    }

    GitError::CommandFailed(message)
}

/// Classifies a failed `libgit2` repository open/discover.
///
/// The remainder — a corrupt object store, an unreadable `.git`, a denied path
/// — is a probe that failed, not an answer. Reporting it as
/// [`GitError::RepositoryNotFound`] is what turns it into "not a Git
/// repository" downstream (`map_git_error` in the worktree service, `NotFound`
/// on the runtime port), which offers the user an initialization that cannot
/// fix any of those. Only a diagnostic that says so, or a path that plainly is
/// not there, gets that verdict.
pub fn classify_repository_open_error(path: &Path, error: &git2::Error) -> GitError {
    let message = error.message().to_string();
    if is_untrusted_repository_message(&message) {
        let repository_path = untrusted_repository_path_from_message(&message)
            .or_else(|| normalize_trust_path(&path.to_string_lossy()));
        return untrusted_error(repository_path, message);
    }

    // `try_exists` rather than `exists`: the latter answers `false` when it
    // could not look (a denied parent directory), which is the very case that
    // must not be called "no repository".
    let definitely_absent = matches!(path.try_exists(), Ok(false));
    if definitely_absent || is_missing_repository_message(&message) {
        return GitError::RepositoryNotFound(error.to_string());
    }

    GitError::CommandFailed(error.to_string())
}

/// Candidate `safe.directory` values for a rejected repository, most precise
/// first. Git names the exact path it validated, so that value is authoritative;
/// the worktree root is only a fallback for the case where the rejection came
/// from the administrative `.git` directory.
fn trust_candidates(repository_path: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(primary) = normalize_trust_path(repository_path) {
        if let Some(worktree) = primary.strip_suffix("/.git") {
            if let Some(parent) = normalize_trust_path(worktree) {
                candidates.push(primary.clone());
                candidates.push(parent);
                return candidates;
            }
        }
        candidates.push(primary);
    }
    candidates
}

/// Runs a Git command that must not depend on the repository under inspection.
async fn run_global_git_command(args: &[&str], env: GitTrustEnv<'_>) -> Result<String, GitError> {
    let working_directory = std::env::temp_dir();
    let working_directory = working_directory.to_string_lossy().to_string();
    execute_git_hardened_command_with_env(&working_directory, args, env).await
}

async fn configured_safe_directories(env: GitTrustEnv<'_>) -> Vec<String> {
    // A missing key exits non-zero; that is "no entries", not a failure.
    let raw = run_global_git_command(&["config", "--global", "--get-all", "safe.directory"], env)
        .await
        .unwrap_or_default();

    raw.lines()
        .filter_map(normalize_trust_path)
        .collect::<Vec<_>>()
}

/// Environment overrides applied to every Git invocation of a trust probe.
/// Production callers pass none; in-crate tests use it to exercise the real Git
/// ownership gate against a throwaway global configuration.
type GitTrustEnv<'a> = &'a [(&'a str, &'a str)];

/// Reports whether Git accepts the repository at `path`, without changing any
/// configuration.
pub async fn inspect_repository_trust(path: &str) -> Result<GitTrustReport, GitError> {
    inspect_repository_trust_with_env(path, &[]).await
}

async fn inspect_repository_trust_with_env(
    path: &str,
    env: GitTrustEnv<'_>,
) -> Result<GitTrustReport, GitError> {
    if path.trim().is_empty() {
        return Err(GitError::InvalidPath(
            "Repository path cannot be empty".to_string(),
        ));
    }
    // `try_exists`, not `exists`: the latter also answers `false` when it could
    // not look — a denied parent directory — and this probe's `false` becomes
    // "not a Git repository" for the caller, offering an initialization that
    // cannot fix a permission problem. Same rule as
    // `classify_repository_open_error`. A path we cannot stat falls through to
    // Git, whose diagnostic is worth more than our guess.
    if matches!(Path::new(path).try_exists(), Ok(false)) {
        return Ok(GitTrustReport {
            state: GitTrustState::NotARepository,
            repository_path: normalize_trust_path(path),
            detail: Some("Path does not exist".to_string()),
            manual_command: None,
        });
    }

    let probe =
        execute_git_hardened_command_with_env(path, &["rev-parse", "--show-toplevel"], env).await;
    classify_toplevel_probe(path, probe)
}

/// Turns one `rev-parse --show-toplevel` outcome into a trust report.
///
/// Split out from the invocation so every branch — including the ones a real
/// local Git will not produce on demand — is reachable from a test. The remote
/// probe in the desktop app classifies the same four shapes; the two must not
/// drift apart.
fn classify_toplevel_probe(
    path: &str,
    probe: Result<String, GitError>,
) -> Result<GitTrustReport, GitError> {
    match probe {
        // A `--show-toplevel` that exits zero with nothing to say did not
        // answer. Calling that "trusted" manufactures a pass out of a broken
        // transport or a wrapper that swallowed the output; the remote probe
        // already refuses to, and the local one must not disagree.
        Ok(output) if output.trim().is_empty() => Err(GitError::CommandFailed(
            "git rev-parse --show-toplevel reported success without naming a repository"
                .to_string(),
        )),
        Ok(output) => Ok(GitTrustReport {
            state: GitTrustState::Trusted,
            repository_path: normalize_trust_path(&output).or_else(|| normalize_trust_path(path)),
            detail: None,
            manual_command: None,
        }),
        Err(error) => {
            // Two shapes of the same wall: the typed ownership rejection every
            // in-crate executor produces, and — defensively — a raw command
            // failure whose prose still carries the markers. Both resolve to
            // the same report.
            let (repository_path, detail) = match error {
                GitError::RepositoryUntrusted {
                    repository_path,
                    detail,
                } => (
                    normalize_trust_path(&repository_path)
                        .or_else(|| untrusted_repository_path_from_message(&detail))
                        .or_else(|| normalize_trust_path(path)),
                    detail,
                ),
                GitError::CommandFailed(message) if is_untrusted_repository_message(&message) => (
                    untrusted_repository_path_from_message(&message)
                        .or_else(|| normalize_trust_path(path)),
                    message,
                ),
                GitError::CommandFailed(message) if is_missing_repository_message(&message) => {
                    return Ok(GitTrustReport {
                        state: GitTrustState::NotARepository,
                        repository_path: normalize_trust_path(path),
                        detail: Some(message),
                        manual_command: None,
                    })
                }
                other => return Err(other),
            };
            let manual_command = repository_path.as_deref().map(manual_trust_command);
            Ok(GitTrustReport {
                state: GitTrustState::TrustRequired,
                repository_path,
                detail: Some(detail),
                manual_command,
            })
        }
    }
}

/// Whether a configured `safe.directory` entry already covers the candidate.
/// Git compares entries case-insensitively on Windows, so the dedupe check
/// must too — otherwise a grant appends a duplicate that differs only by
/// drive-letter casing.
fn safe_directory_entry_matches(existing: &str, candidate: &str) -> bool {
    if cfg!(windows) {
        existing.eq_ignore_ascii_case(candidate)
    } else {
        existing == candidate
    }
}

/// Applies an explicitly confirmed trust decision for `path`.
///
/// Callers must obtain user confirmation first: this writes to the user's
/// global Git configuration, which is exactly the exception Git asks the user
/// to make deliberately. The write is idempotent, is skipped when the entry is
/// already present, and is always verified by re-probing the repository, so a
/// decision that does not actually resolve the rejection is reported instead of
/// silently assumed.
pub async fn trust_repository(path: &str) -> Result<GitTrustOutcome, GitError> {
    trust_repository_with_env(path, &[]).await
}

async fn trust_repository_with_env(
    path: &str,
    env: GitTrustEnv<'_>,
) -> Result<GitTrustOutcome, GitError> {
    let report = inspect_repository_trust_with_env(path, env).await?;
    match report.state {
        GitTrustState::Trusted => {
            return Ok(GitTrustOutcome {
                state: GitTrustState::Trusted,
                repository_path: report.repository_path,
                already_trusted: true,
                added_entries: Vec::new(),
                detail: None,
                manual_command: None,
            })
        }
        GitTrustState::NotARepository => {
            return Err(GitError::RepositoryNotFound(
                report
                    .detail
                    .unwrap_or_else(|| format!("No Git repository at {path}")),
            ))
        }
        GitTrustState::TrustRequired => {}
    }

    let rejected_path = report
        .repository_path
        .clone()
        .or_else(|| normalize_trust_path(path))
        .ok_or_else(|| GitError::InvalidPath("Repository path cannot be empty".to_string()))?;
    let existing = configured_safe_directories(env).await;
    let mut added_entries = Vec::new();
    let mut last_report = report;
    let mut skipped_pattern = false;
    let mut attempted = false;

    for candidate in trust_candidates(&rejected_path) {
        // Never write a value Git reads as a pattern: the user consented to one
        // directory, and the entry would grant a whole tree.
        if is_pattern_safe_directory(&candidate) {
            skipped_pattern = true;
            continue;
        }
        attempted = true;
        if !existing
            .iter()
            .any(|entry| safe_directory_entry_matches(entry, &candidate))
        {
            run_global_git_command(
                &["config", "--global", "--add", "safe.directory", &candidate],
                env,
            )
            .await?;
            added_entries.push(candidate.clone());
        }

        last_report = inspect_repository_trust_with_env(path, env).await?;
        if last_report.state == GitTrustState::Trusted {
            return Ok(GitTrustOutcome {
                state: GitTrustState::Trusted,
                repository_path: last_report.repository_path,
                already_trusted: false,
                added_entries,
                detail: None,
                manual_command: None,
            });
        }
    }

    // Trust could not be established (for example a read-only global config or
    // a rejection Git reports against a path we cannot express). Report it
    // loudly with the exact manual step instead of pretending it worked.
    //
    // A path Git can only read as a pattern still gets the manual step — that is
    // the command Git's own advice prints, and the probe would report it anyway.
    // What it also gets is a `detail` saying what the entry would actually do,
    // because there is no escaping in `safe.directory`: the value cannot name
    // this repository alone, and applying it silently is what this refuses.
    let unexpressible = skipped_pattern && !attempted;
    Ok(GitTrustOutcome {
        state: last_report.state,
        repository_path: last_report.repository_path.clone(),
        already_trusted: false,
        added_entries,
        detail: if unexpressible {
            Some(format!(
                "Git reads '{rejected_path}' as a safe.directory pattern, not as a literal path, \
                 so no entry can trust this repository alone — the value below would trust every \
                 repository the pattern covers. Rename or move the directory instead."
            ))
        } else {
            last_report.detail
        },
        manual_command: last_report
            .repository_path
            .as_deref()
            .or(Some(rejected_path.as_str()))
            .map(manual_trust_command),
    })
}

/// True when Git would read the value as a pattern rather than as the literal
/// directory it names.
///
/// `safe.directory = *` disables the ownership check for every repository, and a
/// value ending in `/*` trusts everything beneath that prefix. Both are legal
/// directory names on POSIX, so a repository at `/tmp/*` would silently turn one
/// "trust this folder" into a blanket grant over `/tmp`. `%(prefix)/` is Git's
/// runtime-prefix interpolation, which resolves to a different path than the one
/// probed.
fn is_pattern_safe_directory(candidate: &str) -> bool {
    candidate == "*" || candidate.ends_with("/*") || candidate.starts_with("%(prefix)/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLI_REJECTION: &str = "fatal: detected dubious ownership in repository at 'C:/work/repo'\n'C:/work/repo' is owned by:\n\t'S-1-5-21-1'\nbut the current user is:\n\t'S-1-5-21-2'\nTo add an exception for this directory, call:\n\n\tgit config --global --add safe.directory 'C:/work/repo'";
    const LIBGIT2_REJECTION: &str =
        "repository path '/srv/shared/repo/.git/' is not owned by current user";
    const POSIX_CLI_REJECTION: &str =
        "fatal: detected dubious ownership in repository at '/srv/shared/repo'";

    /// Every interface builds this string from the same producer, and each one
    /// carries the rejected path so a frontend can name the folder without a
    /// second round trip.
    #[test]
    fn the_boundary_error_string_carries_the_rejected_path() {
        let message = untrusted_repository_error_message("/srv/shared/repo");

        assert!(message.starts_with(REPOSITORY_UNTRUSTED_ERROR_PREFIX));
        assert_eq!(
            message[REPOSITORY_UNTRUSTED_ERROR_PREFIX.len()..].trim(),
            "/srv/shared/repo"
        );
    }

    #[test]
    fn recognizes_cli_and_libgit2_ownership_rejections() {
        assert!(is_untrusted_repository_message(CLI_REJECTION));
        assert!(is_untrusted_repository_message(LIBGIT2_REJECTION));
        assert!(is_untrusted_repository_message(POSIX_CLI_REJECTION));
        assert!(!is_untrusted_repository_message(
            "fatal: not a git repository (or any of the parent directories): .git"
        ));
    }

    #[test]
    fn extracts_the_repository_path_git_rejected() {
        assert_eq!(
            untrusted_repository_path_from_message(CLI_REJECTION).as_deref(),
            Some("C:/work/repo")
        );
        assert_eq!(
            untrusted_repository_path_from_message(POSIX_CLI_REJECTION).as_deref(),
            Some("/srv/shared/repo")
        );
        assert_eq!(
            untrusted_repository_path_from_message(LIBGIT2_REJECTION).as_deref(),
            Some("/srv/shared/repo/.git")
        );
        assert_eq!(
            untrusted_repository_path_from_message("fatal: not a git repository"),
            None
        );
    }

    /// Git does not escape the quote characters inside the path it prints, so a
    /// reader that stops at the first closing quote invents a shorter path —
    /// and that path outranks the caller's, gets written to the user's global
    /// config, and never resolves the rejection it was written for.
    #[test]
    fn keeps_a_quote_that_belongs_to_the_directory_name() {
        assert_eq!(
            untrusted_repository_path_from_message(
                "fatal: detected dubious ownership in repository at '/srv/o'brien/repo'"
            )
            .as_deref(),
            Some("/srv/o'brien/repo")
        );
        assert_eq!(
            untrusted_repository_path_from_message(
                "repository path '/srv/a'b/repo/' is not owned by current user"
            )
            .as_deref(),
            Some("/srv/a'b/repo")
        );
    }

    /// The CLI repeats the path in its advice block. Reading the whole message
    /// instead of the rejection line makes the result depend on how much of
    /// that block survived, and the advice line ends with a quoted path of its
    /// own.
    #[test]
    fn reads_the_path_from_the_rejection_line_not_the_advice_block() {
        let message = concat!(
            "fatal: detected dubious ownership in repository at '/srv/shared/repo'\n",
            "'/srv/shared/repo' is owned by:\n",
            "    uid 0\n",
            "but the current user is:\n",
            "    uid 1000\n",
            "To add an exception for this directory, call:\n\n",
            "    git config --global --add safe.directory '/srv/shared/repo'\n",
        );
        assert_eq!(
            untrusted_repository_path_from_message(message).as_deref(),
            Some("/srv/shared/repo")
        );
    }

    #[test]
    fn normalizes_paths_into_the_shape_git_compares() {
        assert_eq!(
            normalize_trust_path("\\\\?\\C:\\work\\repo\\").as_deref(),
            Some("C:/work/repo")
        );
        assert_eq!(normalize_trust_path("C:/").as_deref(), Some("C:/"));
        assert_eq!(normalize_trust_path("   ").as_deref(), None);
    }

    /// Backslash is an ordinary filename character on POSIX. Rewriting it there
    /// produces a `safe.directory` entry that can never match and a manual
    /// command naming a directory that does not exist — so the rewrite is gated
    /// on the path shapes only Windows produces, not on the host we run on.
    #[test]
    fn leaves_a_posix_backslash_inside_the_directory_name() {
        assert_eq!(
            normalize_trust_path("/srv/we\\ird/repo").as_deref(),
            Some("/srv/we\\ird/repo")
        );
        assert_eq!(
            normalize_trust_path("/srv/trailing\\").as_deref(),
            Some("/srv/trailing\\")
        );
        assert_eq!(
            normalize_trust_path("C:\\work\\repo").as_deref(),
            Some("C:/work/repo")
        );
    }

    /// `safe.directory` has no escaping: `*` disables the ownership check
    /// everywhere and a trailing `/*` trusts the whole tree beneath it. A
    /// directory may legally be named `*` on POSIX, so writing the entry the
    /// dialog implies would grant far more than the user approved.
    #[test]
    fn refuses_to_write_a_safe_directory_value_git_reads_as_a_pattern() {
        assert!(is_pattern_safe_directory("*"));
        assert!(is_pattern_safe_directory("/tmp/*"));
        assert!(is_pattern_safe_directory("%(prefix)/srv/repo"));
        assert!(!is_pattern_safe_directory("/tmp/*/repo"));
        assert!(!is_pattern_safe_directory("/tmp/*/.git"));
        assert!(!is_pattern_safe_directory("/srv/shared/repo"));
        // The gitdir of a repository named `*` still names one repository, so
        // the recovery is not lost — only the blanket entry is.
        assert_eq!(
            trust_candidates("/tmp/*"),
            vec!["/tmp/*".to_string()],
            "the candidate list itself is unchanged; the grant loop is what skips it"
        );
    }

    /// `\\?\UNC\server\share` is the extended-length spelling of
    /// `\\server\share`. Dropping the whole prefix leaves `UNC/server/share`,
    /// a path that exists nowhere — so both the grant and the manual command
    /// we print would name the wrong folder.
    #[test]
    fn rewrites_the_extended_length_spelling_of_a_unc_path() {
        assert_eq!(
            normalize_trust_path("\\\\?\\UNC\\build01\\shared\\repo").as_deref(),
            Some("//build01/shared/repo")
        );
        assert_eq!(
            normalize_trust_path("\\\\?\\unc\\build01\\shared\\repo\\").as_deref(),
            Some("//build01/shared/repo")
        );
        assert_eq!(
            normalize_trust_path("\\\\build01\\shared\\repo").as_deref(),
            Some("//build01/shared/repo")
        );
    }

    /// A probe that failed is not an answer. Calling a corrupt object store or
    /// a denied path "not a Git repository" is what offers the user an
    /// initialization that cannot fix either.
    #[test]
    fn keeps_an_inconclusive_open_failure_out_of_not_a_repository() {
        let existing = std::env::temp_dir();
        let corrupt = git2::Error::from_str("failed to parse object header");
        assert!(matches!(
            classify_repository_open_error(&existing, &corrupt),
            GitError::CommandFailed(_)
        ));

        let missing = existing.join("openbitfun-trust-test-absent-directory");
        assert!(matches!(
            classify_repository_open_error(&missing, &corrupt),
            GitError::RepositoryNotFound(_)
        ));

        let diagnosed = git2::Error::from_str(
            "could not find repository at '/srv/shared/repo'; class=Repository",
        );
        assert!(matches!(
            classify_repository_open_error(&existing, &diagnosed),
            GitError::RepositoryNotFound(_)
        ));

        let rejected = git2::Error::from_str(LIBGIT2_REJECTION);
        assert!(matches!(
            classify_repository_open_error(&existing, &rejected),
            GitError::RepositoryUntrusted { .. }
        ));
    }

    #[test]
    fn falls_back_to_the_worktree_root_for_a_rejected_git_directory() {
        assert_eq!(
            trust_candidates("/srv/shared/repo/.git"),
            vec![
                "/srv/shared/repo/.git".to_string(),
                "/srv/shared/repo".to_string()
            ]
        );
        assert_eq!(
            trust_candidates("/srv/shared/repo"),
            vec!["/srv/shared/repo".to_string()]
        );
    }

    #[test]
    fn classifies_command_failures_without_losing_the_diagnostic() {
        let error = classify_command_failure("C:/work/repo", CLI_REJECTION.to_string());
        assert!(matches!(&error, GitError::RepositoryUntrusted { .. }));
        assert_eq!(error.untrusted_repository_path(), Some("C:/work/repo"));

        let other = classify_command_failure("C:/work/repo", "fatal: bad revision".to_string());
        assert!(matches!(&other, GitError::CommandFailed(_)));
        assert_eq!(other.untrusted_repository_path(), None);
    }

    /// The manual command exists to be pasted into a shell, so a path is data
    /// there, never syntax. A repository path can arrive from another machine.
    #[test]
    fn quotes_the_path_in_the_manual_command_so_a_shell_cannot_read_it_as_syntax() {
        // An ordinary path is syntax in no shell, so it goes in bare — the
        // spelling Git's own advice prints, and the only one `cmd.exe` reads
        // correctly, since it does not strip single quotes.
        assert_eq!(
            manual_trust_command("/srv/shared/repo"),
            "git config --global --add safe.directory /srv/shared/repo"
        );
        assert_eq!(
            manual_trust_command("C:/work/repo"),
            "git config --global --add safe.directory C:/work/repo"
        );

        // Command substitution, a backtick, a double quote and a backslash are
        // all literal inside single quotes — nothing here may execute or vanish.
        let hostile = manual_trust_command(r#"/srv/$(id)/`whoami`/a"b\c"#);
        assert_eq!(
            hostile,
            r#"git config --global --add safe.directory '/srv/$(id)/`whoami`/a"b\c'"#
        );

        // A space is the everyday reason to quote, and `C:/Users/John Doe/repo`
        // is an everyday Windows path — so it gets the one quoting all three
        // shells strip. Single quotes would have become part of the value on
        // `cmd.exe`: a command that succeeds and grants nothing.
        assert_eq!(
            manual_trust_command("C:/Users/a b/repo"),
            r#"git config --global --add safe.directory "C:/Users/a b/repo""#
        );

        // An apostrophe is literal inside double quotes everywhere, so this no
        // longer needs the POSIX-only `'\''` break-out.
        assert_eq!(
            manual_trust_command("/srv/o'brien/repo"),
            r#"git config --global --add safe.directory "/srv/o'brien/repo""#
        );

        // `^` is `cmd.exe`'s escape character and `,` is PowerShell's array
        // operator — bare, either one silently rewrites the path. Both are inert
        // once quoted.
        assert_eq!(
            manual_trust_command("/srv/a^b"),
            r#"git config --global --add safe.directory "/srv/a^b""#
        );
        assert_eq!(
            manual_trust_command("D:/Reports, 2024/repo"),
            r#"git config --global --add safe.directory "D:/Reports, 2024/repo""#
        );

        // Nothing at all must not collapse into a bare, invisible argument.
        assert_eq!(
            manual_trust_command(""),
            r#"git config --global --add safe.directory """#
        );

        // The everyday Windows path here: Git would quote it, and the quotes
        // would then be part of the value on `cmd.exe`. Nothing outside ASCII
        // is syntax in any shell, so it stays bare.
        assert_eq!(
            manual_trust_command("D:/工作区/仓库"),
            "git config --global --add safe.directory D:/工作区/仓库"
        );

        // The non-ASCII allowance is for letters. An ideographic space is a
        // space: bare, the shell would split the argument on it and grant a
        // path that stops short. A no-break space is worse — it is invisible.
        assert_eq!(
            manual_trust_command("D:/工作\u{3000}区/仓库"),
            "git config --global --add safe.directory \"D:/工作\u{3000}区/仓库\""
        );
        assert_eq!(
            manual_trust_command("/srv/a\u{00a0}b/repo"),
            "git config --global --add safe.directory \"/srv/a\u{00a0}b/repo\""
        );
        // A Unicode control character cannot be shown at all, so it takes the
        // strictest form we have rather than riding along inside double quotes.
        assert_eq!(
            manual_trust_command("/srv/a\u{0085}b/repo"),
            "git config --global --add safe.directory '/srv/a\u{0085}b/repo'"
        );
    }

    /// "does not exist" on its own is Git talking about an object, a ref or a
    /// pathspec just as often as about a repository. Matching it turns a
    /// perfectly present repository into an offer to initialize one.
    #[test]
    fn reads_only_repository_level_prose_as_a_missing_repository() {
        assert!(is_missing_repository_message(
            "fatal: not a git repository (or any of the parent directories): .git"
        ));
        assert!(is_missing_repository_message(
            "could not find repository at '/srv/shared/repo'"
        ));
        assert!(is_missing_repository_message(
            "fatal: repository does not exist"
        ));

        assert!(!is_missing_repository_message(
            "fatal: path 'src/main.rs' does not exist in 'HEAD'"
        ));
        assert!(!is_missing_repository_message(
            "fatal: Not a valid object name: 'refs/heads/main' does not exist"
        ));
        assert!(!is_missing_repository_message(
            "error: object file .git/objects/ab/cdef is empty"
        ));
    }

    #[test]
    fn classifies_every_shape_a_toplevel_probe_can_return() {
        let trusted = classify_toplevel_probe("/srv/shared/repo", Ok("/srv/shared/repo\n".into()))
            .expect("trust report");
        assert_eq!(trusted.state, GitTrustState::Trusted);
        assert_eq!(trusted.repository_path.as_deref(), Some("/srv/shared/repo"));

        // An exit-zero that names no repository did not answer. Calling it
        // "trusted" manufactures a pass out of a broken transport or a wrapper
        // that swallowed the output; the remote probe already refuses to.
        assert!(matches!(
            classify_toplevel_probe("/srv/shared/repo", Ok("   \n".into())),
            Err(GitError::CommandFailed(_))
        ));

        let rejected = classify_toplevel_probe(
            "C:/work/repo",
            Err(GitError::CommandFailed(CLI_REJECTION.to_string())),
        )
        .expect("trust report");
        assert_eq!(rejected.state, GitTrustState::TrustRequired);
        assert_eq!(rejected.repository_path.as_deref(), Some("C:/work/repo"));
        assert_eq!(
            rejected.manual_command.as_deref(),
            Some("git config --global --add safe.directory C:/work/repo")
        );

        let absent = classify_toplevel_probe(
            "/srv/shared/nowhere",
            Err(GitError::CommandFailed(
                "fatal: not a git repository (or any of the parent directories): .git".into(),
            )),
        )
        .expect("trust report");
        assert_eq!(absent.state, GitTrustState::NotARepository);

        // Anything else is a probe that failed, not a verdict about the
        // repository — it must stay an error rather than become "no repository
        // here" and offer an initialization that cannot fix it.
        assert!(matches!(
            classify_toplevel_probe(
                "/srv/shared/repo",
                Err(GitError::CommandFailed(
                    "error: object file .git/objects/ab/cdef is empty".into()
                ))
            ),
            Err(GitError::CommandFailed(_))
        ));
    }

    #[tokio::test]
    async fn reports_a_normal_repository_as_trusted() {
        let temp = tempfile::tempdir().expect("tempdir");
        git2::Repository::init(temp.path()).expect("repository");

        let report = inspect_repository_trust(&temp.path().to_string_lossy())
            .await
            .expect("trust report");
        assert_eq!(report.state, GitTrustState::Trusted);
        assert!(report.repository_path.is_some());
        assert!(report.manual_command.is_none());
    }

    #[tokio::test]
    async fn reports_a_plain_directory_as_not_a_repository() {
        let temp = tempfile::tempdir().expect("tempdir");

        let report = inspect_repository_trust(&temp.path().to_string_lossy())
            .await
            .expect("trust report");
        assert_eq!(report.state, GitTrustState::NotARepository);
    }

    /// Drives the real Git ownership gate: `GIT_TEST_ASSUME_DIFFERENT_OWNER`
    /// makes Git reject the repository exactly as it does for a foreign owner,
    /// and `GIT_CONFIG_GLOBAL` keeps the granted exception inside the test.
    #[tokio::test]
    async fn grants_and_verifies_trust_against_the_real_ownership_gate() {
        let repository = tempfile::tempdir().expect("repository tempdir");
        let config_home = tempfile::tempdir().expect("config tempdir");
        let global_config = config_home.path().join("gitconfig");
        let global_config = global_config.to_string_lossy().to_string();
        git2::Repository::init(repository.path()).expect("repository");
        let path = repository.path().to_string_lossy().to_string();
        let env = [
            ("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1"),
            ("GIT_CONFIG_GLOBAL", global_config.as_str()),
            ("GIT_CONFIG_NOSYSTEM", "1"),
        ];

        let report = inspect_repository_trust_with_env(&path, &env)
            .await
            .expect("trust report");
        if report.state != GitTrustState::TrustRequired {
            eprintln!(
                "skipping: installed Git does not honor GIT_TEST_ASSUME_DIFFERENT_OWNER (state={:?})",
                report.state
            );
            return;
        }
        assert!(report.manual_command.is_some());

        let outcome = trust_repository_with_env(&path, &env)
            .await
            .expect("trust outcome");
        assert_eq!(outcome.state, GitTrustState::Trusted);
        assert!(!outcome.already_trusted);
        assert_eq!(outcome.added_entries.len(), 1);
        assert!(outcome.manual_command.is_none());

        // The exception is written to the throwaway global config only.
        let written = std::fs::read_to_string(&global_config).expect("global config");
        assert!(written.contains("[safe]"));
        assert!(written.contains(&outcome.added_entries[0]));

        let repeated = trust_repository_with_env(&path, &env)
            .await
            .expect("repeat trust outcome");
        assert!(repeated.already_trusted);
        assert!(repeated.added_entries.is_empty());
    }

    #[tokio::test]
    async fn trusting_an_already_trusted_repository_changes_nothing() {
        let temp = tempfile::tempdir().expect("tempdir");
        git2::Repository::init(temp.path()).expect("repository");

        let outcome = trust_repository(&temp.path().to_string_lossy())
            .await
            .expect("trust outcome");
        assert_eq!(outcome.state, GitTrustState::Trusted);
        assert!(outcome.already_trusted);
        assert!(outcome.added_entries.is_empty());
    }
}

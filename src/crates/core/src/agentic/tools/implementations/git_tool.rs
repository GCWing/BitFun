//! Git tool implementation - reuses GitService implementation
//!
//! Provides safe and convenient Git command execution functionality, reuses underlying GitService

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::service::git::{
    execute_git_command, execute_git_command_raw, GitAddParams, GitCommitParams, GitDiffParams,
    GitLogParams, GitPullParams, GitPushParams, GitService,
};
use crate::util::elapsed_ms_u64;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::debug;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Constants for git diff argument parsing
// ---------------------------------------------------------------------------

/// Separator between refs and file paths in git diff commands.
const GIT_DIFF_FILE_SEPARATOR: &str = " -- ";

/// Two-dot range separator (symmetric difference).
const RANGE_TWO_DOT: &str = "..";

/// Three-dot range separator (merge base).
const RANGE_THREE_DOT: &str = "...";

/// Known diff flags that should be excluded when extracting commit refs.
const DIFF_FLAGS: &[&str] = &["--staged", "--cached", "--stat"];

/// Prefix for short flags (e.g. `-p`, `-U5`).
const SHORT_FLAG_PREFIX: &str = "-";

/// Allowed Git operation types
const ALLOWED_OPERATIONS: &[&str] = &[
    "status",      // View working tree status
    "diff",        // View differences
    "log",         // View commit history
    "add",         // Add files to staging area
    "commit",      // Commit changes
    "branch",      // Branch operations
    "checkout",    // Switch branches
    "switch",      // Switch branches (new syntax)
    "pull",        // Pull remote changes
    "push",        // Push to remote
    "fetch",       // Fetch remote updates
    "merge",       // Merge branches
    "rebase",      // Rebase operations
    "stash",       // Stash changes
    "reset",       // Reset changes
    "restore",     // Restore files
    "show",        // Show objects
    "tag",         // Tag operations
    "remote",      // Remote repository operations
    "clone",       // Clone repository
    "init",        // Initialize repository
    "blame",       // View file history
    "cherry-pick", // Cherry-pick commits
    "rev-parse",   // Parse references
    "describe",    // Describe version
    "shortlog",    // Short log
    "clean",       // Clean working directory
];

/// Dangerous Git operations (require special warning)
const DANGEROUS_OPERATIONS: &[&str] = &["push --force", "reset --hard", "clean -fd", "rebase"];

/// Parsed result of a `git diff` args string.
#[derive(Debug, PartialEq)]
struct ParsedDiffArgs {
    staged: bool,
    stat: bool,
    source: Option<String>,
    target: Option<String>,
    files: Option<Vec<String>>,
}

impl Default for ParsedDiffArgs {
    fn default() -> Self {
        Self {
            staged: false,
            stat: false,
            source: None,
            target: None,
            files: None,
        }
    }
}

/// Git tool
pub struct GitTool;

impl GitTool {
    pub fn new() -> Self {
        Self
    }

    /// Check if operation is dangerous
    fn is_dangerous_operation(operation: &str, args: &str) -> bool {
        let full_cmd = format!("{} {}", operation, args);
        DANGEROUS_OPERATIONS
            .iter()
            .any(|&danger| full_cmd.contains(danger))
    }

    fn sh_quote(s: &str) -> String {
        format!("'{}'", s.replace('\'', "'\\''"))
    }

    /// Resolve repository root: workspace root or a path resolved with the same rules as file tools
    /// (POSIX on remote SSH).
    fn get_repo_path(
        working_directory: Option<&str>,
        context: &ToolUseContext,
    ) -> BitFunResult<String> {
        if let Some(dir) = working_directory {
            let trimmed = dir.trim();
            if trimmed.is_empty() {
                return context
                    .workspace
                    .as_ref()
                    .map(|w| w.root_path_string())
                    .ok_or_else(|| BitFunError::tool("No workspace path available".to_string()));
            }
            context.resolve_workspace_tool_path(trimmed)
        } else {
            context
                .workspace
                .as_ref()
                .map(|w| w.root_path_string())
                .ok_or_else(|| BitFunError::tool("No workspace path available".to_string()))
        }
    }

    /// Run `git` on the remote host over SSH (same environment as native CLI on the server).
    async fn execute_remote_git_cli(
        repo_path: &str,
        operation: &str,
        args: Option<&str>,
        context: &ToolUseContext,
    ) -> BitFunResult<Value> {
        let shell = context.ws_shell().ok_or_else(|| {
            BitFunError::tool("Remote Git requires workspace shell (SSH)".to_string())
        })?;

        let args_str = args.unwrap_or("").trim();
        let cmd = if args_str.is_empty() {
            format!(
                "git --no-pager -C {} {}",
                Self::sh_quote(repo_path),
                operation
            )
        } else {
            format!(
                "git --no-pager -C {} {} {}",
                Self::sh_quote(repo_path),
                operation,
                args_str
            )
        };

        let (stdout, stderr, exit_code) = shell
            .exec(&cmd, Some(180_000))
            .await
            .map_err(|e| BitFunError::tool(format!("Remote git failed: {}", e)))?;

        Ok(json!({
            "success": exit_code == 0,
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
            "command": cmd,
            "remote_execution": true,
        }))
    }

    /// Execute status operation using GitService
    async fn execute_status(repo_path: &str) -> BitFunResult<Value> {
        let status = GitService::get_status(repo_path)
            .await
            .map_err(|e| BitFunError::tool(format!("Git status failed: {}", e)))?;

        // Build output text
        let mut output_lines = vec![];
        output_lines.push(format!("On branch {}", status.current_branch));

        if status.ahead > 0 || status.behind > 0 {
            output_lines.push(format!(
                "Your branch is {} ahead, {} behind",
                status.ahead, status.behind
            ));
        }

        if !status.staged.is_empty() {
            output_lines.push("\nChanges to be committed:".to_string());
            for file in &status.staged {
                output_lines.push(format!("  {}: {}", file.status, file.path));
            }
        }

        if !status.unstaged.is_empty() {
            output_lines.push("\nChanges not staged for commit:".to_string());
            for file in &status.unstaged {
                output_lines.push(format!("  {}: {}", file.status, file.path));
            }
        }

        if !status.untracked.is_empty() {
            output_lines.push("\nUntracked files:".to_string());
            for file in &status.untracked {
                output_lines.push(format!("  {}", file));
            }
        }

        if status.staged.is_empty() && status.unstaged.is_empty() && status.untracked.is_empty() {
            output_lines.push("nothing to commit, working tree clean".to_string());
        }

        Ok(json!({
            "success": true,
            "exit_code": 0,
            "stdout": output_lines.join("\n"),
            "stderr": "",
            "data": status
        }))
    }

    /// Parse a `git diff` args string into structured [`ParsedDiffArgs`].
    ///
    /// Supported patterns:
    /// - `HEAD~7..HEAD --stat` → source=HEAD~7, target=HEAD, stat=true
    /// - `HEAD --stat -- src/foo.rs` → source=HEAD, stat=true, files=[src/foo.rs]
    /// - `--staged` → staged=true
    /// - `origin/main...HEAD` → source=origin/main, target=HEAD (three-dot)
    fn parse_diff_args(args_str: &str) -> ParsedDiffArgs {
        let mut result = ParsedDiffArgs::default();

        result.staged = args_str.contains("--staged") || args_str.contains("--cached");
        result.stat = args_str.contains("--stat");

        // Split on " -- " to separate options/refs from file paths
        let (refs_part, files_part) = if let Some(sep_pos) = args_str.find(GIT_DIFF_FILE_SEPARATOR)
        {
            let refs = args_str[..sep_pos].trim();
            let files = args_str[sep_pos + GIT_DIFF_FILE_SEPARATOR.len()..].trim();
            (refs, Some(files))
        } else if let Some(stripped) = args_str.strip_prefix("-- ") {
            // Handle "-- file1 file2" (no leading space before --)
            ("", Some(stripped.trim()))
        } else {
            (args_str.trim(), None)
        };

        // Extract non-flag tokens from refs_part as commit references
        let ref_tokens: Vec<&str> = refs_part
            .split_whitespace()
            .filter(|token| {
                !DIFF_FLAGS.iter().any(|flag| token == flag)
                    && !token.starts_with(SHORT_FLAG_PREFIX)
            })
            .collect();

        let refs_text = if ref_tokens.len() == 1 {
            ref_tokens[0]
        } else if ref_tokens.len() >= 2 {
            // Re-join multi-token refs so spaces inside refs are preserved
            &ref_tokens.join(" ")
        } else {
            ""
        };

        if !refs_text.is_empty() {
            let (src, tgt) = Self::split_range(refs_text);
            result.source = src;
            result.target = tgt;
        }

        result.files = files_part.map(|fp| {
            fp.split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        });

        result
    }

    /// Split a ref expression on the first `..` or `...` range separator.
    ///
    /// Returns `(Some(source), Some(target))` when both sides are non-empty,
    /// otherwise falls back to treating the whole text as a single source.
    fn split_range(text: &str) -> (Option<String>, Option<String>) {
        let (sep_len, pos) = if let Some(p) = text.find(RANGE_THREE_DOT) {
            (RANGE_THREE_DOT.len(), p)
        } else if let Some(p) = text.find(RANGE_TWO_DOT) {
            (RANGE_TWO_DOT.len(), p)
        } else {
            return (Some(text.to_string()), None);
        };

        let src = text[..pos].trim();
        let tgt = text[pos + sep_len..].trim();

        match (src.is_empty(), tgt.is_empty()) {
            (false, false) => (Some(src.to_string()), Some(tgt.to_string())),
            (false, true) => (Some(src.to_string()), None),
            (true, false) => (None, Some(tgt.to_string())),
            (true, true) => (None, None),
        }
    }

    /// Execute diff operation using GitService
    async fn execute_diff(repo_path: &str, args: Option<&str>) -> BitFunResult<Value> {
        let parsed = Self::parse_diff_args(args.unwrap_or(""));

        let params = GitDiffParams {
            staged: Some(parsed.staged),
            stat: Some(parsed.stat),
            source: parsed.source,
            target: parsed.target,
            files: parsed.files,
        };

        let diff_output = GitService::get_diff(repo_path, &params)
            .await
            .map_err(|e| BitFunError::tool(format!("Git diff failed: {}", e)))?;

        // When there are no differences, git diff returns exit code 0 with an
        // empty stdout. Return a friendly message so the model (and user) see
        // a clear "no changes" indication instead of a bare empty string.
        let stdout = if diff_output.trim().is_empty() {
            "No differences found.".to_string()
        } else {
            diff_output
        };

        Ok(json!({
            "success": true,
            "exit_code": 0,
            "stdout": stdout,
            "stderr": ""
        }))
    }

    /// Execute log operation using GitService
    async fn execute_log(repo_path: &str, args: Option<&str>) -> BitFunResult<Value> {
        let args_str = args.unwrap_or("");

        // Parse parameters
        let mut max_count = 10;
        let oneline = args_str.contains("--oneline");
        let stat = args_str.contains("--stat");

        // Parse -n or -number
        if let Some(pos) = args_str.find("-n") {
            if let Some(num_str) = args_str
                .get(pos + 2..)
                .and_then(|s| s.split_whitespace().next())
            {
                if let Ok(n) = num_str.trim().parse::<i32>() {
                    max_count = n;
                }
            }
        } else if let Some(pos) = args_str.find('-') {
            if let Some(num_str) = args_str
                .get(pos + 1..)
                .and_then(|s| s.split_whitespace().next())
            {
                if let Ok(n) = num_str.parse::<i32>() {
                    max_count = n;
                }
            }
        }

        let params = GitLogParams {
            max_count: Some(max_count),
            stat: Some(stat),
            ..Default::default()
        };

        let commits = GitService::get_commits(repo_path, params)
            .await
            .map_err(|e| BitFunError::tool(format!("Git log failed: {}", e)))?;

        // Build output
        let output_lines: Vec<String> = commits
            .iter()
            .map(|c| {
                if oneline {
                    format!(
                        "{} {}",
                        c.short_hash,
                        c.message.lines().next().unwrap_or("")
                    )
                } else {
                    format!(
                        "commit {}\nAuthor: {} <{}>\nDate:   {}\n\n    {}\n",
                        c.hash, c.author, c.author_email, c.date, c.message
                    )
                }
            })
            .collect();

        Ok(json!({
            "success": true,
            "exit_code": 0,
            "stdout": output_lines.join(if oneline { "\n" } else { "" }),
            "stderr": "",
            "data": commits
        }))
    }

    /// Execute add operation using GitService
    async fn execute_add(repo_path: &str, args: Option<&str>) -> BitFunResult<Value> {
        let args_str = args.unwrap_or(".");
        let all = args_str.contains("-A") || args_str.contains("--all");
        let update = args_str.contains("-u") || args_str.contains("--update");

        let files: Vec<String> = if all || update {
            vec![]
        } else {
            args_str
                .split_whitespace()
                .filter(|s| !s.starts_with('-'))
                .map(|s| s.to_string())
                .collect()
        };

        let params = GitAddParams {
            files,
            all: Some(all),
            update: Some(update),
        };

        let result = GitService::add_files(repo_path, params)
            .await
            .map_err(|e| BitFunError::tool(format!("Git add failed: {}", e)))?;

        Ok(json!({
            "success": result.success,
            "exit_code": if result.success { 0 } else { 1 },
            "stdout": result.output.unwrap_or_default(),
            "stderr": result.error.unwrap_or_default(),
            "execution_time_ms": result.duration
        }))
    }

    /// Execute commit operation using GitService
    async fn execute_commit(repo_path: &str, args: Option<&str>) -> BitFunResult<Value> {
        let args_str = args.unwrap_or("");

        // Parse commit message
        let message = if let Some(pos) = args_str.find("-m") {
            // Try to parse -m "message" or -m 'message'
            let rest = &args_str[pos + 2..].trim_start();
            if rest.starts_with('"') {
                rest.trim_start_matches('"')
                    .split('"')
                    .next()
                    .unwrap_or("")
                    .to_string()
            } else if rest.starts_with('\'') {
                rest.trim_start_matches('\'')
                    .split('\'')
                    .next()
                    .unwrap_or("")
                    .to_string()
            } else {
                rest.split_whitespace().next().unwrap_or("").to_string()
            }
        } else {
            return Err(BitFunError::tool(
                "Commit message is required (-m \"message\")".to_string(),
            ));
        };

        let params = GitCommitParams {
            message,
            amend: Some(args_str.contains("--amend")),
            all: Some(args_str.contains("-a")),
            no_verify: Some(args_str.contains("--no-verify")),
            author: None,
        };

        let result = GitService::commit(repo_path, params)
            .await
            .map_err(|e| BitFunError::tool(format!("Git commit failed: {}", e)))?;

        Ok(json!({
            "success": result.success,
            "exit_code": if result.success { 0 } else { 1 },
            "stdout": result.output.unwrap_or_default(),
            "stderr": result.error.unwrap_or_default(),
            "execution_time_ms": result.duration
        }))
    }

    /// Execute push operation using GitService
    async fn execute_push(repo_path: &str, args: Option<&str>) -> BitFunResult<Value> {
        let args_str = args.unwrap_or("");
        let parts: Vec<&str> = args_str
            .split_whitespace()
            .filter(|s| !s.starts_with('-'))
            .collect();

        let params = GitPushParams {
            remote: parts.first().map(|s| s.to_string()),
            branch: parts.get(1).map(|s| s.to_string()),
            force: Some(args_str.contains("--force") || args_str.contains("-f")),
            set_upstream: Some(args_str.contains("-u") || args_str.contains("--set-upstream")),
        };

        let result = GitService::push(repo_path, params)
            .await
            .map_err(|e| BitFunError::tool(format!("Git push failed: {}", e)))?;

        Ok(json!({
            "success": result.success,
            "exit_code": if result.success { 0 } else { 1 },
            "stdout": result.output.unwrap_or_default(),
            "stderr": result.error.unwrap_or_default(),
            "execution_time_ms": result.duration
        }))
    }

    /// Execute pull operation using GitService
    async fn execute_pull(repo_path: &str, args: Option<&str>) -> BitFunResult<Value> {
        let args_str = args.unwrap_or("");
        let parts: Vec<&str> = args_str
            .split_whitespace()
            .filter(|s| !s.starts_with('-'))
            .collect();

        let params = GitPullParams {
            remote: parts.first().map(|s| s.to_string()),
            branch: parts.get(1).map(|s| s.to_string()),
            rebase: Some(args_str.contains("--rebase")),
        };

        let result = GitService::pull(repo_path, params)
            .await
            .map_err(|e| BitFunError::tool(format!("Git pull failed: {}", e)))?;

        Ok(json!({
            "success": result.success,
            "exit_code": if result.success { 0 } else { 1 },
            "stdout": result.output.unwrap_or_default(),
            "stderr": result.error.unwrap_or_default(),
            "execution_time_ms": result.duration
        }))
    }

    /// Execute checkout/switch operation using GitService
    async fn execute_checkout(repo_path: &str, args: Option<&str>) -> BitFunResult<Value> {
        let args_str = args.unwrap_or("");
        let create_branch = args_str.contains("-b");

        // Extract branch name
        let branch_name = args_str
            .split_whitespace()
            .rfind(|s| !s.starts_with('-'))
            .ok_or_else(|| BitFunError::tool("Branch name is required".to_string()))?;

        let result = if create_branch {
            // Create and switch to new branch
            let start_point = args_str
                .split_whitespace()
                .rfind(|s| !s.starts_with('-') && *s != branch_name);
            GitService::create_branch(repo_path, branch_name, start_point).await
        } else {
            // Switch to existing branch
            GitService::checkout_branch(repo_path, branch_name).await
        }
        .map_err(|e| BitFunError::tool(format!("Git checkout failed: {}", e)))?;

        Ok(json!({
            "success": result.success,
            "exit_code": if result.success { 0 } else { 1 },
            "stdout": result.output.unwrap_or_default(),
            "stderr": result.error.unwrap_or_default(),
            "execution_time_ms": result.duration
        }))
    }

    /// Execute branch operation using GitService
    async fn execute_branch(repo_path: &str, args: Option<&str>) -> BitFunResult<Value> {
        let args_str = args.unwrap_or("");

        // Check if it's a list branches operation
        let is_list = args_str.is_empty()
            || args_str.contains("-l")
            || args_str.contains("--list")
            || args_str.contains("-a")
            || args_str.contains("-r");

        if is_list {
            let include_remote = args_str.contains("-a") || args_str.contains("-r");
            let branches = GitService::get_branches(repo_path, include_remote)
                .await
                .map_err(|e| BitFunError::tool(format!("Git branch failed: {}", e)))?;

            let output: Vec<String> = branches
                .iter()
                .map(|b| {
                    if b.current {
                        format!("* {}", b.name)
                    } else {
                        format!("  {}", b.name)
                    }
                })
                .collect();

            Ok(json!({
                "success": true,
                "exit_code": 0,
                "stdout": output.join("\n"),
                "stderr": "",
                "data": branches
            }))
        } else if args_str.contains("-d") || args_str.contains("-D") {
            // Delete branch
            let force = args_str.contains("-D");
            let branch_name = args_str
                .split_whitespace()
                .find(|s| !s.starts_with('-'))
                .ok_or_else(|| {
                    BitFunError::tool("Branch name is required for deletion".to_string())
                })?;

            let result = GitService::delete_branch(repo_path, branch_name, force)
                .await
                .map_err(|e| BitFunError::tool(format!("Git branch delete failed: {}", e)))?;

            Ok(json!({
                "success": result.success,
                "exit_code": if result.success { 0 } else { 1 },
                "stdout": result.output.unwrap_or_default(),
                "stderr": result.error.unwrap_or_default()
            }))
        } else {
            // Create new branch (without switching) - use original command
            let mut cmd_args: Vec<&str> = vec!["branch"];
            for arg in args_str.split_whitespace() {
                cmd_args.push(arg);
            }

            let output = execute_git_command(repo_path, &cmd_args)
                .await
                .map_err(|e| BitFunError::tool(format!("Git branch failed: {}", e)))?;

            Ok(json!({
                "success": true,
                "exit_code": 0,
                "stdout": output,
                "stderr": ""
            }))
        }
    }

    /// Execute other Git operations using generic command
    async fn execute_generic(
        repo_path: &str,
        operation: &str,
        args: Option<&str>,
    ) -> BitFunResult<Value> {
        let mut cmd_args: Vec<&str> = vec![operation];

        if let Some(args_str) = args {
            for arg in args_str.split_whitespace() {
                cmd_args.push(arg);
            }
        }

        let start_time = std::time::Instant::now();

        // Use raw execution so we can distinguish git diff exit code 1 (has differences)
        // from actual errors.
        match execute_git_command_raw(repo_path, &cmd_args).await {
            Ok(raw) => {
                let duration = elapsed_ms_u64(start_time);

                // git diff returns exit code 1 when there are differences, which is not an error.
                // Other commands may also use exit code 1 for non-error conditions (e.g. grep with no matches).
                // We treat exit code 0 and exit code 1 with non-empty stdout as success,
                // but exit code >1 or exit code 1 with empty stdout and non-empty stderr as failure.
                let is_diff_like = operation == "diff";
                let success = if raw.exit_code == 0 {
                    true
                } else if is_diff_like && raw.exit_code == 1 && !raw.stdout.is_empty() {
                    true
                } else {
                    false
                };

                Ok(json!({
                    "success": success,
                    "exit_code": raw.exit_code,
                    "stdout": raw.stdout,
                    "stderr": raw.stderr,
                    "execution_time_ms": duration
                }))
            }
            Err(e) => {
                let duration = elapsed_ms_u64(start_time);
                Ok(json!({
                    "success": false,
                    "exit_code": -1,
                    "stdout": "",
                    "stderr": e.to_string(),
                    "execution_time_ms": duration
                }))
            }
        }
    }
}

#[async_trait]
impl Tool for GitTool {
    fn name(&self) -> &str {
        "Git"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Executes Git commands for version control operations.

This tool provides a safe and convenient way to execute Git commands. It supports common Git operations like status, diff, log, add, commit, branch, checkout, pull, push, and more.

## Supported Operations

- **status**: Show working tree status
- **diff**: Show changes between commits, commit and working tree, etc.
- **log**: Show commit logs
- **add**: Add file contents to the index
- **commit**: Record changes to the repository
- **branch**: List, create, or delete branches
- **checkout/switch**: Switch branches or restore working tree files
- **pull**: Fetch from and integrate with another repository or a local branch
- **push**: Update remote refs along with associated objects
- **fetch**: Download objects and refs from another repository
- **merge**: Join two or more development histories together
- **rebase**: Reapply commits on top of another base tip
- **stash**: Stash the changes in a dirty working directory away
- **reset**: Reset current HEAD to the specified state
- **restore**: Restore working tree files
- **show**: Show various types of objects
- **tag**: Create, list, delete or verify a tag object
- **remote**: Manage set of tracked repositories
- **clone**: Clone a repository into a new directory
- **init**: Create an empty Git repository
- **blame**: Show what revision and author last modified each line
- **cherry-pick**: Apply the changes introduced by some existing commits

## Usage Examples

1. Check status:
   ```json
   {"operation": "status"}
   ```

2. View diff of staged changes:
   ```json
   {"operation": "diff", "args": "--staged"}
   ```

3. View recent commits:
   ```json
   {"operation": "log", "args": "--oneline -10"}
   ```

4. Add files:
   ```json
   {"operation": "add", "args": "."}
   ```

5. Commit with message:
   ```json
   {"operation": "commit", "args": "-m \"Your commit message\""}
   ```

6. Create a new branch:
   ```json
   {"operation": "branch", "args": "feature/new-feature"}
   ```

7. Switch to a branch:
   ```json
   {"operation": "switch", "args": "main"}
   ```

## Important: `args` Field Rules

- The `operation` field already specifies the Git subcommand (e.g. `diff`, `log`, `add`).
- The `args` field must contain **only additional arguments** for that subcommand.
- **Do NOT include the subcommand name itself in `args`.** For example, use `{"operation": "diff", "args": "HEAD~2..HEAD --stat"}` — NOT `{"operation": "diff", "args": "diff HEAD~2..HEAD --stat"}`.

## Safety Notes

- This tool validates operations to ensure only allowed Git commands are executed
- Dangerous operations (like `push --force`, `reset --hard`) will show warnings
- Never run `git config` to modify user settings
- Always verify changes before committing
  - Use `--dry-run` for push/pull operations when unsure

## Remote SSH

When the workspace is opened over Remote SSH, Git runs on the **server** (see tool description context at runtime).

## Commit Message Guidelines

When creating commits, use this format for the commit message:
- Start with a concise summary (50 chars or less)
- Leave a blank line after the summary
- Add detailed description if needed
- End with:
  
  Generated with BitFun

  Co-Authored-By: BitFun"#.to_string())
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        let mut base = self.description().await?;
        if context.map(|c| c.is_remote()).unwrap_or(false) {
            base.push_str(
                "\n\n**Remote workspace:** Commands execute on the **SSH host** via `git -C <repo> …`, using the same repository and Git install as a native terminal on that server (equivalent to Claude Code / CLI on the remote machine). Paths are POSIX paths on the server.",
            );
        }
        Ok(base)
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "description": "The Git operation to perform (e.g., status, diff, log, add, commit, branch, checkout, pull, push)",
                    "enum": ALLOWED_OPERATIONS
                },
                "args": {
                    "type": "string",
                    "description": "Additional arguments for the Git command (e.g., file paths, flags, options)"
                },
                "working_directory": {
                    "type": "string",
                    "description": "The directory to run the Git command in (defaults to current workspace)"
                }
            },
            "required": ["operation"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn needs_permissions(&self, input: Option<&Value>) -> bool {
        // Read-only operations don't need permissions
        if let Some(input) = input {
            if let Some(operation) = input.get("operation").and_then(|v| v.as_str()) {
                let readonly_ops = [
                    "status",
                    "diff",
                    "log",
                    "show",
                    "branch",
                    "remote",
                    "tag",
                    "blame",
                    "describe",
                    "shortlog",
                    "rev-parse",
                ];
                // For branch command, if just listing branches (no args or using -l), it's read-only
                if operation == "branch" {
                    if let Some(args) = input.get("args").and_then(|v| v.as_str()) {
                        // If there are args but not viewing commands, permissions are needed
                        if !args.is_empty()
                            && !args.contains("-l")
                            && !args.contains("--list")
                            && !args.contains("-a")
                            && !args.contains("-r")
                        {
                            return true;
                        }
                    }
                    return false;
                }
                return !readonly_ops.contains(&operation);
            }
        }
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        // Validate operation parameter
        let operation = match input.get("operation").and_then(|v| v.as_str()) {
            Some(op) => op,
            None => {
                return ValidationResult {
                    result: false,
                    message: Some("operation is required".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        // Check if operation is allowed
        if !ALLOWED_OPERATIONS.contains(&operation) {
            return ValidationResult {
                result: false,
                message: Some(format!(
                    "Operation '{}' is not allowed. Allowed operations: {}",
                    operation,
                    ALLOWED_OPERATIONS.join(", ")
                )),
                error_code: Some(403),
                meta: None,
            };
        }

        // Get arguments (if any)
        let args = input.get("args").and_then(|v| v.as_str()).unwrap_or("");

        // Security check: prohibit interactive operations
        if args.contains("-i") || args.contains("--interactive") {
            return ValidationResult {
                result: false,
                message: Some("Interactive mode (-i) is not supported".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        // Check if operation is dangerous, add warning message
        if Self::is_dangerous_operation(operation, args) {
            return ValidationResult {
                result: true,
                message: Some(format!(
                    "Warning: This is a potentially dangerous operation: git {} {}",
                    operation, args
                )),
                error_code: None,
                meta: Some(json!({ "warning": "dangerous_operation" })),
            };
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let operation = input
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let args = input.get("args").and_then(|v| v.as_str()).unwrap_or("");

        if args.is_empty() {
            format!("git {}", operation)
        } else {
            format!("git {} {}", operation, args)
        }
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        let stdout = output
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let stderr = output
            .get("stderr")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let exit_code = output
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let command = output.get("command").and_then(|v| v.as_str()).unwrap_or("");

        let mut result_parts = Vec::new();

        // Command execution information
        if !command.is_empty() {
            result_parts.push(format!("$ {}", command));
        }

        // Main output content
        if !stdout.is_empty() {
            result_parts.push(stdout.to_string());
        }

        // Error output
        if !stderr.is_empty() {
            result_parts.push(stderr.to_string());
        }

        // Exit status
        if exit_code != 0 {
            result_parts.push(format!("\n[Exit code: {} - command failed]", exit_code));
        }

        if result_parts.is_empty() {
            "(no output)".to_string()
        } else {
            result_parts.join("\n")
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let operation = input
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("operation is required".to_string()))?;

        let args = input.get("args").and_then(|v| v.as_str());

        // Tolerance: strip a leading operation name from args if the model
        // mistakenly includes it (e.g. "diff HEAD~2..HEAD --stat" when
        // operation is already "diff"). This prevents commands like
        // "git diff diff HEAD~2..HEAD --stat".
        let args = args.map(|a| {
            let trimmed = a.trim();
            let prefix = format!("{} ", operation);
            if trimmed.starts_with(&prefix) {
                &trimmed[prefix.len()..]
            } else {
                trimmed
            }
        });

        let working_directory = input.get("working_directory").and_then(|v| v.as_str());

        // Get repository path
        let repo_path = Self::get_repo_path(working_directory, context)?;

        debug!(
            "Git tool executing operation: {} in repository: {}, args: {}",
            operation,
            repo_path,
            args.unwrap_or("")
        );

        let start_time = std::time::Instant::now();

        // Remote SSH workspace: run git on the server (not libgit2 on the PC).
        let result = if context.is_remote() {
            Self::execute_remote_git_cli(&repo_path, operation, args, context).await?
        } else {
            match operation {
                "status" => Self::execute_status(&repo_path).await?,
                "diff" => Self::execute_diff(&repo_path, args).await?,
                "log" => Self::execute_log(&repo_path, args).await?,
                "add" => Self::execute_add(&repo_path, args).await?,
                "commit" => Self::execute_commit(&repo_path, args).await?,
                "push" => Self::execute_push(&repo_path, args).await?,
                "pull" => Self::execute_pull(&repo_path, args).await?,
                "checkout" | "switch" => Self::execute_checkout(&repo_path, args).await?,
                "branch" => Self::execute_branch(&repo_path, args).await?,
                _ => Self::execute_generic(&repo_path, operation, args).await?,
            }
        };

        let duration = start_time.elapsed();
        debug!(
            "Git tool command completed, operation: {}, duration: {}ms",
            operation,
            duration.as_millis()
        );

        // Add execution time and command information
        let mut result_with_meta = result.clone();
        if let Some(obj) = result_with_meta.as_object_mut() {
            obj.insert(
                "execution_time_ms".to_string(),
                json!(duration.as_millis() as u64),
            );
            if !context.is_remote() {
                obj.insert(
                    "command".to_string(),
                    json!(format!("git {} {}", operation, args.unwrap_or(""))),
                );
            }
            obj.insert("operation".to_string(), json!(operation));
            obj.insert("working_directory".to_string(), json!(repo_path));
        }

        // Build result for assistant
        let result_for_assistant = self.render_result_for_assistant(&result_with_meta);

        Ok(vec![ToolResult::Result {
            data: result_with_meta,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

impl Default for GitTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{GitTool, ParsedDiffArgs};

    #[test]
    fn parse_diff_args_empty() {
        let r = GitTool::parse_diff_args("");
        assert_eq!(
            r,
            ParsedDiffArgs {
                staged: false,
                stat: false,
                source: None,
                target: None,
                files: None,
            }
        );
    }

    #[test]
    fn parse_diff_args_staged_only() {
        let r = GitTool::parse_diff_args("--staged");
        assert_eq!(
            r,
            ParsedDiffArgs {
                staged: true,
                stat: false,
                source: None,
                target: None,
                files: None,
            }
        );
    }

    #[test]
    fn parse_diff_args_cached_and_stat() {
        let r = GitTool::parse_diff_args("--cached --stat");
        assert_eq!(
            r,
            ParsedDiffArgs {
                staged: true,
                stat: true,
                source: None,
                target: None,
                files: None,
            }
        );
    }

    #[test]
    fn parse_diff_args_single_ref() {
        let r = GitTool::parse_diff_args("HEAD");
        assert_eq!(
            r,
            ParsedDiffArgs {
                staged: false,
                stat: false,
                source: Some("HEAD".to_string()),
                target: None,
                files: None,
            }
        );
    }

    #[test]
    fn parse_diff_args_single_ref_with_stat() {
        let r = GitTool::parse_diff_args("HEAD --stat");
        assert_eq!(
            r,
            ParsedDiffArgs {
                staged: false,
                stat: true,
                source: Some("HEAD".to_string()),
                target: None,
                files: None,
            }
        );
    }

    #[test]
    fn parse_diff_args_range_two_dot() {
        let r = GitTool::parse_diff_args("HEAD~7..HEAD --stat");
        assert_eq!(
            r,
            ParsedDiffArgs {
                staged: false,
                stat: true,
                source: Some("HEAD~7".to_string()),
                target: Some("HEAD".to_string()),
                files: None,
            }
        );
    }

    #[test]
    fn parse_diff_args_range_three_dot() {
        let r = GitTool::parse_diff_args("origin/main...HEAD");
        assert_eq!(
            r,
            ParsedDiffArgs {
                staged: false,
                stat: false,
                source: Some("origin/main".to_string()),
                target: Some("HEAD".to_string()),
                files: None,
            }
        );
    }

    #[test]
    fn parse_diff_args_range_with_files() {
        let r = GitTool::parse_diff_args("HEAD~7..HEAD --stat -- src/foo.rs src/bar.rs");
        assert_eq!(
            r,
            ParsedDiffArgs {
                staged: false,
                stat: true,
                source: Some("HEAD~7".to_string()),
                target: Some("HEAD".to_string()),
                files: Some(vec!["src/foo.rs".to_string(), "src/bar.rs".to_string()]),
            }
        );
    }

    #[test]
    fn parse_diff_args_single_ref_with_files() {
        let r = GitTool::parse_diff_args("HEAD -- src/foo.rs");
        assert_eq!(
            r,
            ParsedDiffArgs {
                staged: false,
                stat: false,
                source: Some("HEAD".to_string()),
                target: None,
                files: Some(vec!["src/foo.rs".to_string()]),
            }
        );
    }

    #[test]
    fn parse_diff_args_files_only() {
        let r = GitTool::parse_diff_args("-- -- src/foo.rs");
        assert_eq!(
            r,
            ParsedDiffArgs {
                staged: false,
                stat: false,
                source: None,
                target: None,
                files: Some(vec!["src/foo.rs".to_string()]),
            }
        );
    }

    #[test]
    fn parse_diff_args_multi_token_range() {
        let r = GitTool::parse_diff_args("feature/foo..main");
        assert_eq!(
            r,
            ParsedDiffArgs {
                staged: false,
                stat: false,
                source: Some("feature/foo".to_string()),
                target: Some("main".to_string()),
                files: None,
            }
        );
    }
}

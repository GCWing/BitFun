use super::types::*;
use crate::infrastructure::ai::AIClientFactory;
use chrono::{Local, Timelike};
/**
 * Work state analyzer
 *
 * Analyzes the user's current work state, including Git status and file changes
 */
use log::{debug, info};
use std::path::Path;
use std::sync::Arc;

pub struct WorkStateAnalyzer;

impl WorkStateAnalyzer {
    pub async fn analyze_work_state(
        factory: Arc<AIClientFactory>,
        repo_path: &Path,
        options: WorkStateOptions,
    ) -> AgentResult<WorkStateAnalysis> {
        info!("Analyzing work state: repo_path={:?}", repo_path);

        let greeting = Self::generate_greeting(&options);

        let git_state = if options.analyze_git {
            Self::analyze_git_state(repo_path).await.ok()
        } else {
            None
        };

        let git_diff = if git_state
            .as_ref()
            .map_or(false, |g| g.unstaged_files > 0 || g.staged_files > 0)
        {
            Self::get_git_diff(repo_path).await.unwrap_or_default()
        } else {
            String::new()
        };

        let time_info = Self::get_time_info(repo_path).await;

        let ai_analysis =
            Self::generate_complete_analysis_with_ai(factory, &git_state, &git_diff, &options)
                .await?;

        debug!("AI complete analysis generation succeeded");
        let summary = ai_analysis.summary;
        let ongoing_work = ai_analysis.ongoing_work;
        let predicted_actions = if options.predict_next_actions {
            ai_analysis.predicted_actions
        } else {
            Vec::new()
        };
        let quick_actions = if options.include_quick_actions {
            ai_analysis.quick_actions
        } else {
            Vec::new()
        };

        let current_state = CurrentWorkState {
            summary,
            git_state,
            ongoing_work,
            time_info,
        };

        Ok(WorkStateAnalysis {
            greeting,
            current_state,
            predicted_actions,
            quick_actions,
            analyzed_at: Local::now().to_rfc3339(),
        })
    }

    fn generate_greeting(_options: &WorkStateOptions) -> GreetingMessage {
        // Frontend uses its own static greeting from i18n.
        GreetingMessage {
            title: String::new(),
            subtitle: String::new(),
            tagline: None,
        }
    }

    async fn get_git_diff(repo_path: &Path) -> AgentResult<String> {
        debug!("Getting Git diff");

        let unstaged_output = crate::util::process_manager::create_command("git")
            .arg("diff")
            .arg("HEAD")
            .current_dir(repo_path)
            .output()
            .map_err(|e| AgentError::git_error(format!("Failed to get git diff: {}", e)))?;

        let mut diff = String::from_utf8_lossy(&unstaged_output.stdout).to_string();

        let staged_output = crate::util::process_manager::create_command("git")
            .arg("diff")
            .arg("--cached")
            .current_dir(repo_path)
            .output()
            .map_err(|e| AgentError::git_error(format!("Failed to get staged diff: {}", e)))?;

        let staged_diff = String::from_utf8_lossy(&staged_output.stdout);

        if !staged_diff.is_empty() {
            diff.push_str("\n\n=== Staged Changes ===\n\n");
            diff.push_str(&staged_diff);
        }

        debug!("Git diff retrieved: length={} chars", diff.len());

        Ok(diff)
    }

    async fn generate_complete_analysis_with_ai(
        factory: Arc<AIClientFactory>,
        git_state: &Option<GitWorkState>,
        git_diff: &str,
        options: &WorkStateOptions,
    ) -> AgentResult<AIGeneratedAnalysis> {
        use super::ai_service::AIWorkStateService;

        debug!("Starting AI complete analysis generation");

        let ai_service =
            AIWorkStateService::new_with_agent_config(factory, "startchat-func-agent").await?;
        ai_service
            .generate_complete_analysis(git_state, git_diff, &options.language)
            .await
    }

    async fn analyze_git_state(repo_path: &Path) -> AgentResult<GitWorkState> {
        let current_branch = Self::get_current_branch(repo_path)?;

        let status_output = crate::util::process_manager::create_command("git")
            .arg("status")
            .arg("--porcelain")
            .current_dir(repo_path)
            .output()
            .map_err(|e| AgentError::git_error(format!("Failed to get git status: {}", e)))?;

        let status_str = String::from_utf8_lossy(&status_output.stdout);

        let mut unstaged_files = 0;
        let mut staged_files = 0;
        let mut modified_files = Vec::new();

        for line in status_str.lines() {
            if line.is_empty() {
                continue;
            }

            let status_code = &line[0..2];
            let file_path = if line.len() > 3 {
                line[3..].trim().to_string()
            } else {
                continue;
            };

            let (change_type, is_staged) = match status_code {
                "A " => (FileChangeType::Added, true),
                " M" => (FileChangeType::Modified, false),
                "M " => (FileChangeType::Modified, true),
                "MM" => (FileChangeType::Modified, true),
                " D" => (FileChangeType::Deleted, false),
                "D " => (FileChangeType::Deleted, true),
                "??" => (FileChangeType::Untracked, false),
                "R " => (FileChangeType::Renamed, true),
                _ => (FileChangeType::Modified, false),
            };

            if is_staged {
                staged_files += 1;
            } else {
                unstaged_files += 1;
            }

            if modified_files.len() < 10 {
                modified_files.push(FileModification {
                    path: file_path.clone(),
                    change_type,
                    module: Self::extract_module(&file_path),
                });
            }
        }

        let unpushed_commits = Self::get_unpushed_commits(repo_path)?;
        let ahead_behind = Self::get_ahead_behind(repo_path).ok();

        Ok(GitWorkState {
            current_branch,
            unstaged_files,
            staged_files,
            unpushed_commits,
            ahead_behind,
            modified_files,
        })
    }

    fn get_current_branch(repo_path: &Path) -> AgentResult<String> {
        let output = crate::util::process_manager::create_command("git")
            .arg("branch")
            .arg("--show-current")
            .current_dir(repo_path)
            .output()
            .map_err(|e| AgentError::git_error(format!("Failed to get current branch: {}", e)))?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn get_unpushed_commits(repo_path: &Path) -> AgentResult<u32> {
        let output = crate::util::process_manager::create_command("git")
            .arg("log")
            .arg("@{u}..")
            .arg("--oneline")
            .current_dir(repo_path)
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let count = String::from_utf8_lossy(&output.stdout).lines().count() as u32;
                return Ok(count);
            }
        }

        Ok(0)
    }

    fn get_ahead_behind(repo_path: &Path) -> AgentResult<AheadBehind> {
        let output = crate::util::process_manager::create_command("git")
            .arg("rev-list")
            .arg("--left-right")
            .arg("--count")
            .arg("HEAD...@{u}")
            .current_dir(repo_path)
            .output()
            .map_err(|e| AgentError::git_error(format!("Failed to get ahead/behind: {}", e)))?;

        if !output.status.success() {
            return Err(AgentError::git_error("No upstream branch configured"));
        }

        let result = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = result.trim().split_whitespace().collect();

        if parts.len() >= 2 {
            let ahead = parts[0].parse().unwrap_or(0);
            let behind = parts[1].parse().unwrap_or(0);
            Ok(AheadBehind { ahead, behind })
        } else {
            Err(AgentError::git_error("Failed to parse ahead/behind info"))
        }
    }

    fn extract_module(file_path: &str) -> Option<String> {
        let path = Path::new(file_path);

        if let Some(component) = path.components().next() {
            return Some(component.as_os_str().to_string_lossy().to_string());
        }

        None
    }

    async fn get_time_info(repo_path: &Path) -> TimeInfo {
        let hour = Local::now().hour();
        let time_of_day = match hour {
            5..=11 => TimeOfDay::Morning,
            12..=17 => TimeOfDay::Afternoon,
            18..=22 => TimeOfDay::Evening,
            _ => TimeOfDay::Night,
        };

        let output = crate::util::process_manager::create_command("git")
            .arg("log")
            .arg("-1")
            .arg("--format=%ct")
            .current_dir(repo_path)
            .output();

        let (minutes_since_last_commit, last_commit_time_desc) = if let Ok(output) = output {
            if output.status.success() {
                let timestamp_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Ok(timestamp) = timestamp_str.parse::<i64>() {
                    let now = Local::now().timestamp();
                    let diff_seconds = now - timestamp;
                    let minutes = (diff_seconds / 60) as u64;

                    // Don't format time description here, let frontend handle i18n
                    (Some(minutes), None)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        TimeInfo {
            minutes_since_last_commit,
            last_commit_time_desc,
            time_of_day,
        }
    }
}

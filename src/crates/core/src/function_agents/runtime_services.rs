//! Core-owned concrete runtime services for function-agent product domains.
//!
//! Product-domain crates own prompt, parser, and facade policy. This module
//! keeps concrete Git snapshots, AI provider acquisition, and transport error
//! mapping in core.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bitfun_product_domains::function_agents::git_func_agent::{
    parse_commit_ai_response, prepare_commit_ai_prompt, AICommitAnalysis, CommitMessageOptions,
    ProjectContext,
};
use bitfun_product_domains::function_agents::ports::{
    GitCommitSnapshot, StartchatGitSnapshot, StartchatTimeSnapshot,
};
use bitfun_product_domains::function_agents::startchat_func_agent::{
    build_work_state_analysis_prompt, parse_work_state_analysis_response, AIGeneratedAnalysis,
    AheadBehind, GitWorkState,
};
use log::{debug, error, warn};

use crate::function_agents::common::{AgentError, AgentResult, Language};
use crate::function_agents::git_func_agent::ContextAnalyzer;
use crate::infrastructure::ai::{AIClient, AIClientFactory};
use crate::service::git::{GitDiffParams, GitService};
use crate::util::types::Message;

#[derive(Debug, Default, Clone)]
pub(crate) struct CoreFunctionAgentGitService;

impl CoreFunctionAgentGitService {
    pub(crate) async fn git_commit_snapshot(repo_path: PathBuf) -> AgentResult<GitCommitSnapshot> {
        let status = GitService::get_status(&repo_path)
            .await
            .map_err(|e| AgentError::git_error(format!("Failed to get Git status: {}", e)))?;

        let diff_params = GitDiffParams {
            staged: Some(true),
            stat: Some(false),
            files: None,
            ..Default::default()
        };
        let diff_content = GitService::get_diff(&repo_path, &diff_params)
            .await
            .map_err(|e| AgentError::git_error(format!("Failed to get diff: {}", e)))?;

        let project_context = ContextAnalyzer::analyze_project_context(&repo_path)
            .await
            .unwrap_or_default();

        Ok(GitCommitSnapshot {
            staged_paths: status.staged.iter().map(|file| file.path.clone()).collect(),
            staged_count: status.staged.len(),
            unstaged_count: status.unstaged.len(),
            diff_content,
            project_context,
        })
    }

    pub(crate) async fn startchat_git_snapshot(
        repo_path: PathBuf,
    ) -> AgentResult<StartchatGitSnapshot> {
        let current_branch = git_stdout_lenient(&repo_path, &["branch", "--show-current"])?
            .trim()
            .to_string();
        let status_porcelain = git_stdout_lenient(&repo_path, &["status", "--porcelain"])?;
        let unstaged_diff = git_stdout_lenient(&repo_path, &["diff", "HEAD"])?;
        let staged_diff = git_stdout_lenient(&repo_path, &["diff", "--cached"])?;
        let unpushed_commits = git_unpushed_commits(&repo_path);
        let ahead_behind = git_ahead_behind(&repo_path);
        let last_commit_timestamp = git_last_commit_timestamp(&repo_path);

        Ok(StartchatGitSnapshot {
            current_branch,
            status_porcelain,
            unstaged_diff,
            staged_diff,
            unpushed_commits,
            ahead_behind,
            last_commit_timestamp,
        })
    }

    pub(crate) fn startchat_time_snapshot(repo_path: &Path) -> StartchatTimeSnapshot {
        StartchatTimeSnapshot {
            last_commit_timestamp: git_last_commit_timestamp(repo_path),
        }
    }
}

pub struct CoreCommitAiAnalysisService {
    ai_client: Arc<AIClient>,
}

impl CoreCommitAiAnalysisService {
    pub async fn new_with_agent_config(
        factory: Arc<AIClientFactory>,
        agent_name: &str,
    ) -> AgentResult<Self> {
        let ai_client = match factory.get_client_by_func_agent(agent_name).await {
            Ok(client) => client,
            Err(e) => {
                error!("Failed to get AI client: {}", e);
                return Err(AgentError::internal_error(format!(
                    "Failed to get AI client: {}",
                    e
                )));
            }
        };

        Ok(Self { ai_client })
    }

    pub async fn generate_commit_message_ai(
        &self,
        diff_content: &str,
        project_context: &ProjectContext,
        options: &CommitMessageOptions,
    ) -> AgentResult<AICommitAnalysis> {
        if diff_content.is_empty() {
            return Err(AgentError::invalid_input("Code changes are empty"));
        }

        let prepared_prompt = prepare_commit_ai_prompt(diff_content, project_context, options);
        if prepared_prompt.truncated {
            warn!(
                "Diff too large ({} chars), truncating to {} chars",
                diff_content.len(),
                50_000
            );
        }

        let ai_response = self.call_ai(&prepared_prompt.prompt).await?;

        self.parse_commit_response(&ai_response)
    }

    async fn call_ai(&self, prompt: &str) -> AgentResult<String> {
        debug!("Sending request to AI: prompt_length={}", prompt.len());

        let messages = vec![Message::user(prompt.to_string())];
        let response = self
            .ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| {
                error!("AI call failed: {}", e);
                AgentError::internal_error(format!("AI call failed: {}", e))
            })?;

        debug!(
            "AI response received: response_length={}",
            response.text.len()
        );

        if response.text.is_empty() {
            error!("AI response is empty");
            Err(AgentError::internal_error(
                "AI response is empty".to_string(),
            ))
        } else {
            Ok(response.text)
        }
    }

    fn parse_commit_response(&self, response: &str) -> AgentResult<AICommitAnalysis> {
        parse_commit_ai_response(response)
    }
}

pub struct CoreWorkStateAiAnalysisService {
    ai_client: Arc<AIClient>,
}

impl CoreWorkStateAiAnalysisService {
    pub async fn new_with_agent_config(
        factory: Arc<AIClientFactory>,
        agent_name: &str,
    ) -> AgentResult<Self> {
        let ai_client = match factory.get_client_by_func_agent(agent_name).await {
            Ok(client) => client,
            Err(e) => {
                error!("Failed to get AI client: {}", e);
                return Err(AgentError::internal_error(format!(
                    "Failed to get AI client: {}",
                    e
                )));
            }
        };

        Ok(Self { ai_client })
    }

    pub async fn generate_complete_analysis(
        &self,
        git_state: &Option<GitWorkState>,
        git_diff: &str,
        language: &Language,
    ) -> AgentResult<AIGeneratedAnalysis> {
        let prompt = build_work_state_analysis_prompt(git_state, git_diff, language);

        debug!(
            "Calling AI to generate complete analysis: prompt_length={}",
            prompt.len()
        );

        let response = self.call_ai(&prompt).await?;

        self.parse_complete_analysis(&response)
    }

    async fn call_ai(&self, prompt: &str) -> AgentResult<String> {
        debug!("Sending request to AI: prompt_length={}", prompt.len());

        let messages = vec![Message::user(prompt.to_string())];
        let response = self
            .ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| {
                error!("AI call failed: {}", e);
                AgentError::internal_error(format!("AI call failed: {}", e))
            })?;

        debug!(
            "AI response received: response_length={}",
            response.text.len()
        );

        if response.text.is_empty() {
            error!("AI response is empty");
            Err(AgentError::internal_error(
                "AI response is empty".to_string(),
            ))
        } else {
            Ok(response.text)
        }
    }

    fn parse_complete_analysis(&self, response: &str) -> AgentResult<AIGeneratedAnalysis> {
        let parsed_analysis = parse_work_state_analysis_response(response).map_err(|error| {
            error!("{}, response: {}", error.message, response);
            error
        })?;

        if parsed_analysis.predicted_actions_count < 3 {
            warn!(
                "AI generated insufficient predicted actions ({}), adding defaults",
                parsed_analysis.predicted_actions_count
            );
        } else if parsed_analysis.predicted_actions_count > 3 {
            warn!(
                "AI generated too many predicted actions ({}), truncating to 3",
                parsed_analysis.predicted_actions_count
            );
        }

        if parsed_analysis.quick_actions_count < 6 {
            warn!(
                "AI generated insufficient quick actions ({}), frontend will use defaults",
                parsed_analysis.quick_actions_count
            );
        } else if parsed_analysis.quick_actions_count > 6 {
            warn!(
                "AI generated too many quick actions ({}), truncating to 6",
                parsed_analysis.quick_actions_count
            );
        }

        debug!(
            "Parsing completed: predicted_actions={}, quick_actions={}",
            parsed_analysis.analysis.predicted_actions.len(),
            parsed_analysis.analysis.quick_actions.len()
        );

        Ok(parsed_analysis.analysis)
    }
}

fn git_stdout_lenient(repo_path: &Path, args: &[&str]) -> AgentResult<String> {
    let output = crate::util::process_manager::create_command("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| AgentError::git_error(format!("Failed to run git {:?}: {}", args, e)))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_unpushed_commits(repo_path: &Path) -> u32 {
    let output = crate::util::process_manager::create_command("git")
        .arg("log")
        .arg("@{u}..")
        .arg("--oneline")
        .current_dir(repo_path)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            return String::from_utf8_lossy(&output.stdout).lines().count() as u32;
        }
    }

    0
}

fn git_ahead_behind(repo_path: &Path) -> Option<AheadBehind> {
    let output = crate::util::process_manager::create_command("git")
        .arg("rev-list")
        .arg("--left-right")
        .arg("--count")
        .arg("HEAD...@{u}")
        .current_dir(repo_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let result = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = result.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    Some(AheadBehind {
        ahead: parts[0].parse().unwrap_or(0),
        behind: parts[1].parse().unwrap_or(0),
    })
}

fn git_last_commit_timestamp(repo_path: &Path) -> Option<i64> {
    let output = crate::util::process_manager::create_command("git")
        .arg("log")
        .arg("-1")
        .arg("--format=%ct")
        .current_dir(repo_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<i64>()
        .ok()
}

#[cfg(test)]
mod tests {
    use bitfun_ai_adapters::types::ReasoningMode;

    use super::*;
    use crate::function_agents::common::AgentErrorType;
    use crate::util::types::AIConfig;

    fn test_ai_client() -> Arc<AIClient> {
        Arc::new(AIClient::new(AIConfig {
            name: "test".to_string(),
            base_url: "http://127.0.0.1".to_string(),
            request_url: "http://127.0.0.1".to_string(),
            api_key: "test".to_string(),
            model: "test-model".to_string(),
            format: "openai".to_string(),
            context_window: 8192,
            max_tokens: None,
            temperature: None,
            top_p: None,
            reasoning_mode: ReasoningMode::Default,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            custom_request_body: None,
            custom_request_body_mode: None,
        }))
    }

    #[test]
    fn parse_commit_response_preserves_product_domain_response_policy() {
        let service = CoreCommitAiAnalysisService {
            ai_client: test_ai_client(),
        };
        let parsed = service
            .parse_commit_response(
                r#"The answer is:
```json
{
  "type": "refactor",
  "title": "refactor(product-domains): add runtime baseline",
  "body": "Keep behavior stable.",
  "confidence": 0.91
}
```
"#,
            )
            .unwrap();

        assert_eq!(
            parsed.title,
            "refactor(product-domains): add runtime baseline"
        );
        assert_eq!(parsed.body.as_deref(), Some("Keep behavior stable."));
        assert_eq!(parsed.confidence, 0.91);

        let missing_json = service.parse_commit_response("no json here").unwrap_err();
        assert_eq!(missing_json.error_type, AgentErrorType::AnalysisError);
        assert_eq!(missing_json.message, "Cannot extract JSON from response");

        let missing_title = service
            .parse_commit_response(r#"{"type":"refactor","body":"missing title"}"#)
            .unwrap_err();
        assert_eq!(missing_title.error_type, AgentErrorType::AnalysisError);
        assert_eq!(missing_title.message, "Missing title field");
    }

    #[test]
    fn parse_complete_analysis_preserves_product_domain_response_policy() {
        let service = CoreWorkStateAiAnalysisService {
            ai_client: test_ai_client(),
        };
        let analysis = service
            .parse_complete_analysis(
                r#"The answer is:
```json
{
  "summary": "Working on product-domain owner closure.",
  "predicted_actions": [
    {"description": "Run checks", "priority": "High", "icon": "check", "is_reminder": false}
  ],
  "quick_actions": [
    {"title": "Status", "command": "git status", "icon": "git", "action_type": "ViewStatus"}
  ]
}
```
"#,
            )
            .unwrap();

        assert_eq!(analysis.summary, "Working on product-domain owner closure.");
        assert_eq!(analysis.predicted_actions.len(), 3);
        assert_eq!(analysis.quick_actions.len(), 1);

        let missing_json = service.parse_complete_analysis("no json here").unwrap_err();
        assert_eq!(missing_json.error_type, AgentErrorType::InternalError);
        assert_eq!(
            missing_json.message,
            "Failed to extract JSON from analysis response"
        );

        let invalid_json = service
            .parse_complete_analysis(
                r#"```json
not json
```"#,
            )
            .unwrap_err();
        assert_eq!(invalid_json.error_type, AgentErrorType::InternalError);
        assert_eq!(
            invalid_json.message,
            "Failed to extract JSON from analysis response"
        );
    }
}

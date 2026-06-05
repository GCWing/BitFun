//! Pure Startchat function-agent helper utilities.

use crate::function_agents::common::{
    extract_json_from_ai_response, AgentError, AgentResult, Language,
};
use crate::function_agents::startchat_func_agent::types::*;

pub const WORK_STATE_ANALYSIS_PROMPT: &str = include_str!("prompts/work_state_analysis.md");

pub fn language_instruction(language: &Language) -> &'static str {
    match language {
        Language::Chinese => "Please respond in Chinese.",
        Language::English => "Please respond in English.",
    }
}

pub fn build_complete_analysis_prompt(
    template: &str,
    git_state: &Option<GitWorkState>,
    git_diff: &str,
    language: &Language,
) -> String {
    template
        .replace("{lang_instruction}", language_instruction(language))
        .replace("{git_state_section}", &build_git_state_section(git_state))
        .replace(
            "{git_diff_section}",
            &build_git_diff_section(git_diff, 8000),
        )
}

pub fn build_work_state_analysis_prompt(
    git_state: &Option<GitWorkState>,
    git_diff: &str,
    language: &Language,
) -> String {
    build_complete_analysis_prompt(WORK_STATE_ANALYSIS_PROMPT, git_state, git_diff, language)
}

pub fn build_git_state_section(git_state: &Option<GitWorkState>) -> String {
    let Some(git) = git_state else {
        return String::new();
    };

    let mut section = format!(
        "## Git Status\n\n- Current branch: {}\n- Unstaged files: {}\n- Staged files: {}\n- Unpushed commits: {}\n",
        git.current_branch, git.unstaged_files, git.staged_files, git.unpushed_commits
    );

    if !git.modified_files.is_empty() {
        section.push_str("\nModified files:\n");
        for file in git.modified_files.iter().take(10) {
            section.push_str(&format!("  - {} ({:?})\n", file.path, file.change_type));
        }
    }

    section
}

pub fn build_git_diff_section(git_diff: &str, max_diff_length: usize) -> String {
    if git_diff.is_empty() {
        return String::new();
    }

    if git_diff.len() > max_diff_length {
        let truncated_diff = git_diff
            .char_indices()
            .take_while(|(idx, _)| *idx < max_diff_length)
            .map(|(_, c)| c)
            .collect::<String>();
        format!(
            "## Code Changes (Git Diff)\n\n{}\n\n... (diff content too long, truncated, total length: {} characters)\n",
            truncated_diff,
            git_diff.len()
        )
    } else {
        format!("## Code Changes (Git Diff)\n\n{}", git_diff)
    }
}

pub fn combine_git_diffs(unstaged_diff: &str, staged_diff: &str) -> String {
    let mut diff = unstaged_diff.to_string();

    if !staged_diff.is_empty() {
        diff.push_str("\n\n=== Staged Changes ===\n\n");
        diff.push_str(staged_diff);
    }

    diff
}

pub fn parse_predicted_actions_from_values(
    actions_array: &[serde_json::Value],
) -> Vec<PredictedAction> {
    actions_array
        .iter()
        .map(|action_value| PredictedAction {
            description: action_value["description"]
                .as_str()
                .unwrap_or("Continue current work")
                .to_string(),
            priority: parse_action_priority_label(
                action_value["priority"].as_str().unwrap_or("Medium"),
            ),
            icon: action_value["icon"].as_str().unwrap_or("").to_string(),
            is_reminder: action_value["is_reminder"].as_bool().unwrap_or(false),
        })
        .collect()
}

pub fn normalize_predicted_actions(mut actions: Vec<PredictedAction>) -> Vec<PredictedAction> {
    while actions.len() < 3 {
        actions.push(PredictedAction {
            description: "Continue current development".to_string(),
            priority: ActionPriority::Medium,
            icon: String::new(),
            is_reminder: false,
        });
    }

    if actions.len() > 3 {
        actions.truncate(3);
    }

    actions
}

pub fn parse_quick_actions_from_values(actions_array: &[serde_json::Value]) -> Vec<QuickAction> {
    actions_array
        .iter()
        .map(|action_value| QuickAction {
            title: action_value["title"]
                .as_str()
                .unwrap_or("Quick Action")
                .to_string(),
            command: action_value["command"].as_str().unwrap_or("").to_string(),
            icon: action_value["icon"].as_str().unwrap_or("").to_string(),
            action_type: parse_quick_action_type_label(
                action_value["action_type"].as_str().unwrap_or("Custom"),
            ),
        })
        .collect()
}

pub fn limit_quick_actions(mut actions: Vec<QuickAction>) -> Vec<QuickAction> {
    if actions.len() > 6 {
        actions.truncate(6);
    }
    actions
}

#[derive(Debug, Clone)]
pub struct ParsedCompleteAnalysis {
    pub analysis: AIGeneratedAnalysis,
    pub predicted_actions_count: usize,
    pub quick_actions_count: usize,
}

pub fn parse_complete_analysis_value(parsed: &serde_json::Value) -> ParsedCompleteAnalysis {
    let summary = parsed["summary"]
        .as_str()
        .unwrap_or("You were working on development, with multiple files modified.")
        .to_string();

    let predicted_actions = parsed["predicted_actions"]
        .as_array()
        .map(|actions_array| parse_predicted_actions_from_values(actions_array))
        .unwrap_or_default();
    let predicted_actions_count = predicted_actions.len();
    let predicted_actions = normalize_predicted_actions(predicted_actions);

    let quick_actions = parsed["quick_actions"]
        .as_array()
        .map(|actions_array| parse_quick_actions_from_values(actions_array))
        .unwrap_or_default();
    let quick_actions_count = quick_actions.len();
    let quick_actions = limit_quick_actions(quick_actions);

    ParsedCompleteAnalysis {
        analysis: AIGeneratedAnalysis {
            summary,
            ongoing_work: Vec::new(),
            predicted_actions,
            quick_actions,
        },
        predicted_actions_count,
        quick_actions_count,
    }
}

pub fn parse_complete_analysis_json(json: &str) -> Result<ParsedCompleteAnalysis, String> {
    let parsed = serde_json::from_str::<serde_json::Value>(json)
        .map_err(|error| format!("Failed to parse complete analysis response: {}", error))?;
    Ok(parse_complete_analysis_value(&parsed))
}

pub fn parse_work_state_analysis_response(response: &str) -> AgentResult<ParsedCompleteAnalysis> {
    let json_str = extract_json_from_ai_response(response).ok_or_else(|| {
        AgentError::internal_error("Failed to extract JSON from analysis response")
    })?;

    log::debug!(
        "Parsing function-agent work state JSON response: length={}",
        json_str.len()
    );

    parse_complete_analysis_json(&json_str).map_err(AgentError::internal_error)
}

pub fn parse_action_priority_label(label: &str) -> ActionPriority {
    match label {
        "High" => ActionPriority::High,
        "Low" => ActionPriority::Low,
        _ => ActionPriority::Medium,
    }
}

pub fn parse_quick_action_type_label(label: &str) -> QuickActionType {
    match label {
        "Continue" => QuickActionType::Continue,
        "ViewStatus" => QuickActionType::ViewStatus,
        "Commit" => QuickActionType::Commit,
        "Visualize" => QuickActionType::Visualize,
        _ => QuickActionType::Custom,
    }
}

pub fn parse_git_status_porcelain(status: &str) -> (u32, u32, Vec<FileModification>) {
    let mut unstaged_files = 0;
    let mut staged_files = 0;
    let mut modified_files = Vec::new();

    for line in status.lines() {
        if line.is_empty() || line.len() <= 3 {
            continue;
        }

        let Some((change_type, is_staged, file_path)) = parse_git_status_line(line) else {
            continue;
        };

        if is_staged {
            staged_files += 1;
        } else {
            unstaged_files += 1;
        }

        if modified_files.len() < 10 {
            modified_files.push(FileModification {
                module: extract_top_level_module(&file_path),
                path: file_path,
                change_type,
            });
        }
    }

    (unstaged_files, staged_files, modified_files)
}

pub fn parse_git_status_line(line: &str) -> Option<(FileChangeType, bool, String)> {
    if line.len() <= 3 {
        return None;
    }

    let status_code = &line[0..2];
    let file_path = line[3..].trim().to_string();

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

    Some((change_type, is_staged, file_path))
}

pub fn extract_top_level_module(file_path: &str) -> Option<String> {
    let path = std::path::Path::new(file_path);
    path.components()
        .next()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
}

pub fn time_of_day_for_hour(hour: u32) -> TimeOfDay {
    match hour {
        5..=11 => TimeOfDay::Morning,
        12..=17 => TimeOfDay::Afternoon,
        18..=22 => TimeOfDay::Evening,
        _ => TimeOfDay::Night,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function_agents::common::AgentErrorType;

    #[test]
    fn work_state_ai_prompt_uses_product_domain_template() {
        let git_state = Some(GitWorkState {
            current_branch: "feature/runtime".to_string(),
            unstaged_files: 1,
            staged_files: 2,
            unpushed_commits: 3,
            ahead_behind: None,
            modified_files: vec![FileModification {
                module: Some("src".to_string()),
                path: "src/lib.rs".to_string(),
                change_type: FileChangeType::Modified,
            }],
        });

        let prompt = build_work_state_analysis_prompt(
            &git_state,
            "diff --git a/src/lib.rs b/src/lib.rs",
            &Language::English,
        );

        assert!(prompt.contains("BitFun AI assistant"));
        assert!(prompt.contains("Current branch: feature/runtime"));
        assert!(prompt.contains("diff --git a/src/lib.rs b/src/lib.rs"));
    }

    #[test]
    fn work_state_ai_response_policy_extracts_json_and_maps_domain_errors() {
        let analysis = parse_work_state_analysis_response(
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

        assert_eq!(
            analysis.analysis.summary,
            "Working on product-domain owner closure."
        );
        assert_eq!(analysis.analysis.predicted_actions.len(), 3);
        assert_eq!(analysis.analysis.quick_actions.len(), 1);

        let missing_json = parse_work_state_analysis_response("no json here").unwrap_err();
        assert_eq!(missing_json.error_type, AgentErrorType::InternalError);
        assert_eq!(
            missing_json.message,
            "Failed to extract JSON from analysis response"
        );

        let invalid_json = parse_work_state_analysis_response(
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

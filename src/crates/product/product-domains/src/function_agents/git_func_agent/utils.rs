//! Pure Git function-agent helper utilities.

use crate::function_agents::common::extract_json_from_ai_response;
use crate::function_agents::git_func_agent::types::*;
use std::path::Path;

pub const COMMIT_MESSAGE_PROMPT: &str = include_str!("prompts/commit_message.md");

const COMMIT_MESSAGE_PROMPT_MAX_CHARS: usize = 50_000;

pub fn infer_file_type(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn extract_module_name(path: &str) -> Option<String> {
    let path = Path::new(path);

    if let Some(parent) = path.parent() {
        if let Some(dir_name) = parent.file_name() {
            return Some(dir_name.to_string_lossy().to_string());
        }
    }

    path.file_stem()
        .map(|name| name.to_string_lossy().to_string())
}

pub fn is_config_file(path: &str) -> bool {
    let config_patterns = [
        ".json",
        ".yaml",
        ".yml",
        ".toml",
        ".xml",
        ".ini",
        ".conf",
        "config",
        "package.json",
        "cargo.toml",
        "tsconfig",
    ];

    let path_lower = path.to_lowercase();
    config_patterns
        .iter()
        .any(|pattern| path_lower.contains(pattern))
}

pub fn is_doc_file(path: &str) -> bool {
    let doc_patterns = [".md", ".txt", ".rst", "readme", "changelog", "license"];

    let path_lower = path.to_lowercase();
    doc_patterns
        .iter()
        .any(|pattern| path_lower.contains(pattern))
}

pub fn is_test_file(path: &str) -> bool {
    let test_patterns = ["test", "spec", "__tests__", ".test.", ".spec."];

    let path_lower = path.to_lowercase();
    test_patterns
        .iter()
        .any(|pattern| path_lower.contains(pattern))
}

pub fn detect_change_patterns(file_changes: &[FileChange]) -> Vec<ChangePattern> {
    let mut patterns = Vec::new();

    let mut has_code_changes = false;
    let mut has_test_changes = false;
    let mut has_doc_changes = false;
    let mut has_config_changes = false;
    let mut has_new_files = false;

    for change in file_changes {
        if change.change_type == FileChangeType::Added {
            has_new_files = true
        }

        if is_test_file(&change.path) {
            has_test_changes = true;
        } else if is_doc_file(&change.path) {
            has_doc_changes = true;
        } else if is_config_file(&change.path) {
            has_config_changes = true;
        } else {
            has_code_changes = true;
        }
    }

    if has_new_files && has_code_changes {
        patterns.push(ChangePattern::FeatureAddition);
    }

    if has_code_changes && !has_new_files {
        patterns.push(ChangePattern::BugFix);
    }

    if has_test_changes {
        patterns.push(ChangePattern::TestUpdate);
    }

    if has_doc_changes {
        patterns.push(ChangePattern::DocumentationUpdate);
    }

    if has_config_changes {
        if file_changes.iter().any(|f| {
            f.path.contains("package.json")
                || f.path.contains("cargo.toml")
                || f.path.contains("requirements.txt")
        }) {
            patterns.push(ChangePattern::DependencyUpdate);
        } else {
            patterns.push(ChangePattern::ConfigChange);
        }
    }

    let total_lines = file_changes
        .iter()
        .map(|f| f.additions + f.deletions)
        .sum::<u32>();

    if has_code_changes && total_lines > 200 && file_changes.len() < 5 {
        patterns.push(ChangePattern::Refactoring);
    }

    patterns
}

pub fn build_changes_summary_from_paths(
    changed_files: &[String],
    staged_count: usize,
    unstaged_count: usize,
) -> ChangesSummary {
    let total_additions = (staged_count as u32 * 10) + (unstaged_count as u32 * 10);
    let total_deletions = (staged_count as u32 * 5) + (unstaged_count as u32 * 5);

    let file_changes: Vec<FileChange> = changed_files
        .iter()
        .map(|path| FileChange {
            path: path.clone(),
            change_type: FileChangeType::Modified,
            additions: 10,
            deletions: 5,
            file_type: infer_file_type(path),
        })
        .collect();

    let affected_modules = changed_files
        .iter()
        .filter_map(|path| extract_module_name(path))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .take(3)
        .collect();

    let change_patterns = detect_change_patterns(&file_changes);

    ChangesSummary {
        total_additions,
        total_deletions,
        files_changed: changed_files.len() as u32,
        file_changes,
        affected_modules,
        change_patterns,
    }
}

pub fn assemble_commit_message(
    title: &str,
    body: &Option<String>,
    footer: &Option<String>,
) -> String {
    let mut parts = vec![title.to_string()];

    if let Some(body_text) = body {
        if !body_text.is_empty() {
            parts.push(String::new());
            parts.push(body_text.clone());
        }
    }

    if let Some(footer_text) = footer {
        if !footer_text.is_empty() {
            parts.push(String::new());
            parts.push(footer_text.clone());
        }
    }

    parts.join("\n")
}

pub fn commit_format_description(format: &CommitFormat) -> &'static str {
    match format {
        CommitFormat::Conventional => "Conventional Commits",
        CommitFormat::Angular => "Angular Style",
        CommitFormat::Simple => "Simple Format",
        CommitFormat::Custom => "Custom Format",
    }
}

pub fn commit_language_description(language: &Language) -> &'static str {
    match language {
        Language::Chinese => "Chinese",
        Language::English => "English",
    }
}

pub fn build_commit_prompt(
    template: &str,
    diff_content: &str,
    project_context: &ProjectContext,
    options: &CommitMessageOptions,
) -> String {
    template
        .replace("{project_type}", &project_context.project_type)
        .replace("{tech_stack}", &project_context.tech_stack.join(", "))
        .replace("{format_desc}", commit_format_description(&options.format))
        .replace(
            "{language_desc}",
            commit_language_description(&options.language),
        )
        .replace("{diff_content}", diff_content)
        .replace("{max_title_length}", &options.max_title_length.to_string())
}

pub fn truncate_diff_for_commit_prompt(diff: &str, max_chars: usize) -> String {
    if diff.len() <= max_chars {
        return diff.to_string();
    }

    let keep_chars = max_chars.saturating_sub(100);
    let mut truncated = diff.chars().take(keep_chars).collect::<String>();
    truncated.push_str("\n\n... [content truncated] ...");
    truncated
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCommitPrompt {
    pub prompt: String,
    pub diff_content: String,
    pub truncated: bool,
}

pub fn prepare_commit_prompt(
    template: &str,
    diff_content: &str,
    project_context: &ProjectContext,
    options: &CommitMessageOptions,
    max_chars: usize,
) -> PreparedCommitPrompt {
    let truncated = diff_content.len() > max_chars;
    let diff_content = truncate_diff_for_commit_prompt(diff_content, max_chars);
    let prompt = build_commit_prompt(template, &diff_content, project_context, options);

    PreparedCommitPrompt {
        prompt,
        diff_content,
        truncated,
    }
}

pub fn prepare_commit_ai_prompt(
    diff_content: &str,
    project_context: &ProjectContext,
    options: &CommitMessageOptions,
) -> PreparedCommitPrompt {
    prepare_commit_prompt(
        COMMIT_MESSAGE_PROMPT,
        diff_content,
        project_context,
        options,
        COMMIT_MESSAGE_PROMPT_MAX_CHARS,
    )
}

pub fn parse_commit_type_label(label: &str) -> CommitType {
    match label.to_lowercase().as_str() {
        "feat" | "feature" => CommitType::Feat,
        "fix" => CommitType::Fix,
        "docs" | "doc" => CommitType::Docs,
        "style" => CommitType::Style,
        "refactor" => CommitType::Refactor,
        "perf" | "performance" => CommitType::Perf,
        "test" => CommitType::Test,
        "chore" => CommitType::Chore,
        "ci" => CommitType::CI,
        "revert" => CommitType::Revert,
        _ => CommitType::Chore,
    }
}

pub fn parse_commit_analysis_value(value: &serde_json::Value) -> Result<AICommitAnalysis, String> {
    Ok(AICommitAnalysis {
        commit_type: parse_commit_type_label(value["type"].as_str().unwrap_or("chore")),
        scope: value["scope"].as_str().map(|s| s.to_string()),
        title: value["title"]
            .as_str()
            .ok_or_else(|| "Missing title field".to_string())?
            .to_string(),
        body: value["body"].as_str().map(|s| s.to_string()),
        breaking_changes: value["breaking_changes"].as_str().map(|s| s.to_string()),
        reasoning: value["reasoning"]
            .as_str()
            .unwrap_or("AI analysis")
            .to_string(),
        confidence: value["confidence"].as_f64().unwrap_or(0.8) as f32,
    })
}

pub fn parse_commit_analysis_json(json: &str) -> Result<AICommitAnalysis, String> {
    let value = serde_json::from_str::<serde_json::Value>(json)
        .map_err(|error| format!("Failed to parse AI response: {}", error))?;
    parse_commit_analysis_value(&value)
}

pub fn parse_commit_ai_response(response: &str) -> AgentResult<AICommitAnalysis> {
    let json_str = extract_json_from_ai_response(response)
        .ok_or_else(|| AgentError::analysis_error("Cannot extract JSON from response"))?;

    parse_commit_analysis_json(&json_str).map_err(AgentError::analysis_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function_agents::common::AgentErrorType;

    fn project_context() -> ProjectContext {
        ProjectContext {
            project_type: "Rust workspace".to_string(),
            tech_stack: vec!["Rust".to_string()],
            project_docs: None,
            code_standards: None,
        }
    }

    fn commit_options() -> CommitMessageOptions {
        CommitMessageOptions {
            format: CommitFormat::Conventional,
            language: Language::English,
            max_title_length: 72,
            include_body: true,
            include_files: true,
        }
    }

    #[test]
    fn commit_ai_prompt_uses_product_domain_template_and_truncation_policy() {
        let large_diff = "a".repeat(50_010);
        let prompt = prepare_commit_ai_prompt(&large_diff, &project_context(), &commit_options());

        assert!(prompt.truncated);
        assert!(prompt.prompt.contains("Commit Message Generation Prompt"));
        assert!(prompt.prompt.contains("... [content truncated] ..."));
        assert!(prompt.diff_content.len() < 50_010);
    }

    #[test]
    fn commit_ai_response_policy_extracts_json_and_maps_domain_errors() {
        let parsed = parse_commit_ai_response(
            r#"The answer is:
```json
{
  "type": "refactor",
  "title": "refactor(product-domains): move response policy",
  "body": "Keep behavior stable.",
  "confidence": 0.91
}
```
"#,
        )
        .unwrap();

        assert_eq!(
            parsed.title,
            "refactor(product-domains): move response policy"
        );
        assert_eq!(parsed.body.as_deref(), Some("Keep behavior stable."));
        assert_eq!(parsed.confidence, 0.91);

        let missing_json = parse_commit_ai_response("no json here").unwrap_err();
        assert_eq!(missing_json.error_type, AgentErrorType::AnalysisError);
        assert_eq!(missing_json.message, "Cannot extract JSON from response");

        let missing_title =
            parse_commit_ai_response(r#"{"type":"refactor","body":"missing title"}"#).unwrap_err();
        assert_eq!(missing_title.error_type, AgentErrorType::AnalysisError);
        assert_eq!(missing_title.message, "Missing title field");
    }
}

//! Pure Git function-agent helper utilities.

use crate::function_agents::git_func_agent::types::*;
use std::path::Path;

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

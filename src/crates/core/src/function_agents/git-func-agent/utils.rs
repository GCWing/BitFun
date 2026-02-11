/**
 * Git Function Agent - utility functions
 *
 * Provides various helper utilities
 */
use super::types::*;
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
        match change.change_type {
            FileChangeType::Added => has_new_files = true,
            _ => {}
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

    // Large code changes with few files may indicate refactoring
    let total_lines = file_changes
        .iter()
        .map(|f| f.additions + f.deletions)
        .sum::<u32>();

    if has_code_changes && total_lines > 200 && file_changes.len() < 5 {
        patterns.push(ChangePattern::Refactoring);
    }

    patterns
}

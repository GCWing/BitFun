use super::flashgrep::SearchResults;
use super::types::ContentSearchOutputMode;
use bitfun_services_core::filesystem::{FileSearchResult, SearchMatchType};
use std::path::Path;

pub(crate) fn convert_search_results(
    search_results: &SearchResults,
    output_mode: ContentSearchOutputMode,
) -> Vec<FileSearchResult> {
    match output_mode {
        ContentSearchOutputMode::Content => {
            let line_results = convert_hits_to_file_search_results(search_results);
            if !line_results.is_empty() {
                return line_results;
            }

            let count_results = convert_file_counts_to_search_results(search_results);
            if !count_results.is_empty() {
                return count_results;
            }

            let match_count_results = convert_file_match_counts_to_search_results(search_results);
            if !match_count_results.is_empty() {
                return match_count_results;
            }

            convert_matched_paths_to_file_only_results(search_results)
        }
        ContentSearchOutputMode::Count => convert_file_counts_to_search_results(search_results),
        ContentSearchOutputMode::FilesWithMatches => {
            convert_matched_paths_to_file_only_results(search_results)
        }
    }
}

fn convert_hits_to_file_search_results(search_results: &SearchResults) -> Vec<FileSearchResult> {
    search_results
        .line_matches
        .iter()
        .map(|matched| {
            let matched_content = matched
                .line_text
                .clone()
                .unwrap_or_else(|| format!("line {}", matched.line_number));
            let (preview_before, preview_inside, preview_after) = matched
                .line_text
                .as_deref()
                .map(split_preview)
                .unwrap_or((None, None, None));

            FileSearchResult {
                path: matched.path.clone(),
                name: Path::new(&matched.path)
                    .file_name()
                    .and_then(|file_name| file_name.to_str())
                    .unwrap_or(&matched.path)
                    .to_string(),
                is_directory: false,
                match_type: SearchMatchType::Content,
                line_number: Some(matched.line_number),
                matched_content: Some(matched_content),
                preview_before,
                preview_inside,
                preview_after,
            }
        })
        .collect()
}

fn split_preview(line_text: &str) -> (Option<String>, Option<String>, Option<String>) {
    (None, Some(line_text.to_string()), None)
}

fn convert_file_counts_to_search_results(search_results: &SearchResults) -> Vec<FileSearchResult> {
    search_results
        .file_counts
        .iter()
        .map(|count| FileSearchResult {
            path: count.path.clone(),
            name: Path::new(&count.path)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or(&count.path)
                .to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: None,
            matched_content: Some(count.matched_lines.to_string()),
            preview_before: None,
            preview_inside: None,
            preview_after: None,
        })
        .collect()
}

fn convert_file_match_counts_to_search_results(
    search_results: &SearchResults,
) -> Vec<FileSearchResult> {
    search_results
        .file_match_counts
        .iter()
        .map(|count| FileSearchResult {
            path: count.path.clone(),
            name: Path::new(&count.path)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or(&count.path)
                .to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: None,
            matched_content: Some(count.matched_occurrences.to_string()),
            preview_before: None,
            preview_inside: None,
            preview_after: None,
        })
        .collect()
}

fn convert_matched_paths_to_file_only_results(
    search_results: &SearchResults,
) -> Vec<FileSearchResult> {
    search_results
        .matched_paths
        .iter()
        .map(|path| FileSearchResult {
            path: path.clone(),
            name: Path::new(path)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or(path)
                .to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: None,
            matched_content: None,
            preview_before: None,
            preview_inside: None,
            preview_after: None,
        })
        .collect()
}

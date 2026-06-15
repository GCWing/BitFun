use super::flashgrep::SearchResults;
use super::types::ContentSearchOutputMode;
use bitfun_services_core::filesystem::{FileSearchResult, SearchMatchType};
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) fn convert_search_results(
    search_results: &SearchResults,
    output_mode: ContentSearchOutputMode,
) -> Vec<FileSearchResult> {
    match output_mode {
        ContentSearchOutputMode::Content => {
            let hit_results = convert_hits_to_file_search_results(search_results);
            if !hit_results.is_empty() {
                return hit_results;
            }

            let line_results = convert_line_matches_to_file_search_results(search_results);
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

fn convert_line_matches_to_file_search_results(
    search_results: &SearchResults,
) -> Vec<FileSearchResult> {
    search_results
        .line_matches
        .iter()
        .map(|matched| FileSearchResult {
            path: matched.path.clone(),
            name: Path::new(&matched.path)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or(&matched.path)
                .to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: Some(matched.line_number),
            matched_content: Some(matched.line_text.clone()),
            preview_before: None,
            preview_inside: Some(matched.line_text.clone()),
            preview_after: None,
        })
        .collect()
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

fn convert_hits_to_file_search_results(search_results: &SearchResults) -> Vec<FileSearchResult> {
    let mut file_results = Vec::new();
    for hit in &search_results.hits {
        let name = Path::new(&hit.path)
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or(&hit.path)
            .to_string();

        let mut lines = BTreeMap::new();
        for file_match in &hit.matches {
            lines
                .entry(file_match.location.line)
                .or_insert_with(|| file_match.clone());
        }

        for (_, file_match) in lines {
            let (preview_before, preview_inside, preview_after) =
                split_preview(&file_match.snippet, &file_match.matched_text);
            file_results.push(FileSearchResult {
                path: hit.path.clone(),
                name: name.clone(),
                is_directory: false,
                match_type: SearchMatchType::Content,
                line_number: Some(file_match.location.line),
                matched_content: Some(file_match.snippet),
                preview_before,
                preview_inside,
                preview_after,
            });
        }
    }
    file_results
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

fn split_preview(
    snippet: &str,
    matched_text: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    if matched_text.is_empty() {
        return (None, Some(snippet.to_string()), None);
    }

    if let Some(offset) = snippet.find(matched_text) {
        let before = snippet[..offset].to_string();
        let inside = matched_text.to_string();
        let after = snippet[offset + matched_text.len()..].to_string();
        return (
            (!before.is_empty()).then_some(before),
            Some(inside),
            (!after.is_empty()).then_some(after),
        );
    }

    (None, Some(snippet.to_string()), None)
}

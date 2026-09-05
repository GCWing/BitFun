use super::flashgrep::SearchResults;
use super::types::ContentSearchOutputMode;
use openbitfun_services_core::filesystem::{FileSearchResult, SearchMatchType};
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

/// Maps `search` line matches to results carrying positions only.
///
/// The daemon never sends line text on the wire, so callers that cannot read the
/// matched files themselves (the remote SSH transport) surface path + line number
/// without content. Local searches use `search/grouped_line_matches` plus
/// `line_hydration` instead, which fills in the text and the previews.
fn convert_hits_to_file_search_results(search_results: &SearchResults) -> Vec<FileSearchResult> {
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
            matched_content: None,
            preview_before: None,
            preview_inside: None,
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

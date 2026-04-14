use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::Value;

use crate::{
    config::{BuildConfig, CorpusMode, QueryConfig},
    error::{AppError, Result},
    path_filter::{PathFilter, PathFilterArgs},
    path_utils::normalize_lexical_path,
    search::{
        FileContext, FileMatch, MatchLocation, SearchHit, SearchLine, SearchMode, SearchResults,
    },
};

pub(super) fn run_rg_search(
    build_config: &BuildConfig,
    query: &QueryConfig,
    scope: &PathFilterArgs,
) -> Result<SearchResults> {
    match query.search_mode {
        SearchMode::CountOnly => run_rg_count_search(build_config, query, scope, false),
        SearchMode::CountMatches => run_rg_count_search(build_config, query, scope, true),
        SearchMode::FirstHitOnly | SearchMode::MaterializeMatches => {
            run_rg_json_search(build_config, query, scope)
        }
    }
}

pub(super) fn run_rg_glob(
    build_config: &BuildConfig,
    scope: &PathFilterArgs,
    filter: Option<&PathFilter>,
) -> Result<Vec<String>> {
    let mut command = build_rg_files_command(build_config);
    command.arg("--files");
    append_rg_scope_roots(&mut command, build_config, scope);

    let output = command.output()?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(AppError::Protocol(format!(
            "rg glob failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let mut paths = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let candidate = Path::new(line);
            let absolute = if candidate.is_absolute() {
                candidate.to_path_buf()
            } else {
                build_config.repo_path.join(candidate)
            };
            let absolute = normalize_lexical_path(&absolute);
            if filter.is_some_and(|active| !active.matches_file(&absolute)) {
                return None;
            }
            let metadata = std::fs::metadata(&absolute).ok()?;
            if !metadata.is_file() || metadata.len() > build_config.max_file_size {
                return None;
            }
            Some(absolute.to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    paths.sort_unstable();
    Ok(paths)
}

fn run_rg_count_search(
    build_config: &BuildConfig,
    query: &QueryConfig,
    scope: &PathFilterArgs,
    count_matches: bool,
) -> Result<SearchResults> {
    let (matched_lines, searches_with_match, file_counts) =
        run_rg_count_command(build_config, query, scope, false)?;
    let (matched_occurrences, file_match_counts) = if count_matches {
        let (matched_occurrences, _, file_counts) =
            run_rg_count_command(build_config, query, scope, true)?;
        (
            matched_occurrences,
            file_counts
                .into_iter()
                .map(|count| crate::search::FileMatchCount {
                    path: count.path,
                    matched_occurrences: count.matched_lines,
                })
                .collect(),
        )
    } else {
        (0, Vec::new())
    };
    Ok(SearchResults {
        candidate_docs: searches_with_match,
        searches_with_match,
        bytes_searched: 0,
        matched_lines,
        matched_occurrences,
        file_counts,
        file_match_counts,
        hits: Vec::new(),
    })
}

fn run_rg_count_command(
    build_config: &BuildConfig,
    query: &QueryConfig,
    scope: &PathFilterArgs,
    count_matches: bool,
) -> Result<(usize, usize, Vec<crate::search::FileCount>)> {
    let mut command = build_rg_base_command(build_config, query, scope);
    if count_matches {
        command.arg("--count-matches");
    } else {
        command.arg("--count");
    }

    let output = command.output()?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(AppError::Protocol(format!(
            "rg fallback failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let mut matched_total = 0usize;
    let mut searches_with_match = 0usize;
    let mut file_counts = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((path, count)) = line.rsplit_once(':') else {
            continue;
        };
        let count = count.parse::<usize>().map_err(|error| {
            AppError::Protocol(format!("failed to parse rg count output {line:?}: {error}"))
        })?;
        if count == 0 {
            continue;
        }
        matched_total += count;
        searches_with_match += 1;
        file_counts.push(crate::search::FileCount {
            path: normalize_lexical_path(&build_config.repo_path.join(path))
                .to_string_lossy()
                .into_owned(),
            matched_lines: count,
        });
    }
    Ok((matched_total, searches_with_match, file_counts))
}

fn run_rg_json_search(
    build_config: &BuildConfig,
    query: &QueryConfig,
    scope: &PathFilterArgs,
) -> Result<SearchResults> {
    let mut command = build_rg_base_command(build_config, query, scope);
    command.arg("--json");
    if query.before_context > 0 {
        command.arg("-B").arg(query.before_context.to_string());
    }
    if query.after_context > 0 {
        command.arg("-A").arg(query.after_context.to_string());
    }
    if let Some(max_count) = query.max_count {
        command.arg("-m").arg(max_count.to_string());
    } else if matches!(query.search_mode, SearchMode::FirstHitOnly) {
        command.arg("-m").arg("1");
    }

    let output = command.output()?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(AppError::Protocol(format!(
            "rg fallback failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let reader = BufReader::new(output.stdout.as_slice());
    let mut hits = Vec::<SearchHit>::new();
    let mut hit_indexes = HashMap::<String, usize>::new();
    let mut matched_lines = 0usize;
    let mut matched_occurrences = 0usize;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event: Value = serde_json::from_str(&line).map_err(|error| {
            AppError::Protocol(format!("failed to decode rg json event: {error}"))
        })?;
        let Some(kind) = event.get("type").and_then(Value::as_str) else {
            continue;
        };
        let data = event.get("data").unwrap_or(&Value::Null);
        match kind {
            "match" => {
                let Some(path) = rg_text_field(data.get("path")) else {
                    continue;
                };
                let line_number = rg_line_number(data).unwrap_or(1);
                let snippet = rg_text_field(data.get("lines")).unwrap_or_default();
                let snippet = trim_rg_newline(&snippet).to_string();
                let index = ensure_rg_hit(&mut hits, &mut hit_indexes, &path);
                push_rg_context_break_if_needed(&mut hits[index], line_number);
                let submatches = data
                    .get("submatches")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if submatches.is_empty() {
                    continue;
                }
                matched_lines += 1;
                for submatch in submatches {
                    let file_match = rg_file_match(&snippet, line_number, &submatch)
                        .unwrap_or_else(|| FileMatch {
                            location: MatchLocation {
                                line: line_number,
                                column: 1,
                            },
                            snippet: snippet.clone(),
                            matched_text: snippet.clone(),
                        });
                    hits[index].matches.push(file_match.clone());
                    hits[index].lines.push(SearchLine::Match(file_match));
                    matched_occurrences += 1;
                }
            }
            "context" => {
                let Some(path) = rg_text_field(data.get("path")) else {
                    continue;
                };
                let line_number = rg_line_number(data).unwrap_or(1);
                let snippet = rg_text_field(data.get("lines")).unwrap_or_default();
                let snippet = trim_rg_newline(&snippet).to_string();
                let index = ensure_rg_hit(&mut hits, &mut hit_indexes, &path);
                push_rg_context_break_if_needed(&mut hits[index], line_number);
                hits[index].lines.push(SearchLine::Context(FileContext {
                    line_number,
                    snippet,
                }));
            }
            _ => {}
        }
    }

    let searches_with_match = hits.iter().filter(|hit| !hit.matches.is_empty()).count();
    hits.retain(|hit| !hit.matches.is_empty());
    Ok(SearchResults {
        candidate_docs: searches_with_match,
        searches_with_match,
        bytes_searched: 0,
        matched_lines,
        matched_occurrences,
        file_counts: Vec::new(),
        file_match_counts: Vec::new(),
        hits,
    })
}

fn build_rg_base_command(
    build_config: &BuildConfig,
    query: &QueryConfig,
    scope: &PathFilterArgs,
) -> Command {
    let mut command = build_rg_scope_command(build_config, scope);
    command.arg("--with-filename");

    if query.case_insensitive {
        command.arg("--ignore-case");
    }
    if query.multiline {
        command.arg("--multiline");
    }
    if query.dot_matches_new_line {
        command.arg("--multiline-dotall");
    }
    if query.fixed_strings {
        command.arg("--fixed-strings");
        for pattern in &query.patterns {
            command.arg("-e").arg(pattern);
        }
    } else {
        if query.word_regexp {
            command.arg("--word-regexp");
        }
        if query.line_regexp {
            command.arg("--line-regexp");
        }
        command.arg("-e").arg(&query.regex_pattern);
    }

    append_rg_scope_roots(&mut command, build_config, scope);
    command
}

fn build_rg_scope_command(build_config: &BuildConfig, scope: &PathFilterArgs) -> Command {
    let mut command = Command::new("rg");
    command
        .current_dir(&build_config.repo_path)
        .arg("--color")
        .arg("never")
        .arg("--no-messages")
        .arg("--max-filesize")
        .arg(build_config.max_file_size.to_string());
    if matches!(build_config.corpus_mode, CorpusMode::NoIgnore) {
        command.arg("--no-ignore");
    }
    if build_config.include_hidden {
        command.arg("--hidden");
    }

    for glob in &scope.globs {
        command.arg("-g").arg(glob);
    }
    for glob in &scope.iglobs {
        command.arg("--iglob").arg(glob);
    }
    for spec in &scope.type_add {
        command.arg("--type-add").arg(spec);
    }
    for name in &scope.type_clear {
        command.arg("--type-clear").arg(name);
    }
    for name in &scope.types {
        command.arg("-t").arg(name);
    }
    for name in &scope.type_not {
        command.arg("-T").arg(name);
    }
    if let Some(exclude_glob) =
        rg_index_exclude_glob(&build_config.repo_path, &build_config.index_path)
    {
        command.arg("-g").arg(format!("!{exclude_glob}"));
    }
    command
}

fn build_rg_files_command(build_config: &BuildConfig) -> Command {
    let mut command = Command::new("rg");
    command
        .current_dir(&build_config.repo_path)
        .arg("--color")
        .arg("never")
        .arg("--no-messages");
    if matches!(build_config.corpus_mode, CorpusMode::NoIgnore) {
        command.arg("--no-ignore");
    }
    if build_config.include_hidden {
        command.arg("--hidden");
    }
    if let Some(exclude_glob) =
        rg_index_exclude_glob(&build_config.repo_path, &build_config.index_path)
    {
        command.arg("-g").arg(format!("!{exclude_glob}"));
    }
    command
}

fn append_rg_scope_roots(
    command: &mut Command,
    build_config: &BuildConfig,
    scope: &PathFilterArgs,
) {
    command.arg("--");
    let roots = rg_scope_roots(&build_config.repo_path, &scope.roots);
    if roots.is_empty() {
        command.arg(".");
    } else {
        for root in roots {
            command.arg(root);
        }
    }
}

fn rg_scope_roots(repo_root: &Path, roots: &[PathBuf]) -> Vec<PathBuf> {
    if roots.is_empty() {
        return Vec::new();
    }

    roots
        .iter()
        .map(|root| {
            if root.is_absolute() {
                if let Ok(relative) = root.strip_prefix(repo_root) {
                    if relative.as_os_str().is_empty() {
                        PathBuf::from(".")
                    } else {
                        relative.to_path_buf()
                    }
                } else {
                    root.clone()
                }
            } else if root.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                root.clone()
            }
        })
        .collect()
}

fn rg_index_exclude_glob(repo_root: &Path, index_path: &Path) -> Option<String> {
    let relative = index_path.strip_prefix(repo_root).ok()?;
    if relative.as_os_str().is_empty() {
        return None;
    }
    Some(format!(
        "{}/**",
        relative.to_string_lossy().replace('\\', "/")
    ))
}

fn rg_text_field(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|field| field.get("text"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn rg_line_number(data: &Value) -> Option<usize> {
    data.get("line_number")
        .and_then(Value::as_u64)
        .and_then(|line| usize::try_from(line).ok())
}

fn trim_rg_newline(value: &str) -> &str {
    value.trim_end_matches(['\r', '\n'])
}

fn ensure_rg_hit(
    hits: &mut Vec<SearchHit>,
    indexes: &mut HashMap<String, usize>,
    path: &str,
) -> usize {
    if let Some(index) = indexes.get(path).copied() {
        return index;
    }
    let index = hits.len();
    hits.push(SearchHit {
        path: path.to_string(),
        matches: Vec::new(),
        lines: Vec::new(),
    });
    indexes.insert(path.to_string(), index);
    index
}

fn push_rg_context_break_if_needed(hit: &mut SearchHit, line_number: usize) {
    let previous_line = hit.lines.iter().rev().find_map(|line| match line {
        SearchLine::Match(value) => Some(value.location.line),
        SearchLine::Context(value) => Some(value.line_number),
        SearchLine::ContextBreak => None,
    });
    if previous_line.is_some_and(|previous| line_number > previous.saturating_add(1))
        && !matches!(hit.lines.last(), Some(SearchLine::ContextBreak))
    {
        hit.lines.push(SearchLine::ContextBreak);
    }
}

fn rg_file_match(snippet: &str, line_number: usize, submatch: &Value) -> Option<FileMatch> {
    let start = submatch.get("start")?.as_u64()?;
    let end = submatch.get("end")?.as_u64()?;
    let start = usize::try_from(start).ok()?;
    let end = usize::try_from(end).ok()?;
    let matched_text = submatch
        .get("match")
        .and_then(|value| value.get("text"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| snippet.get(start..end).map(ToOwned::to_owned))
        .unwrap_or_default();
    let column = snippet
        .get(..start)
        .map(|prefix| prefix.chars().count() + 1)
        .unwrap_or(1);
    Some(FileMatch {
        location: MatchLocation {
            line: line_number,
            column,
        },
        snippet: snippet.to_string(),
        matched_text,
    })
}

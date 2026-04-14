use std::{collections::HashSet, process::Command};

use crate::{
    config::QueryConfig,
    error::{AppError, Result},
    path_filter::PathFilter,
    search::{SearchMode, SearchResults},
    tokenizer::create,
};

use super::IndexSearcher;

impl IndexSearcher {
    pub(super) fn should_use_rg_scan_backend(
        &self,
        config: &QueryConfig,
        plan: &crate::planner::QueryPlan,
        filter: Option<&PathFilter>,
        allowed_paths: Option<&HashSet<String>>,
        excluded_paths: Option<&HashSet<String>>,
    ) -> Result<bool> {
        if !cfg!(target_os = "linux")
            || !matches!(
                config.search_mode,
                SearchMode::CountOnly | SearchMode::CountMatches
            )
            || config.has_context()
            || config.max_count.is_some()
            || config.multiline
            || config.dot_matches_new_line
            || filter.is_some()
            || allowed_paths.is_some()
            || excluded_paths.is_some_and(|paths| !paths.is_empty())
            || self.repo_root.is_none()
        {
            return Ok(false);
        }

        if plan.fallback_to_scan {
            return Ok(true);
        }

        let tokenizer = create(self.tokenizer_mode, self.tokenizer_options.clone());
        Ok(self
            .try_collect_candidates_without_literal_scan(
                &plan.branches,
                tokenizer.as_ref(),
                config.top_k_tokens,
                None,
                None,
            )?
            .is_none())
    }

    pub(super) fn search_counts_with_rg(
        &self,
        config: &QueryConfig,
    ) -> Result<Option<SearchResults>> {
        let Some((matched_lines, searches_with_match, file_counts)) =
            self.run_rg_count_query(config, false)?
        else {
            return Ok(None);
        };
        let (matched_occurrences, file_match_counts) =
            if matches!(config.search_mode, SearchMode::CountMatches) {
                let Some((matched_occurrences, _, file_counts)) =
                    self.run_rg_count_query(config, true)?
                else {
                    return Ok(None);
                };
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

        Ok(Some(SearchResults {
            candidate_docs: self.docs.len(),
            searches_with_match,
            bytes_searched: self.docs.iter().map(|doc| doc.size()).sum(),
            matched_lines,
            matched_occurrences,
            file_counts,
            file_match_counts,
            hits: Vec::new(),
        }))
    }

    fn run_rg_count_query(
        &self,
        config: &QueryConfig,
        count_matches: bool,
    ) -> Result<Option<(usize, usize, Vec<crate::search::FileCount>)>> {
        let Some(repo_root) = self.repo_root.as_ref() else {
            return Ok(None);
        };
        let mut command = Command::new("rg");
        command
            .current_dir(repo_root)
            .arg("--with-filename")
            .arg("--color")
            .arg("never")
            .arg("--no-messages");
        if count_matches {
            command.arg("--count-matches");
        } else {
            command.arg("--count");
        }
        if config.case_insensitive {
            command.arg("--ignore-case");
        }
        if config.fixed_strings {
            command.arg("--fixed-strings");
            for pattern in &config.patterns {
                command.arg("-e").arg(pattern);
            }
        } else {
            if config.word_regexp {
                command.arg("--word-regexp");
            }
            if config.line_regexp {
                command.arg("--line-regexp");
            }
            command.arg("-e").arg(&config.regex_pattern);
        }

        if let Some(build) = self.build_settings() {
            command
                .arg("--max-filesize")
                .arg(build.max_file_size.to_string());
            if matches!(build.corpus_mode, crate::config::CorpusMode::NoIgnore) {
                command.arg("--no-ignore");
            }
            if build.include_hidden {
                command.arg("--hidden");
            }
        }

        command.arg("--");
        command.arg(".");

        let output = match command.output() {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if !output.status.success() && output.status.code() != Some(1) {
            return Err(AppError::Protocol(format!(
                "rg scan backend failed with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
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
                path: path.to_string(),
                matched_lines: count,
            });
        }

        Ok(Some((matched_total, searches_with_match, file_counts)))
    }
}

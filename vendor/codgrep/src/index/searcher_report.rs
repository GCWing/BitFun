use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Instant,
};

use crate::{
    config::QueryConfig,
    error::Result,
    index::format::DocMetaRef,
    path_filter::PathFilter,
    planner::plan,
    search::{FileCount, FileMatchCount},
};

use super::{
    doc_by_id, query, requires_multiline_verification, CountVerifyKind, CountVerifyPlan,
    IndexSearcher, SearchProfile,
};

impl IndexSearcher {
    pub fn count_matches_by_file(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
    ) -> Result<Vec<FileCount>> {
        self.count_matches_by_file_internal(config, filter, None, false)
    }

    pub fn count_matches_by_file_including_zero(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
    ) -> Result<Vec<FileCount>> {
        self.count_matches_by_file_internal(config, filter, None, true)
    }

    fn count_matches_by_file_internal(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
        allowed_paths: Option<&HashSet<String>>,
        include_zero: bool,
    ) -> Result<Vec<FileCount>> {
        let profile = SearchProfile::enabled();
        let plan_started = Instant::now();
        let plan = plan(&config.regex_pattern)?;
        profile.record_plan(plan_started.elapsed());
        let line_prefilter = query::LinePrefilter::compile(config, &plan);

        let candidate_ids = self
            .prepare_indexed_search(config, &plan, filter, allowed_paths, &profile)?
            .candidate_ids;
        let candidate_ids = self.filter_candidate_ids(&candidate_ids, filter, allowed_paths)?;
        let multiline_verifier = requires_multiline_verification(config)?;
        let regex_started = Instant::now();
        let verify_plan = CountVerifyPlan::compile(
            config,
            multiline_verifier,
            line_prefilter,
            CountVerifyKind::Lines,
        )?;
        profile.record_regex_compile(regex_started.elapsed());
        let resolve_path = |doc_id| self.resolve_doc_path_by_id(doc_id);
        let match_started = Instant::now();
        let counts = verify_plan.verify_candidate_counts_by_doc(
            &candidate_ids,
            self.docs.len(),
            &resolve_path,
        )?;
        profile.record_verify(match_started.elapsed());
        if include_zero {
            self.file_counts_from_doc_counts_including_zero(counts, filter, allowed_paths)
        } else {
            self.file_counts_from_doc_counts(counts)
        }
    }

    pub fn count_total_matches_by_file(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
    ) -> Result<Vec<FileMatchCount>> {
        self.count_total_matches_by_file_internal(config, filter, None, false)
    }

    pub fn count_total_matches_by_file_including_zero(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
    ) -> Result<Vec<FileMatchCount>> {
        self.count_total_matches_by_file_internal(config, filter, None, true)
    }

    fn count_total_matches_by_file_internal(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
        allowed_paths: Option<&HashSet<String>>,
        include_zero: bool,
    ) -> Result<Vec<FileMatchCount>> {
        let profile = SearchProfile::enabled();
        let plan_started = Instant::now();
        let plan = plan(&config.regex_pattern)?;
        profile.record_plan(plan_started.elapsed());

        let preparation =
            self.prepare_indexed_search(config, &plan, filter, allowed_paths, &profile)?;
        let candidate_ids = preparation.candidate_ids;
        let candidate_ids = self.filter_candidate_ids(&candidate_ids, filter, allowed_paths)?;
        let multiline_verifier = requires_multiline_verification(config)?;
        let regex_started = Instant::now();
        let verify_plan = CountVerifyPlan::compile(
            config,
            multiline_verifier,
            preparation.line_prefilter,
            CountVerifyKind::Occurrences,
        )?;
        profile.record_regex_compile(regex_started.elapsed());
        let match_started = Instant::now();
        let resolve_path = |doc_id| self.resolve_doc_path_by_id(doc_id);
        let counts = verify_plan.verify_candidate_counts_by_doc(
            &candidate_ids,
            self.docs.len(),
            &resolve_path,
        )?;
        profile.record_verify(match_started.elapsed());

        let counts = counts
            .into_iter()
            .map(|task| {
                Ok(FileMatchCount {
                    path: self.doc_display_path_ref(doc_by_id(&self.docs, task.doc_id)?),
                    matched_occurrences: task.matched_occurrences,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if include_zero {
            self.file_match_counts_including_zero(counts, filter, allowed_paths)
        } else {
            Ok(counts
                .into_iter()
                .filter(FileMatchCount::has_match)
                .collect::<Vec<_>>())
        }
    }

    pub fn files_with_matches(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
    ) -> Result<Vec<String>> {
        self.files_with_matches_and_allowed_paths(config, filter, None)
    }

    pub(crate) fn files_with_matches_and_allowed_paths(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
        allowed_paths: Option<&HashSet<String>>,
    ) -> Result<Vec<String>> {
        Ok(self
            .count_matches_by_file_internal(config, filter, allowed_paths, false)?
            .into_iter()
            .map(|count| count.path)
            .collect())
    }

    pub fn files_without_matches(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
    ) -> Result<Vec<String>> {
        self.files_without_matches_and_allowed_paths(config, filter, None)
    }

    pub(crate) fn files_without_matches_and_allowed_paths(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
        allowed_paths: Option<&HashSet<String>>,
    ) -> Result<Vec<String>> {
        let matched = self
            .files_with_matches_and_allowed_paths(config, filter, allowed_paths)?
            .into_iter()
            .collect::<HashSet<_>>();
        Ok(self
            .indexed_paths_with_allowed_paths(filter, allowed_paths)
            .into_iter()
            .filter(|path| !matched.contains(path))
            .collect())
    }

    pub fn indexed_paths(&self, filter: Option<&PathFilter>) -> Vec<String> {
        self.indexed_paths_with_allowed_paths(filter, None)
    }

    pub fn indexed_doc_stats(&self, filter: Option<&PathFilter>) -> (usize, u64) {
        self.docs
            .iter()
            .filter(|doc| self.doc_matches_filters(*doc, filter, None))
            .fold((0usize, 0u64), |(count, bytes), doc| {
                (count + 1, bytes + doc.size())
            })
    }

    pub fn indexed_path_count(&self, filter: Option<&PathFilter>) -> usize {
        self.docs
            .iter()
            .filter(|doc| self.doc_matches_filters(*doc, filter, None))
            .count()
    }

    #[cfg(test)]
    pub(crate) fn doc_display_path_by_id(&self, doc_id: u32) -> Result<String> {
        Ok(self.doc_display_path_ref(doc_by_id(&self.docs, doc_id)?))
    }

    fn file_counts_from_doc_counts(
        &self,
        counts: Vec<super::count::DocMatchCount>,
    ) -> Result<Vec<FileCount>> {
        counts
            .into_iter()
            .map(|count| {
                Ok(FileCount {
                    path: self.doc_display_path_ref(doc_by_id(&self.docs, count.doc_id)?),
                    matched_lines: count.matched_lines,
                })
            })
            .collect()
    }

    fn file_counts_from_doc_counts_including_zero(
        &self,
        counts: Vec<super::count::DocMatchCount>,
        filter: Option<&PathFilter>,
        allowed_paths: Option<&HashSet<String>>,
    ) -> Result<Vec<FileCount>> {
        let matched = self.file_counts_from_doc_counts(counts)?;
        let matched = matched
            .into_iter()
            .map(|count| (count.path.clone(), count))
            .collect::<HashMap<_, _>>();
        Ok(self
            .indexed_paths_with_allowed_paths(filter, allowed_paths)
            .into_iter()
            .map(|path| {
                matched.get(&path).cloned().unwrap_or(FileCount {
                    path,
                    matched_lines: 0,
                })
            })
            .collect())
    }

    fn file_match_counts_including_zero(
        &self,
        counts: Vec<FileMatchCount>,
        filter: Option<&PathFilter>,
        allowed_paths: Option<&HashSet<String>>,
    ) -> Result<Vec<FileMatchCount>> {
        let matched = counts
            .into_iter()
            .map(|count| (count.path.clone(), count))
            .collect::<HashMap<_, _>>();
        Ok(self
            .indexed_paths_with_allowed_paths(filter, allowed_paths)
            .into_iter()
            .map(|path| {
                matched.get(&path).cloned().unwrap_or(FileMatchCount {
                    path,
                    matched_occurrences: 0,
                })
            })
            .collect())
    }

    pub(super) fn doc_matches_filters(
        &self,
        doc: DocMetaRef<'_>,
        filter: Option<&PathFilter>,
        allowed_paths: Option<&HashSet<String>>,
    ) -> bool {
        self.doc_matches_scope(doc, filter, allowed_paths, None)
    }

    pub(super) fn doc_matches_scope(
        &self,
        doc: DocMetaRef<'_>,
        filter: Option<&PathFilter>,
        allowed_paths: Option<&HashSet<String>>,
        excluded_paths: Option<&HashSet<String>>,
    ) -> bool {
        let display_path = self.doc_display_path_ref(doc);
        if filter.is_some_and(|filter| !filter.matches_file(Path::new(&display_path))) {
            return false;
        }
        if allowed_paths.is_some_and(|allowed_paths| !allowed_paths.contains(&display_path)) {
            return false;
        }
        excluded_paths.is_none_or(|excluded_paths| !excluded_paths.contains(&display_path))
    }
}

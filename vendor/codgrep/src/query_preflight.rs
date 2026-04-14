use std::{collections::HashSet, env};

use crate::{
    config::{QueryConfig, TokenizerMode},
    error::Result,
    index::IndexSearcher,
    path_filter::PathFilter,
    planner::plan,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct QueryPreflightMetrics {
    pub(crate) doc_count: usize,
    pub(crate) branch_candidate_counts: Vec<usize>,
    pub(crate) union_candidate_count: usize,
    pub(crate) branch_candidate_bytes: Vec<u64>,
    pub(crate) union_candidate_bytes: u64,
}

impl QueryPreflightMetrics {
    fn from_branch_candidates(
        searcher: &IndexSearcher,
        branch_candidates: &[Vec<u32>],
    ) -> Result<Self> {
        let branch_candidate_counts = branch_candidates.iter().map(Vec::len).collect::<Vec<_>>();
        let branch_candidate_bytes = branch_candidates
            .iter()
            .map(|candidate_ids| searcher.sum_doc_sizes(candidate_ids))
            .collect::<Result<Vec<_>>>()?;
        let union_candidate_ids = branch_candidates
            .iter()
            .flat_map(|candidate_ids| candidate_ids.iter().copied())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let union_candidate_count = union_candidate_ids.len();
        let union_candidate_bytes = searcher.sum_doc_sizes(&union_candidate_ids)?;

        Ok(Self {
            doc_count: searcher.doc_count(),
            branch_candidate_counts,
            union_candidate_count,
            branch_candidate_bytes,
            union_candidate_bytes,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueryPreflightReason {
    NonSparseTokenizer,
    CandidatesWithinLimits,
    PlannerFallbackToScan,
    LiteralScanRequired,
    BranchCandidatesExceeded,
    UnionCandidatesExceeded,
    BranchBytesExceeded,
    UnionBytesExceeded,
}

impl QueryPreflightReason {
    pub(crate) fn requires_scan_backend(self) -> bool {
        matches!(
            self,
            Self::PlannerFallbackToScan | Self::LiteralScanRequired
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueryPreflightDecision {
    pub(crate) reason: QueryPreflightReason,
    pub(crate) metrics: QueryPreflightMetrics,
    pub(crate) detail: String,
}

impl QueryPreflightDecision {
    fn new(
        reason: QueryPreflightReason,
        metrics: QueryPreflightMetrics,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            reason,
            metrics,
            detail: detail.into(),
        }
    }
}

pub(crate) fn evaluate_index_query_preflight(
    searcher: &IndexSearcher,
    config: &QueryConfig,
    filter: Option<&PathFilter>,
    allowed_paths: Option<&HashSet<String>>,
) -> Result<QueryPreflightDecision> {
    let plan = plan(&config.regex_pattern)?;
    if plan.fallback_to_scan {
        return Ok(QueryPreflightDecision::new(
            QueryPreflightReason::PlannerFallbackToScan,
            QueryPreflightMetrics {
                doc_count: searcher.doc_count(),
                ..QueryPreflightMetrics::default()
            },
            "planner requested fallback_to_scan",
        ));
    }

    if searcher.tokenizer_mode() != TokenizerMode::SparseNgram {
        return Ok(QueryPreflightDecision::new(
            QueryPreflightReason::NonSparseTokenizer,
            QueryPreflightMetrics {
                doc_count: searcher.doc_count(),
                ..QueryPreflightMetrics::default()
            },
            "tokenizer is not sparse-ngram",
        ));
    }

    let Some(branch_candidates) = searcher
        .try_candidate_doc_ids_by_branch_without_literal_scan_for_plan_with_allowed_paths(
            config,
            &plan,
            filter,
            allowed_paths,
        )?
    else {
        return Ok(QueryPreflightDecision::new(
            QueryPreflightReason::LiteralScanRequired,
            QueryPreflightMetrics {
                doc_count: searcher.doc_count(),
                ..QueryPreflightMetrics::default()
            },
            "candidate preflight required literal scan",
        ));
    };

    let metrics = QueryPreflightMetrics::from_branch_candidates(searcher, &branch_candidates)?;
    Ok(evaluate_metrics(metrics, config.search_mode))
}

pub(crate) fn preflight_enabled() -> bool {
    env::var("BITFUN_ENABLE_PREFLIGHT")
        .map(|value| !value.is_empty() && value != "0")
        .unwrap_or(false)
}

fn evaluate_metrics(
    metrics: QueryPreflightMetrics,
    search_mode: crate::search::SearchMode,
) -> QueryPreflightDecision {
    let branch_limit = preflight_branch_limit(metrics.doc_count);
    let union_limit = preflight_union_limit(metrics.doc_count);
    let branch_byte_limit = preflight_branch_byte_limit(search_mode);
    let union_byte_limit = preflight_union_byte_limit(search_mode);

    for (index, candidate_count) in metrics.branch_candidate_counts.iter().enumerate() {
        if *candidate_count > branch_limit {
            let detail = format!(
                "branch {index} candidates {candidate_count} exceeded limit {branch_limit}; branch_sizes={:?}",
                metrics.branch_candidate_counts
            );
            return QueryPreflightDecision::new(
                QueryPreflightReason::BranchCandidatesExceeded,
                metrics,
                detail,
            );
        }

        let candidate_bytes = metrics.branch_candidate_bytes[index];
        if candidate_bytes > branch_byte_limit {
            let detail = format!(
                "branch {index} candidate bytes {candidate_bytes} exceeded limit {branch_byte_limit}; branch_sizes={:?}; branch_bytes={:?}",
                metrics.branch_candidate_counts,
                metrics.branch_candidate_bytes
            );
            return QueryPreflightDecision::new(
                QueryPreflightReason::BranchBytesExceeded,
                metrics,
                detail,
            );
        }
    }

    if metrics.union_candidate_count > union_limit {
        let detail = format!(
            "union candidates {} exceeded limit {union_limit}; branch_sizes={:?}",
            metrics.union_candidate_count, metrics.branch_candidate_counts
        );
        return QueryPreflightDecision::new(
            QueryPreflightReason::UnionCandidatesExceeded,
            metrics,
            detail,
        );
    }

    if metrics.union_candidate_bytes > union_byte_limit {
        let detail = format!(
            "union candidate bytes {} exceeded limit {union_byte_limit}; branch_sizes={:?}; branch_bytes={:?}",
            metrics.union_candidate_bytes,
            metrics.branch_candidate_counts,
            metrics.branch_candidate_bytes
        );
        return QueryPreflightDecision::new(
            QueryPreflightReason::UnionBytesExceeded,
            metrics,
            detail,
        );
    }

    let detail = format!(
        "candidate preflight stayed within limits (branch<={branch_limit}, union<={union_limit}, branch_bytes<={branch_byte_limit}, union_bytes<={union_byte_limit}, branch_sizes={:?}, union_size={}, branch_bytes={:?}, union_bytes={})",
        metrics.branch_candidate_counts,
        metrics.union_candidate_count,
        metrics.branch_candidate_bytes,
        metrics.union_candidate_bytes
    );
    QueryPreflightDecision::new(
        QueryPreflightReason::CandidatesWithinLimits,
        metrics,
        detail,
    )
}

fn preflight_branch_limit(doc_count: usize) -> usize {
    (doc_count / 8).max(20_000)
}

fn preflight_union_limit(doc_count: usize) -> usize {
    (doc_count / 28).max(5_000)
}

fn preflight_branch_byte_limit(search_mode: crate::search::SearchMode) -> u64 {
    preflight_limit_from_env(
        "BITFUN_PREFLIGHT_BRANCH_BYTES",
        if search_mode.materializes_matches() {
            64 * 1024 * 1024
        } else {
            128 * 1024 * 1024
        },
    )
}

fn preflight_union_byte_limit(search_mode: crate::search::SearchMode) -> u64 {
    preflight_limit_from_env(
        "BITFUN_PREFLIGHT_UNION_BYTES",
        if search_mode.materializes_matches() {
            96 * 1024 * 1024
        } else {
            192 * 1024 * 1024
        },
    )
}

fn preflight_limit_from_env(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::{evaluate_metrics, QueryPreflightMetrics, QueryPreflightReason};
    use crate::search::SearchMode;

    #[test]
    fn preflight_uses_branch_candidate_limit() {
        let decision = evaluate_metrics(
            QueryPreflightMetrics {
                doc_count: 120_000,
                branch_candidate_counts: vec![7_089, 6_461, 40_796],
                union_candidate_count: 54_346,
                branch_candidate_bytes: vec![4, 5, 6],
                union_candidate_bytes: 15,
            },
            SearchMode::MaterializeMatches,
        );

        assert_eq!(
            decision.reason,
            QueryPreflightReason::BranchCandidatesExceeded
        );
        assert!(decision
            .detail
            .contains("branch 2 candidates 40796 exceeded limit 20000"));
        assert!(decision.detail.contains("branch_sizes=[7089, 6461, 40796]"));
    }

    #[test]
    fn preflight_uses_union_candidate_limit() {
        let decision = evaluate_metrics(
            QueryPreflightMetrics {
                doc_count: 120_000,
                branch_candidate_counts: vec![8_000, 9_001, 7_000],
                union_candidate_count: 5_001,
                branch_candidate_bytes: vec![4, 5, 6],
                union_candidate_bytes: 15,
            },
            SearchMode::MaterializeMatches,
        );

        assert_eq!(
            decision.reason,
            QueryPreflightReason::UnionCandidatesExceeded
        );
        assert!(decision
            .detail
            .contains("union candidates 5001 exceeded limit 5000"));
        assert!(decision.detail.contains("branch_sizes=[8000, 9001, 7000]"));
    }

    #[test]
    fn preflight_uses_branch_byte_limit() {
        let decision = evaluate_metrics(
            QueryPreflightMetrics {
                doc_count: 1_000,
                branch_candidate_counts: vec![200],
                union_candidate_count: 200,
                branch_candidate_bytes: vec![64 * 1024 * 1024 + 1],
                union_candidate_bytes: 64 * 1024 * 1024 + 1,
            },
            SearchMode::MaterializeMatches,
        );

        assert_eq!(decision.reason, QueryPreflightReason::BranchBytesExceeded);
        assert!(decision
            .detail
            .contains("branch 0 candidate bytes 67108865 exceeded limit 67108864"));
    }

    #[test]
    fn preflight_keeps_indexed_for_narrow_candidate_sets() {
        let decision = evaluate_metrics(
            QueryPreflightMetrics {
                doc_count: 120_000,
                branch_candidate_counts: vec![2_182, 3_126, 3],
                union_candidate_count: 4_628,
                branch_candidate_bytes: vec![12, 23, 34],
                union_candidate_bytes: 56,
            },
            SearchMode::MaterializeMatches,
        );

        assert_eq!(
            decision.reason,
            QueryPreflightReason::CandidatesWithinLimits
        );
        assert!(decision.detail.contains("union_size=4628"));
    }

    #[test]
    fn preflight_uses_higher_byte_limit_for_non_materializing_modes() {
        let decision = evaluate_metrics(
            QueryPreflightMetrics {
                doc_count: 1_000,
                branch_candidate_counts: vec![200],
                union_candidate_count: 200,
                branch_candidate_bytes: vec![64 * 1024 * 1024 + 1],
                union_candidate_bytes: 64 * 1024 * 1024 + 1,
            },
            SearchMode::CountMatches,
        );

        assert_eq!(
            decision.reason,
            QueryPreflightReason::CandidatesWithinLimits
        );
        assert!(decision.detail.contains("branch_bytes<="));
    }
}

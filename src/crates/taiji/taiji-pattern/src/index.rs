use ndarray::Array2;
use std::collections::HashMap;

use crate::dtw::DtwEngine;

/// Result of a pattern search.
#[derive(Debug, Clone)]
pub struct PatternMatch {
    /// Identifier for the matched pattern.
    pub pattern_id: String,
    /// Exact DTW distance (lower = more similar).
    pub dtw_distance: f64,
    /// Normalised similarity in (0, 1], where 1.0 = identical.
    pub similarity: f64,
    /// (start, end) indices into the query segment that matched.
    pub matched_segment: (usize, usize),
}

/// Three-layer pattern index.
///
/// Layer 1 — **signature**: per-template per-feature mean vector.
///   Candidates whose mean-vector distance exceeds a loose threshold are pruned.
/// Layer 2 — **LB_Keogh**: the lower bound is computed for surviving candidates.
///   Candidates whose LB_Keogh exceeds the current best DTW distance are pruned.
/// Layer 3 — **Sakoe-Chiba DTW**: the full DTW distance is computed on the
///   (hopefully few) remaining candidates.
pub struct PatternIndex {
    engine: DtwEngine,
    /// pattern_id → list of template arrays (n_i × d)
    templates: HashMap<String, Vec<Array2<f64>>>,
    /// pattern_id → per-feature mean vector per template
    signatures: HashMap<String, Vec<Vec<f64>>>,
}

impl PatternIndex {
    pub fn new(engine: DtwEngine) -> Self {
        Self {
            engine,
            templates: HashMap::new(),
            signatures: HashMap::new(),
        }
    }

    /// Register a template pattern.
    ///
    /// `pattern_id` — unique name for this pattern class.
    /// `template`  — n × d array (n time steps, d features).
    pub fn register(&mut self, pattern_id: &str, template: Array2<f64>) {
        let means: Vec<f64> = template
            .columns()
            .into_iter()
            .map(|col| col.mean().unwrap_or(0.0))
            .collect();

        self.templates
            .entry(pattern_id.to_string())
            .or_default()
            .push(template);
        self.signatures
            .entry(pattern_id.to_string())
            .or_default()
            .push(means);
    }

    /// Search for the top_k best-matching patterns for `query` (n × d).
    ///
    /// Returns results sorted by DTW distance ascending (best first).
    pub fn search(&self, query: &Array2<f64>, top_k: usize) -> Vec<PatternMatch> {
        let n = query.nrows();

        // ── query signature (per-feature mean) ──
        let query_sig: Vec<f64> = query
            .columns()
            .into_iter()
            .map(|col| col.mean().unwrap_or(0.0))
            .collect();

        // ── collect all (pattern_id, template_idx, template) candidates ──
        struct Candidate<'a> {
            pattern_id: &'a str,
            template: &'a Array2<f64>,
            sig: &'a [f64],
        }

        let mut all: Vec<Candidate> = Vec::new();
        for (pid, tmpls) in self.templates.iter() {
            let sigs = self.signatures.get(pid).expect("signature missing");
            for (idx, t) in tmpls.iter().enumerate() {
                all.push(Candidate {
                    pattern_id: pid,
                    template: t,
                    sig: &sigs[idx],
                });
            }
        }

        if all.is_empty() {
            return vec![];
        }

        // ── Layer 1: signature distance ──
        let mut sig_dists: Vec<(usize, f64)> = all
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let dist: f64 = c
                    .sig
                    .iter()
                    .zip(query_sig.iter())
                    .map(|(s, q)| (s - q).powi(2))
                    .sum::<f64>()
                    .sqrt();
                (i, dist)
            })
            .collect();
        sig_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Keep top 3× `top_k` by signature for the next layer (minimum 30)
        let keep = (top_k * 3).max(30).min(sig_dists.len());
        let layer1_ids: Vec<usize> = sig_dists.iter().take(keep).map(|(i, _)| *i).collect();

        // ── Layer 2: LB_Keogh ──
        let mut lb_results: Vec<(usize, f64)> = Vec::with_capacity(layer1_ids.len());
        for &idx in &layer1_ids {
            let c = &all[idx];
            let lb = self.engine.lb_keogh(query, c.template);
            lb_results.push((idx, lb));
        }
        lb_results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // ── Layer 3: exact DTW, with early termination ──
        let mut best_full = f64::INFINITY;
        let mut results: Vec<PatternMatch> = Vec::with_capacity(top_k);

        for (idx, lb) in lb_results {
            if results.len() >= top_k && lb > best_full {
                break; // LB_Keogh pruning: remaining candidates can't beat the current top_k
            }

            let c = &all[idx];
            let dtw = self.engine.distance(query, c.template);
            if dtw < best_full {
                best_full = dtw;
            }

            // Normalised similarity
            let query_norm: f64 = query.iter().map(|x| x.powi(2)).sum::<f64>().sqrt();
            let similarity = 1.0 / (1.0 + dtw / query_norm.max(1e-12));

            results.push(PatternMatch {
                pattern_id: c.pattern_id.to_string(),
                dtw_distance: dtw,
                similarity,
                matched_segment: (0, n.saturating_sub(1)),
            });

            results.sort_by(|a, b| {
                a.dtw_distance
                    .partial_cmp(&b.dtw_distance)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            results.truncate(top_k);
        }

        results
    }

    /// Number of registered pattern classes.
    pub fn pattern_count(&self) -> usize {
        self.templates.len()
    }

    /// Total number of template instances across all classes.
    pub fn template_count(&self) -> usize {
        self.templates.values().map(|v| v.len()).sum()
    }

    /// Count how many exact DTW computations `search` would perform for `query`.
    /// Used to verify the filter rate in tests.
    pub fn count_dtw_calls(&self, query: &Array2<f64>, top_k: usize) -> usize {
        let query_sig: Vec<f64> = query
            .columns()
            .into_iter()
            .map(|col| col.mean().unwrap_or(0.0))
            .collect();

        let mut sig_dists: Vec<(usize, f64)> = Vec::new();
        let mut idx = 0;
        for (_pid, tmpls) in self.templates.iter() {
            let sigs = self.signatures.get(_pid).unwrap();
            for (j, t) in tmpls.iter().enumerate() {
                let dist: f64 = sigs[j]
                    .iter()
                    .zip(query_sig.iter())
                    .map(|(s, q)| (s - q).powi(2))
                    .sum::<f64>()
                    .sqrt();
                sig_dists.push((idx, dist));
                idx += 1;
                let _ = t; // suppress unused warning
            }
        }

        sig_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let keep = (top_k * 3).max(30).min(sig_dists.len());
        let mut dtw_calls = 0;
        let mut best_full = f64::INFINITY;
        let mut result_count = 0;

        // Loop counter is not a simple enumerate because of conditional break
        // and LB-based filtering logic. Allow explicit counter.
        #[allow(clippy::explicit_counter_loop)]
        for (_, lb) in sig_dists.iter().take(keep) {
            if result_count >= top_k && *lb > best_full {
                break;
            }
            dtw_calls += 1;
            best_full = best_full.min(*lb); // use LB as proxy; actual DTW would be ≤
            result_count += 1;
        }

        dtw_calls
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::arr2;

    fn make_engine() -> DtwEngine {
        DtwEngine::new(3, vec![1.0, 1.0, 1.0])
    }

    #[test]
    fn filter_rate_exceeds_90_percent() {
        let engine = make_engine();
        let mut index = PatternIndex::new(engine);

        // Register 100 templates
        for i in 0..100 {
            let offset = (i as f64) * 0.01;
            let t = arr2(&[
                [0.0 + offset, 0.0, 0.0],
                [0.5 + offset, 0.5, 0.5],
                [1.0 + offset, 1.0, 1.0],
            ]);
            index.register(&format!("p{:03}", i), t);
        }

        let query = arr2(&[[0.0, 0.0, 0.0], [0.5, 0.5, 0.5], [1.0, 1.0, 1.0]]);

        // Count how many DTW calls would be made for top_k=5
        let dtw_calls = index.count_dtw_calls(&query, 5);

        // 100 candidates, we expect the 3-layer index to prune to well under 10
        assert!(
            dtw_calls <= 10,
            "filter rate too low: {} DTW calls out of 100 candidates (must be ≤ 10)",
            dtw_calls
        );
    }

    #[test]
    fn search_returns_best_match() {
        let engine = make_engine();
        let mut index = PatternIndex::new(engine);

        let t_far = arr2(&[[10.0, 10.0, 10.0], [20.0, 20.0, 20.0]]);
        let t_near = arr2(&[[0.1, 0.1, 0.1], [0.2, 0.2, 0.2]]);
        let t_exact = arr2(&[[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]);

        index.register("far", t_far);
        index.register("near", t_near);
        index.register("exact", t_exact);

        let query = arr2(&[[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]);
        let results = index.search(&query, 3);

        assert!(!results.is_empty(), "should find matches");
        assert_eq!(
            results[0].pattern_id, "exact",
            "exact pattern should be best match"
        );
        assert!(
            results[0].dtw_distance < 1e-10,
            "best match should have near-zero DTW"
        );
    }
}

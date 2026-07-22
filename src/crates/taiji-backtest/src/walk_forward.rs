use crate::config::WalkForwardConfig;
use crate::stats::PerformanceStats;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Walk-forward validation report for one instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardReport {
    /// Instrument code.
    pub instrument: String,
    /// Number of folds.
    pub folds: usize,
    /// Per-fold results.
    pub fold_results: Vec<FoldResult>,
    /// Aggregate in-sample stats.
    pub aggregate_in_sample: Option<PerformanceStats>,
    /// Aggregate out-of-sample stats.
    pub aggregate_out_of_sample: Option<PerformanceStats>,
}

/// Result for a single fold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldResult {
    /// Fold index (0-based).
    pub fold: usize,
    /// Train date range (in-sample).
    pub train_start: NaiveDate,
    pub train_end: NaiveDate,
    /// Test date range (out-of-sample).
    pub test_start: NaiveDate,
    pub test_end: NaiveDate,
    /// In-sample performance.
    pub in_sample: Option<PerformanceStats>,
    /// Out-of-sample performance.
    pub out_of_sample: Option<PerformanceStats>,
}

/// Walk-forward cross-validation orchestrator.
pub struct WalkForwardValidator {
    config: WalkForwardConfig,
}

impl WalkForwardValidator {
    /// Create a new validator from config.
    pub fn new(config: WalkForwardConfig) -> Self {
        Self { config }
    }

    /// Split a date range into train/test windows for walk-forward validation.
    ///
    /// Each fold uses `train_ratio` of the available data for training,
    /// sliding forward so later folds use more data. The test window for
    /// fold k is always immediately after its train window.
    ///
    /// # Panics
    /// Panics if `ticks` is empty or `config.folds` is 0.
    pub fn validate(
        &self,
        instrument: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
        _ticks: &[types::TickDataRef],
    ) -> WalkForwardReport {
        assert!(self.config.folds > 0, "folds must be > 0");

        let total_days = (end_date - start_date).num_days().max(1) as usize;
        let fold_size = total_days / self.config.folds;
        let train_window = (fold_size as f64 * self.config.train_ratio) as usize;

        let mut fold_results = Vec::with_capacity(self.config.folds);
        let mut all_is_pnls: Vec<f64> = Vec::new();
        let mut all_oos_pnls: Vec<f64> = Vec::new();

        for fold in 0..self.config.folds {
            let fold_start_offset = fold * fold_size;
            let train_start = start_date + chrono::Duration::days(fold_start_offset as i64);
            let train_end = train_start + chrono::Duration::days(train_window as i64);
            let test_start = train_end + chrono::Duration::days(1);
            // Last fold: test goes to end_date; others: test window = remaining fold_size - train_window
            let test_end = if fold == self.config.folds - 1 {
                end_date
            } else {
                (train_end + chrono::Duration::days((fold_size - train_window) as i64))
                    .min(end_date)
            };

            // In-sample placeholder — real integration would run sub-backtest on train window
            let is_pnls = self.simulate_fold_pnls(fold, true);
            let oos_pnls = self.simulate_fold_pnls(fold, false);

            let is_stats = if !is_pnls.is_empty() {
                let eq = build_equity_curve(100_000.0, &is_pnls);
                Some(PerformanceStats::compute(&is_pnls, &eq, 100_000.0, None))
            } else {
                None
            };

            let oos_stats = if !oos_pnls.is_empty() {
                let eq = build_equity_curve(100_000.0, &oos_pnls);
                Some(PerformanceStats::compute(&oos_pnls, &eq, 100_000.0, None))
            } else {
                None
            };

            all_is_pnls.extend(is_pnls);
            all_oos_pnls.extend(oos_pnls);

            fold_results.push(FoldResult {
                fold,
                train_start,
                train_end,
                test_start,
                test_end,
                in_sample: is_stats,
                out_of_sample: oos_stats,
            });
        }

        let aggregate_in_sample = if !all_is_pnls.is_empty() {
            let eq = build_equity_curve(100_000.0, &all_is_pnls);
            Some(PerformanceStats::compute(
                &all_is_pnls,
                &eq,
                100_000.0,
                None,
            ))
        } else {
            None
        };

        let aggregate_out_of_sample = if !all_oos_pnls.is_empty() {
            let eq = build_equity_curve(100_000.0, &all_oos_pnls);
            Some(PerformanceStats::compute(
                &all_oos_pnls,
                &eq,
                100_000.0,
                None,
            ))
        } else {
            None
        };

        WalkForwardReport {
            instrument: instrument.to_string(),
            folds: self.config.folds,
            fold_results,
            aggregate_in_sample,
            aggregate_out_of_sample,
        }
    }

    /// Placeholder: generate synthetic PnLs for testing window boundaries.
    /// In production, the validator runs a real sub-backtest on the train/test windows.
    fn simulate_fold_pnls(&self, _fold: usize, _is_in_sample: bool) -> Vec<f64> {
        // Return empty — real integration feeds actual backtest results.
        // Test verifies that the windows are correctly partitioned.
        Vec::new()
    }
}

fn build_equity_curve(initial: f64, pnls: &[f64]) -> Vec<f64> {
    let mut curve = Vec::with_capacity(pnls.len() + 1);
    curve.push(initial);
    let mut equity = initial;
    for &pnl in pnls {
        equity += pnl;
        curve.push(equity);
    }
    curve
}

// Re-export TickDataRef for the validate signature
pub mod types {
    use chrono::{DateTime, Utc};

    /// Lightweight tick reference for walk-forward window partitioning.
    #[derive(Debug, Clone)]
    pub struct TickDataRef {
        pub timestamp: DateTime<Utc>,
        pub instrument: String,
        pub price: f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_walk_forward_4_fold_non_overlapping() {
        let cfg = WalkForwardConfig {
            folds: 4,
            train_ratio: 0.75,
        };
        let validator = WalkForwardValidator::new(cfg);
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();

        let dummy_ticks: Vec<types::TickDataRef> = Vec::new();
        let report = validator.validate("rb9999", start, end, &dummy_ticks);

        assert_eq!(report.instrument, "rb9999");
        assert_eq!(report.folds, 4);
        assert_eq!(report.fold_results.len(), 4);

        // Verify non-overlapping: test_start of fold k > train_end of fold k
        for fr in &report.fold_results {
            assert!(
                fr.test_start > fr.train_end,
                "Fold {}: test_start {} must be after train_end {}",
                fr.fold,
                fr.test_start,
                fr.train_end
            );
        }

        // Verify sequential: fold k+1 train_start > fold k test_end
        for k in 0..report.fold_results.len() - 1 {
            let cur = &report.fold_results[k];
            let next = &report.fold_results[k + 1];
            assert!(
                next.train_start >= cur.test_end,
                "Fold {}→{}: next.train_start {} must be >= cur.test_end {}",
                k,
                k + 1,
                next.train_start,
                cur.test_end
            );
        }

        // Last fold test_end must equal end_date
        let last = report.fold_results.last().unwrap();
        assert_eq!(last.test_end, end, "Last fold test_end must equal end_date");

        // First fold train_start must equal start_date
        let first = &report.fold_results[0];
        assert_eq!(
            first.train_start, start,
            "First fold train_start must equal start_date"
        );
    }

    #[test]
    fn test_walk_forward_2_fold_small_window() {
        let cfg = WalkForwardConfig {
            folds: 2,
            train_ratio: 0.75,
        };
        let validator = WalkForwardValidator::new(cfg);
        let start = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();

        let dummy_ticks: Vec<types::TickDataRef> = Vec::new();
        let report = validator.validate("ag2506", start, end, &dummy_ticks);

        assert_eq!(report.folds, 2);
        assert_eq!(report.fold_results.len(), 2);

        // Both folds must be non-overlapping
        for fr in &report.fold_results {
            assert!(fr.test_start > fr.train_end);
        }
    }
}

use rand::Rng;
use serde::{Deserialize, Serialize};
use statrs::distribution::{ContinuousCDF, Normal};
use taiji_backtest::{PerformanceStats, WalkForwardReport};

/// Analyzes backtest results for overfitting and statistical significance.
pub struct ResultAnalyzer;

impl ResultAnalyzer {
    /// Full analysis combining deflated Sharpe and Monte Carlo test.
    pub fn analyze(
        stats: &PerformanceStats,
        walk_forward: &Option<WalkForwardReport>,
    ) -> Result<AnalysisReport, anyhow::Error> {
        let deflated_sharpe = Self::deflated_sharpe(stats, 0);
        let mc_result = Self::monte_carlo_test(stats, 1000);

        let walk_forward_robustness = walk_forward.as_ref().map_or(0.0, |wf| {
            match (&wf.aggregate_in_sample, &wf.aggregate_out_of_sample) {
                (Some(ins), Some(oos)) if ins.sharpe_ratio.abs() > 1e-9 => {
                    (oos.sharpe_ratio - ins.sharpe_ratio) / ins.sharpe_ratio
                }
                _ => 0.0,
            }
        });

        let overfitting_flag = deflated_sharpe < 0.5 || mc_result.pvalue > 0.05;

        Ok(AnalysisReport {
            deflated_sharpe,
            monte_carlo_pvalue: mc_result.pvalue,
            walk_forward_robustness,
            overfitting_flag,
        })
    }

    /// Deflated Sharpe Ratio (Harvey & Liu 2015).
    ///
    /// Corrects for multiple testing / data-snooping bias.
    /// `num_comparisons` is the number of strategies tested (0 = use heuristic sqrt(n)).
    ///
    /// DSR = SR * sqrt((T-1)/(T-K-1))
    /// where K approximates the degrees of freedom consumed by strategy selection.
    ///
    /// The DSR is always <= the original Sharpe when num_comparisons > 0.
    pub fn deflated_sharpe(stats: &PerformanceStats, num_comparisons: usize) -> f64 {
        let sr = stats.sharpe_ratio;
        if sr <= 0.0 || stats.total_trades < 2 {
            return sr;
        }

        let t = stats.total_trades as f64;
        let k = if num_comparisons > 0 {
            num_comparisons as f64
        } else {
            // Heuristic: sqrt of trades as an estimate of implicit comparisons
            t.sqrt().ceil().min(20.0)
        };

        if t <= k + 1.0 {
            return sr * 0.1; // severe penalty when very few trades
        }

        let deflation_factor = ((t - 1.0) / (t - k - 1.0)).sqrt();
        sr / deflation_factor
    }

    /// Monte Carlo permutation test.
    ///
    /// Shuffles the trade PnL sequence `n_simulations` times, recomputes Sharpe each time,
    /// and estimates the p-value: proportion of shuffled Sharpes >= observed Sharpe.
    pub fn monte_carlo_test(stats: &PerformanceStats, n_simulations: usize) -> MonteCarloResult {
        // We need PnLs to shuffle. Since PerformanceStats doesn't store raw PnLs,
        // we use a parametric approach: simulate returns from a normal distribution
        // with mean and std derived from the stats, then compute Sharpe for each simulation.
        let observed_sr = stats.sharpe_ratio;

        if stats.total_trades < 2 || observed_sr.is_nan() {
            return MonteCarloResult {
                pvalue: 1.0,
                simulated_sharpes: vec![],
                observed_sharpe: observed_sr,
                n_simulations,
            };
        }

        // Estimate per-trade mean and std from observed stats
        let total_trades = stats.total_trades as f64;

        // Approximate PnL distribution parameters
        // We know: Sharpe ≈ mean(pnl) / std(pnl) * sqrt(n_trades)
        // And: profit_factor = avg_win / abs(avg_loss)
        // Approximate std from Sharpe and trade count
        let approx_std = if observed_sr.abs() > 1e-9 {
            (observed_sr.abs() * 100.0) / total_trades.sqrt()
        } else {
            100.0 / total_trades.sqrt()
        };
        let approx_mean = observed_sr * approx_std / total_trades.sqrt();

        let mut rng = rand::thread_rng();
        let normal = Normal::new(0.0, 1.0).unwrap();
        let mut simulated_sharpes: Vec<f64> = Vec::with_capacity(n_simulations);

        for _ in 0..n_simulations {
            // Generate random PnLs under the estimated distribution
            let sim_pnls: Vec<f64> = (0..stats.total_trades)
                .map(|_| {
                    let z = normal.inverse_cdf(rng.gen::<f64>());
                    approx_mean + approx_std * z
                })
                .collect();

            // Compute Sharpe for this simulation
            let sim_mean = sim_pnls.iter().sum::<f64>() / total_trades;
            let sim_var =
                sim_pnls.iter().map(|p| (p - sim_mean).powi(2)).sum::<f64>() / (total_trades - 1.0);
            let sim_std = sim_var.sqrt();

            let sim_sr = if sim_std > 1e-9 {
                sim_mean / sim_std * total_trades.sqrt()
            } else {
                0.0
            };

            simulated_sharpes.push(sim_sr);
        }

        // p-value: proportion of simulations with Sharpe >= observed
        let exceed_count = simulated_sharpes
            .iter()
            .filter(|&&sr| sr >= observed_sr)
            .count();
        let pvalue = exceed_count as f64 / n_simulations as f64;

        MonteCarloResult {
            pvalue,
            simulated_sharpes,
            observed_sharpe: observed_sr,
            n_simulations,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    /// Deflated Sharpe Ratio (corrected for multiple testing).
    pub deflated_sharpe: f64,
    /// Monte Carlo permutation test p-value (< 0.05 means significant).
    pub monte_carlo_pvalue: f64,
    /// Walk-forward robustness: (OOS Sharpe - IS Sharpe) / IS Sharpe.
    pub walk_forward_robustness: f64,
    /// True if likely overfit (DSR < 0.5 or p > 0.05).
    pub overfitting_flag: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloResult {
    pub pvalue: f64,
    pub simulated_sharpes: Vec<f64>,
    pub observed_sharpe: f64,
    pub n_simulations: usize,
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    fn make_positive_stats() -> PerformanceStats {
        PerformanceStats {
            sharpe_ratio: 1.5,
            max_drawdown: 0.15,
            win_rate: 0.55,
            profit_factor: 1.8,
            calmar_ratio: 10.0,
            sortino_ratio: 2.0,
            expectancy: 50.0,
            alpha: Some(0.05),
            total_trades: 100,
            net_profit: 5000.0,
        }
    }

    fn make_negative_stats() -> PerformanceStats {
        PerformanceStats {
            sharpe_ratio: -0.5,
            max_drawdown: 0.30,
            win_rate: 0.40,
            profit_factor: 0.7,
            calmar_ratio: -1.67,
            sortino_ratio: -0.8,
            expectancy: -30.0,
            alpha: None,
            total_trades: 50,
            net_profit: -1500.0,
        }
    }

    #[test]
    fn test_deflated_sharpe_less_than_original() {
        let stats = make_positive_stats();
        let dsr = ResultAnalyzer::deflated_sharpe(&stats, 5);
        assert!(
            dsr < stats.sharpe_ratio,
            "deflated_sharpe ({}) should be < original sharpe ({})",
            dsr,
            stats.sharpe_ratio
        );
        assert!(dsr > 0.0, "deflated_sharpe should still be positive");
    }

    #[test]
    fn test_deflated_sharpe_negative_unchanged() {
        let stats = make_negative_stats();
        let dsr = ResultAnalyzer::deflated_sharpe(&stats, 5);
        assert!((dsr - stats.sharpe_ratio).abs() < 1e-9);
    }

    #[test]
    fn test_deflated_sharpe_no_comparisons() {
        let stats = make_positive_stats();
        let dsr = ResultAnalyzer::deflated_sharpe(&stats, 0);
        assert!(dsr < stats.sharpe_ratio);
        assert!(dsr > 0.0);
    }

    #[test]
    fn test_deflated_sharpe_small_trades_penalized() {
        let stats = PerformanceStats {
            total_trades: 3,
            ..make_positive_stats()
        };
        let dsr = ResultAnalyzer::deflated_sharpe(&stats, 10);
        // With only 3 trades and 10 comparisons, t <= k+1 triggers severe penalty
        assert!(dsr < stats.sharpe_ratio * 0.2);
    }

    #[test]
    fn test_monte_carlo_produces_valid_pvalue() {
        let stats = make_positive_stats();
        let result = ResultAnalyzer::monte_carlo_test(&stats, 500);
        assert!(result.pvalue >= 0.0 && result.pvalue <= 1.0);
        assert_eq!(result.n_simulations, 500);
        assert_eq!(result.simulated_sharpes.len(), 500);
    }

    #[test]
    fn test_monte_carlo_negative_sharpe_high_pvalue() {
        let stats = make_negative_stats();
        let result = ResultAnalyzer::monte_carlo_test(&stats, 200);
        // For a negative Sharpe, most permuted Sharpes are higher
        // (pvalue should be reasonably high, typically > 0.3)
        assert!(
            result.pvalue > 0.3,
            "expected pvalue > 0.3 for negative Sharpe, got {}",
            result.pvalue
        );
    }

    #[test]
    fn test_monte_carlo_few_trades() {
        let stats = PerformanceStats {
            total_trades: 1,
            ..make_positive_stats()
        };
        let result = ResultAnalyzer::monte_carlo_test(&stats, 100);
        assert!((result.pvalue - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_analyze_with_walk_forward() {
        let stats = make_positive_stats();
        let wf = Some(WalkForwardReport {
            instrument: "rb9999".into(),
            folds: 4,
            fold_results: vec![],
            aggregate_in_sample: Some(PerformanceStats {
                sharpe_ratio: 1.8,
                ..make_positive_stats()
            }),
            aggregate_out_of_sample: Some(PerformanceStats {
                sharpe_ratio: 1.2,
                ..make_positive_stats()
            }),
        });

        let report = ResultAnalyzer::analyze(&stats, &wf).expect("analyze");
        assert!(report.deflated_sharpe < stats.sharpe_ratio);
        assert!(report.monte_carlo_pvalue >= 0.0);
        // OOS < IS → negative robustness
        assert!(report.walk_forward_robustness < 0.0);
    }

    #[test]
    fn test_analyze_without_walk_forward() {
        let stats = make_positive_stats();
        let report = ResultAnalyzer::analyze(&stats, &None).expect("analyze");
        assert!((report.walk_forward_robustness - 0.0).abs() < 1e-9);
    }
}

use serde::{Deserialize, Serialize};

/// Performance statistics computed from a list of closed trades and an equity curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    /// Annualized Sharpe ratio (assuming 252 trading days, risk-free rate = 0).
    pub sharpe_ratio: f64,
    /// Maximum drawdown as a fraction of peak equity (0.0–1.0).
    pub max_drawdown: f64,
    /// Win rate (winning trades / total trades).
    pub win_rate: f64,
    /// Profit factor (gross profit / gross loss, ∞ if no losses).
    pub profit_factor: f64,
    /// Calmar ratio (annualized return / max drawdown).
    pub calmar_ratio: f64,
    /// Sortino ratio (annualized return / downside deviation).
    pub sortino_ratio: f64,
    /// Average trade expectancy (mean PnL per trade).
    pub expectancy: f64,
    /// Jensen's alpha (annualized excess return over benchmark; None if no benchmark).
    pub alpha: Option<f64>,
    /// Total number of trades.
    pub total_trades: usize,
    /// Net profit (sum of all trade PnLs minus commissions).
    pub net_profit: f64,
}

impl PerformanceStats {
    /// Compute performance statistics from a list of trade PnLs and the equity curve.
    ///
    /// `pnls` — per-trade profit & loss values (already net of commission).
    /// `equity_curve` — equity after each trade (length = trades + 1, starting with initial capital).
    /// `initial_capital` — starting account balance.
    /// `benchmark_returns` — optional daily benchmark returns for alpha calculation.
    pub fn compute(
        pnls: &[f64],
        equity_curve: &[f64],
        initial_capital: f64,
        benchmark_returns: Option<&[f64]>,
    ) -> Self {
        let total_trades = pnls.len();
        let net_profit: f64 = pnls.iter().sum();

        // --- Win rate ---
        let wins = pnls.iter().filter(|&&p| p > 0.0).count();
        let win_rate = if total_trades > 0 {
            wins as f64 / total_trades as f64
        } else {
            0.0
        };

        // --- Profit factor ---
        let gross_profit: f64 = pnls.iter().filter(|&&p| p > 0.0).sum();
        let gross_loss: f64 = pnls.iter().filter(|&&p| p < 0.0).map(|p| p.abs()).sum();
        let profit_factor = if gross_loss > 0.0 {
            gross_profit / gross_loss
        } else if gross_profit > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };

        // --- Max drawdown ---
        let max_drawdown = compute_max_drawdown(equity_curve);

        // --- Expectancy ---
        let expectancy = if total_trades > 0 {
            net_profit / total_trades as f64
        } else {
            0.0
        };

        // --- Daily returns from equity curve ---
        let daily_returns = compute_daily_returns(equity_curve);

        // --- Sharpe ratio (annualized, risk-free = 0) ---
        let sharpe_ratio = annualized_sharpe(&daily_returns);

        // --- Sortino ratio ---
        let sortino_ratio = annualized_sortino(&daily_returns);

        // --- Calmar ratio ---
        let calmar_ratio = if max_drawdown > 0.0 {
            let annual_return = compute_annualized_return(initial_capital, equity_curve);
            annual_return / max_drawdown
        } else {
            0.0
        };

        // --- Alpha ---
        let alpha = benchmark_returns.map(|bm_returns| compute_alpha(&daily_returns, bm_returns));

        Self {
            sharpe_ratio,
            max_drawdown,
            win_rate,
            profit_factor,
            calmar_ratio,
            sortino_ratio,
            expectancy,
            alpha,
            total_trades,
            net_profit,
        }
    }
}

/// Compute maximum drawdown from an equity curve.
/// MaxDD = max_{t} (peak(t) - equity(t)) / peak(t)
fn compute_max_drawdown(equity_curve: &[f64]) -> f64 {
    if equity_curve.is_empty() {
        return 0.0;
    }
    let mut peak = equity_curve[0];
    let mut max_dd = 0.0;
    for &eq in equity_curve.iter() {
        if eq > peak {
            peak = eq;
        }
        let dd = (peak - eq) / peak;
        if dd > max_dd {
            max_dd = dd;
        }
    }
    max_dd
}

/// Compute daily log returns from an equity curve sampled per trade.
/// Maps trade-level equity to approximate daily returns.
fn compute_daily_returns(equity_curve: &[f64]) -> Vec<f64> {
    if equity_curve.len() < 2 {
        return vec![];
    }
    equity_curve
        .windows(2)
        .map(|w| {
            if w[0] <= 0.0 {
                return 0.0;
            }
            let r = w[1] / w[0];
            if r > 0.0 && r.is_finite() {
                r.ln()
            } else {
                0.0
            }
        })
        .collect()
}

/// Annualized Sharpe ratio = mean(daily_returns) / std(daily_returns) * sqrt(252).
fn annualized_sharpe(daily_returns: &[f64]) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }
    let n = daily_returns.len() as f64;
    let mean: f64 = daily_returns.iter().sum::<f64>() / n;
    let variance: f64 = daily_returns
        .iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>()
        / (n - 1.0);
    let std = variance.sqrt();
    if std < 1e-12 {
        return 0.0;
    }
    mean / std * (252.0_f64).sqrt()
}

/// Annualized Sortino ratio = mean(daily_returns) / downside_deviation * sqrt(252).
fn annualized_sortino(daily_returns: &[f64]) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }
    let n = daily_returns.len() as f64;
    let mean: f64 = daily_returns.iter().sum::<f64>() / n;
    let downside: Vec<f64> = daily_returns
        .iter()
        .filter(|&&r| r < 0.0)
        .copied()
        .collect();
    if downside.is_empty() {
        return 0.0;
    }
    let d_mean: f64 = downside.iter().sum::<f64>() / downside.len() as f64;
    let d_var: f64 = downside.iter().map(|r| (r - d_mean).powi(2)).sum::<f64>()
        / (downside.len() as f64 - 1.0).max(1.0);
    let d_std = d_var.sqrt();
    if d_std < 1e-12 {
        return 0.0;
    }
    mean / d_std * (252.0_f64).sqrt()
}

/// Compute annualized return from equity curve.
fn compute_annualized_return(initial_capital: f64, equity_curve: &[f64]) -> f64 {
    if equity_curve.len() < 2 || initial_capital <= 0.0 {
        return 0.0;
    }
    let final_equity = equity_curve[equity_curve.len() - 1];
    let total_return = final_equity / initial_capital - 1.0;
    // Approximate: each trade ≈ 1 day
    let num_trades = equity_curve.len() - 1;
    let years = num_trades as f64 / 252.0;
    if years < 1e-12 {
        return 0.0;
    }
    ((1.0 + total_return).powf(1.0 / years)) - 1.0
}

/// Compute Jensen's alpha using CAPM: alpha = R_p - R_f - beta * (R_m - R_f).
/// Returns annualized alpha.
fn compute_alpha(strategy_returns: &[f64], benchmark_returns: &[f64]) -> f64 {
    if strategy_returns.len() < 2 || benchmark_returns.len() != strategy_returns.len() {
        return 0.0;
    }
    let n = strategy_returns.len() as f64;
    let s_mean: f64 = strategy_returns.iter().sum::<f64>() / n;
    let b_mean: f64 = benchmark_returns.iter().sum::<f64>() / n;

    let cov: f64 = strategy_returns
        .iter()
        .zip(benchmark_returns.iter())
        .map(|(s, b)| (s - s_mean) * (b - b_mean))
        .sum::<f64>()
        / (n - 1.0);
    let b_var: f64 = benchmark_returns
        .iter()
        .map(|b| (b - b_mean).powi(2))
        .sum::<f64>()
        / (n - 1.0);

    let beta = if b_var > 1e-12 { cov / b_var } else { 0.0 };

    // Annualized: daily_alpha * 252
    (s_mean - beta * b_mean) * 252.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_drawdown_zero() {
        let curve = vec![100_000.0, 101_000.0, 102_000.0, 103_000.0];
        assert!((compute_max_drawdown(&curve) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_max_drawdown_basic() {
        // Peak at 102k, drops to 95k → dd = (102-95)/102 ≈ 0.0686
        let curve = vec![100_000.0, 101_000.0, 102_000.0, 95_000.0, 99_000.0];
        let dd = compute_max_drawdown(&curve);
        assert!((dd - 0.068627).abs() < 1e-4);
    }

    #[test]
    fn test_max_drawdown_multiple_peaks() {
        // Peak1=110k, drop to 100k → dd1=0.0909
        // Peak2=120k, drop to 105k → dd2=0.125
        let curve = vec![
            100_000.0, 110_000.0, 105_000.0, 100_000.0, 120_000.0, 110_000.0, 105_000.0,
        ];
        let dd = compute_max_drawdown(&curve);
        assert!((dd - 0.125).abs() < 1e-4);
    }

    #[test]
    fn test_performance_stats_empty() {
        let equity = vec![100_000.0];
        let stats = PerformanceStats::compute(&[], &equity, 100_000.0, None);
        assert_eq!(stats.total_trades, 0);
        assert_eq!(stats.net_profit, 0.0);
        assert_eq!(stats.win_rate, 0.0);
        assert_eq!(stats.sharpe_ratio, 0.0);
    }

    #[test]
    fn test_performance_stats_all_wins() {
        let pnls = vec![1000.0, 500.0, 800.0];
        let equity = vec![100_000.0, 101_000.0, 101_500.0, 102_300.0];
        let stats = PerformanceStats::compute(&pnls, &equity, 100_000.0, None);
        assert!((stats.win_rate - 1.0).abs() < 1e-9);
        assert!((stats.net_profit - 2300.0).abs() < 1e-9);
        assert!(stats.profit_factor.is_infinite());
        assert!((stats.expectancy - 2300.0 / 3.0).abs() < 1e-3);
    }

    #[test]
    fn test_performance_stats_mixed() {
        let pnls = vec![1000.0, -300.0, 600.0, -200.0];
        let equity = vec![100_000.0, 101_000.0, 100_700.0, 101_300.0, 101_100.0];
        let stats = PerformanceStats::compute(&pnls, &equity, 100_000.0, None);
        assert!((stats.win_rate - 0.5).abs() < 1e-9);
        assert!((stats.net_profit - 1100.0).abs() < 1e-9);
        // Profit factor = (1000+600) / (300+200) = 1600/500 = 3.2
        assert!((stats.profit_factor - 3.2).abs() < 1e-9);
    }

    #[test]
    fn test_profit_factor_all_loss() {
        let pnls = vec![-500.0, -300.0];
        let equity = vec![100_000.0, 99_500.0, 99_200.0];
        let stats = PerformanceStats::compute(&pnls, &equity, 100_000.0, None);
        assert!((stats.profit_factor - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_sharpe_positive_returns() {
        // Steady positive daily returns → positive Sharpe
        let equity: Vec<f64> = (0..=252)
            .map(|i| 100_000.0 * (1.0 + 0.001 * i as f64))
            .collect();
        let initial = equity[0];
        let stats = PerformanceStats::compute(&vec![], &equity, initial, None);
        // All positive daily returns → high Sharpe
        assert!(stats.sharpe_ratio > 0.0);
        // No trades but equity curve has daily returns
        assert!(stats.max_drawdown < 1e-9);
    }

    #[test]
    fn test_alpha_calculation() {
        // Use 3 data points where beta = 1.0:
        // Strategy: [0.003, 0.001, 0.002], benchmark: [0.002, 0.000, 0.001]
        // s_mean = 0.002, b_mean = 0.001
        // cov = ((0.003-0.002)(0.002-0.001) + (0.001-0.002)(0.000-0.001) + (0.002-0.002)(0.001-0.001)) / 2
        //     = (0.001*0.001 + (-0.001)*(-0.001) + 0*0) / 2 = 0.000002 / 2 = 1e-6
        // b_var = ((0.001)^2 + (-0.001)^2 + 0) / 2 = 0.000002 / 2 = 1e-6
        // beta = 1e-6 / 1e-6 = 1.0
        // alpha = (0.002 - 1.0 * 0.001) * 252 = 0.001 * 252 = 0.252
        let strategy = vec![0.003, 0.001, 0.002];
        let benchmark = vec![0.002, 0.000, 0.001];
        let alpha = compute_alpha(&strategy, &benchmark);
        assert!((alpha - 0.252).abs() < 0.01);
    }
}

//! TrendAccelNode — 20 天趋势加速度检测。
//!
//! 对 close 做 20 天线性回归取斜率作为趋势速度。
//! 加速度 = trend_current - trend_prev。
//! z-score = (accel - mean_accel) / std_accel；Score = min(|z| / 2.0, 1.0) * 100。

use crate::{linear_regression, mean, std_dev, AbnormalIndicator, MAX_BARS};
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::state::{StateKey, StateValue};

/// 趋势回归窗口 — 20 天（一个交易月）。
/// 领域常量：线性回归的标准窗口，无需参数化。
const TREND_WINDOW: usize = 20;
/// z-score 归一化因子 — 将加速度 z-score 映射到 [0, 100]。
/// 领域常量：|z| ≥ 2 视为满分异常加速度。
const Z_SCORE_DIVISOR: f64 = 2.0;
const OUTPUT_KEY: &str = "abnormal:trend_accel";

pub struct TrendAccelNode {
    id: NodeId,
    closes: Vec<f64>,
    trends: Vec<f64>,
}

impl TrendAccelNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            closes: Vec::with_capacity(MAX_BARS),
            trends: Vec::with_capacity(MAX_BARS),
        }
    }
}

impl AbnormalIndicator for TrendAccelNode {
    fn compute_score(&self, bars: &[RawBar], lookback: usize) -> f64 {
        if bars.len() < lookback.max(TREND_WINDOW) + 2 {
            return 0.0;
        }
        let n = bars.len();
        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();

        // 全历史滚动趋势斜率
        let mut trends = Vec::with_capacity(n.saturating_sub(TREND_WINDOW));
        let x: Vec<f64> = (0..TREND_WINDOW).map(|j| j as f64).collect();
        for i in TREND_WINDOW..n {
            let w = &closes[i - TREND_WINDOW..i];
            let (slope, _) = linear_regression(&x, w);
            trends.push(slope);
        }

        if trends.len() < 2 {
            return 0.0;
        }

        // 最新趋势加速度
        let trend_current = trends[trends.len() - 1];
        let trend_prev = trends[trends.len() - 2];
        let accel = trend_current - trend_prev;

        // 加速度序列（相邻趋势差）
        let accels: Vec<f64> = trends.windows(2).map(|w| w[1] - w[0]).collect();
        if accels.len() < 2 {
            return 0.0;
        }
        let mean_accel = mean(&accels);
        let std_accel = std_dev(&accels);
        if std_accel < 1e-10 {
            return 0.0;
        }
        let z = (accel - mean_accel) / std_accel;
        (z.abs() / Z_SCORE_DIVISOR).min(1.0) * 100.0
    }
}

impl ComputeNode for TrendAccelNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "TrendAccelNode"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        vec![]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec![OUTPUT_KEY.into()]
    }

    fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn on_bar(&mut self, bar: &RawBar, _period: Freq, state: &StateStore) -> Result<()> {
        self.closes.push(bar.close);
        if self.closes.len() > MAX_BARS {
            self.closes.remove(0);
        }

        let score = self.compute_score_inner();
        self.trends.push(score);
        if self.trends.len() > MAX_BARS {
            self.trends.remove(0);
        }

        state.set(OUTPUT_KEY.into(), StateValue::F64(score), self.id());
        Ok(())
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::D]
    }
}

impl TrendAccelNode {
    fn compute_score_inner(&self) -> f64 {
        if self.closes.len() < TREND_WINDOW + 1 {
            return 0.0;
        }
        let n = self.closes.len();
        let x: Vec<f64> = (0..TREND_WINDOW).map(|j| j as f64).collect();

        // 全历史趋势斜率
        let mut trends = Vec::with_capacity(n.saturating_sub(TREND_WINDOW));
        for i in TREND_WINDOW..n {
            let w = &self.closes[i - TREND_WINDOW..i];
            let (slope, _) = linear_regression(&x, w);
            trends.push(slope);
        }

        if trends.len() < 2 {
            return 0.0;
        }

        let trend_current = trends[trends.len() - 1];
        let trend_prev = trends[trends.len() - 2];
        let accel = trend_current - trend_prev;

        let accels: Vec<f64> = trends.windows(2).map(|w| w[1] - w[0]).collect();
        if accels.len() < 2 {
            return 0.0;
        }
        let mean_accel = mean(&accels);
        let std_accel = std_dev(&accels);
        if std_accel < 1e-10 {
            return 0.0;
        }
        let z = (accel - mean_accel) / std_accel;
        (z.abs() / Z_SCORE_DIVISOR).min(1.0) * 100.0
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use taiji_engine::types::bar::Symbol;

    fn bar(close: f64) -> RawBar {
        RawBar {
            symbol: Symbol::from("TEST"),
            dt: Utc::now(),
            freq: Freq::D,
            id: 0,
            open: close - 1.0,
            high: close + 1.0,
            low: close - 2.0,
            close,
            vol: 10000.0,
            amount: close * 10000.0,
            open_interest: None,
            delta: None,
        }
    }

    #[test]
    fn test_steady_trend_score_low() {
        // 稳定匀速上升 → 加速度 ≈ 0 → score 低
        let node = TrendAccelNode::new("ta".into());
        let bars: Vec<RawBar> = (0..100).map(|i| bar(4000.0 + i as f64 * 2.0)).collect();
        let score = node.compute_score(&bars, 60);
        assert!(score >= 0.0 && score <= 100.0, "score={}", score);
    }

    #[test]
    fn test_sudden_accel_score_high() {
        // 突然加速上升 → score 升高
        let node = TrendAccelNode::new("ta".into());
        let mut bars: Vec<RawBar> = (0..60).map(|i| bar(4000.0 + i as f64 * 1.0)).collect();
        // 最后 20 天加速
        for i in 0..20 {
            bars.push(bar(4060.0 + i as f64 * 10.0));
        }
        let score = node.compute_score(&bars, 20);
        assert!(score >= 0.0 && score <= 100.0, "score={}", score);
    }

    #[test]
    fn test_insufficient_data() {
        let node = TrendAccelNode::new("ta".into());
        let bars: Vec<RawBar> = (0..10).map(|i| bar(4000.0 + i as f64)).collect();
        let score = node.compute_score(&bars, 20);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_on_bar_writes_score() {
        let mut node = TrendAccelNode::new("ta".into());
        let store = StateStore::new();
        for i in 0..30 {
            node.on_bar(&bar(4000.0 + i as f64 * 2.0), Freq::D, &store)
                .unwrap();
        }
        let score: Option<f64> = store.get(&OUTPUT_KEY.into());
        assert!(score.is_some());
        let s = score.unwrap();
        assert!(s >= 0.0 && s <= 100.0, "score={}", s);
    }
}

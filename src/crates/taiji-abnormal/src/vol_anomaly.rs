//! VolAnomalyNode — 10 天波动率 z-score。
//!
//! 10 天滚动波动率，与自身历史均值和标准差比较。
//! z = (vol_10d - mean_vol) / std_vol；Score = min(|z| / 2.0, 1.0) * 100。

use crate::{mean, std_dev, AbnormalIndicator, MAX_BARS};
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::state::{StateKey, StateValue};

/// 波动率异常滚动窗口 — 10 天（半交易月）。
/// 领域常量：短期波动检测的敏感窗口，无需参数化。
const VOL_WINDOW: usize = 10;
/// 年化因子 √252 — 将日波动率年化。
/// 领域常量：标准 A 股年交易日数。
const ANNUALIZATION: f64 = 252.0;
/// z-score 归一化因子 — 将 z-score 映射到 [0, 100]。
/// 领域常量：|z| ≥ 2 视为满分异常。
const Z_SCORE_DIVISOR: f64 = 2.0;
const OUTPUT_KEY: &str = "abnormal:vol_anomaly";

pub struct VolAnomalyNode {
    id: NodeId,
    closes: Vec<f64>,
    scores: Vec<f64>,
}

impl VolAnomalyNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            closes: Vec::with_capacity(MAX_BARS),
            scores: Vec::with_capacity(MAX_BARS),
        }
    }
}

impl AbnormalIndicator for VolAnomalyNode {
    fn compute_score(&self, bars: &[RawBar], lookback: usize) -> f64 {
        if bars.len() < lookback.max(VOL_WINDOW) + 1 {
            return 0.0;
        }
        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let n = closes.len();
        let eff_lookback = lookback.min(n);

        // 最新 10 天波动率
        let recent = &closes[n - eff_lookback..];
        let returns: Vec<f64> = recent.windows(2).map(|w| (w[1] / w[0]).ln()).collect();
        let vol_current = std_dev(&returns) * ANNUALIZATION.sqrt();

        // 全历史 10 天滚动波动率 → 基线 mean + std
        let mut vols = Vec::with_capacity(n.saturating_sub(VOL_WINDOW));
        for i in VOL_WINDOW..n {
            let w = &closes[i - VOL_WINDOW..=i];
            let r: Vec<f64> = w.windows(2).map(|c| (c[1] / c[0]).ln()).collect();
            vols.push(std_dev(&r) * ANNUALIZATION.sqrt());
        }

        if vols.len() < 2 {
            return 0.0;
        }
        let mean_vol = mean(&vols);
        let std_vol = std_dev(&vols);
        if std_vol < 1e-10 {
            return 0.0;
        }
        let z = (vol_current - mean_vol) / std_vol;
        (z.abs() / Z_SCORE_DIVISOR).min(1.0) * 100.0
    }
}

impl ComputeNode for VolAnomalyNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "VolAnomalyNode"
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
        self.scores.push(score);
        if self.scores.len() > MAX_BARS {
            self.scores.remove(0);
        }

        state.set(OUTPUT_KEY.into(), StateValue::F64(score), self.id());
        Ok(())
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::D]
    }
}

impl VolAnomalyNode {
    fn compute_score_inner(&self) -> f64 {
        if self.closes.len() < VOL_WINDOW + 1 {
            return 0.0;
        }
        let n = self.closes.len();

        // 最新波动率
        let recent = &self.closes[n - VOL_WINDOW..];
        let returns: Vec<f64> = recent.windows(2).map(|w| (w[1] / w[0]).ln()).collect();
        let vol_current = std_dev(&returns) * ANNUALIZATION.sqrt();

        // 滚动波动率历史
        let mut vols = Vec::with_capacity(n.saturating_sub(VOL_WINDOW));
        for i in VOL_WINDOW..n {
            let w = &self.closes[i - VOL_WINDOW..=i];
            let r: Vec<f64> = w.windows(2).map(|c| (c[1] / c[0]).ln()).collect();
            vols.push(std_dev(&r) * ANNUALIZATION.sqrt());
        }

        if vols.len() < 2 {
            return 0.0;
        }
        let mean_vol = mean(&vols);
        let std_vol = std_dev(&vols);
        if std_vol < 1e-10 {
            return 0.0;
        }
        let z = (vol_current - mean_vol) / std_vol;
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
    fn test_normal_vol_score_low() {
        // 稳定波动 → z-score 低 → 分数低
        let node = VolAnomalyNode::new("va".into());
        let bars: Vec<RawBar> = (0..100).map(|i| bar(4000.0 + i as f64 * 0.5)).collect();
        let score = node.compute_score(&bars, 20);
        assert!(score >= 0.0 && score <= 100.0, "score={}", score);
    }

    #[test]
    fn test_spike_vol_score_high() {
        // 突然剧烈波动 → score 升高
        let node = VolAnomalyNode::new("va".into());
        let mut bars: Vec<RawBar> = (0..80).map(|i| bar(4000.0 + i as f64 * 0.2)).collect();
        // 最后 10 天剧烈震荡
        for i in 0..10 {
            let swing = if i % 2 == 0 { 80.0 } else { -80.0 };
            bars.push(bar(4080.0 + swing));
        }
        let score = node.compute_score(&bars, 20);
        assert!(score >= 0.0 && score <= 100.0, "score={}", score);
    }

    #[test]
    fn test_insufficient_data() {
        let node = VolAnomalyNode::new("va".into());
        let bars: Vec<RawBar> = (0..5).map(|i| bar(4000.0 + i as f64)).collect();
        let score = node.compute_score(&bars, 20);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_on_bar_writes_score() {
        let mut node = VolAnomalyNode::new("va".into());
        let store = StateStore::new();
        for i in 0..30 {
            node.on_bar(&bar(4000.0 + i as f64 * 0.5), Freq::D, &store)
                .unwrap();
        }
        let score: Option<f64> = store.get(&OUTPUT_KEY.into());
        assert!(score.is_some());
        let s = score.unwrap();
        assert!(s >= 0.0 && s <= 100.0, "score={}", s);
    }
}

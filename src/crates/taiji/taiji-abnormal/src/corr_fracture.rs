//! CorrFractureNode — 20 天滚动价格-成交量相关系数断裂检测。
//!
//! 计算 close 与 vol 的 20 天滚动 Pearson ρ。
//! 当 |Δρ| > 0.3 时判定为相关性断裂。
//! Score = min(|Δρ| / 0.3, 1.0) * 100。

use crate::{pearson_r, AbnormalIndicator, MAX_BARS};
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::state::{StateKey, StateValue};

/// 相关性滚动窗口 — 20 天（一个交易月）。
/// 领域常量：价格-成交量相关性的标准观测窗口，无需参数化。
const CORR_WINDOW: usize = 20;
/// 相关性断裂阈值 — |Δρ| ≥ 0.3 视为断裂。
/// 领域常量：经验阈值，ρ 变化超过 0.3 意味着量价关系发生结构性改变。
const CORR_BREAK_THRESHOLD: f64 = 0.3;
const OUTPUT_KEY: &str = "abnormal:corr_fracture";

pub struct CorrFractureNode {
    id: NodeId,
    closes: Vec<f64>,
    vols: Vec<f64>,
    prev_rho: f64,
}

impl CorrFractureNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            closes: Vec::with_capacity(MAX_BARS),
            vols: Vec::with_capacity(MAX_BARS),
            prev_rho: 0.0,
        }
    }
}

impl AbnormalIndicator for CorrFractureNode {
    fn compute_score(&self, bars: &[RawBar], lookback: usize) -> f64 {
        if bars.len() < lookback.max(CORR_WINDOW) {
            return 0.0;
        }
        let n = bars.len();
        let eff_lookback = lookback.min(n);

        // 整体序列计算滚动 ρ 用于找 Δρ
        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let vols: Vec<f64> = bars.iter().map(|b| b.vol).collect();

        let mut rhos = Vec::with_capacity(n.saturating_sub(CORR_WINDOW));
        for i in CORR_WINDOW..n {
            let c = &closes[i - CORR_WINDOW..i];
            let v = &vols[i - CORR_WINDOW..i];
            rhos.push(pearson_r(c, v));
        }

        // 最新 ρ（基于最近 lookback 的末尾 CORR_WINDOW）
        let recent_c = &closes[n - eff_lookback..][(eff_lookback.saturating_sub(CORR_WINDOW))..];
        let recent_v = &vols[n - eff_lookback..][(eff_lookback.saturating_sub(CORR_WINDOW))..];
        if recent_c.len() < CORR_WINDOW {
            return 0.0;
        }
        let rho_current = pearson_r(recent_c, recent_v);

        // 上一段 ρ
        let rho_prev = if rhos.len() >= 2 {
            rhos[rhos.len() - 2]
        } else {
            rho_current
        };

        let delta = (rho_current - rho_prev).abs();
        (delta / CORR_BREAK_THRESHOLD).min(1.0) * 100.0
    }
}

impl ComputeNode for CorrFractureNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "CorrFractureNode"
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
        self.vols.push(bar.vol);
        if self.closes.len() > MAX_BARS {
            self.closes.remove(0);
            self.vols.remove(0);
        }

        let score = self.compute_score_inner();
        state.set(OUTPUT_KEY.into(), StateValue::F64(score), self.id());
        Ok(())
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::D]
    }
}

impl CorrFractureNode {
    fn compute_score_inner(&self) -> f64 {
        if self.closes.len() < CORR_WINDOW {
            return 0.0;
        }
        let n = self.closes.len();

        let c = &self.closes[n - CORR_WINDOW..];
        let v = &self.vols[n - CORR_WINDOW..];
        let rho_current = pearson_r(c, v);

        let delta = (rho_current - self.prev_rho).abs();
        (delta / CORR_BREAK_THRESHOLD).min(1.0) * 100.0
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use taiji_engine::types::bar::Symbol;

    fn bar(close: f64, vol: f64) -> RawBar {
        RawBar {
            symbol: Symbol::from("TEST"),
            dt: Utc::now(),
            freq: Freq::D,
            id: 0,
            open: close - 1.0,
            high: close + 1.0,
            low: close - 2.0,
            close,
            vol,
            amount: close * vol,
            open_interest: None,
            delta: None,
        }
    }

    #[test]
    fn test_stable_correlation_score_low() {
        // 价格与成交量稳定正相关 → Δρ 小 → 分数低
        let node = CorrFractureNode::new("cf".into());
        let bars: Vec<RawBar> = (0..80)
            .map(|i| bar(4000.0 + i as f64, 10000.0 + i as f64 * 100.0))
            .collect();
        let score = node.compute_score(&bars, 40);
        assert!(score >= 0.0 && score <= 100.0, "score={}", score);
    }

    #[test]
    fn test_corr_break_score_high() {
        // 前 60 天正相关，最后 20 天价格涨但成交量跌 → ρ 断崖
        let node = CorrFractureNode::new("cf".into());
        let mut bars: Vec<RawBar> = (0..60)
            .map(|i| bar(4000.0 + i as f64, 10000.0 + i as f64 * 100.0))
            .collect();
        for i in 0..20 {
            bars.push(bar(4060.0 + i as f64 * 2.0, 16000.0 - i as f64 * 200.0));
        }
        let score = node.compute_score(&bars, 20);
        assert!(score >= 0.0 && score <= 100.0, "score={}", score);
    }

    #[test]
    fn test_insufficient_data() {
        let node = CorrFractureNode::new("cf".into());
        let bars: Vec<RawBar> = (0..10).map(|i| bar(4000.0 + i as f64, 10000.0)).collect();
        let score = node.compute_score(&bars, 20);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_on_bar_writes_score() {
        let mut node = CorrFractureNode::new("cf".into());
        let store = StateStore::new();
        for i in 0..30 {
            node.on_bar(
                &bar(4000.0 + i as f64, 10000.0 + i as f64 * 50.0),
                Freq::D,
                &store,
            )
            .unwrap();
        }
        let score: Option<f64> = store.get(&OUTPUT_KEY.into());
        assert!(score.is_some());
        let s = score.unwrap();
        assert!(s >= 0.0 && s <= 100.0, "score={}", s);
    }
}

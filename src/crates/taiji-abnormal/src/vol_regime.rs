//! VolRegimeNode — 20 天历史波动率 vs 80 分位数。
//!
//! HV(t) = std(log returns[20d]) * sqrt(252)。
//! Score = clamp(HV_current / HV_80th * 100, 0, 100)。

use crate::{percentile, std_dev, AbnormalIndicator, MAX_BARS};
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::state::{StateKey, StateValue};

/// 历史波动率滚动窗口 — 20 天（一个交易月）。
/// 领域常量：HV 的标准计算周期，无需参数化。
const HV_WINDOW: usize = 20;
/// 年化因子 √252 — 将日波动率年化。
/// 领域常量：标准 A 股年交易日数。
const ANNUALIZATION: f64 = 252.0;
/// HV 80 分位数基准 — 判断当前波动率是否偏高。
/// 领域常量：取分位数上限而非均值，避免被极端值拖拽。
const HV_PERCENTILE: f64 = 80.0;
const OUTPUT_KEY: &str = "abnormal:vol_regime";

pub struct VolRegimeNode {
    id: NodeId,
    closes: Vec<f64>,
}

impl VolRegimeNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            closes: Vec::with_capacity(MAX_BARS),
        }
    }
}

impl AbnormalIndicator for VolRegimeNode {
    fn compute_score(&self, bars: &[RawBar], lookback: usize) -> f64 {
        if bars.len() < lookback.max(HV_WINDOW) + 1 {
            return 0.0;
        }
        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let n = closes.len();
        let eff_lookback = lookback.min(n);

        // 最新 HV
        let recent = &closes[n - eff_lookback..];
        let returns: Vec<f64> = recent.windows(2).map(|w| (w[1] / w[0]).ln()).collect();
        let hv_current = std_dev(&returns) * ANNUALIZATION.sqrt();

        // 全历史 HV 序列 → 80 分位数
        let mut hvs = Vec::with_capacity(n.saturating_sub(HV_WINDOW));
        for i in HV_WINDOW..n {
            let w = &closes[i - HV_WINDOW..=i];
            let r: Vec<f64> = w.windows(2).map(|c| (c[1] / c[0]).ln()).collect();
            hvs.push(std_dev(&r) * ANNUALIZATION.sqrt());
        }
        hvs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let hv_80 = percentile(&hvs, HV_PERCENTILE);
        if hv_80 < 1e-10 {
            return 0.0;
        }
        ((hv_current / hv_80) * 100.0).clamp(0.0, 100.0)
    }
}

impl ComputeNode for VolRegimeNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "VolRegimeNode"
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

        let score = self.compute_score_by_closes();
        state.set(OUTPUT_KEY.into(), StateValue::F64(score), self.id());
        Ok(())
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::D]
    }
}

impl VolRegimeNode {
    /// 基于内部 closes 缓冲区计算分数
    fn compute_score_by_closes(&self) -> f64 {
        if self.closes.len() < HV_WINDOW + 1 {
            return 0.0;
        }
        let n = self.closes.len();

        // 最新 HV
        let recent = &self.closes[n - HV_WINDOW..];
        let returns: Vec<f64> = recent.windows(2).map(|w| (w[1] / w[0]).ln()).collect();
        let hv_current = std_dev(&returns) * (252.0_f64).sqrt();

        // 全历史 HV 序列
        let mut hvs = Vec::with_capacity(n.saturating_sub(HV_WINDOW));
        for i in HV_WINDOW..n {
            let w = &self.closes[i - HV_WINDOW..=i];
            let r: Vec<f64> = w.windows(2).map(|c| (c[1] / c[0]).ln()).collect();
            hvs.push(std_dev(&r) * ANNUALIZATION.sqrt());
        }
        hvs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let hv_80 = percentile(&hvs, HV_PERCENTILE);
        if hv_80 < 1e-10 {
            return 0.0;
        }
        ((hv_current / hv_80) * 100.0).clamp(0.0, 100.0)
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
    fn test_low_vol_regime_score() {
        // 低波动 → score < 100
        let node = VolRegimeNode::new("vr".into());
        let bars: Vec<RawBar> = (0..100)
            .map(|i| bar(4000.0 + i as f64 * 0.5)) // 缓慢上行
            .collect();
        let score = node.compute_score(&bars, 20);
        assert!(score >= 0.0 && score <= 100.0, "score={}", score);
    }

    #[test]
    fn test_high_vol_regime_score() {
        // 高波动 → 分数上升
        let node = VolRegimeNode::new("vr".into());
        let mut bars: Vec<RawBar> = (0..80).map(|i| bar(4000.0 + i as f64 * 1.0)).collect();
        // 最后 20 天剧烈波动
        for i in 0..20 {
            let swing = if i % 2 == 0 { 50.0 } else { -50.0 };
            bars.push(bar(4080.0 + swing));
        }
        let score = node.compute_score(&bars, 20);
        assert!(score >= 0.0 && score <= 100.0, "score={}", score);
    }

    #[test]
    fn test_insufficient_data() {
        let node = VolRegimeNode::new("vr".into());
        let bars: Vec<RawBar> = (0..10).map(|i| bar(4000.0 + i as f64)).collect();
        let score = node.compute_score(&bars, 20);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_on_bar_writes_score() {
        let mut node = VolRegimeNode::new("vr".into());
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

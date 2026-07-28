//! GapAlertNode — 跳空缺口检测。
//!
//! gap = |open - prev_close| / prev_close。
//! 当 gap > 1.5% 时触发告警。
//! Score = min(gap / 0.03, 1.0) * 100（3% 缺口 → 满分）。

use crate::AbnormalIndicator;
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::state::{StateKey, StateValue};

/// 跳空阈值 — 3% 缺口 → 满分 100。
/// 领域常量：经验阈值，3% 以上跳空视为极端异常。
const GAP_FULL_SCORE: f64 = 0.03;
const OUTPUT_KEY: &str = "abnormal:gap_alert";

pub struct GapAlertNode {
    id: NodeId,
    prev_close: Option<f64>,
}

impl GapAlertNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            prev_close: None,
        }
    }
}

impl AbnormalIndicator for GapAlertNode {
    fn compute_score(&self, bars: &[RawBar], lookback: usize) -> f64 {
        if bars.len() < 2 {
            return 0.0;
        }
        let _ = lookback;
        // 找最近一次跳空
        let mut max_score = 0.0_f64;
        for w in bars.windows(2).rev().take(lookback.max(1)) {
            let prev = &w[0];
            let curr = &w[1];
            if prev.close <= 0.0 {
                continue;
            }
            let gap = (curr.open - prev.close).abs() / prev.close;
            let score = (gap / GAP_FULL_SCORE).min(1.0) * 100.0;
            max_score = max_score.max(score);
        }
        max_score
    }
}

impl ComputeNode for GapAlertNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "GapAlertNode"
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
        let score = if let Some(prev_close) = self.prev_close {
            if prev_close > 0.0 {
                let gap = (bar.open - prev_close).abs() / prev_close;
                (gap / GAP_FULL_SCORE).min(1.0) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        self.prev_close = Some(bar.close);

        state.set(OUTPUT_KEY.into(), StateValue::F64(score), self.id());
        Ok(())
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::D]
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use taiji_engine::types::bar::Symbol;

    fn bar_with_open(open: f64, close: f64) -> RawBar {
        RawBar {
            symbol: Symbol::from("TEST"),
            dt: Utc::now(),
            freq: Freq::D,
            id: 0,
            open,
            high: open.max(close) + 1.0,
            low: open.min(close) - 1.0,
            close,
            vol: 10000.0,
            amount: close * 10000.0,
            open_interest: None,
            delta: None,
        }
    }

    #[test]
    fn test_no_gap_score_zero() {
        let node = GapAlertNode::new("ga".into());
        let bars = vec![
            bar_with_open(4000.0, 4010.0), // prev close = 4010
            bar_with_open(4011.0, 4020.0), // open ≈ prev_close, no gap
        ];
        let score = node.compute_score(&bars, 1);
        assert!((score - 0.0).abs() < 0.5 || score < 10.0, "score={}", score);
    }

    #[test]
    fn test_gap_up_score() {
        // 2% 跳空
        let node = GapAlertNode::new("ga".into());
        let bars = vec![
            bar_with_open(3990.0, 4000.0), // prev close = 4000
            bar_with_open(4080.0, 4100.0), // open 4080 vs prev_close 4000 = 2% gap
        ];
        let score = node.compute_score(&bars, 1);
        let expected = (0.02 / 0.03) * 100.0; // ≈ 66.67
        assert!(
            (score - expected).abs() < 1.0,
            "score={}, expected={}",
            score,
            expected
        );
        assert!(score >= 0.0 && score <= 100.0);
    }

    #[test]
    fn test_big_gap_clamped() {
        // 5% 跳空 → capped at 100
        let node = GapAlertNode::new("ga".into());
        let bars = vec![
            bar_with_open(3990.0, 4000.0),
            bar_with_open(4200.0, 4200.0), // 5% gap
        ];
        let score = node.compute_score(&bars, 1);
        assert!((score - 100.0).abs() < 1e-10, "score={}", score);
    }

    #[test]
    fn test_on_bar_writes_score() {
        let mut node = GapAlertNode::new("ga".into());
        let store = StateStore::new();

        // first bar: no prev_close → score = 0
        node.on_bar(&bar_with_open(4000.0, 4010.0), Freq::D, &store)
            .unwrap();
        let s1: Option<f64> = store.get(&OUTPUT_KEY.into());
        assert!((s1.unwrap() - 0.0).abs() < 1e-10);

        // second bar: normal open
        node.on_bar(&bar_with_open(4011.0, 4020.0), Freq::D, &store)
            .unwrap();
        let s2: Option<f64> = store.get(&OUTPUT_KEY.into());
        assert!(s2.unwrap() < 10.0);

        // third bar: gap up
        node.on_bar(&bar_with_open(4100.0, 4110.0), Freq::D, &store)
            .unwrap();
        let s3: Option<f64> = store.get(&OUTPUT_KEY.into());
        assert!(s3.unwrap() > 50.0, "score={}", s3.unwrap());
    }
}

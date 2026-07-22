//! Taiji example strategies — reference ComputeNode implementations.
//!
//! MaCross: 经典 MA 双均线金叉/死叉策略。
//! 通用技术指标模板，零太极公式。
//! 任何策略教程都会教的示例——fast_period=5, slow_period=20。

use std::collections::HashMap;

use chrono::Utc;
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::signal::{Signal, SignalAction};
use taiji_engine::types::state::StateKey;

/// MA 双均线交叉策略。
///
/// - `fast_period`: 快线周期（默认 5）
/// - `slow_period`: 慢线周期（默认 20）
///
/// 金叉（快线上穿慢线）→ Long，死叉（快线下穿慢线）→ Short。
pub struct MaCross {
    id: NodeId,
    fast_period: usize,
    slow_period: usize,
    closes: Vec<f64>,
}

impl MaCross {
    pub fn new(node_id: &str) -> Self {
        Self {
            id: node_id.to_string(),
            fast_period: 5,
            slow_period: 20,
            closes: Vec::new(),
        }
    }

    /// 简单移动平均。
    fn sma(data: &[f64], period: usize) -> Option<f64> {
        if data.len() < period {
            return None;
        }
        let sum: f64 = data[data.len() - period..].iter().sum();
        Some(sum / period as f64)
    }
}

impl ComputeNode for MaCross {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "ma_cross"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        vec!["bars:1m".into()]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec!["signals:ma_cross".into()]
    }

    fn on_init(&mut self, config: &NodeConfig, _state: &StateStore) -> Result<()> {
        if let Some(fp) = config.get_i64("fast_period") {
            self.fast_period = fp as usize;
        }
        if let Some(sp) = config.get_i64("slow_period") {
            self.slow_period = sp as usize;
        }
        Ok(())
    }

    fn on_bar(&mut self, bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
        self.closes.push(bar.close);
        Ok(())
    }

    fn on_calculate(&mut self, _state: &StateStore) -> Result<Vec<Signal>> {
        let n = self.closes.len();
        if n < self.slow_period + 1 {
            return Ok(vec![]);
        }

        let prev = &self.closes[..n - 1];
        let curr = &self.closes;

        let prev_fast = Self::sma(prev, self.fast_period);
        let prev_slow = Self::sma(prev, self.slow_period);
        let curr_fast = Self::sma(curr, self.fast_period);
        let curr_slow = Self::sma(curr, self.slow_period);

        match (prev_fast, prev_slow, curr_fast, curr_slow) {
            (Some(pf), Some(ps), Some(cf), Some(cs)) => {
                if pf <= ps && cf > cs {
                    // 金叉：快线上穿慢线 → 做多
                    return Ok(vec![Signal {
                        timestamp: Utc::now(),
                        instrument: String::new(),
                        freq: Freq::F1,
                        action: SignalAction::Long,
                        entry: None,
                        stop_loss: None,
                        take_profit: None,
                        size: None,
                        source: self.id.clone(),
                        confidence: 0.8,
                        metadata: HashMap::from([
                            ("reason".into(), "golden_cross".into()),
                            ("fast_ma".into(), format!("{:.4}", cf)),
                            ("slow_ma".into(), format!("{:.4}", cs)),
                        ]),
                        disclaimer: None,
                    }]);
                } else if pf >= ps && cf < cs {
                    // 死叉：快线下穿慢线 → 做空
                    return Ok(vec![Signal {
                        timestamp: Utc::now(),
                        instrument: String::new(),
                        freq: Freq::F1,
                        action: SignalAction::Short,
                        entry: None,
                        stop_loss: None,
                        take_profit: None,
                        size: None,
                        source: self.id.clone(),
                        confidence: 0.8,
                        metadata: HashMap::from([
                            ("reason".into(), "death_cross".into()),
                            ("fast_ma".into(), format!("{:.4}", cf)),
                            ("slow_ma".into(), format!("{:.4}", cs)),
                        ]),
                        disclaimer: None,
                    }]);
                }
                Ok(vec![])
            }
            _ => Ok(vec![]),
        }
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::F1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ma_cross_new() {
        let node = MaCross::new("test_ma");
        assert_eq!(node.id(), "test_ma");
        assert_eq!(node.name(), "ma_cross");
        assert_eq!(node.fast_period, 5);
        assert_eq!(node.slow_period, 20);
    }

    #[test]
    fn test_ma_cross_on_init_reads_config() {
        let mut node = MaCross::new("test_ma");
        let mut config = NodeConfig::new();
        config
            .params
            .insert("fast_period".into(), serde_json::json!(10));
        config
            .params
            .insert("slow_period".into(), serde_json::json!(30));

        let state = StateStore::new();
        node.on_init(&config, &state).unwrap();
        assert_eq!(node.fast_period, 10);
        assert_eq!(node.slow_period, 30);
    }

    #[test]
    fn test_sma() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(MaCross::sma(&data, 3), Some((3.0 + 4.0 + 5.0) / 3.0));
        assert_eq!(
            MaCross::sma(&data, 5),
            Some((1.0 + 2.0 + 3.0 + 4.0 + 5.0) / 5.0)
        );
        assert_eq!(MaCross::sma(&data, 6), None);
    }

    #[test]
    fn test_no_signal_without_enough_bars() {
        let mut node = MaCross::new("test_ma");
        let state = StateStore::new();

        // Feed fewer bars than slow_period
        let bar = RawBar {
            symbol: "test".into(),
            dt: Utc::now(),
            freq: Freq::F1,
            id: 0,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.5,
            vol: 1000.0,
            amount: 100_000.0,
            open_interest: None,
            delta: None,
        };
        for _ in 0..10 {
            node.on_bar(&bar, Freq::F1, &state).unwrap();
        }

        let signals = node.on_calculate(&state).unwrap();
        assert!(signals.is_empty());
    }

    #[test]
    fn test_golden_cross_signal() {
        let mut node = MaCross::new("test_ma");
        let state = StateStore::new();

        // 前 21 根 bar：所有 close = 100.0（fast_ma == slow_ma）
        for i in 0..21 {
            let bar = RawBar {
                symbol: "test".into(),
                dt: Utc::now(),
                freq: Freq::F1,
                id: i,
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.0,
                vol: 1000.0,
                amount: 100_000.0,
                open_interest: None,
                delta: None,
            };
            node.on_bar(&bar, Freq::F1, &state).unwrap();
        }

        // 第 22 根 bar：大幅拉升 → fast_ma(104.0) > slow_ma(≈100.95) → 金叉
        let bar_up = RawBar {
            symbol: "test".into(),
            dt: Utc::now(),
            freq: Freq::F1,
            id: 21,
            open: 105.0,
            high: 125.0,
            low: 104.0,
            close: 120.0,
            vol: 5000.0,
            amount: 500_000.0,
            open_interest: None,
            delta: None,
        };
        node.on_bar(&bar_up, Freq::F1, &state).unwrap();

        let signals = node.on_calculate(&state).unwrap();
        assert_eq!(signals.len(), 1);
        assert!(matches!(signals[0].action, SignalAction::Long));
        assert_eq!(signals[0].metadata.get("reason").unwrap(), "golden_cross");
    }

    #[test]
    fn test_death_cross_signal() {
        let mut node = MaCross::new("test_ma");
        let state = StateStore::new();

        // 前 21 根 bar：所有 close = 100.0（fast_ma == slow_ma）
        for i in 0..21 {
            let bar = RawBar {
                symbol: "test".into(),
                dt: Utc::now(),
                freq: Freq::F1,
                id: i,
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.0,
                vol: 1000.0,
                amount: 100_000.0,
                open_interest: None,
                delta: None,
            };
            node.on_bar(&bar, Freq::F1, &state).unwrap();
        }

        // 第 22 根 bar：大幅下跌 → fast_ma(96.0) < slow_ma(≈99.05) → 死叉
        let bar_down = RawBar {
            symbol: "test".into(),
            dt: Utc::now(),
            freq: Freq::F1,
            id: 21,
            open: 100.0,
            high: 102.0,
            low: 75.0,
            close: 80.0,
            vol: 5000.0,
            amount: 500_000.0,
            open_interest: None,
            delta: None,
        };
        node.on_bar(&bar_down, Freq::F1, &state).unwrap();

        let signals = node.on_calculate(&state).unwrap();
        assert_eq!(signals.len(), 1);
        assert!(matches!(signals[0].action, SignalAction::Short));
        assert_eq!(signals[0].metadata.get("reason").unwrap(), "death_cross");
    }
}

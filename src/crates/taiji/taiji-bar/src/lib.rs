//! Tick-to-KLine 聚合引擎 — BarNode 实现 ComputeNode。
//! 薄包装：委托给 taiji-engine::pipeline::bar_gen::BarGenerator。
//! 参考: czsc BarGenerator (Apache 2.0)

use std::sync::Arc;

use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::pipeline::bar_gen::{AggMode, BarGenerator};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar, Symbol};
use taiji_engine::types::state::{StateKey, StateValue};
use taiji_engine::types::tick::TickData;

// ── BarNode ───────────────────────────────────────────────────────────

/// Bar 生成节点。
///
/// 实现 `ComputeNode`，通过 `on_tick` 接收逐笔 tick，按时间边界聚合为 `RawBar`，
/// 写入 `StateStore`（key = `"bars:{freq_key}"`，如 `"bars:1m"`）。
///
/// 内部委托给 `BarGenerator` 做实际的 tick→bar 聚合。
///
/// 配置参数（NodeConfig）：
/// - `freq` (str): 周期标识，如 `"1m"`, `"5m"`, `"1h"`, `"1d"`。默认 `"1m"`。
pub struct BarNode {
    id: NodeId,
    freq: Freq,
    generator: Option<BarGenerator>,
}

impl BarNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            freq: Freq::F1,
            generator: None,
        }
    }

    fn output_key(&self) -> StateKey {
        format!("bars:{}", self.freq.freq_key())
    }
}

impl ComputeNode for BarNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "BarNode"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        vec![]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec![self.output_key()]
    }

    fn on_init(&mut self, config: &NodeConfig, _state: &StateStore) -> Result<()> {
        if let Some(freq_str) = config.get_str("freq") {
            self.freq = Freq::from_key(freq_str).unwrap_or(Freq::F1);
        }
        Ok(())
    }

    fn on_tick(&mut self, tick: &TickData, state: &StateStore) -> Result<()> {
        // 延迟初始化：symbol 来自第一条 tick
        if self.generator.is_none() {
            let symbol = Symbol::from(tick.instrument.as_str());
            self.generator = Some(BarGenerator::new(
                symbol,
                vec![AggMode::Time],
                vec![self.freq],
            ));
        }

        let bg = self.generator.as_mut().unwrap();
        let closed = bg.update_tick(tick);

        for (_freq, bar) in &closed {
            let key = self.output_key();
            let bars: Arc<Vec<Arc<RawBar>>> =
                state.get(&key).unwrap_or_else(|| Arc::new(Vec::new()));
            let mut new_bars: Vec<Arc<RawBar>> = (*bars).clone();
            new_bars.push(Arc::new(bar.clone()));
            state.set(key, StateValue::Bars(Arc::new(new_bars)), self.id());
        }

        Ok(())
    }

    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![self.freq]
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone, Timelike, Utc};

    fn make_tick(ts_ms: i64, price: f64, vol: f64, amount: f64, oi: f64) -> TickData {
        TickData {
            instrument: "rb9999".into(),
            timestamp_ms: ts_ms,
            last_price: price,
            volume: vol,
            turnover: amount,
            open_interest: oi,
            ..TickData::default()
        }
    }

    fn ts(hour: u32, min: u32, sec: u32) -> i64 {
        Utc.with_ymd_and_hms(2026, 7, 22, hour, min, sec)
            .unwrap()
            .timestamp_millis()
    }

    fn ts_day(day: u32, hour: u32, min: u32, sec: u32) -> i64 {
        Utc.with_ymd_and_hms(2026, 7, day, hour, min, sec)
            .unwrap()
            .timestamp_millis()
    }

    // ── 单 tick 累加 + 边界闭合 ──

    #[test]
    fn test_single_tick_no_close() {
        let mut node = BarNode::new("bar1".into());
        let store = StateStore::new();

        node.on_tick(
            &make_tick(ts(9, 1, 0), 4000.0, 100.0, 400_000.0, 5000.0),
            &store,
        )
        .unwrap();

        // 同一个桶内的 tick 不会闭合 bar
        assert!(store
            .get::<Arc<Vec<Arc<RawBar>>>>(&node.output_key())
            .is_none());
    }

    #[test]
    fn test_boundary_close() {
        let mut node = BarNode::new("bar1".into());
        node.freq = Freq::F5;
        let store = StateStore::new();

        // 09:01 → 桶 09:00
        node.on_tick(
            &make_tick(ts(9, 1, 0), 4000.0, 100.0, 400_000.0, 5000.0),
            &store,
        )
        .unwrap();
        // 09:03 → 桶 09:00（同一桶）
        node.on_tick(
            &make_tick(ts(9, 3, 0), 4010.0, 200.0, 802_000.0, 5000.0),
            &store,
        )
        .unwrap();
        // 09:05 → 桶 09:05（跨边界）
        node.on_tick(
            &make_tick(ts(9, 5, 0), 4020.0, 300.0, 1_206_000.0, 5100.0),
            &store,
        )
        .unwrap();

        let bars: Arc<Vec<Arc<RawBar>>> = store.get(&node.output_key()).unwrap();
        assert_eq!(bars.len(), 1);
        let bar = &bars[0];
        assert_eq!(bar.open, 4000.0);
        assert_eq!(bar.high, 4010.0);
        assert_eq!(bar.low, 4000.0);
        assert_eq!(bar.close, 4010.0);
        assert_eq!(bar.vol, 100.0);
        assert_eq!(bar.amount, 402_000.0);
        assert_eq!(bar.open_interest, Some(5000.0));
    }

    #[test]
    fn test_volume_rollback_handling() {
        let mut node = BarNode::new("bar1".into());
        let store = StateStore::new();

        // 主力换月导致累计成交量回退：vol 200→100
        node.on_tick(
            &make_tick(ts(9, 0, 0), 4000.0, 200.0, 800_000.0, 5000.0),
            &store,
        )
        .unwrap();
        node.on_tick(
            &make_tick(ts(9, 1, 0), 4010.0, 100.0, 400_000.0, 5000.0),
            &store,
        )
        .unwrap();

        let bars: Arc<Vec<Arc<RawBar>>> = store.get(&node.output_key()).unwrap();
        assert_eq!(bars.len(), 1);
        let bar = &bars[0];
        // vol: 0 + max(0, 100-200) = 0
        assert_eq!(bar.vol, 0.0);
        // amount: 0 + max(0, 400k-800k) = 0
        assert_eq!(bar.amount, 0.0);
    }

    // ── 跨日处理 ──

    #[test]
    fn test_cross_day() {
        let mut node = BarNode::new("bar1".into());
        node.freq = Freq::F5;
        let store = StateStore::new();

        // 7/22 23:58 → 桶 23:55 (5min)
        node.on_tick(
            &make_tick(ts_day(22, 23, 58, 0), 4000.0, 100.0, 400_000.0, 5000.0),
            &store,
        )
        .unwrap();

        // 7/23 00:01 → 桶 00:00，跨日跨桶
        node.on_tick(
            &make_tick(ts_day(23, 0, 1, 0), 4010.0, 200.0, 802_000.0, 5000.0),
            &store,
        )
        .unwrap();

        let bars: Arc<Vec<Arc<RawBar>>> = store.get(&node.output_key()).unwrap();
        assert_eq!(bars.len(), 1);
        // bar 结束时间 = bucket 边界，即次日 00:00
        assert_eq!(bars[0].dt.day(), 23);
        assert_eq!(bars[0].dt.hour(), 0);
        assert_eq!(bars[0].dt.minute(), 0);
    }

    // ── on_init 读取 freq 配置 ──

    #[test]
    fn test_on_init_custom_freq() {
        let mut node = BarNode::new("bar1".into());
        let store = StateStore::new();
        let mut config = NodeConfig::new();
        config
            .params
            .insert("freq".into(), serde_json::Value::String("1h".into()));

        node.on_init(&config, &store).unwrap();

        assert_eq!(node.freq, Freq::F60);
        assert_eq!(node.output_key(), "bars:1h");
        assert_eq!(node.subscribed_freqs(), vec![Freq::F60]);
    }

    #[test]
    fn test_on_init_default_freq() {
        let mut node = BarNode::new("bar1".into());
        let store = StateStore::new();
        let config = NodeConfig::new();

        node.on_init(&config, &store).unwrap();

        assert_eq!(node.freq, Freq::F1);
        assert_eq!(node.output_key(), "bars:1m");
    }

    // ── 多 bar 连续闭合 ──

    #[test]
    fn test_multiple_bars() {
        let mut node = BarNode::new("bar1".into());
        node.freq = Freq::F5;
        let store = StateStore::new();

        // Bar 1: 09:00-09:04
        node.on_tick(
            &make_tick(ts(9, 0, 0), 4000.0, 100.0, 400_000.0, 5000.0),
            &store,
        )
        .unwrap();
        node.on_tick(
            &make_tick(ts(9, 4, 59), 4010.0, 200.0, 802_000.0, 5000.0),
            &store,
        )
        .unwrap();

        // Bar 2: 09:05-09:09
        node.on_tick(
            &make_tick(ts(9, 5, 0), 4020.0, 300.0, 1_206_000.0, 5100.0),
            &store,
        )
        .unwrap();
        node.on_tick(
            &make_tick(ts(9, 9, 59), 4030.0, 400.0, 1_612_000.0, 5200.0),
            &store,
        )
        .unwrap();

        // Bar 3: 09:10+
        node.on_tick(
            &make_tick(ts(9, 10, 0), 4040.0, 500.0, 2_020_000.0, 5300.0),
            &store,
        )
        .unwrap();

        let bars: Arc<Vec<Arc<RawBar>>> = store.get(&node.output_key()).unwrap();
        assert_eq!(bars.len(), 2);

        // Bar 1
        assert_eq!(bars[0].open, 4000.0);
        assert_eq!(bars[0].close, 4010.0);
        assert_eq!(bars[0].vol, 100.0);
        // Bar 2
        assert_eq!(bars[1].open, 4020.0);
        assert_eq!(bars[1].close, 4030.0);
        assert_eq!(bars[1].vol, 100.0);
    }
}

//! VPIN — Volume-Synchronized Probability of Informed Trading.
//!
//! Reference: Easley, López de Prado, O'Hara (2011–2012).
//! Volume-bucket approach: classify each tick as buy/sell, accumulate volume
//! into fixed-size buckets, then compute VPIN = E[|V_buy - V_sell|] / V_bucket.
//! VPIN > 0.8 signals high toxicity (flash-crash / liquidity-drain warning).

use crate::welford::WelfordStats;
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};

use taiji_engine::types::state::{StateKey, StateValue};
use taiji_engine::types::tick::TickData;

/// VPIN computation node.
///
/// Accumulates per-tick volume deltas into fixed-size buckets, classifies each
/// tick as buyer- or seller-initiated using the tick rule, and computes VPIN on
/// every bucket completion. Maintains an online Welford distribution of VPIN
/// values for CDF-based toxicity scoring.
pub struct VpinNode {
    id: NodeId,
    bucket_volume: u32,
    current_bucket_vol: u32,
    bucket_buy_vol: u32,
    bucket_sell_vol: u32,
    prev_cum_volume: f64,
    prev_price: f64,
    vpin_stats: WelfordStats,
}

impl VpinNode {
    pub fn new(node_id: &str, bucket_volume: u32) -> Self {
        Self {
            id: node_id.to_string(),
            bucket_volume,
            current_bucket_vol: 0,
            bucket_buy_vol: 0,
            bucket_sell_vol: 0,
            prev_cum_volume: 0.0,
            prev_price: 0.0,
            vpin_stats: WelfordStats::new(),
        }
    }
}

impl ComputeNode for VpinNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "vpin"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        vec![]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec!["vpin".into(), "vpin_cdf".into()]
    }

    fn on_init(&mut self, config: &NodeConfig, _state: &StateStore) -> Result<()> {
        if let Some(bv) = config.get_i64("bucket_volume") {
            self.bucket_volume = bv as u32;
        }
        Ok(())
    }

    fn on_tick(&mut self, tick: &TickData, state: &StateStore) -> Result<()> {
        // ── per-tick volume delta ──────────────────────────────────────
        let tick_vol = if tick.volume >= self.prev_cum_volume {
            (tick.volume - self.prev_cum_volume) as u32
        } else {
            // cumulative volume reset (new session / instrument switch)
            0u32
        };
        self.prev_cum_volume = tick.volume;

        if tick_vol == 0 {
            return Ok(());
        }

        // ── buyer / seller classification (tick rule) ──────────────────
        let is_buy = classify_tick(tick, self.prev_price);
        self.prev_price = tick.last_price;

        // ── accumulate into current bucket ──────────────────────────────
        self.current_bucket_vol += tick_vol;
        if is_buy {
            self.bucket_buy_vol += tick_vol;
        } else {
            self.bucket_sell_vol += tick_vol;
        }

        // ── bucket complete → emit VPIN ─────────────────────────────────
        if self.current_bucket_vol >= self.bucket_volume {
            let vpin = (self.bucket_buy_vol as f64 - self.bucket_sell_vol as f64).abs()
                / self.current_bucket_vol as f64;

            self.vpin_stats.update(vpin);
            let cdf_val = self.vpin_stats.cdf(vpin);

            state.set("vpin".into(), StateValue::F64(vpin), self.id());
            state.set("vpin_cdf".into(), StateValue::F64(cdf_val), self.id());

            // Reset for next bucket
            self.current_bucket_vol = 0;
            self.bucket_buy_vol = 0;
            self.bucket_sell_vol = 0;
        }

        Ok(())
    }

    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::Tick]
    }
}

/// Tick-rule classification: buyer-initiated = true, seller-initiated = false.
///
/// Priority:
/// 1. Trade at ask or above → buy
/// 2. Trade at bid or below  → sell
/// 3. Mid-quote comparison (Lee–Ready fallback)
/// 4. Previous price comparison (last resort)
fn classify_tick(tick: &TickData, prev_price: f64) -> bool {
    if tick.last_price >= tick.ask_price1 && tick.ask_price1 > 0.0 {
        return true;
    }
    if tick.last_price <= tick.bid_price1 && tick.bid_price1 > 0.0 {
        return false;
    }
    let mid = (tick.bid_price1 + tick.ask_price1) / 2.0;
    if mid > 0.0 {
        tick.last_price >= mid
    } else {
        tick.last_price >= prev_price
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tick(last_price: f64, bid: f64, ask: f64, volume: f64) -> TickData {
        let mut t = TickData::default();
        t.last_price = last_price;
        t.bid_price1 = bid;
        t.ask_price1 = ask;
        t.volume = volume;
        t
    }

    #[test]
    fn test_classify_buy_at_ask() {
        let t = make_tick(100.0, 99.0, 100.0, 10.0);
        assert!(classify_tick(&t, 0.0));
    }

    #[test]
    fn test_classify_sell_at_bid() {
        let t = make_tick(99.0, 99.0, 100.0, 10.0);
        assert!(!classify_tick(&t, 0.0));
    }

    #[test]
    fn test_classify_mid_quote_fallback() {
        // mid = 99.5; last_price=99.6 >= 99.5 → buy
        let t = make_tick(99.6, 99.0, 100.0, 10.0);
        assert!(classify_tick(&t, 0.0));

        // mid = 99.5; last_price=99.4 < 99.5 → sell
        let t = make_tick(99.4, 99.0, 100.0, 10.0);
        assert!(!classify_tick(&t, 0.0));
    }

    #[test]
    fn test_classify_prev_price_fallback() {
        // zero bid/ask → fallback to prev_price
        let t = make_tick(100.0, 0.0, 0.0, 10.0);
        assert!(classify_tick(&t, 99.0)); // 100 >= 99 → buy
        assert!(!classify_tick(&t, 101.0)); // 100 < 101 → sell
    }

    #[test]
    fn test_vpin_bucket_formation() {
        let mut node = VpinNode::new("vpin_test", 100);
        let store = StateStore::new();

        // Tick 1: 60 vol @ 100.0, bid=99 ask=101 → mid=100, >=100 → buy
        let t1 = make_tick(100.0, 99.0, 101.0, 60.0);
        node.on_tick(&t1, &store).unwrap();
        // Bucket not full yet (60 < 100)
        assert!(store.get_json(&"vpin".into()).is_none());

        // Tick 2: 50 vol @ 101.5, bid=100 ask=102 → price > ask → buy
        let t2 = make_tick(101.5, 100.0, 102.0, 110.0);
        node.on_tick(&t2, &store).unwrap();

        // Bucket full (60 + 50 = 110 >= 100), both buys
        let vpin: Option<f64> = store.get(&"vpin".into());
        assert!(vpin.is_some());
        // VPIN = |110 - 0| / 110 = 1.0
        assert!((vpin.unwrap() - 1.0).abs() < 1e-10);

        let cdf: Option<f64> = store.get(&"vpin_cdf".into());
        assert!(cdf.is_some());
        assert!(cdf.unwrap() >= 0.0 && cdf.unwrap() <= 1.0);
    }

    #[test]
    fn test_vpin_mixed_buy_sell() {
        let mut node = VpinNode::new("vpin_mix", 100);
        let store = StateStore::new();

        // Tick 1: 70 vol → buy (price at ask)
        let t1 = make_tick(101.0, 100.0, 101.0, 70.0);
        node.on_tick(&t1, &store).unwrap();

        // Tick 2: 40 vol → sell (price at bid)
        let t2 = make_tick(99.0, 99.0, 100.0, 110.0);
        node.on_tick(&t2, &store).unwrap();

        // Bucket full: buy=70, sell=40, total=110
        let vpin: Option<f64> = store.get(&"vpin".into());
        assert!(vpin.is_some());
        // VPIN = |70 - 40| / 110 = 30/110 ≈ 0.2727
        assert!((vpin.unwrap() - 30.0 / 110.0).abs() < 1e-10);
    }

    #[test]
    fn test_vpin_no_tick_volume() {
        let mut node = VpinNode::new("vpin_zero", 100);
        let store = StateStore::new();

        // Cumulative volume unchanged → no delta
        let t1 = make_tick(100.0, 99.0, 101.0, 100.0);
        node.on_tick(&t1, &store).unwrap();
        assert!(store.get_json(&"vpin".into()).is_none());

        // Same cumulative volume again
        let t2 = make_tick(100.0, 99.0, 101.0, 100.0);
        node.on_tick(&t2, &store).unwrap();
        assert!(store.get_json(&"vpin".into()).is_none());
    }

    #[test]
    fn test_vpin_on_init_config() {
        let mut node = VpinNode::new("vpin_cfg", 50);
        let store = StateStore::new();

        let mut config = NodeConfig::new();
        config.params.insert(
            "bucket_volume".into(),
            serde_json::Value::Number(200.into()),
        );

        node.on_init(&config, &store).unwrap();
        assert_eq!(node.bucket_volume, 200);
    }
}

//! OFI — Order Flow Imbalance (5-level order book).
//!
//! Reference: Cont, Kukanov, Stoikov (2014), "The Price Impact of Order Book Events".
//! OFI = Σ Δbid_vol[i] - Σ Δask_vol[i]  (per tick, 5 levels).
//! Positive OFI → buying pressure, negative OFI → selling pressure.

use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};

use taiji_engine::types::state::{StateKey, StateValue};
use taiji_engine::types::tick::TickData;

/// OFI direction signal value.
const OFI_BUY: i32 = 1;
const OFI_SELL: i32 = -1;
const OFI_NEUTRAL: i32 = 0;

/// OFI computation node.
///
/// On every tick, computes the 5-level order flow imbalance as sum of
/// bid-volume changes minus sum of ask-volume changes from the previous tick.
/// Outputs a raw OFI value and a direction signal (buy/sell/neutral).
pub struct OfiNode {
    id: NodeId,
    bid_vol: [u32; 5],
    ask_vol: [u32; 5],
}

impl OfiNode {
    pub fn new(node_id: &str) -> Self {
        Self {
            id: node_id.to_string(),
            bid_vol: [0u32; 5],
            ask_vol: [0u32; 5],
        }
    }
}

impl ComputeNode for OfiNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "ofi"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        vec![]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec!["ofi".into(), "ofi_signal".into()]
    }

    fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn on_tick(&mut self, tick: &TickData, state: &StateStore) -> Result<()> {
        // ── snapshot current 5-level volumes ────────────────────────────
        let cur_bid: [u32; 5] = [
            tick.bid_volume1 as u32,
            tick.bid_volume2 as u32,
            tick.bid_volume3 as u32,
            tick.bid_volume4 as u32,
            tick.bid_volume5 as u32,
        ];
        let cur_ask: [u32; 5] = [
            tick.ask_volume1 as u32,
            tick.ask_volume2 as u32,
            tick.ask_volume3 as u32,
            tick.ask_volume4 as u32,
            tick.ask_volume5 as u32,
        ];

        // ── OFI = Σ Δbid - Σ Δask ──────────────────────────────────────
        let mut ofi: f64 = 0.0;
        for i in 0..5 {
            let delta_bid = cur_bid[i] as i64 - self.bid_vol[i] as i64;
            let delta_ask = cur_ask[i] as i64 - self.ask_vol[i] as i64;
            ofi += (delta_bid - delta_ask) as f64;
        }

        // Update stored state for next tick
        self.bid_vol = cur_bid;
        self.ask_vol = cur_ask;

        // ── direction signal ────────────────────────────────────────────
        let direction = if ofi > 0.0 {
            OFI_BUY
        } else if ofi < 0.0 {
            OFI_SELL
        } else {
            OFI_NEUTRAL
        };

        let bid_sum: u32 = cur_bid.iter().sum();
        let ask_sum: u32 = cur_ask.iter().sum();

        state.set("ofi".into(), StateValue::F64(ofi), self.id());
        state.set(
            "ofi_signal".into(),
            StateValue::Json(serde_json::json!({
                "ofi": ofi,
                "direction": direction,
                "bid_sum": bid_sum,
                "ask_sum": ask_sum,
            })),
            self.id(),
        );

        Ok(())
    }

    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::Tick]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tick(bid: [i32; 5], ask: [i32; 5]) -> TickData {
        let mut t = TickData::default();
        t.bid_volume1 = bid[0];
        t.bid_volume2 = bid[1];
        t.bid_volume3 = bid[2];
        t.bid_volume4 = bid[3];
        t.bid_volume5 = bid[4];
        t.ask_volume1 = ask[0];
        t.ask_volume2 = ask[1];
        t.ask_volume3 = ask[2];
        t.ask_volume4 = ask[3];
        t.ask_volume5 = ask[4];
        t
    }

    #[test]
    fn test_ofi_first_tick() {
        let mut node = OfiNode::new("ofi_test");
        let store = StateStore::new();

        let t = make_tick([100, 50, 30, 20, 10], [80, 40, 25, 15, 5]);
        node.on_tick(&t, &store).unwrap();

        // All deltas from zero: ΣΔbid=210, ΣΔask=165, OFI=45
        let ofi: Option<f64> = store.get(&"ofi".into());
        assert!(ofi.is_some());
        assert!((ofi.unwrap() - 45.0).abs() < 1e-10);
    }

    #[test]
    fn test_ofi_second_tick_delta() {
        let mut node = OfiNode::new("ofi_delta");
        let store = StateStore::new();

        // First tick establishes baseline
        let t1 = make_tick([100, 50, 30, 20, 10], [80, 40, 25, 15, 5]);
        node.on_tick(&t1, &store).unwrap();

        // Second tick: bid L1 +20, bid L2 +10, ask L1 -10, ask L2 -10
        let t2 = make_tick([120, 60, 30, 20, 10], [70, 30, 25, 15, 5]);
        node.on_tick(&t2, &store).unwrap();

        // Δbid = (20+10+0+0+0) = 30, Δask = (-10-10+0+0+0) = -20
        // OFI = 30 - (-20) = 50
        let ofi: Option<f64> = store.get(&"ofi".into());
        assert!(ofi.is_some());
        assert!((ofi.unwrap() - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_ofi_direction_buy() {
        let mut node = OfiNode::new("ofi_dir_buy");
        let store = StateStore::new();

        // Increase bids only → buying pressure
        node.on_tick(&make_tick([100, 0, 0, 0, 0], [0, 0, 0, 0, 0]), &store)
            .unwrap();
        node.on_tick(&make_tick([150, 0, 0, 0, 0], [0, 0, 0, 0, 0]), &store)
            .unwrap();

        let sig = store.get_json(&"ofi_signal".into()).unwrap();
        assert_eq!(sig["direction"].as_i64(), Some(OFI_BUY as i64));
        assert!((sig["ofi"].as_f64().unwrap() - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_ofi_direction_sell() {
        let mut node = OfiNode::new("ofi_dir_sell");
        let store = StateStore::new();

        // Increase asks only → selling pressure
        node.on_tick(&make_tick([0, 0, 0, 0, 0], [100, 0, 0, 0, 0]), &store)
            .unwrap();
        node.on_tick(&make_tick([0, 0, 0, 0, 0], [150, 0, 0, 0, 0]), &store)
            .unwrap();

        let sig = store.get_json(&"ofi_signal".into()).unwrap();
        assert_eq!(sig["direction"].as_i64(), Some(OFI_SELL as i64));
        assert!((sig["ofi"].as_f64().unwrap() + 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_ofi_neutral() {
        let mut node = OfiNode::new("ofi_neutral");
        let store = StateStore::new();

        // No volume change → OFI = 0
        let t = make_tick([100, 50, 30, 20, 10], [80, 40, 25, 15, 5]);
        node.on_tick(&t, &store).unwrap();
        node.on_tick(&t, &store).unwrap();

        let sig = store.get_json(&"ofi_signal".into()).unwrap();
        assert_eq!(sig["direction"].as_i64(), Some(OFI_NEUTRAL as i64));
        assert!((sig["ofi"].as_f64().unwrap() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_ofi_output_bid_ask_sum() {
        let mut node = OfiNode::new("ofi_sum");
        let store = StateStore::new();

        let t = make_tick([100, 50, 30, 20, 10], [80, 40, 25, 15, 5]);
        node.on_tick(&t, &store).unwrap();

        let sig = store.get_json(&"ofi_signal".into()).unwrap();
        assert_eq!(sig["bid_sum"].as_u64(), Some(210));
        assert_eq!(sig["ask_sum"].as_u64(), Some(165));
    }

    #[test]
    fn test_ofi_five_level_independence() {
        let mut node = OfiNode::new("ofi_5level");
        let store = StateStore::new();

        // Baseline: all zero
        node.on_tick(&make_tick([0; 5], [0; 5]), &store).unwrap();

        // Change only level 5 → OFI should capture it
        let t2 = make_tick([0, 0, 0, 0, 50], [0, 0, 0, 0, 10]);
        node.on_tick(&t2, &store).unwrap();

        let ofi: Option<f64> = store.get(&"ofi".into());
        assert!(ofi.is_some());
        // Δbid L5 = 50, Δask L5 = 10 → OFI = 40
        assert!((ofi.unwrap() - 40.0).abs() < 1e-10);
    }
}

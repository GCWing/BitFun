use std::sync::Arc;

use ndarray::Array2;
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::signal::{Signal, SignalAction};
use taiji_engine::types::state::{StateKey, StateValue};
use taiji_engine::types::tick::TickData;
use taiji_engine::types::NodeId;

use crate::index::PatternIndex;

/// Feature extraction — builds a 6-dimensional time-series matrix from bars.
///
/// Feature columns (order):
/// 0. close_logret  = ln(close_t / close_{t-1})
/// 1. vol_logret    = ln(vol_t / vol_{t-1})
/// 2. amount_logret = ln(amount_t / amount_{t-1})
/// 3. delta         = bar.delta (or 0 if None)
/// 4. RSI(14)       = 14-period Relative Strength Index
/// 5. MACD_hist     = MACD(12,26,9) histogram
pub fn extract_features(bars: &[RawBar]) -> Array2<f64> {
    let n = bars.len();
    if n < 2 {
        return Array2::zeros((n, 6));
    }

    let rsi = compute_rsi(bars, 14);
    let macd_hist = compute_macd_hist(bars);

    let mut feats = Array2::zeros((n, 6));

    for i in 0..n {
        // 0: close_logret
        if i > 0 && bars[i - 1].close > 0.0 && bars[i].close > 0.0 {
            feats[[i, 0]] = (bars[i].close / bars[i - 1].close).ln();
        }
        // 1: vol_logret
        if i > 0 && bars[i - 1].vol > 0.0 && bars[i].vol > 0.0 {
            feats[[i, 1]] = (bars[i].vol / bars[i - 1].vol).ln();
        }
        // 2: amount_logret
        if i > 0 && bars[i - 1].amount > 0.0 && bars[i].amount > 0.0 {
            feats[[i, 2]] = (bars[i].amount / bars[i - 1].amount).ln();
        }
        // 3: delta
        feats[[i, 3]] = bars[i].delta.unwrap_or(0.0);
        // 4: RSI(14)
        feats[[i, 4]] = rsi[i];
        // 5: MACD histogram
        feats[[i, 5]] = macd_hist[i];
    }

    feats
}

// ── indicator helpers ──

fn ema(data: &[f64], period: usize) -> Vec<f64> {
    let mut out = vec![0.0; data.len()];
    if data.len() < period {
        return out;
    }
    let alpha = 2.0 / (period as f64 + 1.0);
    out[period - 1] = data[..period].iter().sum::<f64>() / period as f64;
    for i in period..data.len() {
        out[i] = data[i] * alpha + out[i - 1] * (1.0 - alpha);
    }
    out
}

fn compute_rsi(bars: &[RawBar], period: usize) -> Vec<f64> {
    let n = bars.len();
    let mut rsi = vec![0.0; n];
    if n < period + 1 {
        return rsi;
    }

    let mut gains = 0.0_f64;
    let mut losses = 0.0_f64;
    for i in 1..=period {
        let diff = bars[i].close - bars[i - 1].close;
        if diff > 0.0 {
            gains += diff;
        } else {
            losses -= diff;
        }
    }
    let mut avg_gain = gains / period as f64;
    let mut avg_loss = losses / period as f64;

    for i in period..n {
        rsi[i] = if avg_loss == 0.0 {
            100.0
        } else {
            100.0 - 100.0 / (1.0 + avg_gain / avg_loss)
        };
        if i + 1 < n {
            let diff = bars[i + 1].close - bars[i].close;
            let gain = if diff > 0.0 { diff } else { 0.0 };
            let loss = if diff < 0.0 { -diff } else { 0.0 };
            avg_gain = (avg_gain * (period - 1) as f64 + gain) / period as f64;
            avg_loss = (avg_loss * (period - 1) as f64 + loss) / period as f64;
        }
    }
    rsi
}

fn compute_macd_hist(bars: &[RawBar]) -> Vec<f64> {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let ema12 = ema(&closes, 12);
    let ema26 = ema(&closes, 26);
    let macd: Vec<f64> = ema12.iter().zip(ema26.iter()).map(|(a, b)| a - b).collect();
    let signal = ema(&macd, 9);
    macd.iter().zip(signal.iter()).map(|(m, s)| m - s).collect()
}

// ── PatternMatchNode ──

/// ComputeNode that extracts a multi-dimensional feature segment from the
/// most recent `lookback_bars` bars, searches the pattern index, and writes
/// the top matches into the StateStore.
pub struct PatternMatchNode {
    node_id: NodeId,
    index: Arc<PatternIndex>,
    lookback_bars: usize,
    pub feature_fields: Vec<String>,
    bar_buffer: Vec<RawBar>,
}

impl PatternMatchNode {
    pub fn new(node_id: NodeId, index: Arc<PatternIndex>, lookback_bars: usize) -> Self {
        Self {
            node_id,
            index,
            lookback_bars,
            feature_fields: vec![
                "close_logret".into(),
                "vol_logret".into(),
                "amount_logret".into(),
                "delta".into(),
                "rsi_14".into(),
                "macd_hist".into(),
            ],
            bar_buffer: Vec::new(),
        }
    }
}

impl ComputeNode for PatternMatchNode {
    fn id(&self) -> NodeId {
        self.node_id.clone()
    }

    fn name(&self) -> &'static str {
        "PatternMatch"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        // Reads bars from a well-known key set by a data-source node.
        vec!["bars".into()]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec!["pattern_matches".into()]
    }

    fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> {
        self.bar_buffer.clear();
        Ok(())
    }

    fn on_tick(&mut self, tick: &TickData, state: &StateStore) -> Result<()> {
        let _ = (tick, state);
        Ok(())
    }

    fn on_bar(&mut self, bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
        self.bar_buffer.push(bar.clone());
        // Keep 2× lookback to have enough for indicator warm-up
        let limit = self.lookback_bars * 2;
        if self.bar_buffer.len() > limit {
            let drain = self.bar_buffer.len() - limit;
            self.bar_buffer.drain(..drain);
        }
        Ok(())
    }

    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<Signal>> {
        if self.bar_buffer.len() < self.lookback_bars {
            return Ok(vec![]);
        }

        let recent = &self.bar_buffer[self.bar_buffer.len() - self.lookback_bars..];
        let query = extract_features(recent);

        let matches = self.index.search(&query, 3);

        // Write results to state
        let json = serde_json::to_value(
            matches
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "pattern_id": m.pattern_id,
                        "dtw_distance": m.dtw_distance,
                        "similarity": m.similarity,
                        "matched_segment": [m.matched_segment.0, m.matched_segment.1],
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();

        state.set(
            "pattern_matches".into(),
            StateValue::Json(json),
            self.node_id.clone(),
        );

        let signals: Vec<Signal> = matches
            .iter()
            .filter(|m| m.similarity > 0.7)
            .map(|m| Signal {
                timestamp: recent.last().map(|b| b.dt).unwrap_or_default(),
                instrument: recent
                    .last()
                    .map(|b| b.symbol.0.to_string())
                    .unwrap_or_default(),
                freq: Freq::F5,
                action: SignalAction::Hold,
                entry: None,
                stop_loss: None,
                take_profit: None,
                size: None,
                source: self.node_id.clone(),
                confidence: m.similarity,
                metadata: std::collections::HashMap::from([(
                    "pattern_id".into(),
                    m.pattern_id.clone(),
                )]),
                disclaimer: None,
            })
            .collect();

        Ok(signals)
    }

    fn on_session_begin(&mut self, _date: u32, _state: &StateStore) -> Result<()> {
        self.bar_buffer.clear();
        Ok(())
    }

    fn on_session_end(&mut self, _date: u32, _state: &StateStore) -> Result<()> {
        Ok(())
    }

    fn is_ready(&self, _state: &StateStore) -> bool {
        self.bar_buffer.len() >= self.lookback_bars
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::F5]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use std::sync::Arc;
    use taiji_engine::types::bar::Symbol;

    fn bar(close: f64, vol: f64, amount: f64, delta: Option<f64>) -> RawBar {
        RawBar {
            symbol: Symbol::from("ag2506"),
            dt: DateTime::from_timestamp_millis(0).unwrap(),
            freq: Freq::F5,
            id: 0,
            open: close - 0.5,
            high: close + 0.5,
            low: close - 1.0,
            close,
            vol,
            amount,
            open_interest: None,
            delta,
        }
    }

    #[test]
    fn extract_features_output_shape() {
        let bars: Vec<RawBar> = (0..20)
            .map(|i| bar(100.0 + i as f64 * 0.1, 1000.0, 100_000.0, Some(0.0)))
            .collect();
        let feats = extract_features(&bars);
        assert_eq!(feats.nrows(), 20);
        assert_eq!(feats.ncols(), 6);
    }

    #[test]
    fn extract_features_contains_logret() {
        // close goes from 100 to 101 → logret ≈ ln(1.01) ≈ 0.00995
        let bars = vec![
            bar(100.0, 1000.0, 100_000.0, None),
            bar(101.0, 1000.0, 100_000.0, None),
        ];
        let feats = extract_features(&bars);
        assert!((feats[[1, 0]] - 0.00995).abs() < 1e-4);
        // First bar has no previous close → logret = 0
        assert_eq!(feats[[0, 0]], 0.0);
    }

    #[test]
    fn node_is_ready_after_enough_bars() {
        let engine = crate::dtw::DtwEngine::new(3, vec![1.0; 6]);
        let index = Arc::new(PatternIndex::new(engine));
        let mut node = PatternMatchNode::new("pm1".into(), index, 10);
        let store = StateStore::new();

        assert!(!node.is_ready(&store));

        for i in 0..10 {
            node.on_bar(
                &bar(100.0 + i as f64, 1000.0, 100_000.0, None),
                Freq::F5,
                &store,
            )
            .unwrap();
        }
        assert!(node.is_ready(&store));
    }
}

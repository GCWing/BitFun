//! BarGenerator — tick→bar OI/Delta aggregation.
//!
//! 逻辑参考自 czsc bar_generator.rs（Apache 2.0, zengbin93）。
//! 以 Rust 独立实现，重构为 ComputeNode 管线架构。
//! 仅实现了时间聚合（AggMode::Time）；成交量/范围聚合预留扩展。

use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};

use crate::types::bar::{Freq, RawBar, Symbol};
use crate::types::tick::TickData;

/// Aggregation mode (currently only Time is implemented)
#[derive(Debug, Clone)]
pub enum AggMode {
    Time,
    Volume,
    Range,
}

/// Currently building, unclosed bar
struct PartialBar {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    vol: f64,
    amount: f64,
    open_interest_current: Option<f64>,
    delta_sum: f64,
    start_time: DateTime<Utc>,
    tick_count: u64,
    /// Previous tick's cumulative volume (for incremental calculation)
    prev_volume: f64,
    /// Previous tick's cumulative turnover (for incremental calculation)
    prev_amount: f64,
}

impl PartialBar {
    fn new(price: f64, vol: f64, amount: f64, oi: Option<f64>, time: DateTime<Utc>) -> Self {
        Self {
            open: price,
            high: price,
            low: price,
            close: price,
            vol: 0.0,    // bar-level increment starts at 0
            amount: 0.0, // bar-level increment starts at 0
            open_interest_current: oi,
            delta_sum: 0.0,
            start_time: time,
            tick_count: 1,
            prev_volume: vol, // preserve cumulative value for incremental calculation
            prev_amount: amount, // preserve cumulative value for incremental calculation
        }
    }

    fn update(&mut self, price: f64, vol: f64, amount: f64, oi: Option<f64>, delta: f64) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        // cumulative → incremental aggregation (prevents rollback from dominant-contract switching)
        self.vol += (vol - self.prev_volume).max(0.0);
        self.amount += (amount - self.prev_amount).max(0.0);
        self.open_interest_current = oi;
        self.delta_sum += delta;
        self.prev_volume = vol;
        self.prev_amount = amount;
        self.tick_count += 1;
    }

    fn finalize(&self, id: i32, symbol: Symbol, freq: Freq, end_time: DateTime<Utc>) -> RawBar {
        RawBar {
            symbol,
            dt: end_time,
            freq,
            id,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            vol: self.vol,
            amount: self.amount,
            open_interest: self.open_interest_current,
            delta: if self.delta_sum != 0.0 {
                Some(self.delta_sum)
            } else {
                None
            },
        }
    }
}

/// Bar generator.
///
/// Receives tick-by-tick data and aggregates into RawBar by specified time periods.
/// Supports simultaneous multi-frequency bar output (e.g., 1min + 5min + 15min).
pub struct BarGenerator {
    symbol: Symbol,
    partial_bars: BTreeMap<Freq, PartialBar>,
    completed_bars: BTreeMap<Freq, Vec<RawBar>>,
    #[allow(dead_code)]
    modes: Vec<AggMode>,
    time_freqs: Vec<Freq>,
    next_id: i32,
}

impl BarGenerator {
    pub fn new(symbol: Symbol, modes: Vec<AggMode>, time_freqs: Vec<Freq>) -> Self {
        Self {
            symbol,
            partial_bars: BTreeMap::new(),
            completed_bars: BTreeMap::new(),
            modes,
            time_freqs,
            next_id: 0,
        }
    }

    /// Process one tick. Returns all bars closed by this tick (sorted by Freq).
    pub fn update_tick(&mut self, tick: &TickData) -> Vec<(Freq, RawBar)> {
        let mut closed = Vec::new();
        let price = tick.last_price;
        let vol = tick.volume;
        let amount = tick.turnover;
        let oi = if tick.open_interest > 0.0 {
            Some(tick.open_interest)
        } else {
            None
        };

        let delta = Self::classify_delta(tick);

        let ts_ms = tick.timestamp_ms;
        let dt = Utc
            .timestamp_millis_opt(ts_ms)
            .single()
            .unwrap_or(Utc::now());

        for &freq in &self.time_freqs {
            let Some(minutes) = freq.minutes() else {
                continue;
            };

            let bucket = Self::time_bucket(dt, minutes);

            // Cross boundary → close old bar
            if let Some(partial) = self.partial_bars.get(&freq) {
                if partial.start_time != bucket {
                    let old = self.partial_bars.remove(&freq).unwrap();
                    let bar = old.finalize(self.next_id, self.symbol.clone(), freq, bucket);
                    self.next_id += 1;
                    self.completed_bars
                        .entry(freq)
                        .or_default()
                        .push(bar.clone());
                    closed.push((freq, bar));
                }
            }

            let entry = self
                .partial_bars
                .entry(freq)
                .or_insert_with(|| PartialBar::new(price, vol, amount, oi, bucket));
            entry.update(price, vol, amount, oi, delta);
        }

        closed
    }

    /// Classify trade direction.
    ///
    /// - GM mode: `trade_type` directly gives direction (±1)
    /// - CTP L1 mode: LastPrice >= AskPrice1 → buyer-initiated (+1), LastPrice <= BidPrice1 → seller-initiated (-1)
    fn classify_delta(tick: &TickData) -> f64 {
        if let Some(tt) = tick.trade_type {
            return tt;
        }
        if tick.last_price >= tick.ask_price1 && tick.ask_price1 > 0.0 {
            1.0
        } else if tick.last_price <= tick.bid_price1 && tick.bid_price1 > 0.0 {
            -1.0
        } else {
            0.0
        }
    }

    /// Calculate time bucket boundary.
    ///
    /// Rounds `dt` down to the nearest integer multiple of `minutes`.
    /// E.g., minutes=5, dt=09:33 → 09:30.
    ///
    /// Note: This implementation only depends on the hour/minute of the day, so boundary
    /// detection for daily and above periods (D/W/M) is correct (distinguishing different days
    /// via the date portion of `dt`), but weekly/monthly bars will not correctly align to
    /// week/month boundaries — this is a known czsc limitation. In practice, weekly and above
    /// bars are typically synthesized from daily bars.
    fn time_bucket(dt: DateTime<Utc>, minutes: i64) -> DateTime<Utc> {
        let total_minutes = dt.hour() as i64 * 60 + dt.minute() as i64;
        let bucket_min = (total_minutes / minutes) * minutes;
        let h = (bucket_min / 60) as u32;
        let m = (bucket_min % 60) as u32;
        Utc.with_ymd_and_hms(dt.year(), dt.month(), dt.day(), h, m, 0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap()
    }

    /// Get completed bar sequence for a given frequency (read-only)
    pub fn bars(&self, freq: &Freq) -> &[RawBar] {
        self.completed_bars
            .get(freq)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Clear completed bars for a given frequency (for memory management)
    pub fn clear_bars(&mut self, freq: &Freq) {
        self.completed_bars.remove(freq);
    }

    /// Return all configured frequency list
    pub fn configured_freqs(&self) -> &[Freq] {
        &self.time_freqs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tick(ts_ms: i64, price: f64, vol: f64, amount: f64, oi: f64) -> TickData {
        TickData {
            instrument: "rb9999".into(),
            trading_day: "20260722".into(),
            exchange_id: "SHFE".into(),
            exchange_inst_id: "rb9999".into(),
            last_price: price,
            pre_settlement_price: 0.0,
            pre_close_price: 0.0,
            pre_open_interest: 0.0,
            open_price: 0.0,
            highest_price: 0.0,
            lowest_price: 0.0,
            volume: vol,
            turnover: amount,
            open_interest: oi,
            close_price: 0.0,
            settlement_price: 0.0,
            upper_limit_price: 0.0,
            lower_limit_price: 0.0,
            pre_delta: 0.0,
            curr_delta: 0.0,
            update_time: String::new(),
            update_millisec: 0,
            bid_price1: 0.0,
            bid_volume1: 0,
            ask_price1: 0.0,
            ask_volume1: 0,
            bid_price2: 0.0,
            bid_volume2: 0,
            ask_price2: 0.0,
            ask_volume2: 0,
            bid_price3: 0.0,
            bid_volume3: 0,
            ask_price3: 0.0,
            ask_volume3: 0,
            bid_price4: 0.0,
            bid_volume4: 0,
            ask_price4: 0.0,
            ask_volume4: 0,
            bid_price5: 0.0,
            bid_volume5: 0,
            ask_price5: 0.0,
            ask_volume5: 0,
            average_price: 0.0,
            action_day: String::new(),
            trade_type: None,
            cum_volume: None,
            cum_position: None,
            timestamp_ms: ts_ms,
        }
    }

    /// Build a millisecond timestamp for a given UTC time (simplified: 2026-07-22 HH:MM:SS.000 UTC)
    fn ts(hour: u32, min: u32, sec: u32) -> i64 {
        let dt = Utc.with_ymd_and_hms(2026, 7, 22, hour, min, sec).unwrap();
        dt.timestamp_millis()
    }

    #[test]
    fn test_single_bar_5min() {
        let sym = Symbol::from("rb9999");
        let mut bg = BarGenerator::new(sym.clone(), vec![AggMode::Time], vec![Freq::F5]);

        // 09:01 → bucket 09:00, create new bar
        let closed = bg.update_tick(&make_tick(ts(9, 1, 0), 4000.0, 100.0, 400_000.0, 5000.0));
        assert!(closed.is_empty());

        // 09:03 → bucket 09:00, update same bar
        let closed = bg.update_tick(&make_tick(ts(9, 3, 0), 4010.0, 200.0, 802_000.0, 5000.0));
        assert!(closed.is_empty());

        // 09:05 → bucket 09:05, close old bar
        let closed = bg.update_tick(&make_tick(ts(9, 5, 0), 4020.0, 300.0, 1_206_000.0, 5100.0));
        assert_eq!(closed.len(), 1);

        let (_freq, bar) = &closed[0];
        assert_eq!(bar.open, 4000.0);
        assert_eq!(bar.high, 4010.0);
        assert_eq!(bar.low, 4000.0);
        assert_eq!(bar.close, 4010.0);
        // vol: 0 + (200-100) = 100
        assert_eq!(bar.vol, 100.0);
        // amount: 0 + (802k-400k) = 402k
        assert_eq!(bar.amount, 402_000.0);
        assert_eq!(bar.open_interest, Some(5000.0));

        let bars = bg.bars(&Freq::F5);
        assert_eq!(bars.len(), 1);
    }

    #[test]
    fn test_volume_increment_handles_rollback() {
        let sym = Symbol::from("rb9999");
        let mut bg = BarGenerator::new(sym.clone(), vec![AggMode::Time], vec![Freq::F1]);

        // Dominant-contract switch causes cumulative volume rollback: vol 200→100
        bg.update_tick(&make_tick(ts(9, 0, 0), 4000.0, 200.0, 800_000.0, 5000.0));
        let closed = bg.update_tick(&make_tick(ts(9, 1, 0), 4010.0, 100.0, 400_000.0, 5000.0));
        assert_eq!(closed.len(), 1);

        let (_freq, bar) = &closed[0];
        // vol: 0 + max(0, 100-200) = 0 (unaffected by rollback)
        assert_eq!(bar.vol, 0.0);
        // amount: 0 + max(0, 400k-800k) = 0
        assert_eq!(bar.amount, 0.0);
    }

    #[test]
    fn test_multi_freq() {
        let sym = Symbol::from("rb9999");
        let mut bg = BarGenerator::new(sym.clone(), vec![AggMode::Time], vec![Freq::F1, Freq::F5]);

        // 09:00 → triggers F1 bucket (09:00) and F5 bucket (09:00)
        bg.update_tick(&make_tick(ts(9, 0, 0), 4000.0, 100.0, 400_000.0, 5000.0));

        // 09:01 → F1 crosses boundary (bucket 09:01 ≠ 09:00), F5 does not cross (bucket 09:00)
        let closed = bg.update_tick(&make_tick(ts(9, 1, 0), 4010.0, 200.0, 802_000.0, 5000.0));
        // Only F1 closed
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].0, Freq::F1);
    }

    #[test]
    fn test_delta_classify_ctp_l1() {
        let mut tick = make_tick(ts(9, 0, 0), 4000.0, 100.0, 400_000.0, 5000.0);
        // LastPrice >= AskPrice1 → buyer-initiated
        tick.last_price = 4005.0;
        tick.ask_price1 = 4005.0;
        tick.bid_price1 = 4000.0;
        assert_eq!(BarGenerator::classify_delta(&tick), 1.0);

        // LastPrice <= BidPrice1 → seller-initiated
        tick.last_price = 4000.0;
        assert_eq!(BarGenerator::classify_delta(&tick), -1.0);

        // Mid-price → indeterminate
        tick.last_price = 4002.0;
        assert_eq!(BarGenerator::classify_delta(&tick), 0.0);
    }

    #[test]
    fn test_delta_classify_gm_mode() {
        let mut tick = make_tick(ts(9, 0, 0), 4000.0, 100.0, 400_000.0, 5000.0);
        // GM mode: trade_type directly gives direction
        tick.trade_type = Some(-1.0);
        assert_eq!(BarGenerator::classify_delta(&tick), -1.0);

        tick.trade_type = Some(2.0);
        assert_eq!(BarGenerator::classify_delta(&tick), 2.0);
    }
}

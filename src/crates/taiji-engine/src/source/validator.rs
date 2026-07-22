use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::types::tick::TickData;

#[derive(Debug, Clone, PartialEq)]
pub enum TickStatus {
    Ok,
    Gap { missing: u64 },
    Rejected(&'static str),
    Late,
    Stale,
    Disconnected,
}

pub struct TickValidator {
    last_seq: HashMap<String, u64>,
    last_tick_time: HashMap<String, Instant>,
    stale_count: HashMap<String, u32>,
    max_interval: Duration,
    #[allow(dead_code)]
    late_window: Duration,
    future_tolerance: Duration,
    max_stale_count: u32,
}

impl Default for TickValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl TickValidator {
    pub fn new() -> Self {
        Self {
            last_seq: HashMap::new(),
            last_tick_time: HashMap::new(),
            stale_count: HashMap::new(),
            max_interval: Duration::from_secs(5),
            late_window: Duration::from_secs(5),
            future_tolerance: Duration::from_secs(3600), // 1h, reference from WT
            max_stale_count: 3,
        }
    }

    /// 验证 tick。返回状态。
    pub fn validate(&mut self, instrument: &str, tick: &TickData, seq: u64) -> TickStatus {
        let now = Instant::now();
        let key = instrument.to_string();

        // Sequence gap detection
        if let Some(&last) = self.last_seq.get(&key) {
            if seq > last + 1 {
                let gap = seq - last - 1;
                self.last_seq.insert(key.clone(), seq);
                self.last_tick_time.insert(key.clone(), now);
                self.stale_count.insert(key.clone(), 0);
                return TickStatus::Gap { missing: gap };
            }
        }

        self.last_seq.insert(key.clone(), seq);

        // Time validation
        let tick_time_ms = tick.timestamp_ms;
        let now_ms = chrono::Utc::now().timestamp_millis();

        if tick_time_ms > now_ms + (self.future_tolerance.as_millis() as i64) {
            return TickStatus::Rejected("future timestamp");
        }

        // Stale detection
        if let Some(&last_time) = self.last_tick_time.get(&key) {
            if last_time.elapsed() > self.max_interval {
                let count = self.stale_count.entry(key.clone()).or_insert(0);
                *count += 1;
                if *count >= self.max_stale_count {
                    return TickStatus::Disconnected;
                }
                return TickStatus::Stale;
            }
        }

        self.last_tick_time.insert(key.clone(), now);
        self.stale_count.insert(key.clone(), 0);

        TickStatus::Ok
    }

    /// 重置某品种的状态（切源后调用）
    pub fn reset(&mut self, instrument: &str) {
        self.last_seq.remove(instrument);
        self.last_tick_time.remove(instrument);
        self.stale_count.remove(instrument);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tick(ts_ms: i64) -> TickData {
        TickData {
            instrument: "test".into(),
            trading_day: String::new(),
            exchange_id: String::new(),
            exchange_inst_id: String::new(),
            last_price: 0.0,
            pre_settlement_price: 0.0,
            pre_close_price: 0.0,
            pre_open_interest: 0.0,
            open_price: 0.0,
            highest_price: 0.0,
            lowest_price: 0.0,
            volume: 0.0,
            turnover: 0.0,
            open_interest: 0.0,
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

    #[test]
    fn test_ok() {
        let mut v = TickValidator::new();
        let tick = make_tick(chrono::Utc::now().timestamp_millis());
        assert_eq!(v.validate("test", &tick, 1), TickStatus::Ok);
    }

    #[test]
    fn test_gap() {
        let mut v = TickValidator::new();
        let tick = make_tick(chrono::Utc::now().timestamp_millis());
        v.validate("test", &tick, 1);
        assert_eq!(v.validate("test", &tick, 5), TickStatus::Gap { missing: 3 });
    }

    #[test]
    fn test_future_rejected() {
        let mut v = TickValidator::new();
        let future = chrono::Utc::now().timestamp_millis() + 7200_000; // +2h
        let tick = make_tick(future);
        assert_eq!(
            v.validate("test", &tick, 1),
            TickStatus::Rejected("future timestamp")
        );
    }
}

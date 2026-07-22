use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Trade direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Long,
    Short,
}

/// A single completed (or open) trade record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    /// Unique trade identifier, e.g. "TR-000001"
    pub trade_id: String,
    /// Instrument code, e.g. "ag2506"
    pub instrument: String,
    /// Entry timestamp
    pub entry_time: DateTime<Utc>,
    /// Exit timestamp (None if position still open at end of backtest)
    pub exit_time: Option<DateTime<Utc>>,
    /// Trade direction
    pub direction: Direction,
    /// Entry fill price (after slippage)
    pub entry_price: f64,
    /// Exit fill price (None if position still open)
    pub exit_price: Option<f64>,
    /// Trade volume (lots)
    pub volume: u32,
    /// Profit & loss in account currency (None if position still open)
    pub pnl: Option<f64>,
    /// Reason for exit: "tp", "sl", "signal_reverse", "eos" (end-of-stream), or empty string if open
    pub exit_reason: String,
    /// Signal confidence from the strategy node (0.0–1.0)
    pub signal_confidence: Option<f64>,
}

impl TradeRecord {
    /// Create a new trade record with an auto-incrementing trade_id.
    pub fn open(
        seq: usize,
        instrument: &str,
        entry_time: DateTime<Utc>,
        direction: Direction,
        entry_price: f64,
        volume: u32,
        confidence: Option<f64>,
    ) -> Self {
        Self {
            trade_id: format!("TR-{:06}", seq),
            instrument: instrument.to_string(),
            entry_time,
            exit_time: None,
            direction,
            entry_price,
            exit_price: None,
            volume,
            pnl: None,
            exit_reason: String::new(),
            signal_confidence: confidence,
        }
    }

    /// Close this trade at the given exit price with a reason.
    pub fn close(
        &mut self,
        exit_time: DateTime<Utc>,
        exit_price: f64,
        reason: &str,
        multiplier: f64,
    ) {
        self.exit_time = Some(exit_time);
        self.exit_price = Some(exit_price);
        self.exit_reason = reason.to_string();
        let price_diff = match self.direction {
            Direction::Long => exit_price - self.entry_price,
            Direction::Short => self.entry_price - exit_price,
        };
        self.pnl = Some(price_diff * self.volume as f64 * multiplier);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_open_trade() {
        let t = TradeRecord::open(
            1,
            "rb9999",
            Utc.with_ymd_and_hms(2026, 7, 21, 9, 0, 0).unwrap(),
            Direction::Long,
            4000.0,
            2,
            Some(0.85),
        );
        assert_eq!(t.trade_id, "TR-000001");
        assert_eq!(t.instrument, "rb9999");
        assert_eq!(t.direction, Direction::Long);
        assert_eq!(t.entry_price, 4000.0);
        assert_eq!(t.volume, 2);
        assert_eq!(t.pnl, None);
        assert!(t.exit_time.is_none());
    }

    #[test]
    fn test_close_long_trade_profit() {
        let mut t = TradeRecord::open(
            2,
            "ag2506",
            Utc.with_ymd_and_hms(2026, 7, 21, 9, 0, 0).unwrap(),
            Direction::Long,
            5000.0,
            1,
            None,
        );
        t.close(
            Utc.with_ymd_and_hms(2026, 7, 21, 14, 30, 0).unwrap(),
            5100.0,
            "tp",
            10.0, // multiplier=10 for ag
        );
        assert!(t.pnl.is_some());
        assert!((t.pnl.unwrap() - 1000.0).abs() < 1e-9); // (5100-5000)*1*10
        assert_eq!(t.exit_reason, "tp");
    }

    #[test]
    fn test_close_short_trade_profit() {
        let mut t = TradeRecord::open(
            3,
            "rb9999",
            Utc.with_ymd_and_hms(2026, 7, 21, 9, 0, 0).unwrap(),
            Direction::Short,
            4000.0,
            2,
            None,
        );
        t.close(
            Utc.with_ymd_and_hms(2026, 7, 21, 14, 30, 0).unwrap(),
            3900.0,
            "tp",
            10.0,
        );
        assert!(t.pnl.is_some());
        assert!((t.pnl.unwrap() - 2000.0).abs() < 1e-9); // (4000-3900)*2*10
    }

    #[test]
    fn test_close_short_trade_loss() {
        let mut t = TradeRecord::open(
            4,
            "rb9999",
            Utc.with_ymd_and_hms(2026, 7, 21, 9, 0, 0).unwrap(),
            Direction::Short,
            4000.0,
            1,
            None,
        );
        t.close(
            Utc.with_ymd_and_hms(2026, 7, 21, 14, 30, 0).unwrap(),
            4100.0,
            "sl",
            10.0,
        );
        assert!(t.pnl.is_some());
        assert!((t.pnl.unwrap() + 1000.0).abs() < 1e-9); // (4000-4100)*1*10 = -1000
        assert_eq!(t.exit_reason, "sl");
    }
}

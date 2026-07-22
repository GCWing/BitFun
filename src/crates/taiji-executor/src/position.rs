use dashmap::DashMap;

use crate::types::{Direction, Fill, Offset, Position};

/// Tracks current positions across all instruments.
///
/// Positions are updated on fill events and can be queried per instrument.
pub struct PositionTracker {
    positions: DashMap<String, Vec<Position>>,
}

impl Default for PositionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl PositionTracker {
    pub fn new() -> Self {
        Self {
            positions: DashMap::new(),
        }
    }

    /// Update tracked positions based on a fill event.
    ///
    /// - `(Buy, Open)`  → increases long position
    /// - `(Sell, Open)` → increases short position
    /// - `(Sell, Close | CloseToday)` → reduces long position
    /// - `(Buy, Close | CloseToday)`  → reduces short position
    pub fn update_on_fill(
        &self,
        fill: &Fill,
        instrument: &str,
        direction: Direction,
        offset: Offset,
        volume: u32,
    ) {
        let mut entry = self.positions.entry(instrument.to_string()).or_default();

        match (direction, offset) {
            (Direction::Buy, Offset::Open) => {
                if let Some(pos) = entry.iter_mut().find(|p| p.direction == Direction::Buy) {
                    let total_cost = pos.avg_price * pos.volume as f64 + fill.price * volume as f64;
                    pos.volume += volume;
                    pos.avg_price = total_cost / pos.volume as f64;
                } else {
                    entry.push(Position {
                        instrument: instrument.to_string(),
                        direction: Direction::Buy,
                        volume,
                        avg_price: fill.price,
                        float_pnl: 0.0,
                    });
                }
            }
            (Direction::Sell, Offset::Open) => {
                if let Some(pos) = entry.iter_mut().find(|p| p.direction == Direction::Sell) {
                    let total_cost = pos.avg_price * pos.volume as f64 + fill.price * volume as f64;
                    pos.volume += volume;
                    pos.avg_price = total_cost / pos.volume as f64;
                } else {
                    entry.push(Position {
                        instrument: instrument.to_string(),
                        direction: Direction::Sell,
                        volume,
                        avg_price: fill.price,
                        float_pnl: 0.0,
                    });
                }
            }
            (Direction::Sell, Offset::Close) | (Direction::Sell, Offset::CloseToday) => {
                if let Some(pos) = entry.iter_mut().find(|p| p.direction == Direction::Buy) {
                    pos.volume = pos.volume.saturating_sub(volume);
                }
            }
            (Direction::Buy, Offset::Close) | (Direction::Buy, Offset::CloseToday) => {
                if let Some(pos) = entry.iter_mut().find(|p| p.direction == Direction::Sell) {
                    pos.volume = pos.volume.saturating_sub(volume);
                }
            }
        }

        // Remove zero-volume positions.
        entry.retain(|p| p.volume > 0);
    }

    /// Return a snapshot of all positions for the given instrument.
    pub fn get_position(&self, instrument: &str) -> Vec<Position> {
        self.positions
            .get(instrument)
            .map(|r| r.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fill(price: f64, volume: u32) -> Fill {
        Fill {
            order_id: "ord-test".into(),
            price,
            volume,
            time: "2026-07-24T09:30:01".into(),
        }
    }

    #[test]
    fn buy_open_creates_long_position() {
        let tracker = PositionTracker::new();
        tracker.update_on_fill(
            &make_fill(5625.0, 2),
            "ag2506",
            Direction::Buy,
            Offset::Open,
            2,
        );

        let positions = tracker.get_position("ag2506");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].direction, Direction::Buy);
        assert_eq!(positions[0].volume, 2);
        assert_eq!(positions[0].avg_price, 5625.0);
    }

    #[test]
    fn sell_close_reduces_long_position() {
        let tracker = PositionTracker::new();
        tracker.update_on_fill(
            &make_fill(5625.0, 3),
            "ag2506",
            Direction::Buy,
            Offset::Open,
            3,
        );
        tracker.update_on_fill(
            &make_fill(5700.0, 2),
            "ag2506",
            Direction::Sell,
            Offset::Close,
            2,
        );

        let positions = tracker.get_position("ag2506");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].direction, Direction::Buy);
        assert_eq!(positions[0].volume, 1);
    }

    #[test]
    fn multi_instrument_positions() {
        let tracker = PositionTracker::new();
        tracker.update_on_fill(
            &make_fill(5625.0, 2),
            "ag2506",
            Direction::Buy,
            Offset::Open,
            2,
        );
        tracker.update_on_fill(
            &make_fill(7000.0, 3),
            "rb2510",
            Direction::Sell,
            Offset::Open,
            3,
        );

        let ag = tracker.get_position("ag2506");
        assert_eq!(ag.len(), 1);
        assert_eq!(ag[0].instrument, "ag2506");
        assert_eq!(ag[0].volume, 2);

        let rb = tracker.get_position("rb2510");
        assert_eq!(rb.len(), 1);
        assert_eq!(rb[0].instrument, "rb2510");
        assert_eq!(rb[0].direction, Direction::Sell);
        assert_eq!(rb[0].volume, 3);
    }

    #[test]
    fn zero_volume_position_removed() {
        let tracker = PositionTracker::new();
        tracker.update_on_fill(
            &make_fill(5625.0, 2),
            "ag2506",
            Direction::Buy,
            Offset::Open,
            2,
        );
        tracker.update_on_fill(
            &make_fill(5700.0, 2),
            "ag2506",
            Direction::Sell,
            Offset::Close,
            2,
        );

        let positions = tracker.get_position("ag2506");
        assert!(positions.is_empty());
    }

    #[test]
    fn sell_open_creates_short_position() {
        let tracker = PositionTracker::new();
        tracker.update_on_fill(
            &make_fill(7000.0, 5),
            "rb2510",
            Direction::Sell,
            Offset::Open,
            5,
        );

        let positions = tracker.get_position("rb2510");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].direction, Direction::Sell);
        assert_eq!(positions[0].volume, 5);
    }

    #[test]
    fn average_price_updates_on_multiple_buys() {
        let tracker = PositionTracker::new();
        tracker.update_on_fill(
            &make_fill(5000.0, 1),
            "ag2506",
            Direction::Buy,
            Offset::Open,
            1,
        );
        tracker.update_on_fill(
            &make_fill(5100.0, 1),
            "ag2506",
            Direction::Buy,
            Offset::Open,
            1,
        );

        let positions = tracker.get_position("ag2506");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].volume, 2);
        assert!((positions[0].avg_price - 5050.0).abs() < 0.01);
    }
}

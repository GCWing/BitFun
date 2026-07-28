use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol(pub Arc<str>);

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        Symbol(Arc::from(s))
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &*self.0)
    }
}

impl Serialize for Symbol {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Symbol {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s: &str = Deserialize::deserialize(deserializer)?;
        Ok(Symbol(Arc::from(s)))
    }
}

/// Bar frequency (adapted from czsc Freq, Apache 2.0)
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Freq {
    Tick,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F10,
    F12,
    F15,
    F20,
    F30,
    F60,
    F120,
    F240,
    F360,
    D,
    W,
    M,
    S,
    Y,
}

impl Freq {
    /// Return the number of minutes for this frequency (used for boundary detection)
    pub fn minutes(&self) -> Option<i64> {
        match self {
            Freq::Tick => None,
            Freq::F1 => Some(1),
            Freq::F2 => Some(2),
            Freq::F3 => Some(3),
            Freq::F4 => Some(4),
            Freq::F5 => Some(5),
            Freq::F6 => Some(6),
            Freq::F10 => Some(10),
            Freq::F12 => Some(12),
            Freq::F15 => Some(15),
            Freq::F20 => Some(20),
            Freq::F30 => Some(30),
            Freq::F60 => Some(60),
            Freq::F120 => Some(120),
            Freq::F240 => Some(240),
            Freq::F360 => Some(360),
            Freq::D => Some(1440),
            Freq::W => Some(10080),
            Freq::M => Some(43200),
            Freq::S => None,
            Freq::Y => None,
        }
    }

    /// Return the StateStore key suffix (human-readable, consistent with YAML config).
    /// F1→"1m", F5→"5m", F60→"1h", D→"1d", W→"1w", M→"1M".
    pub fn freq_key(&self) -> &'static str {
        match self {
            Freq::Tick => "tick",
            Freq::F1 => "1m",
            Freq::F2 => "2m",
            Freq::F3 => "3m",
            Freq::F4 => "4m",
            Freq::F5 => "5m",
            Freq::F6 => "6m",
            Freq::F10 => "10m",
            Freq::F12 => "12m",
            Freq::F15 => "15m",
            Freq::F20 => "20m",
            Freq::F30 => "30m",
            Freq::F60 => "1h",
            Freq::F120 => "2h",
            Freq::F240 => "4h",
            Freq::F360 => "6h",
            Freq::D => "1d",
            Freq::W => "1w",
            Freq::M => "1M",
            Freq::S => "1S",
            Freq::Y => "1Y",
        }
    }

    /// Parse Freq from a StateStore key suffix.
    /// "1m"→F1, "5m"→F5, "1h"→F60, "1d"→D.
    pub fn from_key(s: &str) -> Option<Self> {
        match s {
            "tick" => Some(Freq::Tick),
            "1m" => Some(Freq::F1),
            "2m" => Some(Freq::F2),
            "3m" => Some(Freq::F3),
            "4m" => Some(Freq::F4),
            "5m" => Some(Freq::F5),
            "6m" => Some(Freq::F6),
            "10m" => Some(Freq::F10),
            "12m" => Some(Freq::F12),
            "15m" => Some(Freq::F15),
            "20m" => Some(Freq::F20),
            "30m" => Some(Freq::F30),
            "1h" => Some(Freq::F60),
            "2h" => Some(Freq::F120),
            "4h" => Some(Freq::F240),
            "6h" => Some(Freq::F360),
            "1d" => Some(Freq::D),
            "1w" => Some(Freq::W),
            "1M" => Some(Freq::M),
            "1S" => Some(Freq::S),
            "1Y" => Some(Freq::Y),
            _ => None,
        }
    }
}

/// Bar data. Extended from czsc RawBar with futures-specific open interest and trade delta.
/// Missing fields are not filled with defaults — absent means None.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RawBar {
    pub symbol: Symbol,
    pub dt: DateTime<Utc>,
    pub freq: Freq,
    pub id: i32,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub vol: f64,                   // Volume (single-sided)
    pub amount: f64,                // Turnover
    pub open_interest: Option<f64>, // Open interest snapshot (single-sided)
    pub delta: Option<f64>,         // Trade delta (order flow Delta)
}

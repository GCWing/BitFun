//! State types — R2.2
pub type StateKey = String;

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::bar::RawBar;
use super::signal::Signal;
use super::tick::TickData;
use chrono::{DateTime, Utc};

// === Strategy output types (placeholder structs, strategy crate fills in concrete fields) ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pivot {
    pub idx: usize,
    pub price: f64,
    pub ptype: PivotType,
    pub dt: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PivotType {
    High,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trendline {
    pub slope: f64,
    pub intercept: f64,
    pub state: TrendlineState,
    pub valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendlineState {
    Normal,
    Corrected,
    Accelerated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Swing {
    pub start: usize,
    pub end: usize,
    pub direction: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Magnet {
    pub upper: f64,
    pub lower: f64,
    pub midline: f64,
    pub is_real: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriplePush {
    pub push_points: Vec<usize>,
    pub overshoot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolChannel {
    pub upper: f64,
    pub lower: f64,
    pub midline: f64,
    pub width: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DvmiBuffer {
    // DVMI internal state, to be defined during Phase 2 implementation
}

/// Six core metrics (oi_delta, active_trade_diff, total_volume, long_open, short_open, long_close, short_close, net_long, net_short),
/// derived from OI position change + Delta trade direction + volume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SixCoreMetrics {
    pub oi_delta: f64,
    pub active_trade_diff: f64,
    pub total_volume: f64,
    pub long_open: f64,
    pub short_open: f64,
    pub long_close: f64,
    pub short_close: f64,
    pub net_long: f64,
    pub net_short: f64,
}

// === StateValue ===

#[derive(Debug, Clone, Serialize)]
pub enum StateValue {
    Tick(Arc<TickData>),
    Bars(Arc<Vec<Arc<RawBar>>>),
    Swings(Arc<Vec<Swing>>),
    Signals(Arc<Vec<Signal>>),
    F64(f64),
    Usize(usize),
    Bool(bool),
    /// Generic f64 vector (for numerical arrays not tied to a specific domain type).
    Generic(Vec<f64>),
    /// Self-describing JSON value (for any Serialize/Deserialize type).
    Json(serde_json::Value),
    /// Opaque binary blob with a type tag (for types without a stable Serde schema).
    Custom(String, Vec<u8>),
}

// === FromStateValue ===

pub trait FromStateValue: Sized {
    fn from_value(v: &StateValue) -> Option<Self>;
}

impl FromStateValue for bool {
    fn from_value(v: &StateValue) -> Option<Self> {
        match v {
            StateValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

impl FromStateValue for f64 {
    fn from_value(v: &StateValue) -> Option<Self> {
        match v {
            StateValue::F64(x) => Some(*x),
            _ => None,
        }
    }
}

impl FromStateValue for usize {
    fn from_value(v: &StateValue) -> Option<Self> {
        match v {
            StateValue::Usize(x) => Some(*x),
            _ => None,
        }
    }
}

impl FromStateValue for Arc<Vec<Arc<RawBar>>> {
    fn from_value(v: &StateValue) -> Option<Self> {
        match v {
            StateValue::Bars(b) => Some(Arc::clone(b)),
            _ => None,
        }
    }
}

impl FromStateValue for Arc<Vec<Swing>> {
    fn from_value(v: &StateValue) -> Option<Self> {
        match v {
            StateValue::Swings(s) => Some(s.clone()),
            _ => None,
        }
    }
}

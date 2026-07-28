use super::bar::Freq;
use crate::types::NodeId;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SignalAction {
    Long,
    Short,
    CloseLong,
    CloseShort,
    Hold,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Signal {
    pub timestamp: DateTime<Utc>,
    pub instrument: String,
    pub freq: Freq,
    pub action: SignalAction,
    pub entry: Option<f64>,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub size: Option<f64>,
    pub source: NodeId,
    pub confidence: f64,
    pub metadata: HashMap<String, String>,
    /// Compliance disclaimer injected via [`crate::compliance::append_disclaimer`].
    pub disclaimer: Option<String>,
}

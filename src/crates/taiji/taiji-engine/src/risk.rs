//! RiskMonitor trait — pluggable risk control interface.
//! Reference: WonderTrader RiskMonDefs.h

use crate::error::Result;
use crate::store::StateStore;
use std::collections::HashMap;

/// Pluggable risk monitor. Each monitor checks one risk dimension.
pub trait RiskMonitor: Send + Sync {
    fn init(&mut self, config: &RiskConfig) -> Result<()>;
    fn check_order(&self, order: &RiskOrderRequest, state: &StateStore) -> Result<OrderDecision>;
    fn check_position(&self, position: &RiskPosition, state: &StateStore) -> Result<RiskAction>;
    fn on_fill(&mut self, fill: &RiskFill, state: &StateStore);
    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<RiskAlert>>;
    fn enabled(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct RiskConfig {
    pub params: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct RiskOrderRequest {
    pub instrument: String,
    pub action: String,
    pub price: f64,
    pub volume: f64,
}

#[derive(Debug, Clone)]
pub enum OrderDecision {
    Allow,
    Reject(String),
    Reduce(f64),
}

#[derive(Debug, Clone)]
pub struct RiskPosition {
    pub instrument: String,
    pub volume: f64,
    pub avg_price: f64,
}

#[derive(Debug, Clone)]
pub enum RiskAction {
    None,
    Warn(String),
    ForceClose,
}

#[derive(Debug, Clone)]
pub struct RiskFill {
    pub instrument: String,
    pub price: f64,
    pub volume: f64,
    pub time: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct RiskAlert {
    pub level: String,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

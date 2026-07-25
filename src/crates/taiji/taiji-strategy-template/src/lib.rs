//! Taiji 策略模板 — 复制此 crate 创建你的闭源策略。
//!
//! 示例策略：DualThrust 通道突破
//! 参考：WonderTrader/wtpy demos/Strategies/DualThrust.py（MIT）
//! 参考：Michael Chalek, "Dual Thrust Trading System" (1980s)
//!
//! # 快速开始
//!
//! 1. 复制整个 `taiji-strategy-template/` 目录
//! 2. 修改 `Cargo.toml` 中的 `name` 和 `description`
//! 3. 实现你的 `evaluate()` 方法——替换 DualThrust 逻辑
//! 4. 在 `taiji-cli` 或自己的 binary 中注册：
//!
//! ```ignore
//! use taiji_engine::register_node;
//! register_node!(factory, "my_strategy", my_strategy::MyStrategy, "my_strategy_1");
//! ```
//!
//! # 闭源部署
//!
//! 将此 crate 放在 `src/crates/taiji/` 下，在 workspace `Cargo.toml` 中注册。
//! 如果不希望开源，放在独立私有仓库中，通过 `path` 依赖引用。

use chrono::{DateTime, NaiveTime, Utc};
use std::collections::HashMap;
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::signal::{Signal, SignalAction};
use taiji_engine::types::state::StateKey;

// ═══════════════════════════════════════════════════════════════════════════
// 策略参数（可序列化，支持 YAML/JSON 配置覆盖）
// ═══════════════════════════════════════════════════════════════════════════

/// DualThrust 策略参数
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct DualThrustParams {
    /// 计算 Range 的回看周期
    pub lookback: usize,
    /// 上轨系数
    pub k1: f64,
    /// 下轨系数
    pub k2: f64,
    /// 单笔最大仓位
    pub max_position: u32,
    /// 日盘开盘时间 (HH:MM)
    pub day_open: String,
    /// 夜盘开盘时间 (HH:MM)，None 表示不交易夜盘
    pub night_open: Option<String>,
}

impl Default for DualThrustParams {
    fn default() -> Self {
        Self {
            lookback: 20,
            k1: 0.7,
            k2: 0.7,
            max_position: 1,
            day_open: "09:00".into(),
            night_open: Some("21:00".into()),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 策略实现——替换为你自己的量价时空逻辑
// ═══════════════════════════════════════════════════════════════════════════

pub struct DualThrust {
    id: String,
    params: DualThrustParams,
    // 策略状态（私有）
    bars: Vec<RawBar>,
    upper_bound: f64,
    lower_bound: f64,
    position: i32,
    day_open_triggered: bool,
    last_range_calc: Option<DateTime<Utc>>,
}

impl DualThrust {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            params: DualThrustParams::default(),
            bars: Vec::new(),
            upper_bound: 0.0,
            lower_bound: 0.0,
            position: 0,
            day_open_triggered: false,
            last_range_calc: None,
        }
    }

    pub fn with_params(mut self, params: DualThrustParams) -> Self {
        self.params = params;
        self
    }

    /// 计算 Range = Max(HH - LC, HC - LL)
    fn calc_range(&self) -> f64 {
        if self.bars.len() < self.params.lookback {
            return 0.0;
        }
        let window = &self.bars[self.bars.len() - self.params.lookback..];
        let hh = window.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
        let ll = window.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
        let hc = window.last().unwrap().close;
        let lc = window.last().unwrap().close;
        (hh - lc).max(hc - ll)
    }

    /// 判断是否新交易时段开盘（触发重新计算上下轨）
    fn is_session_open(&self, bar: &RawBar) -> bool {
        let time = bar.dt.time();
        let day_open = NaiveTime::parse_from_str(&self.params.day_open, "%H:%M").ok();
        let night_open = self.params.night_open.as_ref()
            .and_then(|s| NaiveTime::parse_from_str(s, "%H:%M").ok());

        let at_day = day_open.map(|t| time >= t && time < t + chrono::Duration::minutes(5));
        let at_night = night_open.map(|t| time >= t && time < t + chrono::Duration::minutes(5));

        at_day.unwrap_or(false) || at_night.unwrap_or(false)
    }

    /// 核心策略逻辑——在这里替换为你的量价时空公式
    fn evaluate(&mut self, bar: &RawBar) -> Option<Signal> {
        // 新交易时段开盘：重新计算上下轨
        if self.is_session_open(bar) && !self.day_open_triggered {
            // 平掉旧仓位
            self.position = 0;
            let range = self.calc_range();
            if range > 0.0 {
                let open = bar.open;
                self.upper_bound = open + self.params.k1 * range;
                self.lower_bound = open - self.params.k2 * range;
            }
            self.day_open_triggered = true;
        }
        // 收盘后重置
        if bar.dt.time() >= NaiveTime::from_hms_opt(15, 15, 0).unwrap() {
            self.day_open_triggered = false;
        }

        // 突破信号
        if self.upper_bound > 0.0 && bar.close > self.upper_bound && self.position <= 0 {
            self.position = self.params.max_position as i32;
            return Some(Signal {
                timestamp: bar.dt,
                instrument: bar.symbol.to_string(),
                freq: bar.freq,
                action: SignalAction::Long,
                entry: Some(bar.close),
                stop_loss: None,
                take_profit: None,
                size: Some(self.params.max_position as f64),
                source: self.id.clone(),
                confidence: 1.0,
                metadata: HashMap::new(),
                disclaimer: Some(format!("上轨突破: {:.2} > {:.2}", bar.close, self.upper_bound)),
            });
        }
        if self.lower_bound > 0.0 && bar.close < self.lower_bound && self.position >= 0 {
            self.position = -(self.params.max_position as i32);
            return Some(Signal {
                timestamp: bar.dt,
                instrument: bar.symbol.to_string(),
                freq: bar.freq,
                action: SignalAction::Short,
                entry: Some(bar.close),
                stop_loss: None,
                take_profit: None,
                size: Some(self.params.max_position as f64),
                source: self.id.clone(),
                confidence: 1.0,
                metadata: HashMap::new(),
                disclaimer: Some(format!("下轨突破: {:.2} < {:.2}", bar.close, self.lower_bound)),
            });
        }

        None
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ComputeNode trait 实现（不需要修改）
// ═══════════════════════════════════════════════════════════════════════════

impl ComputeNode for DualThrust {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "DualThrust"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        vec![StateKey::new("bars", "1m")]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        vec![StateKey::new("signal", self.id.as_str())]
    }

    fn on_init(&mut self, config: &NodeConfig, _state: &StateStore) -> Result<()> {
        if let Some(v) = config.get_u64("lookback") { self.params.lookback = v as usize; }
        if let Some(v) = config.get_f64("k1") { self.params.k1 = v; }
        if let Some(v) = config.get_f64("k2") { self.params.k2 = v; }
        if let Some(v) = config.get_u64("max_position") { self.params.max_position = v as u32; }
        if let Some(v) = config.get_str("day_open") { self.params.day_open = v.to_string(); }
        if let Some(v) = config.get_str("night_open") { self.params.night_open = Some(v.to_string()); }
        Ok(())
    }

    fn on_bar(&mut self, bar: &RawBar, _period: Freq, state: &StateStore) -> Result<()> {
        self.bars.push(bar.clone());
        if let Some(signal) = self.evaluate(bar) {
            state.insert(StateKey::new("signal", self.id.as_str()), signal);
        }
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.bars.len() >= self.params.lookback
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bar(open: f64, high: f64, low: f64, close: f64, time: &str) -> RawBar {
        RawBar {
            symbol: "rb0".into(),
            freq: Freq::F1,
            id: 0,
            open, high, low, close,
            vol: 1000.0,
            amount: 0.0,
            open_interest: Some(5000.0),
            delta: None,
            dt: DateTime::parse_from_rfc3339(&format!("2026-07-22T{}:00+08:00", time)).unwrap().into(),
        }
    }

    #[test]
    fn test_range_calculation() {
        let mut s = DualThrust::new("test");
        for i in 0..25 {
            s.bars.push(make_bar(i as f64, i as f64 + 2.0, i as f64 - 2.0, i as f64 + 1.0, "09:00"));
        }
        assert!(s.calc_range() > 0.0);
    }

    #[test]
    fn test_insufficient_bars() {
        let mut s = DualThrust::new("test");
        for i in 0..5 {
            s.bars.push(make_bar(i as f64, i as f64, i as f64, i as f64, "09:00"));
        }
        assert!(!s.is_ready());
    }

    #[test]
    fn test_no_signal_before_ready() {
        let mut s = DualThrust::new("test");
        let bar = make_bar(4000.0, 4010.0, 3990.0, 4005.0, "09:00");
        assert!(s.evaluate(&bar).is_none());
    }
}

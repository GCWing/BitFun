//! Tick types — 标准化 47 字段，与 CTP FTD-XML 协议对应。
//! 字段布局参考 openctp（BSD License, https://github.com/openctp/openctp）。
//! 版权声明：Copyright (c) openctp contributors. All rights reserved.

use std::collections::HashMap;

pub type SourceId = String;

/// 数据源原始 tick
#[derive(Debug, Clone)]
pub struct RawTick {
    pub instrument: String,           // 品种代码
    pub source_id: SourceId,          // 数据源标识 "ctp:0"
    pub fields: HashMap<String, f64>, // 原始字段名 → 值
    pub timestamp: i64,               // UTC 毫秒时间戳
    pub sequence: Option<u64>,        // 序列号
}

/// 标准化 47 字段，与 CTP FTD-XML 对应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TickData {
    pub instrument: String,
    pub trading_day: String,
    pub exchange_id: String,
    pub exchange_inst_id: String,
    pub last_price: f64,
    pub pre_settlement_price: f64,
    pub pre_close_price: f64,
    pub pre_open_interest: f64,
    pub open_price: f64,
    pub highest_price: f64,
    pub lowest_price: f64,
    pub volume: f64, // 单边
    pub turnover: f64,
    pub open_interest: f64, // 单边
    pub close_price: f64,
    pub settlement_price: f64,
    pub upper_limit_price: f64,
    pub lower_limit_price: f64,
    pub pre_delta: f64,
    pub curr_delta: f64,
    pub update_time: String,
    pub update_millisec: i32,
    pub bid_price1: f64,
    pub bid_volume1: i32,
    pub ask_price1: f64,
    pub ask_volume1: i32,
    pub bid_price2: f64,
    pub bid_volume2: i32,
    pub ask_price2: f64,
    pub ask_volume2: i32,
    pub bid_price3: f64,
    pub bid_volume3: i32,
    pub ask_price3: f64,
    pub ask_volume3: i32,
    pub bid_price4: f64,
    pub bid_volume4: i32,
    pub ask_price4: f64,
    pub ask_volume4: i32,
    pub bid_price5: f64,
    pub bid_volume5: i32,
    pub ask_price5: f64,
    pub ask_volume5: i32,
    pub average_price: f64,
    pub action_day: String,
    // 掘金扩展字段（CTP 无此数据，Option）
    pub trade_type: Option<f64>,
    pub cum_volume: Option<f64>,
    pub cum_position: Option<f64>,
    // 统一时间戳
    pub timestamp_ms: i64, // UTC 毫秒
}

impl Default for TickData {
    fn default() -> Self {
        Self {
            instrument: String::new(),
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
            timestamp_ms: 0,
        }
    }
}

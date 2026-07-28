//! CtpDataSource — CTP 数据源，实现 DataSource trait。
//!
//! 数据流：
//!   CTP C++ DLL → ctp-sys FFI → CThostFtdcMdSpi callback
//!   → crossbeam::channel::bounded(4096)
//!   → tokio task → SchemaAdapter → TickData
//!   → Pipeline::feed_tick_direct()

use std::collections::HashMap;
use std::path::PathBuf;

use crossbeam::channel::Sender;
use parking_lot::Mutex;
use taiji_engine::error::{Result, TaijiError};
use taiji_engine::source::datasource::{DataSource, DataSourceConfig, FieldDef, SourceHealth};
use taiji_engine::types::tick::{RawTick, TickData};

/// CTP 数据源。
///
/// `receiver` 用 Mutex 包裹以满足 `DataSource: Sync` 约束
/// （crossbeam Receiver 是 `Send` 但非 `Sync`）。
pub struct CtpDataSource {
    flow_path: PathBuf,
    instruments: Vec<String>,
    sender: Sender<TickData>,
    receiver: Mutex<crossbeam::channel::Receiver<TickData>>,
    is_connected: bool,
}

impl CtpDataSource {
    /// 创建 CtpDataSource。
    ///
    /// 内部创建 capacity=4096 的 crossbeam bounded channel。
    pub fn new(flow_path: PathBuf, instruments: Vec<String>) -> Self {
        let (tx, rx) = crossbeam::channel::bounded(4096);
        Self {
            flow_path,
            instruments,
            sender: tx,
            receiver: Mutex::new(rx),
            is_connected: false,
        }
    }

    /// 获取发送端 clone（供 FFI callback / tokio task 使用）。
    pub fn sender(&self) -> Sender<TickData> {
        self.sender.clone()
    }

    /// 将 TickData 转换为 RawTick。
    fn tick_to_raw(&self, tick: TickData) -> RawTick {
        let mut fields: HashMap<String, f64> = HashMap::new();
        fields.insert("last_price".into(), tick.last_price);
        fields.insert("open_price".into(), tick.open_price);
        fields.insert("highest_price".into(), tick.highest_price);
        fields.insert("lowest_price".into(), tick.lowest_price);
        fields.insert("close_price".into(), tick.close_price);
        fields.insert("volume".into(), tick.volume);
        fields.insert("turnover".into(), tick.turnover);
        fields.insert("open_interest".into(), tick.open_interest);
        fields.insert("pre_settlement_price".into(), tick.pre_settlement_price);
        fields.insert("upper_limit_price".into(), tick.upper_limit_price);
        fields.insert("lower_limit_price".into(), tick.lower_limit_price);

        RawTick {
            instrument: tick.instrument,
            source_id: "ctp:0".into(),
            fields,
            timestamp: tick.timestamp_ms,
            sequence: Some(0),
        }
    }
}

impl DataSource for CtpDataSource {
    fn name(&self) -> &'static str {
        "ctp"
    }

    fn schema(&self) -> Vec<FieldDef> {
        vec![
            FieldDef {
                name: "last_price".into(),
                required: true,
            },
            FieldDef {
                name: "volume".into(),
                required: true,
            },
            FieldDef {
                name: "open_interest".into(),
                required: false,
            },
        ]
    }

    fn connect(&mut self, _config: &DataSourceConfig) -> Result<()> {
        // CTP FFI (ctp-sys) is not integrated yet.
        // To enable live CTP data, set TAIJI_CTP_LIB_PATH to the native CTP library
        // directory and rebuild with the ctp-sys feature enabled.
        tracing::warn!(
            flow_path = %self.flow_path.display(),
            "CtpDataSource::connect — CTP native library not available"
        );
        Err(TaijiError::DataSource(
            "CTP data source requires native CTP library. \
             Set TAIJI_CTP_LIB_PATH or use replay source."
                .into(),
        ))
    }

    fn disconnect(&mut self) -> Result<()> {
        if !self.is_connected {
            return Err(TaijiError::DataSource(
                "CtpDataSource is not connected; call connect() first.".into(),
            ));
        }
        // TODO: disconnect CTP session, join tokio task
        self.is_connected = false;
        tracing::info!("CtpDataSource::disconnect — disconnected");
        Ok(())
    }

    fn subscribe(&mut self, instruments: &[&str]) -> Result<()> {
        self.instruments = instruments.iter().map(|s| s.to_string()).collect();
        tracing::info!(?self.instruments, "CtpDataSource::subscribe");
        Ok(())
    }

    fn next_raw(&mut self) -> Result<Option<RawTick>> {
        if !self.is_connected {
            return Err(TaijiError::DataSource(
                "CtpDataSource is not connected; call connect() first.".into(),
            ));
        }
        match self.receiver.get_mut().try_recv() {
            Ok(tick) => Ok(Some(self.tick_to_raw(tick))),
            Err(crossbeam::channel::TryRecvError::Empty) => Ok(None),
            Err(crossbeam::channel::TryRecvError::Disconnected) => {
                Err(TaijiError::DataSource("channel disconnected".into()))
            }
        }
    }

    fn health_check(&self) -> SourceHealth {
        // 检查 sender 是否仍有接收端存活
        if self.sender.is_empty() {
            SourceHealth::Degraded("no receiver connected".into())
        } else {
            SourceHealth::Healthy
        }
    }

    fn supports_resume(&self) -> bool {
        true
    }
}

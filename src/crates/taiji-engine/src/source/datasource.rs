use crate::error::Result;
use crate::types::tick::RawTick;
use std::collections::HashMap;

/// 数据源配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DataSourceConfig {
    pub type_name: String,
    pub params: HashMap<String, serde_json::Value>,
}

/// 字段定义
#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub required: bool,
}

/// 数据源健康状态
#[derive(Debug, Clone)]
pub enum SourceHealth {
    Healthy,
    Degraded(String),
    Down,
}

/// 数据源接口——18 个数据源 → 统一切换。
/// 每个品种独立路由表，断了自动切备源。
pub trait DataSource: Send + Sync {
    /// 数据源名称
    fn name(&self) -> &'static str;

    /// 此数据源提供的字段列表
    fn schema(&self) -> Vec<FieldDef>;

    /// 连接数据源
    fn connect(&mut self, config: &DataSourceConfig) -> Result<()>;

    /// 断开连接
    fn disconnect(&mut self) -> Result<()>;

    /// 订阅品种
    fn subscribe(&mut self, instruments: &[&str]) -> Result<()>;

    /// 获取下一个原始 tick（含品种标识）
    fn next_raw(&mut self) -> Result<Option<RawTick>>;

    /// 健康检查
    fn health_check(&self) -> SourceHealth;

    /// 是否支持断线续传
    fn supports_resume(&self) -> bool {
        false
    }

    /// 最后序列号（用于 RESUME 模式）
    fn last_sequence(&self, instrument: &str) -> Option<u64> {
        let _ = instrument;
        None
    }

    /// 从指定序列号恢复
    fn resume_from(&mut self, instrument: &str, seq: u64) -> Result<()> {
        let _ = (instrument, seq);
        Ok(())
    }
}

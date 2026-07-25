use crate::error::Result;
use crate::store::StateStore;
use crate::types::bar::{Freq, RawBar};
use crate::types::signal::Signal;
use crate::types::state::StateKey;
use crate::types::tick::TickData;
use std::collections::HashMap;

pub type NodeId = String;

/// 节点配置——YAML 中每个节点的 config 字段反序列化为此结构。
#[derive(Debug, Clone)]
pub struct NodeConfig {
    pub type_name: String,
    pub params: HashMap<String, serde_json::Value>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeConfig {
    pub fn new() -> Self {
        Self {
            type_name: String::new(),
            params: HashMap::new(),
        }
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.params.get(key).and_then(|v| v.as_str())
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.params.get(key).and_then(|v| v.as_f64())
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.params.get(key).and_then(|v| v.as_i64())
    }
}

/// 计算节点——可插拔，可组合。
/// Pipeline 根据 input_keys/output_keys 自动推导执行顺序（拓扑排序）。
///
/// 参考：WonderTrader ICtaStraCtx + CtaStrategy（MIT）。
/// 采用无上下文设计——节点不持有 ctx，通过 StateStore 读写共享数据。
pub trait ComputeNode: Send + Sync {
    fn id(&self) -> NodeId;
    fn name(&self) -> &'static str;

    fn input_keys(&self) -> Vec<StateKey>;
    fn output_keys(&self) -> Vec<StateKey>;

    fn on_init(&mut self, config: &NodeConfig, state: &StateStore) -> Result<()>;

    /// Tick 级计算（逐笔节点重写，如 DeltaAgent）
    fn on_tick(&mut self, tick: &TickData, state: &StateStore) -> Result<()> {
        let _ = (tick, state);
        Ok(())
    }

    /// K 线闭合时调用
    fn on_bar(&mut self, bar: &RawBar, period: Freq, state: &StateStore) -> Result<()>;

    /// 定时计算（在 bar 闭合后调用，生成信号）
    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<Signal>> {
        let _ = state;
        Ok(vec![])
    }

    /// 交易日开始（结算后、开盘前）
    fn on_session_begin(&mut self, date: u32, state: &StateStore) -> Result<()> {
        let _ = (date, state);
        Ok(())
    }

    /// 交易日结束（收盘后、结算前）
    fn on_session_end(&mut self, date: u32, state: &StateStore) -> Result<()> {
        let _ = (date, state);
        Ok(())
    }

    /// 预热完成判定
    fn is_ready(&self, state: &StateStore) -> bool {
        let _ = state;
        true
    }

    /// 订阅的周期
    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![]
    }
}

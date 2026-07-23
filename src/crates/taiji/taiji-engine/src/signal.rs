//! 领域信号注册表 —— 受 [`bitfun_tool_contracts::ToolRegistry`] 启发。
//!
//! 本模块提供一个交易领域专用的信号注册表 [`SignalRegistry`]，用于
//! 各策略 crate 注册其产出的信号描述符（[`SignalDescriptor`]），
//! 支持按类别（[`SignalCategory`]）和图节点（[`NodeId`]）查询。
//!
//! # 与 ToolRegistry 的关系
//!
//! - [`ToolRegistry<Tool>`] 是 BitFun 的执行框架通用工具注册表，泛型参数为
//!   `Tool: ToolRegistryItem + ?Sized`，通过 `Arc<Tool>` 管理工具引用。
//! - [`SignalRegistry`] 是交易领域的特化版本，仅管理 [`SignalDescriptor`] 的
//!   命名映射，不涉及工具执行、生命周期或 provider 分组。
//! - 两者共享"命名注册 → 查询"的核心语义：
//!   - `register(&mut self, desc)` ↔ `register_tool(&mut self, tool)`
//!   - `get(&self, name)` ↔ `get_tool(&self, name)`
//!   - `all(&self)` ↔ `get_all_tools(&self)`
//!
//! [`ToolRegistry<Tool>`]: bitfun_tool_contracts::ToolRegistry
//! [`ToolRegistryItem`]: bitfun_tool_contracts::ToolRegistryItem
//! [`NodeId`]: crate::node::NodeId

use crate::node::NodeId;
use std::collections::HashMap;

/// 信号分类
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SignalCategory {
    Pivot,
    Trend,
    Magnet,
    Risk,
    Custom(String),
}

/// 信号描述符——每个策略 crate 注册其产出的信号。
///
/// 对应 [`ToolRegistryItem`] 的领域简化：仅保留名称、来源节点、
/// 分类和描述，不涉及工具执行、输入 schema 或异步生命周期。
///
/// [`ToolRegistryItem`]: bitfun_tool_contracts::ToolRegistryItem
#[derive(Debug, Clone)]
pub struct SignalDescriptor {
    pub name: &'static str,
    pub node: NodeId,
    pub category: SignalCategory,
    pub description: &'static str,
}

/// 信号注册表——交易领域专用的命名信号注册中心。
///
/// 受 [`ToolRegistry<Tool>`] 启发，提供基于名称的信号注册与查询能力。
/// 与 ToolRegistry 不同，本注册表不涉及工具装饰器（decorator）、
/// 快照代数（snapshot generation）、provider 分组或异步生命周期。
///
/// # 使用示例
///
/// ```rust
/// # use taiji_engine::signal::{SignalRegistry, SignalDescriptor, SignalCategory};
/// # use taiji_engine::node::NodeId;
/// let mut registry = SignalRegistry::new();
/// registry.register(SignalDescriptor {
///     name: "magnet_pullback",
///     node: NodeId::from("magnet_v1"),
///     category: SignalCategory::Magnet,
///     description: "磁铁回拉信号",
/// });
///
/// assert!(registry.get("magnet_pullback").is_some());
/// assert_eq!(registry.list_by_category(&SignalCategory::Magnet).len(), 1);
/// ```
///
/// [`ToolRegistry<Tool>`]: bitfun_tool_contracts::ToolRegistry
pub struct SignalRegistry {
    descriptors: HashMap<String, SignalDescriptor>,
}

impl Default for SignalRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalRegistry {
    /// 创建空的信号注册表。
    pub fn new() -> Self {
        Self {
            descriptors: HashMap::new(),
        }
    }

    /// 注册一个信号描述符。
    ///
    /// 对应 [`ToolRegistry::register_tool`]。
    pub fn register(&mut self, desc: SignalDescriptor) {
        self.descriptors.insert(desc.name.to_string(), desc);
    }

    /// 按名称查询信号描述符。
    ///
    /// 对应 [`ToolRegistry::get_tool`]。
    pub fn get(&self, name: &str) -> Option<&SignalDescriptor> {
        self.descriptors.get(name)
    }

    /// 按信号类别列出所有匹配的描述符。
    pub fn list_by_category(&self, cat: &SignalCategory) -> Vec<&SignalDescriptor> {
        self.descriptors
            .values()
            .filter(|d| &d.category == cat)
            .collect()
    }

    /// 按图节点 ID 列出所有匹配的描述符。
    pub fn list_by_node(&self, node: &NodeId) -> Vec<&SignalDescriptor> {
        self.descriptors
            .values()
            .filter(|d| &d.node == node)
            .collect()
    }

    /// 返回所有已注册的信号描述符。
    ///
    /// 对应 [`ToolRegistry::get_all_tools`]。
    pub fn all(&self) -> Vec<&SignalDescriptor> {
        self.descriptors.values().collect()
    }
}

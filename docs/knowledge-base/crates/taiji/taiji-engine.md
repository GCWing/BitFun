# taiji-engine

**路径**: src/crates/taiji/taiji-engine
**描述**: Taiji trading engine — DAG-based compute pipeline

## 依赖
- 内部: taiji-llm
- 外部: serde, serde_json, serde_yaml, chrono, thiserror, parking_lot, tracing, dashmap, csv, anyhow, uuid, rand, async-trait, tokio, petgraph

## 模块结构
- `types` — 核心数据类型（bar/tick/signal/state）
- `pipeline` — 流水线引擎（BarGenerator + DAG + node 调度）
- `node` — ComputeNode trait 定义
- `dag` — petgraph 有向无环图拓扑排序
- `factory` — NodeFactory 反射式节点注册与创建
- `store` — StateStore: 并发安全的 key-value 状态存储
- `config` — PipelineConfig YAML 配置解析
- `signal` — Signal 信号定义与合成
- `risk` — RiskMonitor/OrderDecision 风险控制
- `state` — StateSnapshot 状态快照
- `source` — DataSource 数据源抽象（adapter/mgr/replay/validator）
- `debate` — 多 Agent 辩论编排器（Bull/Bear/Neutral）
- `fusion` — 信号融合
- `compliance` — 风险揭示合规检查
- `error` — TaijiError 错误类型
- `safe_json` — 安全深度限制的 JSON/YAML 解析

## 核心类型
- `ComputeNode` — 所有策略/信号/处理节点的 trait
- `Pipeline` — DAG 流水线主结构
- `StateStore` — 并发状态存储
- `BarGenerator` — Tick→KLine 聚合器
- `Signal` — 交易信号（含 action/confidence/metadata）
- `NodeConfig` / `PipelineConfig` — YAML 配置

## 核心函数
- `Pipeline::from_config()` — 从 YAML 构建流水线
- `Pipeline::feed_tick_direct()` — 注入 tick 驱动全流水线
- `Pipeline::derive_edges()` — 自动推导 DAG 边

## 属于领域
- pipeline / core

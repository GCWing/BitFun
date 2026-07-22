# taiji-engine 核心文件审计报告

审计日期：2026-07-22
审计范围：`src/crates/taiji/taiji-engine/src/` (8 个目标文件，7 个存在)
对比基准：BitFun `src/crates/execution/` `src/crates/contracts/` `src/crates/services/` 等价模块

---

## 总体摘要

| 文件 | 行数 | 重复度 | 风险 |
|------|------|--------|------|
| dag.rs | 164 | **无** — BitFun 无 DAG/拓扑排序等价实现 | 保留 |
| factory.rs | 135 | 低 — 与 ToolRegistry 注册模式表面相似 | 保留 |
| node.rs | 96 | 低 — 与 ToolRegistryItem trait 表面相似 | 保留 |
| store.rs | 174 | 中 — 与 PersistenceService/JsonFileStore 有功能重叠 | 评估迁移 |
| config.rs | 207 | **无** — BitFun 无流水线配置等价物 | 保留 |
| error.rs | 27 | 低 — 与 PortError 模式相似 | 可对齐 |
| feature_flags.rs | 213 | **无** — BitFun 无特性开关/A-B 测试等价物 | 保留 |
| log.rs | — | **文件不存在** | N/A |

---

## 详细发现

### dag.rs:1-164 — 自定义 DAG 拓扑排序
- **重复内容**：Kahn 算法拓扑排序 + 环检测（`Dag` 结构体，`sort()` 方法返回 `Result<Vec<Vec<NodeId>>, Vec<NodeId>>`）
- **BitFun 等价物**：**无**。全仓搜索 `topological`、`Kahn`、`DAG` 仅命中 taiji-engine 自身。BitFun 的 `harness/src/lib.rs` 有 `HarnessWorkflow` 枚举，但仅作标签用途，无执行编排引擎。
- **严重程度**：P0 — 核心差异化能力，不可删除
- **迁移难度**：N/A（保留）
- **备注**：dag.rs 是 taiji 流水线引擎的核心，BitFun 的 agent 运行时不以 DAG 方式编排。5 个单元测试覆盖线性链、分叉、合并、环检测、重复边幂等性。保持不变。

### factory.rs:1-135 — NodeFactory 注册表模式
- **重复内容**：`NodeFactory { registry: HashMap<String, Box<dyn Fn(&NodeConfig) -> Result<Box<dyn ComputeNode>> + Send + Sync>> }` — 名称→构造函数的注册表，含 `register()`、`create()`、`list_types()`、`contains()` 方法 + `register_node!` 宏。
- **BitFun 等价物**：`framework.rs:1420-1486` — `ToolRegistry<Tool>` 使用 `IndexMap<String, ToolRef<Tool>>` 存储名称→工具实例，含 `register_tool()`、`install_static_provider()`、`get_tool()`、`get_tool_names()`。
- **严重程度**：P2 — 表面相似但模式不同：taiji 存的是**闭包构造函数**（延迟实例化），BitFun 存的是**预构建实例**。taiji 的工厂模式服务于流水线节点动态组装。
- **迁移难度**：Easy（如需统一注册表 trait 抽象）
- **备注**：两种注册表在概念上等价（名称→实例），但实例化策略不同。如果 BitFun 的 `ToolRegistryItem` trait 可以适配 ComputeNode，可考虑统一。但当前差异合理：taiji 节点需要 `NodeConfig` 参数化构造，这和 BitFun 的静态工具生命周期不同。3 个单元测试。

### node.rs:1-96 — ComputeNode trait + NodeConfig
- **重复内容**：`ComputeNode` trait — `id()`、`name()`、`input_keys()`、`output_keys()`、`on_init()`、`on_bar()`、`on_tick()`、`on_calculate()`、`on_session_begin()`、`on_session_end()`、`is_ready()`、`subscribed_freqs()`。`NodeConfig` 含 `type_name` + `params: HashMap<String, Value>`。
- **BitFun 等价物**：`framework.rs:671-719` — `ToolRegistryItem` trait — `name()`、`description()`、`input_schema()`、`short_description()`、`default_exposure()`、`is_readonly()`、`is_concurrency_safe()`、`manages_own_execution_timeout()`、`is_enabled()`、`input_schema_for_model()`。
- **严重程度**：P2 — 表面相似（都有 `name()` + description + 输入/输出概念），但生命周期完全不同。ComputeNode 是交易领域特化的（`on_bar`/`on_tick`/`on_calculate`/`on_session_begin`/`on_session_end`），ToolRegistryItem 是通用工具执行生命周期。
- **迁移难度**：Hard（不建议迁移 — 领域差异过大）
- **备注**：可能可以提取一个共享的 `Named + Described` 超 trait（如 `HasIdentity`），但收益有限。保持独立。

### store.rs:1-174 — StateStore（内存键值存储 + 来源追踪）
- **重复内容**：`StateStore { data: DashMap<StateKey, StateValue>, provenance: DashMap<StateKey, NodeId>, last_update: DashMap<StateKey, Instant> }` — 并发键值存储，含泛型 `get<T>()`/`set()`、来源记录、信号收集、JSON 序列化。
- **BitFun 等价物**：
  - `persistence.rs:20-122` — `PersistenceService`：基于文件的 key→JSON 存储，含 `save_json()`、`load_json()`、备份、文件级锁。
  - `json_store.rs:108-229` — `JsonFileStore`：原子 JSON 文件写入，跨进程文件锁，重试逻辑，性能诊断日志。
- **严重程度**：P1 — 功能重叠（键值存储 + JSON 序列化），但存储介质不同：taiji 用内存（DashMap），BitFun 用文件。来源追踪（provenance — 哪个节点写入的）是 taiji 独有的，无 BitFun 等价物。
- **迁移难度**：Medium — 如果未来需要持久化状态，可考虑复用 `JsonFileStore` 的原子写入模式，但内存热路径不应改为文件 I/O。
- **备注**：4 个单元测试。建议：如果流水线需要跨会话状态持久化，可用 `PersistenceService::save_json()` 做快照导出，但运行时状态保持在 DashMap 中。

### config.rs:1-207 — PipelineConfig YAML 反序列化 + 验证
- **重复内容**：`PipelineConfig` 结构体 + `BarGenConfig`、`DataSourceSpec`、`NodeSpec` 子结构体。`from_yaml()` 反序列化，`validate()` — 检查输入键是否存在（排除 `"bars:"` 和 `"signals:"` 前缀）、节点 ID 唯一性。
- **BitFun 等价物**：**无**。BitFun 没有 YAML 流水线定义结构。BitFun 的配置主要是 Tauri 命令参数、会话设置，不是 DAG 执行图。
- **严重程度**：P0 — 核心差异化能力
- **迁移难度**：N/A（保留）
- **备注**：4 个单元测试，包括 `example-pipeline.yaml` 的集成测试。保持不变。

### error.rs:1-27 — TaijiError 枚举
- **重复内容**：`TaijiError` — 9 个变体（Config、DataSource、NodeFailed、KeyNotFound、CycleDetected、Io、Serde、AllSourcesDown、Fusion），thiserror 派生。
- **BitFun 等价物**：`runtime-ports.rs:63-98` — `PortError { kind: PortErrorKind, message: String }` — 8 种错误类别（NotAvailable、NotFound、InvalidRequest、PermissionDenied、Cancelled、Timeout、CleanupRequired、Backend）。
- **严重程度**：P2 — 都使用分类错误枚举 + 字符串消息模式，但语义域不同。taiji 错误是交易流水线特化的（AllSourcesDown、Fusion、CycleDetected），BitFun 错误是通用服务端口的。
- **迁移难度**：Easy — 如果 taiji 需要与 BitFun 端口集成，可以添加 `From<TaijiError> for PortError` 转换。
- **备注**：两种错误类型不直接重叠。如果 taiji 节点需要与 BitFun 的文件系统或终端端口交互，可添加 `impl From<TaijiError> for PortError`。

### feature_flags.rs:1-213 — Unleash 特性开关 SDK 封装
- **重复内容**：`FeatureFlags` 结构体封装 Unleash SDK — `is_strategy_enabled()`、`get_variant()`（A/B 测试）、`get_config_value()`（远程配置带本地回退值）。
- **BitFun 等价物**：`runtime-ports.rs:2064-2066` — `ConfigReadPort` trait — `async fn get_config_value(&self, key: &str) -> PortResult<Option<serde_json::Value>>` — 基础键值配置读取，不支持特性开关、A/B 测试或 Unleash 集成。
- **严重程度**：P1 — ConfigReadPort 提供基础远程配置能力，但不覆盖 Unleash 的特性开关功能（渐进式发布、用户分桶、A/B 测试）。
- **迁移难度**：Medium — 可选择：1) 保留 Unleash 作为 taiji 特化依赖；2) 实现 `ConfigReadPort` trait 并路由到 Unleash 后端，使配置读取路径统一。
- **备注**：3 个单元测试。推荐方案：实现 `ConfigReadPort`，底层委托给 Unleash，这样 taiji 节点可以通过 BitFun 标准端口访问配置，同时保留 Unleash 的高级特性。

### log.rs — 文件不存在
- 目标路径 `src/crates/taiji/taiji-engine/src/log.rs` 不存在。`lib.rs` 的 `pub mod` 声明中也没有 `log` 模块。该文件可能在计划阶段被列出但从未创建，或者已作为 `debug!`/`info!` 宏调用内联到其他文件中（BitFun 使用标准 `log` crate + `env_logger`）。

---

## 建议汇总

| 优先级 | 数量 | 内容 |
|--------|------|------|
| P0（保留） | 2 | dag.rs、config.rs — 无 BitFun 等价物，核心差异化能力 |
| P1（评估） | 2 | store.rs（考虑复用 JsonFileStore 做快照导出）、feature_flags.rs（可选实现 ConfigReadPort） |
| P2（可对齐） | 3 | factory.rs（如有需要，统一注册表抽象）、node.rs（如有需要，提取 HasIdentity trait）、error.rs（添加 PortError 转换） |
| 不存在 | 1 | log.rs — 文件不存在 |

**关键结论**：taiji-engine 的核心（DAG 执行引擎、流水线配置）在 BitFun 中没有等价实现，属于正当的差异化代码。注册表模式和键值存储有两处概念性重叠，但实现策略不同（内存 vs 文件，构造函数 vs 预构建实例），当前状态下合并收益有限。无 P0 级别的必须去重项。

# R-003: Task-Session 统一 + 上下文修复

## 需求来源

1. "subagent等同于agent机制权限工具，这样才能被session机制控制"
2. "UI上不是平铺，而是task subagent的树形裂变"（R-002 已实现递归 walk()）
3. "task这个模式必须大改了，要让他和session功能机制一样才行，但是用task的显示，这样才是完整的ultra模式"
4. 子agent上下文128K→1M（主session创建路径漏了修复）

## R-ID 矩阵

| R-ID | 描述 | 验收标准 | 优先级 | 依赖 |
|------|------|---------|--------|------|
| R-003-001 | AgenticEvent::SubagentTurnCompleted 事件定义与发射 | 见下 | P0 | - |
| R-003-002 | 子agent完成时向父session注入 dialog turn | 见下 | P0 | R-003-001 |
| R-003-003 | 主session创建路径 128K 上下文窗口修复 | 见下 | P0 | - |
| R-003-004 | 后台子agent完成通知链路补齐 | 见下 | P1 | R-003-001, R-003-002 |
| R-003-005 | 端到端集成验证 | 见下 | P0 | R-003-001~R-003-004 |

## 验收标准（逐R-ID）

### R-003-001: AgenticEvent::SubagentTurnCompleted 事件定义与发射

1. `src/crates/contracts/core-types/src/agentic_event.rs`（或等价路径）新增 `SubagentTurnCompleted` 变体，携带字段：
   - `parent_session_id: String`
   - `session_id: String`
   - `subagent_dialog_turn_id: String`
   - `output_text: String`（完成摘要，≤512 字符）
   - `status: SubagentCompletionStatus`（枚举：`Completed` / `Failed` / `Cancelled` / `PartialTimeout`）
2. 同步完成路径（`spawned task` 中 `await` 后）：`complete()` 调用后立即构造并发射该事件到 EventQueue。
3. 变体通过 `Serialize`/`Deserialize` derive，确保可跨进程/跨线程传输。
4. `cargo check --workspace` 零错误；新增事件变体的单元测试通过。

### R-003-002: 子agent完成时向父session注入 dialog turn

1. `complete()` 方法新增 `EventQueue` 参数（或通过已有上下文获取），在 `live_results.insert()` 后：
   - 调用 `submit_dialog_turn(AgentDialogTurnRequest { ... })` 将子agent结果注入父session。
   - `AgentDialogTurnRequest` 的 `reply_route` 指向父session，复用已有 `resolve_agent_session_reply_action()` 自动回复机制。
2. 注入内容包含：
   - `role: "agent"`，`content`: 子agent完成摘要（含 subagent_dialog_turn_id、状态、关键输出）。
   - 元数据标记 `source: "subagent_completion"` 以便前端区分。
3. 父session的 dialog 历史中可见子agent完成记录，无需父agent主动 `AgentWait` 拉取。
4. 上下文压缩后子agent结果不丢失（已持久化到dialog turn）。
5. 不影响已有 `live_results` 拉模型（向后兼容：保留 `live_results.insert()` + `changes.notify_waiters()`）。
6. `complete()` 中状态映射使用显式转换函数 `SubagentResultStatus → SubagentCompletionStatus`（非 `status_label()` 字符串），确保 `Cancelled` 正确传递为 `SubagentCompletionStatus::Cancelled` 而非 `Failed`。

### R-003-003: 主session创建路径 128K 上下文窗口修复

1. `coordinator.rs` 主session创建路径（约 L1537-1546）新增 `refresh_session_context_window` 调用，确保新建session读取最新 `context_window` 配置值。
2. `session.rs` 约 L187：session 初始化时 `context_window` 从配置读取而非硬编码默认值。
3. `agentic_api.rs` 约 L1270：API 层创建session时同步刷新 `context_window`。
4. 验证：在 `config.toml` 中设置 `context_window = 1048576`（1M）后创建新session，session实例的 `context_window` 字段值为 `1048576`。
5. 已有session不受影响（仅新建session时生效）。

### R-003-004: 后台子agent完成通知链路补齐

1. 后台 `spawned task`（`tokio::spawn` 异步执行）完成后：
   - 发射 `AgenticEvent::SubagentTurnCompleted`（复用 R-003-001）。
   - 调用 `submit_dialog_turn` 注入父session（复用 R-003-002）。
2. 同步完成路径同样发射事件（消除"同步完成后无事件"缺口）。
3. 前端 `EventQueue` 消费者接收 `SubagentTurnCompleted` 后：
   - 刷新 task tree 中对应节点的完成状态（绿色勾/红色叉）。
   - 不重复渲染已有的 dialog turn（去重以 `subagent_dialog_turn_id` + `status` 为键）。
4. `complete()` 方法本身不持有 EventQueue 引用的问题已解决（通过参数注入或从 AgentRuntime 上下文获取）。

### R-003-005: 端到端集成验证

1. **端到端注入链路**：Task spawn → `complete()` → 父 session 对话流中出现 `source: "subagent_completion"` 注入消息，消息包含 subagent_dialog_turn_id、status、output_text。
2. **EventQueue 事件可达**：父 session 的 EventQueue 消费者接收到 `AgenticEvent::SubagentTurnCompleted`，`status` 字段与子 agent 实际完成状态一致。
3. **上下文窗口生效**：新创建 session 的 `session.config.max_context_tokens ≥ 1_000_000`（配置值为 1048576 时），非硬编码默认值 128K。
4. **取消状态正确传递**：子 agent 被取消后，注入消息的 `status = SubagentCompletionStatus::Cancelled`（非 `Failed`），EventQueue 事件同。

## 边界与约束

- **工作区**：仅 `E:\finance-trading\lvpa\software\taiji-quant`（taiji-quant fork）
- **不新增 crate**：所有修改在已有 crate 内完成
- **不改前端 UI**：R-002 已完成 task-subagent 树形渲染，本次仅补齐后端事件→前端推送链路
- **不改 protocol schema**：`AgenticEvent` 变体追加，不删除或重命名字段
- **向后兼容**：保留 `live_results` 拉模型 + `changes.notify_waiters()` 不变；已有 session 的 `context_window` 不受影响
- **不涉及**：跨进程 session 同步、relay 模式下的远程子agent通知（本次仅本地 session 闭环）

## 技术上下文

### 关键复用点
- `submit_dialog_turn(AgentDialogTurnRequest)` — 7层接力链路已就绪（SessionMessageTool → AgentRuntime → DialogScheduler → QueuedTurn → Coordinator → EventQueue → Frontend）
- `resolve_agent_session_reply_action()` — 自动回复机制已有
- `live_results` + `changes.notify_waiters()` — 保留为 fallback 拉模型

### 修改文件清单（预期）

| 文件 | R-ID | 变更 |
|------|------|------|
| `src/crates/contracts/core-types/src/agentic_event.rs` | R-003-001 | 新增 `SubagentTurnCompleted` 变体 |
| `src/crates/execution/agent-runtime/src/agent/agentic_executor.rs`（或等价路径） | R-003-001, R-003-002, R-003-004 | `complete()` 注入 EventQueue + submit_dialog_turn |
| `src/crates/execution/agent-runtime/src/coordinator.rs` | R-003-003 | L1537-1546 加 `refresh_session_context_window` |
| `src/crates/services/services-core/src/session.rs`（或等价路径） | R-003-003 | L187 配置读取 |
| `src/apps/desktop/src/api/agentic_api.rs`（或等价路径） | R-003-003 | L1270 API 层刷新 |
| `src/crates/contracts/events/src/agentic_event.rs`（如独立 events crate） | R-003-001 | 前端事件类型映射（如适用） |

> 注：具体文件路径以实际 `taiji-quant` 代码库结构为准，上表为基于 BitFun upstream 的预期路径。

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| `complete()` 获取 EventQueue 引用需要跨层传递 | 通过 AgentRuntime 上下文已有引用传入，避免全局静态变量 |
| dialog turn 注入导致父session对话历史膨胀 | 子agent完成摘要 ≤512 字符，仅注入关键信息 |
| `refresh_session_context_window` 三处调用遗漏一处 | R-003-005 集成测试覆盖三处 |
| 后台 spawned task 中 EventQueue 生命周期问题 | 使用 `Arc<EventQueue>` 共享所有权 |

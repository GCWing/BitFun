# R-003 设计方案：Task-Session 通信统一 + 上下文窗口修复

> 派生自：[r003-requirements.md](r003-requirements.md)
> 设计原则：不新建通信通道，复用已有 7 层注入链路；最小改动，最大复用。

---

## 架构决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|---------|
| D1 | **复用 SessionMessage 注入路径**，不新建通信通道 | `submit_dialog_turn()` 7 层链路（session_message_tool → scheduler → coordinator → event_queue → 前端）已就绪且经过验证；Task 子 agent 完成 → 构造 `AgentDialogTurnRequest` → 走同一条注入路径 | 新建专用通知通道（重复造轮子，增加维护负担） |
| D2 | 新增 `AgenticEvent::SubagentTurnCompleted` 事件变体，其 `status` 字段使用 `SubagentCompletionStatus` 枚举（`Completed` / `Failed` / `Cancelled` / `PartialTimeout`）而非裸 `String` | 前端可订阅实现即时通知，无需轮询；枚举类型消除字符串拼写错误风险，`PartialTimeout` 覆盖超时非完全失败场景 | 仅靠 dialog turn 注入不另发事件（前端无法即时感知，需监听 dialog turn 变化）；用 `String` 存状态（类型不安全，前端需维护 magic string 列表） |
| D3 | coordinator.rs 中两处 spawned task 闭包改造 | 子 agent 执行完成后 → `complete()` 持久化 → `emit_event(SubagentTurnCompleted)` 通知前端 → `submit_dialog_turn()` 注入父对话流。spawned task 闭包需额外捕获 `event_queue` + `parent_info` + `agent_type` | 在 `complete()` 方法内部隐式获取上下文（改动范围更大，且 `complete()` 当前不持有 EventQueue 引用） |
| D4 | **128K→1M 三层修复**：L1 根因 + L2 兜底 + L3 兜底 | 主 session 创建路径漏了 `refresh_session_context_window` 是根因；`session.rs` 和 `agentic_api.rs` 两处硬编码 `128128` 是兜底安全网 | 仅修一处（可能遗漏其他创建路径） |

---

## 数据流

### 子 agent 完成 → 父 session 注入（完整链路）

```
Task::spawn()
  │
  ├─ coordinator.start_background_subagent()  或  execute_subagent()
  │     │
  │     ├─ 创建 Subagent session (SessionKind::Subagent)
  │     ├─ 写入 lineage (parent_session_id, parent_dialog_turn_id, parent_tool_call_id)
  │     │
  │     └─ tokio::spawn {
  │           │
  │           ├─ execute_hidden_subagent_internal(request, cancel_token, timeout)
  │           │     │
  │           │     └─ AI loop 执行...
  │           │
  │           ├─ background_subagent_outcomes.complete(task_pk, result)  ← 存存储（已有）
  │           │
  │           ├─ emit_event(AgenticEvent::SubagentTurnCompleted {       ← 新：通知前端
  │           │       session_id,
  │           │       subagent_dialog_turn_id,
  │           │       parent_session_id,
  │           │       parent_dialog_turn_id,
  │           │       parent_tool_call_id,
  │           │       agent_type,
  │           │       status: SubagentCompletionStatus::Completed, // 枚举：Completed | Failed | Cancelled | PartialTimeout
  │           │       output_text,   // 摘要，≤512 字符
  │           │   })
  │           │     │
  │           │     └─ EventQueue → 前端 EventBus → TaskTree 面板刷新节点状态
  │           │
  │           └─ submit_dialog_turn(AgentDialogTurnRequest {            ← 新：注入父对话流
  │                   session_id: parent_session_id,
  │                   message: formatted_result,   // 子 agent 完成摘要
  │                   agent_type: parent_agent_type,
  │                   policy: DialogSubmissionPolicy { reply: Automatic, ... },
  │                   prepended_reminders: [{
  │                       kind: "task_subagent_result",
  │                       text: "子 agent [{agent_type}] 已完成: {status}..."
  │                   }],
  │               })
  │                 │
  │                 └─ 7 层注入链路:
  │                       session_message_tool / scheduler.submit_dialog_turn()
  │                         → scheduler 构造 QueuedTurn
  │                           → coordinator 入队
  │                             → event_queue 广播 DialogTurnStarted
  │                               → 前端渲染新 dialog turn
  │
  └─ 返回 BackgroundSubagentStartResult { bg_task_id, agent_id }
```

### 同步路径（execute_subagent / foreground）

同上述 spawned task 流程，区别仅在于不经过 `background_subagent_outcomes` — 而是直接在 `execute_hidden_subagent_internal` 返回后执行 `emit_event` + `submit_dialog_turn`。

---

## 文件架构

| 文件 | R-ID | 操作 | 行数 | 变更说明 |
|------|------|------|------|---------|
| `contracts/events/src/agentic.rs` | R-003-001 | 新增 enum + variant | +25 | ① 新增 `SubagentCompletionStatus` 枚举（`Completed` / `Failed` / `Cancelled` / `PartialTimeout`，derive `Serialize`/`Deserialize`/`PartialEq`）；② 在 `SubagentSessionLinked` 之后添加 `SubagentTurnCompleted { session_id, subagent_dialog_turn_id, parent_session_id, parent_dialog_turn_id, parent_tool_call_id, agent_type, status: SubagentCompletionStatus, output_text }`；③ 补齐 `session_id()` / `default_priority()` match arm；④ `frontend_projection.rs` 添加投影 match arm + 测试 |
| `assembly/core/src/agentic/coordination/coordinator.rs` | R-003-001, R-003-002, R-003-004 | 修改 2 处 spawned task | +45 | **位置 1**（~L7320，scheduler 路径）：`complete()` 之后 → 构造事件 → `emit_event()` → `submit_dialog_turn()`。**位置 2**（~L7378，直接执行路径）：同上。spawned task 闭包额外捕获 `event_queue: Arc<EventQueue>`, `parent_session_id`, `parent_dialog_turn_id`, `parent_tool_call_id`, `agent_type` |
| `execution/agent-runtime/src/session.rs` | R-003-003 | 改默认值 | 1 行 | L187: `max_context_tokens: 128128` → `max_context_tokens: 1_048_576` |
| `apps/desktop/src/api/agentic_api.rs` | R-003-003 | 改默认值 | 1 行 | L1270: `unwrap_or(128128)` → `unwrap_or(1_048_576)` |
| `assembly/core/src/agentic/coordination/coordinator.rs` | R-003-003 | 加刷新调用 | +3 | `create_session_with_workspace_and_creator`（~L1537）末尾：`session_manager.refresh_session_context_window(&session.session_id).await?;` |
| `assembly/core/src/agentic/coordination/scheduler.rs` | R-003-002 | 新 reminder kind | +5 | `DialogSubmissionPolicy` / prepended_reminder 处理中注册 `"task_subagent_result"` kind |
| `contracts/events/src/agentic.rs`（测试） | R-003-005 | 新增测试 | +30 | `SubagentTurnCompleted` 序列化/反序列化往返测试，各 status 变体覆盖 |
| `contracts/events/src/frontend_projection.rs`（测试） | R-003-005 | 新增测试 | +20 | 投影测试：验证 `SubagentTurnCompleted` → `agentic://subagent-turn-completed` 映射 |
| `assembly/core/.../coordinator.rs`（测试） | R-003-005 | 新增集成测试 | +40 | mock 验证：spawned task 中 `complete()` → `emit_event()` → `submit_dialog_turn()` 调用链 |
| `execution/agent-runtime/src/session.rs`（测试） | R-003-005 | 新增测试 | +15 | `SessionConfig::default().max_context_tokens == 1_048_576` 断言 |
| `apps/desktop/src/api/agentic_api.rs`（测试） | R-003-005 | 新增测试 | +10 | API 层 `unwrap_or(1_048_576)` 默认值验证 |

### 文件路径（绝对路径基准）

```
E:\finance-trading\lvpa\software\taiji-quant\src\crates\
  ├── contracts\events\src\agentic.rs                          ← R-003-001, R-003-005
  ├── contracts\events\src\frontend_projection.rs              ← R-003-001, R-003-005
  ├── assembly\core\src\agentic\coordination\coordinator.rs    ← R-003-001/002/003/004/005
  ├── assembly\core\src\agentic\coordination\scheduler.rs      ← R-003-002
  ├── execution\agent-runtime\src\session.rs                   ← R-003-003, R-003-005
  └── ..\apps\desktop\src\api\agentic_api.rs                   ← R-003-003, R-003-005 (taiji-quant\src\apps\...)
```

---

## 依赖图

```
                    ┌─────────────────────────┐
                    │   agentic.rs (events)    │
                    │  +SubagentTurnCompleted  │
                    └────────────┬────────────┘
                                 │ derive / use
                    ┌────────────▼────────────┐
                    │     coordinator.rs       │
                    │  +emit_event()           │
                    │  +submit_dialog_turn()   │
                    │  +refresh_context_window │
                    └──────┬──────────┬────────┘
                           │          │
              ┌────────────▼──┐  ┌────▼──────────────┐
              │  scheduler.rs  │  │   session.rs       │
              │  +reminder     │  │   max_context: 1M  │
              │   kind 注册    │  └───────────────────┘
              └──────┬────────┘
                     │
              ┌──────▼──────────┐
              │  agentic_api.rs  │
              │  unwrap_or(1M)   │
              └──────────────────┘

依赖方向：events ← coordinator → scheduler
                         coordinator → session, agentic_api
无新增 crate 依赖，无循环依赖。
```

---

## 128K 修复三层详解

```
L1（根因修复）
  coordinator.rs:create_session_with_workspace_and_creator()
    → 末尾加 refresh_session_context_window(&session_id)
    → SessionConfig 从 config.toml 读取 context_window 值后刷新到 Session 实例
    → 影响面：所有通过此函数创建的 session（主 session + 子 agent session）

L2（兜底修复）
  execution/agent-runtime/src/session.rs:187
    max_context_tokens: 128128 → 1_048_576
    → 当 SessionConfig 未显式设置 context_window 时的 fallback 默认值
    → 影响面：所有未通过 config.toml 显式设置 context_window 的 session

L3（兜底修复）
  apps/desktop/src/api/agentic_api.rs:1270
    unwrap_or(128128) → unwrap_or(1_048_576)
    → API 层创建 session 时，从请求参数取 context_window 的 fallback
    → 影响面：通过 Tauri API 创建的 session（桌面端入口）
```

---

## 风险与降级

| 风险 | 等级 | 缓解措施 | 降级方案 |
|------|------|---------|---------|
| spawned task 闭包捕获 event_queue 导致生命周期问题 | 中 | `event_queue` 在 coordinator 上已是 `Arc`，直接 clone 进闭包 | 若编译失败，改为在 coordinator 上新增 `emit_subagent_turn_completed()` 辅助方法，闭包通过 coordinator Arc clone 调用 |
| `submit_dialog_turn` 在 spawned task 中 panic | 低 | 用 `let _ = ...await` 吞错误；complete() 已先执行，持久化不受影响 | 降级为仅 emit 事件不注入 dialog turn；父 agent 仍可通过 AgentWait 拉取结果 |
| dialog turn 注入导致父 session 上下文膨胀 | 低 | 子 agent 输出摘要 ≤512 字符，仅注入关键信息；prepended_reminder 标记 `task_subagent_result` 让 AI 知道这是通知而非用户消息 | 若膨胀严重，改为仅注入事件不注入 dialog turn（前端事件驱动刷新） |
| `refresh_session_context_window` 调用位置遗漏 | 中 | 在 `create_session_with_workspace_and_creator` 中加一次调用覆盖主路径；`create_hidden_subagent_session` 内部也会经过 `create_session_with_id_and_creator`，自动覆盖子 agent session | L2/L3 兜底确保即使 L1 遗漏也不影响实际 context_window |
| 已有 session 不受影响 | - | 本次修改仅影响新建 session；`refresh_session_context_window` 在创建时执行，不改已有 session 数据 | 无需降级 |
| 集成测试未覆盖端到端链路（R-003-005 遗漏） | **高** | 按 R-003-005 验收标准逐条实现：① 序列化往返测试 ② mock spawn 闭包调用链 ③ 新建 session context_window 断言。`cargo test --workspace` 必须全量通过 | 若跳过集成测试，子 agent 完成后结果可能静默丢失——`emit_event` / `submit_dialog_turn` 中任一步 panic 不会被单元测试捕获，前端 TaskTree 面板永远看不到完成状态 |

---

## 不变式（Invariants）

1. **`complete()` 在 `emit_event()` 和 `submit_dialog_turn()` 之前执行** — 持久化先于通知，保证数据一致性。
2. **`live_results` + `changes.notify_waiters()` 保持不变** — AgentWait 拉模型继续可用，与新增的推模型并存。
3. **不删除、不重命名已有 `AgenticEvent` 变体和字段** — 向后兼容。
4. **不新增 crate**，不修改 protocol schema。
5. **已有 session 的 `context_window` 不受影响** — 仅新创建 session 生效。

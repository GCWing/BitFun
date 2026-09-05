# Agent Runtime 生命周期时序

本文件是 [`agent-runtime-services-design.md`](agent-runtime-services-design.md) 的生命周期补充，
描述 `openbitfun-agent-runtime` 的 `sdk` / `AgentRuntime` 门面如何把智能体（agent）从构建到销毁的
全生命周期映射到具体接口与底层 Port。当前接口名称、字段和消费方以代码为准：
[sdk.rs](../../src/crates/execution/agent-runtime/src/sdk.rs)、
[runtime.rs](../../src/crates/execution/agent-runtime/src/runtime.rs)、
[session_state.rs](../../src/crates/execution/agent-runtime/src/session_state.rs)、
[events.rs](../../src/crates/execution/agent-runtime/src/events.rs)、
[scheduler.rs](../../src/crates/execution/agent-runtime/src/scheduler.rs)、
[post_call_hooks.rs](../../src/crates/execution/agent-runtime/src/post_call_hooks.rs)。

> 范围：本文件只描述 `agent-runtime` crate 暴露的稳定 SDK 接口与对应 Port 的调用顺序，不构成
> 对内部协调器、调度器、持久化或具体工具执行的承诺。`run()` 是 SDK 层对「会话解析 + turn 提交」
> 的编排封装，真实 turn 执行由 `openbitfun-core` owner 在端口实现侧完成。

## 生命周期阶段总览

| 阶段 | 含义 | 入口 |
|---|---|---|
| ① 构建 Build | 注入 Port、校验可用性，构造 `AgentRuntime` | `AgentRuntimeBuilder` |
| ② 会话创建 Create | 建立会话、解析 agent 类型、订阅事件 | `create_session` / `run(Create)` |
| ③ 会话管理 Manage | 列表/重命名/归档、切换模型与模式 | `list/rename/archive/update_*` |
| ④ 提交/运行 Run | 把消息投递为 turn | `run` / `submit_turn` / `submit_dialog_turn` |
| ⑤ 处理中 Process | 权限确认、用户问答、后台回投、事件广播 | `respond_permission` / `deliver_*` / `publish_event` |
| ⑥ 结算 Settle | 等 turn 结束、读 transcript、算 usage、钩子 | `wait_for_turn_settlement` / `generate_session_usage` |
| ⑦ 中断/恢复 Cancel/Restore/Fork | 取消、恢复、分叉会话 | `cancel_turn` / `restore_session` / `fork_*` |
| ⑧ 销毁 Delete | 删除会话 | `delete_session` |

## 时序图

```
调用方(SDK)          AgentRuntime(sdk.rs)        底层Port(runtime.rs)         事件/状态            openbitfun-core实现
    │                       │                          │                     │                       │
    │═══════════════════════╗│                          │                     │                       │
    │ ① 构建期 (Build)      ║│                          │                     │                       │
    │═══════════════════════╝│                          │                     │                       │
    │ AgentRuntimeBuilder    │                          │                     │                       │
    │::new()                 │                          │                     │                       │
    ├──────────────────────►│ with_submission_port()   │                     │                       │
    │  with_*_port() 注入端口│ with_dialog_turn_port() │                     │                       │
    │  (submission/dialog/   │ with_lifecycle_delivery  │                     │                       │
    │   cancellation/...等)  │ _port()                  │                     │                       │
    │                        │ with_cancellation_port() │                     │                       │
    │                        │ with_services()         │                     │                       │
    │                        │ with_event_stream()     │                     │                       │
    │                        │ build() ─────────────►   │ 校验:               │                       │
    │                        │                          │ submission必填      │                       │
    │                        │                          │ plugin_runtime校验  │                       │
    │                        │◄─────────────────────────│ AgentRuntime{}      │                       │
    │◄───────────────────────│ AgentRuntime             │                     │                       │
    │                        │                          │                     │                       │
    │═══════════════════════╗│                          │                     │                       │
    │ ② 会话创建期 (Create)  ║│                          │                     │                       │
    │═══════════════════════╝│                          │                     │                       │
    │ subscribe_events()    │                          │                     │                       │
    ├──────────────────────►│ event_source.subscribe() │                     │ 注册事件接收器         │
    │ create_session(req)   │                          │                     │                       │
    ├──────────────────────►│ submission               │                     │                       │
    │                        ├─────────────────────────►│ create_session()   │                       │
    │                        │                          ├──────────────────► │                       │
    │                        │                          │    (session_id)    │                       │
    │                        │◄─────────────────────────│ AgentSessionCreate │                       │
    │◄───────────────────────│ Result                   │ Result              │                       │
    │                        │                          │                     │                       │
    │ resolve_session_       │                          │                     │                       │
    │  agent_type(sid)       │                          │                     │                       │
    ├──────────────────────►│ submission ────────────►│ resolve_session_    │                       │
    │◄───────────────────────│ ◄────────────────────────│ agent_type()        │                       │
    │                        │                          │                     │                       │
    │═══════════════════════╗│                          │                     │                       │
    │ ③ 会话管理期 (Manage)   ║│                          │                     │                       │
    │═══════════════════════╝│                          │                     │                       │
    │ list_sessions()        │                          │                     │                       │
    ├──────────────────────►│ session_management ────►│ list_sessions()      │                       │
    │ rename_session()      │  (port)                 ├──────────────────► │                       │
    ├──────────────────────►│                         │ rename_session()    │                       │
    │ archive_session()     │                          ├──────────────────► │                       │
    ├──────────────────────►│                         │ archive_session()    │                       │
    │ update_session_model()│ session_model ─────────►│ update_session_model│                       │
    ├──────────────────────►│                         │                      │                       │
    │ update_session_mode() │ session_mode ──────────►│ update_session_mode │                       │
    ├──────────────────────►│                         │                      │                       │
    │ resolve_session_      │ session_management ────►│ resolve_session_     │                       │
    │  workspace_binding()  │                         │  workspace_binding() │                       │
    │◄───────────────────────│                          │                     │                       │
    │                        │                          │                     │                       │
    │═══════════════════════╗│                          │                     │                       │
    │ ④ 提交/运行期 (Run)     ║│                          │                     │                       │
    │═══════════════════════╝│                          │                     │                       │
    │ run(AgentRunRequest)  │                          │                     │                       │
    ├──────────────────────►│ 若 Create: create_session│                     │                       │
    │                        ├─────────────────────────►│ create_session()   │                       │
    │                        │ 若 Existing: resolve_    │                     │                       │
    │                        │  session_agent_type()     │                     │                       │
    │                        │ submit_turn()            │                     │                       │
    │                        ├─────────────────────────►│ submission           │                       │
    │                        │                          │  .submit_message()  │                       │
    │                        │                          ├──────────────────► │ publish_event()        │
    │                        │                          │                     │ TurnStarted            │
    │                        │                          │                     │ SessionState→         │
    │                        │                          │                     │ Processing(Starting)  │
    │                        │◄─────────────────────────│ AgentSubmissionResult│                       │
    │◄───────────────────────│ AgentRunHandle           │                     │ (turn_id,accepted)    │
    │                        │   {session_id,turn_id,   │                     │                       │
    │                        │    agent_type,events}    │                     │                       │
    │                        │                          │                     │                       │
    │ ── 或直接用细分接口 ──  │                          │                     │                       │
    │ submit_turn(req)      │                          │                     │                       │
    ├──────────────────────►│ submission.submit_message│                     │                       │
    │ submit_dialog_turn() │ dialog_turn ────────────►│ submit_dialog_turn()│ (远端对话/带优先级队列) │
    ├──────────────────────►│                         │                     │                       │
    │                        │                          │                     │                       │
    │═══════════════════════╗│                          │                     │                       │
    │ ⑤ 处理中事件流 (Process)║│                          │                     │                       │
    │═══════════════════════╝│                          │                     │                       │
    │                        │                          │                     │ ProcessingPhase流转: │
    │                        │                          │                     │ Starting→Compacting→  │
    │                        │                          │                     │ Thinking→Streaming→   │
    │                        │                          │                     │ ToolCalling           │
    │                        │                          │                     │   ┊                   │
    │                        │                          │                     │   ▼ 工具确认           │
    │ pending_permission_    │ permission_requests ──►│ interactive_pending  │                       │
    │  requests()           │                         │  _requests()        │                       │
    ├──────────────────────►│ subscribe_permission_    │                     │ PermissionRequestEvent│
    │ respond_permission()  │  requests()             │                     │                       │
    ├──────────────────────►│ permission_requests ────►│ reply()             │ PermissionReply        │
    │                        │                          │                     │ →ToolConfirming        │
    │                        │                          │                     │   ┊                   │
    │                        │                          │                     │   ▼ 用户问题工具        │
    │ submit_user_answers() │ interaction_response ──►│ submit_user_answers │                       │
    ├──────────────────────►│                         │ ()                  │                       │
    │                        │                          │                     │                       │
    │ publish_event()       │ services.events 或       │                     │                       │
    ├──────────────────────►│ event_stream ──────────►│ publish_runtime_    │ RuntimeEventEnvelope   │
    │                        │                          │  event()            │ (SessionStateChanged) │
    │                        │                          │                     │                       │
    │  (后台结果/线程目标回投) │                          │                     │                       │
    │ deliver_background_   │ lifecycle_delivery ────►│ deliver_background_ │                       │
    │  result()            │                         │  result()           │                       │
    ├──────────────────────►│                         │                     │ 注入到当前/精确turn     │
    │ deliver_thread_goal() │ lifecycle_delivery ────►│ deliver_thread_goal │                       │
    ├──────────────────────►│                         │ ()                  │                       │
    │                        │                          │                     │                       │
    │  (线程目标管理)         │                          │                     │                       │
    │ create_thread_goal() │ thread_goal_management ►│ create_thread_goal() │                       │
    │ get_thread_goal()    │                         │ get_thread_goal()    │                       │
    │ update_thread_goal_  │                         │ update_thread_goal_  │                       │
    │  status()            │                         │  status()            │                       │
    │                        │                          │                     │                       │
    │═══════════════════════╗│                          │                     │                       │
    │ ⑥ 结算/完成期 (Settle)  ║│                          │                     │                       │
    │═══════════════════════╝│                          │                     │                       │
    │ wait_for_turn_        │ turn_settlement ───────►│ wait_for_turn_      │                       │
    │  settlement(req)     │                         │  settlement()       │                       │
    ├──────────────────────►│                         │                     │ 阻塞至turn结束         │
    │                        │                          │                     │   ┊                   │
    │                        │                          │                     │   ▼ TurnOutcome      │
    │                        │                          │                     │ Completed/Cancelled/  │
    │                        │                          │                     │ Failed                │
    │                        │                          │                     │ →publish TurnCompleted/│
    │                        │                          │                     │ TurnCancelled/TurnFailed│
    │                        │                          │                     │ →SessionState→Idle/Error│
    │                        │                          │                     │                       │
    │  (post-call hooks)     │                          │                     │                       │
    │                        │ hook_registry.hooks()    │                     │ 成功工具调用后:        │
    │                        │ run_successful_tool_     │                     │ DeepReviewSharedContext│
    │                        │  post_call_hooks()       │                     │ ToolUse               │
    │                        │                          │                     │                       │
    │ generate_session_     │ session_usage ─────────►│ generate_session_   │                       │
    │  usage()             │                         │  usage()            │ SessionUsageReport    │
    ├──────────────────────►│                         │                     │                       │
    │ read_session_         │ session_transcript_     │                     │                       │
    │  transcript()        │  reader ────────────────►│ read_session_       │                       │
    ├──────────────────────►│                         │  transcript()      │                       │
    │ record_completed_     │ local_command_turn ────►│ record_completed_  │ (本地CLI命令记录)      │
    │  local_command_turn()│                         │  local_command_turn │                       │
    │◄───────────────────────│                          │ ()                  │                       │
    │                        │                          │                     │                       │
    │═══════════════════════╗│                          │                     │                       │
    │ ⑦ 中断/恢复期 (Cancel/ ║│                          │                     │                       │
    │    Restore/Fork)       ║│                          │                     │                       │
    │═══════════════════════╝│                          │                     │                       │
    │ cancel_turn(req)     │                          │                     │                       │
    ├──────────────────────►│ cancellation ───────────►│ cancel_turn()       │ →TurnCancelled事件    │
    │◄───────────────────────│ ◄────────────────────────│ AgentTurnCancelRes │ →SessionState→Idle    │
    │                        │                          │                     │                       │
    │ restore_session(req) │                          │                     │                       │
    ├──────────────────────►│ session_restore ────────►│ restore_session()  │ 重新加载历史session   │
    │◄───────────────────────│ ◄────────────────────────│ AgentSessionRestore│ →SessionState恢复     │
    │                        │                          │  Result            │                       │
    │                        │                          │                     │                       │
    │ fork_session() /     │ session_fork ───────────►│ fork_session() /    │ 从某turn分叉新session  │
    │ fork_session_at_turn()│                         │  fork_session_     │                       │
    ├──────────────────────►│                         │  at_turn()          │                       │
    │                        │                          │                     │                       │
    │═══════════════════════╗│                          │                     │                       │
    │ ⑧ 销毁期 (Delete)      ║│                          │                     │                       │
    │═══════════════════════╝│                          │                     │                       │
    │ delete_session(req)  │                          │                     │                       │
    ├──────────────────────►│ session_management ────►│ delete_session()    │                       │
    │◄───────────────────────│                          │                     │                       │
```

## 生命周期阶段 ↔ 接口映射表

| 阶段 | 触发的 SDK 接口（[sdk.rs](../../src/crates/execution/agent-runtime/src/sdk.rs)） | 委托的底层 Port（[runtime.rs](../../src/crates/execution/agent-runtime/src/runtime.rs)） | 关键状态/事件 |
|---|---|---|---|
| **① 构建 Build** | `AgentRuntimeBuilder::new()` + `with_*_port()` + `build()` | 注入 `submission`(必填)/`dialog_turn`/`lifecycle_delivery`/`cancellation`/`session_management`/`session_restore`/`turn_settlement`/`session_model`/`session_mode`/`session_fork`/`session_usage`/`local_command_turn`/`session_transcript_reader`/`thread_goal_management`/`interaction_response`/`permission_requests`/`services`/`event_stream`/`event_source` | build 校验：`submission` 必填；`plugin_runtime` 可用性 |
| **② 会话创建 Create** | `subscribe_events()`、`create_session()`、`create_session_with_id()`、`resolve_session_agent_type()` | `AgentSubmissionPort::create_session()` / `create_session_with_id()` / `resolve_session_agent_type()` | `AgentSessionCreateResult{session_id}` |
| **③ 会话管理 Manage** | `list_sessions()`、`rename_session()`、`archive_session()`、`set_session_archived()`、`update_session_model()`、`update_session_mode()`、`resolve_session_workspace_binding()` | `AgentSessionManagementPort`、`AgentSessionModelPort`、`AgentSessionModePort` | 元数据/归档/模型/模式切换 |
| **④ 提交/运行 Run** | `run()`（高层封装）、`submit_turn()`、`submit_dialog_turn()` | `run()` 内部 = `create_session` 或 `resolve_session_agent_type` + `submit_turn`；`AgentSubmissionPort::submit_message()`；`AgentDialogTurnPort::submit_dialog_turn()` | 发 `TurnStarted`；`SessionState→Processing(Starting)`；返回 `AgentRunHandle` |
| **⑤ 处理中 Process** | `pending_permission_requests()`、`subscribe_permission_requests()`、`respond_permission()`、`submit_user_answers()`、`deliver_background_result()`、`deliver_thread_goal()`、`create/get/update_thread_goal_status()`、`publish_event()` | `PermissionRequestManager`、`AgentInteractionResponsePort::submit_user_answers()`、`AgentLifecycleDeliveryPort::deliver_background_result/deliver_thread_goal()`、`AgentThreadGoalManagementPort`、`RuntimeEventSink`(services.events) + `AgentEventStream` | `ProcessingPhase` 流转：Starting→Compacting→Thinking→Streaming→ToolCalling→ToolConfirming；发 `RuntimeEventType::SessionStateChanged` |
| **⑥ 结算 Settle** | `wait_for_turn_settlement()`、`read_session_transcript()`、`generate_session_usage()`、`record_completed_local_command_turn()` | `AgentTurnSettlementPort::wait_for_turn_settlement()`、`SessionTranscriptReader`、`AgentSessionUsagePort`、`AgentLocalCommandTurnPort`；`hook_registry` 内 `run_successful_tool_post_call_hooks()` | `TurnOutcome` Completed/Cancelled/Failed；发 `TurnCompleted`/`TurnCancelled`/`TurnFailed`；`SessionState→Idle/Error`；`SessionUsageReport` |
| **⑦ 中断/恢复 Cancel/Restore/Fork** | `cancel_turn()`、`restore_session()`、`fork_session()`、`fork_session_at_turn()` | `AgentTurnCancellationPort::cancel_turn()`、`AgentSessionRestorePort::restore_session()`、`AgentSessionForkPort::fork_session()/fork_session_at_turn()` | `AgentTurnCancellationResult`；`AgentSessionRestoreResult{session,state}` |
| **⑧ 销毁 Delete** | `delete_session()` | `AgentSessionManagementPort::delete_session()` | 会话移除 |

## 关键设计要点

1. **分层门面（[sdk.rs](../../src/crates/execution/agent-runtime/src/sdk.rs) 顶部）**：`sdk.rs` 是 `runtime.rs`
   的稳定门面，仅暴露 ports / registry / 事件源，不暴露 Plugin Runtime Host ABI；客户端消费 `sdk`，
   产品组装消费内部 `runtime`。

2. **`run()` 是生命周期的编排核心（[runtime.rs:1231-1278](../../src/crates/execution/agent-runtime/src/runtime.rs#L1231-L1278)）**：
   它把「会话解析 → turn 提交」串联：
   - `SessionSelector::Create` → `create_session()`
   - `SessionSelector::Existing` → `resolve_session_agent_type()`（不重复建会话）
   - 最后统一调 `submit_turn()` → `submission.submit_message()`

3. **turn 处理相位流转（[session_state.rs:31-39](../../src/crates/execution/agent-runtime/src/session_state.rs#L31-L39)）**：
   `ProcessingPhase` = `Starting / Compacting / Thinking / Streaming / ToolCalling / ToolConfirming`，对应模型推理、
   流式输出、工具调用、权限确认等子阶段，通过 `publish_event()` 以 `RuntimeEventType::SessionStateChanged`
   广播。

4. **三条交互旁路**（处理中可并发进入）：
   - **权限确认旁路**：`pending_permission_requests` → `respond_permission` → 解锁 `ToolConfirming` 相位
   - **用户问答旁路**：`submit_user_answers` → 回答挂起的 user-question 工具
   - **后台回投旁路**：`deliver_background_result` / `deliver_thread_goal` → 注入到当前 / 精确 turn 的下一轮

5. **结算同步点（[runtime.rs:1038-1051](../../src/crates/execution/agent-runtime/src/runtime.rs#L1038-L1051)）**：
   `wait_for_turn_settlement()` 是阻塞调用，调用方用它等 turn 进入
   `TurnOutcome::{Completed, Cancelled, Failed}`，并取得 Runtime 确认的终态、最终回答和结束原因；
   transcript 与 usage 仍按需通过各自的只读接口获取。

6. **post-call hooks（[post_call_hooks.rs:156-172](../../src/crates/execution/agent-runtime/src/post_call_hooks.rs#L156-L172)）**：
   仅在「成功工具调用后」（`SuccessfulToolPostCall`）触发，当前唯一具体钩子是
   `DeepReviewSharedContextToolUse`，由 `hook_registry` 驱动，不影响主线生命周期但附加观测副作用。

7. **Port 为可选注入**：除 `submission` 外所有 Port 都是 `Option`；缺端口时调用对应接口会返回
   `RuntimeError::Missing*`（如 `MissingDialogTurnPort`、`MissingCancellationPort`），因此同一 `AgentRuntime`
   可在不同产品 profile 下裁剪能力。

8. **事件双通道**：`publish_event()` 会同时写入注入的 `RuntimeServices` 事件 sink（跨进程/跨产品广播，
   `RuntimeEventType` 如 `TurnStarted`/`TurnCompleted`）与进程内 `AgentEventStream`（`run()` 返回的
   `AgentRunHandle.events` 供调用方 drain 快照），二者互不阻塞。

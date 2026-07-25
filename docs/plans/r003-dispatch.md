# R-003 派发指令

> 设计文档：[r003-design.md](r003-design.md) | 需求文档：[r003-requirements.md](r003-requirements.md)
>
> 执行者：姬码锋 | 审批者：姬梦清

---

## 任务总览

| R-ID | 简述 | Phase | 依赖 |
|------|------|-------|------|
| R-003-001 | 新增 `SubagentCompletionStatus` 枚举 + `AgenticEvent::SubagentTurnCompleted` 变体（5处编辑） | Phase 1 | 无 |
| R-003-003 | 修复 128K→1M 上下文窗口（3处） | Phase 1 | 无 |
| R-003-002 | coordinator.rs 注入 dialog turn + 发射事件（双闭包） | Phase 2 | Phase 1 |
| R-003-004 | 全量编译+测试验证 | Phase 3 前半 | Phase 1, Phase 2 |
| R-003-005 | 端到端行为验证 | Phase 3 后半 | R-003-004 |

**执行顺序**：Phase 1（并行）→ Phase 2（串行）→ Phase 3（顺序验证：004 → 005）

---

## Phase 1（并行执行）

### R-003-001：新增 `SubagentCompletionStatus` 枚举 + `AgenticEvent::SubagentTurnCompleted` 变体

**姬码锋执行任务**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\contracts\events\src\agentic.rs` | 5处编辑 |

#### 先读取

```
先 Read agentic.rs L1-75, L125-165, L551-616, L890-920 获取精确上下文。
```

#### 改动步骤

**编辑1**：在 `DeepReviewQueueState` struct（L67 `}`）之后、`AgenticEvent` enum（L69 `#[derive(...)] #[serde(tag = "type")] pub enum AgenticEvent {`）之前，插入新枚举定义。

old_string:
```
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgenticEvent {
```

new_string:
```
}

/// Normalized completion status for background subagent turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentCompletionStatus {
    Completed,
    Failed,
    Cancelled,
    PartialTimeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgenticEvent {
```

---

**编辑2**：在 `SubagentSessionLinked` 变体后（L139 `},` 之后，L141 `DialogTurnCompleted` 之前）插入新变体。

old_string:
```
        /// Resolved model selector stored on the child session.
        #[serde(skip_serializing_if = "Option::is_none")]
        model_id: Option<String>,
    },

    DialogTurnCompleted {
```

new_string:
```
        /// Resolved model selector stored on the child session.
        #[serde(skip_serializing_if = "Option::is_none")]
        model_id: Option<String>,
    },

    /// Emitted when a background subagent turn completes.
    /// session_id is the subagent (child) session.
    SubagentTurnCompleted {
        /// Subagent (child) session id.
        session_id: String,
        /// The dialog turn id of the subagent execution.
        subagent_dialog_turn_id: String,
        /// Parent session that launched the subagent.
        parent_session_id: String,
        /// Parent dialog turn during which the subagent was dispatched.
        parent_dialog_turn_id: String,
        /// Parent tool call that spawned the subagent.
        parent_tool_call_id: String,
        /// Subagent logical agent type (e.g. "GeneralPurpose").
        #[serde(skip_serializing_if = "Option::is_none")]
        agent_type: Option<String>,
        /// Subagent completion status.
        status: SubagentCompletionStatus,
        /// Subagent output text (the AI response), if successful.
        #[serde(skip_serializing_if = "Option::is_none")]
        output_text: Option<String>,
    },

    DialogTurnCompleted {
```

---

**编辑3**：在 `session_id()` match 臂中，`SubagentSessionLinked` 后插入新匹配行。

old_string:
```
            | Self::SubagentSessionLinked { session_id, .. }
            | Self::DialogTurnCompleted { session_id, .. }
```

new_string:
```
            | Self::SubagentSessionLinked { session_id, .. }
            | Self::SubagentTurnCompleted { session_id, .. }
            | Self::DialogTurnCompleted { session_id, .. }
```

---

**编辑4**：在 `default_priority()` match 臂中，把 `SubagentTurnCompleted` 加入 **Normal** 优先级组（在 `DialogTurnCompleted` 之后）。

old_string:
```
            | Self::DialogTurnCompleted { .. }
            | Self::ContextCompressionStarted { .. }
```

new_string:
```
            | Self::DialogTurnCompleted { .. }
            | Self::SubagentTurnCompleted { .. }
            | Self::ContextCompressionStarted { .. }
```

---

**编辑5**：在文件末尾测试模块 `}` 之前（L919 `}` 之后，L920 `}` 之前），添加新的序列化测试。

old_string:
```
        assert_eq!(serialized["model_id"], "fast");
    }
}
```

new_string:
```
        assert_eq!(serialized["model_id"], "fast");
    }

    #[test]
    fn subagent_turn_completed_serializes_stable_contract() {
        let event = AgenticEvent::SubagentTurnCompleted {
            session_id: "child-session".to_string(),
            subagent_dialog_turn_id: "child-turn-1".to_string(),
            parent_session_id: "parent-session".to_string(),
            parent_dialog_turn_id: "turn-1".to_string(),
            parent_tool_call_id: "tool-1".to_string(),
            agent_type: Some("GeneralPurpose".to_string()),
            status: SubagentCompletionStatus::Completed,
            output_text: Some("Task completed successfully.".to_string()),
        };

        assert_eq!(event.session_id(), Some("child-session"));
        assert_eq!(event.default_priority(), AgenticEventPriority::Normal);

        let serialized = serde_json::to_value(event).expect("serialize event");
        assert_eq!(serialized["type"], "SubagentTurnCompleted");
        assert_eq!(serialized["session_id"], "child-session");
        assert_eq!(serialized["subagent_dialog_turn_id"], "child-turn-1");
        assert_eq!(serialized["parent_session_id"], "parent-session");
        assert_eq!(serialized["parent_dialog_turn_id"], "turn-1");
        assert_eq!(serialized["parent_tool_call_id"], "tool-1");
        assert_eq!(serialized["agent_type"], "GeneralPurpose");
        assert_eq!(serialized["status"], "completed");
        assert_eq!(serialized["output_text"], "Task completed successfully.");

        // Verify that optional output_text / agent_type are omitted when None.
        let event_no_output = AgenticEvent::SubagentTurnCompleted {
            session_id: "s".to_string(),
            subagent_dialog_turn_id: "t".to_string(),
            parent_session_id: "ps".to_string(),
            parent_dialog_turn_id: "pt".to_string(),
            parent_tool_call_id: "tc".to_string(),
            agent_type: None,
            status: SubagentCompletionStatus::Failed,
            output_text: None,
        };
        let serialized_no = serde_json::to_value(event_no_output).expect("serialize event");
        assert!(!serialized_no.as_object().unwrap().contains_key("output_text"));
        assert!(!serialized_no.as_object().unwrap().contains_key("agent_type"));
    }

    #[test]
    fn subagent_completion_status_serializes_snake_case() {
        use SubagentCompletionStatus::*;
        let cases = vec![
            (Completed, "completed"),
            (Failed, "failed"),
            (Cancelled, "cancelled"),
            (PartialTimeout, "partial_timeout"),
        ];
        for (variant, expected) in cases {
            let v = serde_json::to_value(variant).expect("serialize");
            assert_eq!(v, serde_json::Value::String(expected.to_string()),
                "variant {:?} should serialize to \"{}\"", variant, expected);
        }
    }
}
```

#### 验证命令

```bash
cargo test -p bitfun-events
```


### R-003-003：128K→1M 上下文窗口修复

**姬码锋执行任务**

#### 文件列表

| 文件 | 行号 | 操作 |
|------|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\coordination\coordinator.rs` | L1569-1570 | 插入 `refresh_session_context_window` 调用 |
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\execution\agent-runtime\src\session.rs` | L187 | `128128` → `1_048_576` |
| `E:\finance-trading\lvpa\software\taiji-quant\src\apps\desktop\src\api\agentic_api.rs` | L1270 | `unwrap_or(128128)` → `unwrap_or(1_048_576)` |

#### 先读取

```
先 Read coordinator.rs L1520-1572, session.rs L184-202, agentic_api.rs L1267-1283 获取精确上下文。
```

#### 改动步骤

**编辑1（coordinator.rs）**：在 `emit_event(SessionCreated)` 之后、`Ok(session)` 之前，插入上下文窗口刷新调用。

old_string:
```
            remote_ssh_host: session.config.remote_ssh_host.clone(),
        })
        .await;
        Ok(session)
    }

    /// Create a hidden internal subagent session
```

new_string:
```
            remote_ssh_host: session.config.remote_ssh_host.clone(),
        })
        .await;

        // Re-apply the 1M context-window default after session creation so the
        // persisted config always carries the current default (the config builder
        // default is 128K and must not overwrite the 1M target).
        self.session_manager
            .refresh_session_context_window(&session.session_id)
            .await?;

        Ok(session)
    }

    /// Create a hidden internal subagent session
```

---

**编辑2（session.rs）**：修改 `SessionConfig::default()` 中的默认值。

old_string:
```
            max_context_tokens: 128128,
```

new_string:
```
            max_context_tokens: 1_048_576,
```

---

**编辑3（agentic_api.rs）**：修改 API 层的 fallback 默认值。

old_string:
```
            max_context_tokens: c.max_context_tokens.unwrap_or(128128),
```

new_string:
```
            max_context_tokens: c.max_context_tokens.unwrap_or(1_048_576),
```

#### 验证命令

```bash
cargo check -p bitfun-agent-runtime
cargo check -p bitfun-core
cargo check -p bitfun-desktop
cargo test -p bitfun-agent-runtime
```


---

## Phase 2（串行执行，依赖 Phase 1 全部完成）

### R-003-002：coordinator.rs 注入 dialog turn + 发射事件

**姬码锋执行任务**

⚠️ **必须 Phase 1 全部完成后才能开始此任务。**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\coordination\scheduler.rs` | 1处编辑：注册 reminder kind |
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\coordination\coordinator.rs` | 4处编辑：imports + 调度路径闭包前后 + 直接执行路径闭包前后 |

#### 先读取

```
先 Read scheduler.rs L2095-2114, coordinator.rs L1-82, L7288-7326, L7360-7386 获取精确上下文。
```

#### 步骤1：scheduler.rs — 注册 `task_subagent_result` reminder kind

**文件**：`E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\coordination\scheduler.rs`

old_string:
```
                "session_message_request" => InternalReminderKind::SessionMessageRequest,
                "scheduled_job" => InternalReminderKind::ScheduledJob,
```

new_string:
```
                "session_message_request" => InternalReminderKind::SessionMessageRequest,
                "scheduled_job" => InternalReminderKind::ScheduledJob,
                "task_subagent_result" => InternalReminderKind::BackgroundResult,
```

---

#### 步骤2：coordinator.rs — 添加 imports

**文件**：`E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\coordination\coordinator.rs`

在 `bitfun_runtime_ports` 导入块中添加 4 个新类型。

old_string:
```
use bitfun_runtime_ports::{
    AgentSessionWorkspaceBinding, AgentThreadGoalDeliveryKind, AgentThreadGoalDeliveryRequest,
    DelegationPolicy, PermissionDelegationContext, PermissionRuntimeCeiling, RemoteExecPort,
    SessionStoragePathRequest, SessionStorePort, SubagentContextMode, TerminalPort, ThreadGoal,
    ThreadGoalContinuationPlan, ThreadGoalStatus,
};
```

new_string:
```
use bitfun_runtime_ports::{
    AgentDialogPrependedReminder, AgentDialogTurnRequest,
    AgentSessionWorkspaceBinding, AgentSubmissionSource,
    AgentThreadGoalDeliveryKind, AgentThreadGoalDeliveryRequest,
    DelegationPolicy, DialogQueuePriority, PermissionDelegationContext,
    PermissionRuntimeCeiling, RemoteExecPort,
    SessionStoragePathRequest, SessionStorePort, SubagentContextMode, TerminalPort, ThreadGoal,
    ThreadGoalContinuationPlan, ThreadGoalStatus,
};
```

⚠️ **额外导入**：闭包中引用了 `SubagentCompletionStatus`（来自 `bitfun_events::agentic`）。确认 coordinator.rs 的 events 导入块已包含 `AgenticEvent`。若未导入 `SubagentCompletionStatus`，需在 events 导入行添加：

```
use crate::agentic::events::{AgenticEvent, ..., SubagentCompletionStatus};
```

---

#### 步骤3：coordinator.rs — 调度路径闭包（L7288-7326）

**上下文说明**：此闭包位于 `if let Some(scheduler) = get_global_scheduler()` 分支内，缩进为 12 空格（3 级）。此处 `self` 可用但闭包不捕获 `self`，需单独 clone 所需变量。

**编辑3a**：在闭包 `tokio::spawn` 前添加 clone 变量。

old_string:
```
            let background_subagent_tasks = self.background_subagent_tasks.clone();
            let background_subagent_outcomes = self.background_subagent_outcomes.clone();

            tokio::spawn(async move {
```

new_string:
```
            let background_subagent_tasks = self.background_subagent_tasks.clone();
            let background_subagent_outcomes = self.background_subagent_outcomes.clone();
            let event_queue_for_spawn = self.event_queue.clone();
            let agent_type_for_spawn = request.agent_type.clone();
            let parent_session_id_for_spawn = subagent_parent_info.session_id.clone();
            let parent_dialog_turn_id_for_spawn = subagent_parent_info.dialog_turn_id.clone();
            let parent_tool_call_id_for_spawn = subagent_parent_info.tool_call_id.clone();
            let subagent_session_id_for_spawn = subagent_session_id.clone();
            let subagent_dialog_turn_id_for_spawn = subagent_dialog_turn_id.clone();

            tokio::spawn(async move {
```

---

**编辑3b**：在 `.complete()` 之后、`.remove()` 之前，插入事件发射 + dialog turn 提交。

old_string:
```
                background_subagent_outcomes
                    .complete(task_pk, result.as_ref())
                    .await;
                background_subagent_tasks.remove(&task_pk);
            });
```

new_string:
```
                background_subagent_outcomes
                    .complete(task_pk, result.as_ref())
                    .await;

                // ── Emit SubagentTurnCompleted event ──
                let output_text = result.as_ref().ok().map(|sr| sr.text.clone());
                let status = match result.as_ref() {
                    Ok(sr) => match sr.status {
                        SubagentResultStatus::Completed => SubagentCompletionStatus::Completed,
                        SubagentResultStatus::PartialTimeout => {
                            SubagentCompletionStatus::PartialTimeout
                        }
                        SubagentResultStatus::Cancelled => SubagentCompletionStatus::Cancelled,
                    },
                    Err(_) => SubagentCompletionStatus::Failed,
                };
                let _ = event_queue_for_spawn
                    .enqueue(
                        AgenticEvent::SubagentTurnCompleted {
                            session_id: subagent_session_id_for_spawn.clone(),
                            subagent_dialog_turn_id: subagent_dialog_turn_id_for_spawn.clone(),
                            parent_session_id: parent_session_id_for_spawn.clone(),
                            parent_dialog_turn_id: parent_dialog_turn_id_for_spawn.clone(),
                            parent_tool_call_id: parent_tool_call_id_for_spawn.clone(),
                            agent_type: Some(agent_type_for_spawn.clone()),
                            status,
                            output_text: output_text.clone(),
                        },
                        Some(EventPriority::Normal),
                    )
                    .await;

                // ── Submit dialog turn to parent session ──
                if let Some(scheduler) = get_global_scheduler() {
                    let reminder = AgentDialogPrependedReminder {
                        kind: "task_subagent_result".to_string(),
                        text: output_text.unwrap_or_default(),
                    };
                    let turn_request = AgentDialogTurnRequest {
                        session_id: parent_session_id_for_spawn.clone(),
                        message: String::new(),
                        original_message: None,
                        turn_id: None,
                        agent_type: agent_type_for_spawn.clone(),
                        workspace_path: None,
                        remote_connection_id: None,
                        remote_ssh_host: None,
                        policy: DialogSubmissionPolicy::new(
                            AgentSubmissionSource::AgentSession,
                            DialogQueuePriority::Normal,
                        ),
                        reply_route: None,
                        prepended_reminders: vec![reminder],
                        attachments: Vec::new(),
                        metadata: serde_json::Map::new(),
                    };
                    if let Err(e) = scheduler
                        .submit_agent_dialog_turn_reject_if_busy(turn_request)
                        .await
                    {
                        warn!(
                            "Failed to submit subagent result dialog turn: session_id={}, error={}",
                            parent_session_id_for_spawn, e
                        );
                    }
                }

                background_subagent_tasks.remove(&task_pk);
            });
```

---

#### 步骤4：coordinator.rs — 直接执行路径闭包（L7360-7386）

**上下文说明**：此闭包位于 `else` 分支（无全局 scheduler），缩进为 8 空格（2 级）。此处 `request` 会被 move 进 `execute_hidden_subagent_internal`，必须在闭包前 clone。

**编辑4a**：在闭包 `tokio::spawn` 前添加 clone 变量。

old_string:
```
        let background_subagent_tasks = self.background_subagent_tasks.clone();
        let background_subagent_outcomes = self.background_subagent_outcomes.clone();

        tokio::spawn(async move {
```

new_string:
```
        let background_subagent_tasks = self.background_subagent_tasks.clone();
        let background_subagent_outcomes = self.background_subagent_outcomes.clone();
        let event_queue_for_spawn = self.event_queue.clone();
        let agent_type_for_spawn = request.agent_type.clone();
        let parent_session_id_for_spawn = subagent_parent_info.session_id.clone();
        let parent_dialog_turn_id_for_spawn = subagent_parent_info.dialog_turn_id.clone();
        let parent_tool_call_id_for_spawn = subagent_parent_info.tool_call_id.clone();
        let subagent_session_id_for_spawn = subagent_session_id.clone();
        let subagent_dialog_turn_id_for_spawn = subagent_dialog_turn_id.clone();

        tokio::spawn(async move {
```

---

**编辑4b**：在 `.complete()` 之后、`.remove()` 之前，插入事件发射 + dialog turn 提交（与步骤3b逻辑相同，缩进为 12 空格而非 16 空格）。

old_string:
```
            background_subagent_outcomes
                .complete(task_pk, result.as_ref())
                .await;
            background_subagent_tasks.remove(&task_pk);
        });
```

new_string:
```
            background_subagent_outcomes
                .complete(task_pk, result.as_ref())
                .await;

            // ── Emit SubagentTurnCompleted event ──
            let output_text = result.as_ref().ok().map(|sr| sr.text.clone());
            let status = match result.as_ref() {
                Ok(sr) => match sr.status {
                    SubagentResultStatus::Completed => SubagentCompletionStatus::Completed,
                    SubagentResultStatus::PartialTimeout => {
                        SubagentCompletionStatus::PartialTimeout
                    }
                    SubagentResultStatus::Cancelled => SubagentCompletionStatus::Cancelled,
                },
                Err(_) => SubagentCompletionStatus::Failed,
            };
            let _ = event_queue_for_spawn
                .enqueue(
                    AgenticEvent::SubagentTurnCompleted {
                        session_id: subagent_session_id_for_spawn.clone(),
                        subagent_dialog_turn_id: subagent_dialog_turn_id_for_spawn.clone(),
                        parent_session_id: parent_session_id_for_spawn.clone(),
                        parent_dialog_turn_id: parent_dialog_turn_id_for_spawn.clone(),
                        parent_tool_call_id: parent_tool_call_id_for_spawn.clone(),
                        agent_type: Some(agent_type_for_spawn.clone()),
                        status,
                        output_text: output_text.clone(),
                    },
                    Some(EventPriority::Normal),
                )
                .await;

            // ── Submit dialog turn to parent session ──
            if let Some(scheduler) = get_global_scheduler() {
                let reminder = AgentDialogPrependedReminder {
                    kind: "task_subagent_result".to_string(),
                    text: output_text.unwrap_or_default(),
                };
                let turn_request = AgentDialogTurnRequest {
                    session_id: parent_session_id_for_spawn.clone(),
                    message: String::new(),
                    original_message: None,
                    turn_id: None,
                    agent_type: agent_type_for_spawn.clone(),
                    workspace_path: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    policy: DialogSubmissionPolicy::new(
                        AgentSubmissionSource::AgentSession,
                        DialogQueuePriority::Normal,
                    ),
                    reply_route: None,
                    prepended_reminders: vec![reminder],
                    attachments: Vec::new(),
                    metadata: serde_json::Map::new(),
                };
                if let Err(e) = scheduler
                    .submit_agent_dialog_turn_reject_if_busy(turn_request)
                    .await
                {
                    warn!(
                        "Failed to submit subagent result dialog turn: session_id={}, error={}",
                        parent_session_id_for_spawn, e
                    );
                }
            }

            background_subagent_tasks.remove(&task_pk);
        });
```

#### 验证命令

```bash
cargo check -p bitfun-core
cargo test -p bitfun-core
```


---

## Phase 3（验证，依赖 Phase 1 + Phase 2 全部完成）

### R-003-004：全量编译+测试验证

**姬码锋执行任务**

#### 验证命令（按顺序执行）

```bash
# 1. 全量编译检查
cargo check --workspace

# 2. 事件 crate 测试（含 SubagentCompletionStatus + SubagentTurnCompleted）
cargo test -p bitfun-events

# 3. 核心 crate 测试（含 coordinator 变更）
cargo test -p bitfun-core

# 4. agent-runtime 测试（含 session config 变更）
cargo test -p bitfun-agent-runtime

# 5. desktop crate 编译 + 测试
cargo check -p bitfun-desktop
cargo test -p bitfun-desktop

# 6. 前端类型检查
pnpm run type-check:web

# 7. i18n 合约测试（若无 i18n 变更可跳过，但建议跑一次确认无回归）
pnpm run i18n:contract:test
```

#### 验收标准

| R-ID | 验收标准 |
|------|----------|
| R-003-001 | `cargo test -p bitfun-events` 全绿，`subagent_turn_completed_serializes_stable_contract` + `subagent_completion_status_serializes_snake_case` 通过 |
| R-003-002 | `cargo check -p bitfun-core` 通过，`cargo test -p bitfun-core` 无新增失败 |
| R-003-003 | `cargo check --workspace` 通过，3处 `128128` 已替换为 `1_048_576` |
| R-003-004 | 上述 7 条命令全部通过 |

---

### R-003-005：端到端行为验证

**姬码锋执行任务**

⚠️ **依赖 R-003-004 全部通过后才能开始。**

#### 验证目标

在 Phase 3 前半（R-003-004）确认编译+测试无回归后，进行以下行为级验证：

#### 验证1：检查 `session.rs` 默认上下文窗口

```bash
# 验证 SessionConfig::default() 的 max_context_tokens 已改为 1_048_576
cargo test -p bitfun-agent-runtime -- session --nocapture
```

> 预期：`SessionConfig::default()` 的 `max_context_tokens` 字段为 `1_048_576`（非 `128128`）。

#### 验证2：检查 `agentic_api.rs` fallback 默认值

```bash
# 验证 API 层的 unwrap_or 默认值已改
cargo test -p bitfun-desktop -- agentic --nocapture
```

> 预期：编译通过，无 `unwrap_or(128128)` 残留。

#### 验证3：检查 coordinator.rs `refresh_session_context_window` 调用

```bash
# 编译检查确保 refresh 调用签名正确
cargo check -p bitfun-core
```

> 预期：编译通过。`self.session_manager.refresh_session_context_window(&session_id)` 方法存在且可调用。

#### 验证4：检查 `SubagentTurnCompleted` 事件结构完整性

```bash
cargo test -p bitfun-events -- subagent_turn_completed --nocapture
cargo test -p bitfun-events -- subagent_completion_status --nocapture
```

> 预期：
> - `SubagentTurnCompleted` 序列化后 `type` 字段 = `"SubagentTurnCompleted"`
> - 8 个字段全部正确序列化（session_id, subagent_dialog_turn_id, parent_session_id, parent_dialog_turn_id, parent_tool_call_id, agent_type, status, output_text）
> - `status` 字段序列化为 snake_case（`"completed"`, `"failed"`, `"cancelled"`, `"partial_timeout"`）
> - `agent_type` 为 None 时字段被省略
> - `output_text` 为 None 时字段被省略

#### 验证5：检查 scheduler.rs 的 `BackgroundResult` reminder kind 映射

```bash
cargo test -p bitfun-core -- scheduler --nocapture
```

> 预期：scheduler 测试中 `"task_subagent_result"` 能正确映射到 `InternalReminderKind::BackgroundResult`，无 `"unsupported agent dialog prepended reminder kind"` 错误。

#### 验证6：检查 coordinator.rs 双闭包新增变量捕获

对 coordinator.rs 中两处 `tokio::spawn` 闭包进行手动代码审查：

- **调度路径闭包**（if let Some(scheduler) 分支，缩进12空格）：
  1. `tokio::spawn` 之前有 7 行 clone（event_queue, agent_type, parent_*, subagent_*）
  2. `.complete()` 调用之后立即有事件发射代码块（`event_queue_for_spawn.enqueue(...)`）
  3. 事件发射之后有 dialog turn 提交代码块（`get_global_scheduler() → submit_agent_dialog_turn_reject_if_busy(...)`）
  4. 两者之后才是 `background_subagent_tasks.remove(&task_pk)`

- **直接执行路径闭包**（else 分支，缩进8空格）：
  1. 同上 7 行 clone
  2. 同上事件发射 + dialog turn 提交模式
  3. 注意 `request` 变量在 `tokio::spawn` 之后被 move 进 `execute_hidden_subagent_internal`，clone 必须在 spawn 之前

> 手动验证方法：Read coordinator.rs 中两处闭包的完整代码块，确认结构正确。

#### 最终验收标准

| 验证项 | 通过条件 |
|--------|----------|
| 验证1 | session.rs 默认值 = 1_048_576 |
| 验证2 | agentic_api.rs unwrap_or(1_048_576) 编译通过 |
| 验证3 | coordinator.rs refresh 调用编译通过 |
| 验证4 | SubagentTurnCompleted 8字段序列化正确，status 为 snake_case |
| 验证5 | scheduler 中 task_subagent_result → BackgroundResult 映射有效 |
| 验证6 | 双闭包代码结构审查通过（手动检查） |

> 全部 6 项通过后，R-003 全量完成。

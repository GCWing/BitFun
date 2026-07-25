# R-004 派发指令

> 设计文档：[r004-design.md](r004-design.md) | 需求文档：[r004-requirements.md](r004-requirements.md)
>
> 执行者：姬码锋 | 审批者：姬梦情

---

## 任务总览

| R-ID | 简述 | Wave | 依赖 |
|------|------|------|------|
| R001 | `build_subagent_session_relationship` depth 从 `None` 改为父深度+1 | Wave 1 | 无 |
| R002 | `session_control_tool.rs` Create handler `depth: Some(1)` → 从父继承 | Wave 1 | 无 |
| R004 | `frontend_projection.rs` `SubagentTurnCompleted` 不再丢弃 | Wave 1 | 无 |
| R005 | `deriveSessionRelationshipFromMetadata` 返回 depth + 前端 TS `SessionMetadata`/`SessionRelationship` 类型加 depth | Wave 1 | 无 |
| R006 | 前端 `Session` TS 类型加 `depth`/`children` 字段 | Wave 1 | 无 |
| R009a | `task/schema.rs` description 加互引 | Wave 1 | 无 |
| R009b | `session_control_tool.rs` description 加互引 | Wave 1 | 无 |
| R009c | `session_message_tool.rs` description 加互引 | Wave 1 | 无 |
| R009d | `session_history_tool.rs` description 加互引 | Wave 1 | 无 |
| R011 | `session_control_tool.rs` Cancel handler 跳过 `ensure_session_exists` list 预检查 | Wave 1 | 无 |
| R003 | coordinator.rs + session_control_tool.rs create/spawn 后注册 `SessionTreeManager` | Wave 2 | R001, R002 |
| R007 | `session_control_tool.rs` list 输出树形 JSON | Wave 2 | R003 |
| R008 | 前端 session 列表组件树形渲染 | Wave 3 | R006, R007 |
| R010 | `agent_type` 合并 | Wave 4 | 无 |
| R012 | Delete 级联 | Wave 4 | R003 |

**执行顺序**：Wave 1（全并行）→ Wave 2（串行，依赖 Wave 1）→ Wave 3（前端渲染，依赖 Wave 1+2）→ Wave 4（P1 + 验证）

---

## Wave 1（全并行执行，无文件冲突）

### R001：`build_subagent_session_relationship` depth 从 `None` → `parent_depth + 1`

**姬码锋执行任务**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\coordination\coordinator.rs` | 3处编辑 |

#### 先读取

```
先 Read coordinator.rs L320-345, L5295-5310, L9445-9463 获取精确上下文。
```

#### 改动步骤

**编辑1**：修改函数签名，新增 `parent_depth: Option<u32>` 参数，把 `depth: None` 改为 `depth: parent_depth.map(|d| d + 1)`。

old_string:
```rust
fn build_subagent_session_relationship(
    parent_info: Option<&SubagentParentInfo>,
    agent_type: &str,
    continuation_policy: SessionContinuationPolicy,
) -> SessionRelationship {
    SessionRelationship {
        kind: Some(SessionRelationshipKind::Subagent),
        parent_session_id: parent_info.map(|info| info.session_id.clone()),
        parent_request_id: None,
        parent_dialog_turn_id: parent_info.map(|info| info.dialog_turn_id.clone()),
        parent_turn_index: None,
        parent_tool_call_id: parent_info.map(|info| info.tool_call_id.clone()),
        subagent_type: Some(agent_type.to_string()),
        continuation_policy: Some(continuation_policy),
        depth: None,
    }
}
```

new_string:
```rust
fn build_subagent_session_relationship(
    parent_info: Option<&SubagentParentInfo>,
    agent_type: &str,
    continuation_policy: SessionContinuationPolicy,
    parent_depth: Option<u32>,
) -> SessionRelationship {
    SessionRelationship {
        kind: Some(SessionRelationshipKind::Subagent),
        parent_session_id: parent_info.map(|info| info.session_id.clone()),
        parent_request_id: None,
        parent_dialog_turn_id: parent_info.map(|info| info.dialog_turn_id.clone()),
        parent_turn_index: None,
        parent_tool_call_id: parent_info.map(|info| info.tool_call_id.clone()),
        subagent_type: Some(agent_type.to_string()),
        continuation_policy: Some(continuation_policy),
        depth: parent_depth.map(|d| d + 1),
    }
}
```

---

**编辑2**：更新调用点（L5305），传入 `parent_depth`。

old_string:
```rust
        if let Err(error) = self
            .session_manager
            .persist_session_lineage(
                &session_id,
                build_subagent_session_relationship(
                    subagent_parent_info.as_ref(),
                    &logical_agent_type,
                    continuation_policy,
                ),
            )
            .await
```

new_string:
```rust
        if let Err(error) = self
            .session_manager
            .persist_session_lineage(
                &session_id,
                build_subagent_session_relationship(
                    subagent_parent_info.as_ref(),
                    &logical_agent_type,
                    continuation_policy,
                    subagent_parent_info.as_ref().and_then(|info| info.depth),
                ),
            )
            .await
```

> ⚠️ 前提：`SubagentParentInfo` 已有 `depth: Option<u32>` 字段。若尚无，需先在 `SubagentParentInfo` struct 中添加 `depth: Option<u32>`。

---

**编辑3**：更新测试调用点（L9453），传入 `parent_depth: None`（测试无父深度）。

old_string:
```rust
        let relationship = build_subagent_session_relationship(
            None,
            &logical_type,
            SessionContinuationPolicy::FreshOnly,
        );
```

new_string:
```rust
        let relationship = build_subagent_session_relationship(
            None,
            &logical_type,
            SessionContinuationPolicy::FreshOnly,
            None,
        );
```

#### 验证命令

```bash
cargo check -p bitfun-core
cargo test -p bitfun-core -- coordinator
```

---

### R002：`session_control_tool.rs` Create handler `depth: Some(1)` → 从父继承

**姬码锋执行任务**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\tools\implementations\session_control_tool.rs` | 1处编辑 |

#### 先读取

```
先 Read session_control_tool.rs L390-415 获取精确上下文。
```

#### 改动步骤

**编辑1**：在 Create handler 的关系写入块中，把 `depth: Some(1u32)` 改为从父 session 的 relationship 中查询 depth 并 +1。

当前代码（L394-412）：
```rust
                // --- R-001: 写入 SessionRelationship ---
                {
                    use bitfun_services_core::session::types::{
                        SessionRelationship, SessionRelationshipKind,
                    };
                    let parent_session_id = context.session_id.clone();
                    // Determine parent depth: 0 if parent has no relationship, otherwise parent_depth + 1
                    let child_depth = 1u32; // default for direct children
                    let relationship = SessionRelationship {
                        kind: Some(SessionRelationshipKind::Subagent),
                        parent_session_id,
                        depth: Some(child_depth),
                        ..Default::default()
                    };
                    let _ = coordinator
                        .session_manager
                        .persist_session_lineage(&created_session_id, relationship)
                        .await;
                }
```

需要替换为从父 session metadata 中读取 depth 并计算 child_depth = parent_depth + 1。

old_string:
```rust
                // --- R-001: 写入 SessionRelationship ---
                {
                    use bitfun_services_core::session::types::{
                        SessionRelationship, SessionRelationshipKind,
                    };
                    let parent_session_id = context.session_id.clone();
                    // Determine parent depth: 0 if parent has no relationship, otherwise parent_depth + 1
                    let child_depth = 1u32; // default for direct children
                    let relationship = SessionRelationship {
                        kind: Some(SessionRelationshipKind::Subagent),
                        parent_session_id,
                        depth: Some(child_depth),
                        ..Default::default()
                    };
                    let _ = coordinator
                        .session_manager
                        .persist_session_lineage(&created_session_id, relationship)
                        .await;
                }
```

new_string:
```rust
                // --- R-001/R-002: 写入 SessionRelationship，depth 从父继承 ---
                {
                    use bitfun_services_core::session::types::{
                        SessionRelationship, SessionRelationshipKind,
                    };
                    let parent_session_id = context.session_id.clone();
                    // Read parent depth from persisted metadata, default 0 for root
                    let parent_depth = if let Some(ref pid) = parent_session_id {
                        coordinator
                            .session_manager
                            .load_session_metadata(
                                &std::path::PathBuf::from(&workspace.display_workspace),
                                pid,
                            )
                            .await
                            .ok()
                            .flatten()
                            .and_then(|m| m.relationship.and_then(|r| r.depth))
                            .unwrap_or(0u32)
                    } else {
                        0u32
                    };
                    let child_depth = parent_depth + 1;
                    let relationship = SessionRelationship {
                        kind: Some(SessionRelationshipKind::Subagent),
                        parent_session_id,
                        depth: Some(child_depth),
                        ..Default::default()
                    };
                    let _ = coordinator
                        .session_manager
                        .persist_session_lineage(&created_session_id, relationship)
                        .await;
                }
```

> ⚠️ **实现注意**：需要一种机制从 `coordinator.session_manager` 读取父 session 的 metadata 以获取其 `relationship.depth`。目前 `session_manager` 没有公开的"按 session_id 读取单个 metadata"方法，可能需要新增或使用 `list_session_metadata_including_internal` + 过滤。若调用成本过高，可考虑在 coordinator 内存中维护一个 `session_id → depth` 的缓存。

#### 验证命令

```bash
cargo check -p bitfun-core
cargo test -p bitfun-core -- session_control
```

---

### R004：`frontend_projection.rs` `SubagentTurnCompleted` 不再丢弃

**姬码锋执行任务**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\contracts\events\src\frontend_projection.rs` | 1处编辑 |

#### 先读取

```
先 Read frontend_projection.rs L430-450 获取精确上下文。
```

#### 改动步骤

**编辑1**：将 `SubagentTurnCompleted { .. } => None` 改为投影为一个有意义的前端事件。

当前代码（L444）：
```rust
        AgenticEvent::SubagentTurnCompleted { .. } => None,
```

需要改为投影成 `FrontendEvent`。参考 `SubagentSessionLinked` 的投影模式，构造一个 `subagent-turn-completed` 事件。

old_string:
```rust
        AgenticEvent::SubagentTurnCompleted { .. } => None,
```

new_string:
```rust
        AgenticEvent::SubagentTurnCompleted {
            session_id,
            subagent_dialog_turn_id,
            parent_session_id,
            parent_dialog_turn_id,
            parent_tool_call_id,
            agent_type,
            status,
            output_text,
        } => Some(FrontendEvent {
            event_name: "agentic://subagent-turn-completed".to_string(),
            session_id: Some(session_id),
            dialog_turn_id: Some(subagent_dialog_turn_id),
            payload: {
                let mut p = serde_json::Map::new();
                p.insert("parentSessionId".to_string(), json!(parent_session_id));
                p.insert("parentDialogTurnId".to_string(), json!(parent_dialog_turn_id));
                p.insert("parentToolCallId".to_string(), json!(parent_tool_call_id));
                if let Some(at) = agent_type {
                    p.insert("agentType".to_string(), json!(at));
                }
                p.insert("status".to_string(), json!(status));
                if let Some(text) = output_text {
                    p.insert("outputText".to_string(), json!(text));
                }
                serde_json::Value::Object(p)
            },
        }),
```

#### 验证命令

```bash
cargo check -p bitfun-events
cargo test -p bitfun-events
```

---

### R005：`deriveSessionRelationshipFromMetadata` 返回 depth + 前端 TS 类型加 depth

**姬码锋执行任务**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\web-ui\src\shared\types\session-history.ts` | 加 `depth` 到 `SessionRelationship` |
| `E:\finance-trading\lvpa\software\taiji-quant\src\web-ui\src\flow_chat\utils\sessionMetadata.ts` | 修改 `deriveSessionRelationshipFromMetadata` 返回 depth |

#### 先读取

```
先 Read session-history.ts L14-22 获取 SessionRelationship 接口。
先 Read sessionMetadata.ts L35-45 获取 ResolvedSessionRelationship 接口。
先 Read sessionMetadata.ts L148-190 获取 deriveSessionRelationshipFromMetadata 函数。
```

#### 编辑1（session-history.ts L14-22）：`SessionRelationship` 加 `depth` 字段

old_string:
```typescript
export interface SessionRelationship {
  kind?: SessionRelationshipKind;
  parentSessionId?: string | null;
  parentRequestId?: string | null;
  parentDialogTurnId?: string | null;
  parentTurnIndex?: number | null;
  parentToolCallId?: string | null;
  subagentType?: string | null;
}
```

new_string:
```typescript
export interface SessionRelationship {
  kind?: SessionRelationshipKind;
  parentSessionId?: string | null;
  parentRequestId?: string | null;
  parentDialogTurnId?: string | null;
  parentTurnIndex?: number | null;
  parentToolCallId?: string | null;
  subagentType?: string | null;
  depth?: number | null;
}
```

#### 编辑2（sessionMetadata.ts L30-33）：`SessionRelationshipInput` 类型加 `'depth'`

`SessionRelationshipInput` 是 `Pick<Session, ...>`，控制 `normalizeSessionRelationship` 的输入类型。

old_string:
```typescript
type SessionRelationshipInput = Pick<
  Session,
  'sessionKind' | 'parentSessionId' | 'btwOrigin' | 'parentToolCallId' | 'subagentType'
>;
```

new_string:
```typescript
type SessionRelationshipInput = Pick<
  Session,
  'sessionKind' | 'parentSessionId' | 'btwOrigin' | 'parentToolCallId' | 'subagentType' | 'depth'
>;
```

#### 编辑3（sessionMetadata.ts L78-83）：`normalizeSessionRelationship` 返回类型加 `'depth'`

old_string:
```typescript
export function normalizeSessionRelationship(
  input?: Partial<SessionRelationshipInput> | null
): Pick<
  Session,
  'sessionKind' | 'parentSessionId' | 'btwOrigin' | 'parentToolCallId' | 'subagentType'
> {
```

new_string:
```typescript
export function normalizeSessionRelationship(
  input?: Partial<SessionRelationshipInput> | null
): Pick<
  Session,
  'sessionKind' | 'parentSessionId' | 'btwOrigin' | 'parentToolCallId' | 'subagentType' | 'depth'
> {
```

#### 编辑4（sessionMetadata.ts L97-104）：`normalizeSessionRelationship` 第一个返回对象（normal/miniapp）加 `depth`

old_string:
```typescript
  if (sessionKind === 'normal' || sessionKind === 'miniapp') {
    return {
      sessionKind,
      parentSessionId: undefined,
      btwOrigin: undefined,
      parentToolCallId: undefined,
      subagentType: undefined,
    };
  }
```

new_string:
```typescript
  if (sessionKind === 'normal' || sessionKind === 'miniapp') {
    return {
      sessionKind,
      parentSessionId: undefined,
      btwOrigin: undefined,
      parentToolCallId: undefined,
      subagentType: undefined,
      depth: undefined,
    };
  }
```

#### 编辑5（sessionMetadata.ts L114-120）：`normalizeSessionRelationship` 第二个返回对象加 `depth`

old_string:
```typescript
  return {
    sessionKind,
    parentSessionId,
    btwOrigin: origin,
    parentToolCallId,
    subagentType,
  };
}
```

new_string:
```typescript
  return {
    sessionKind,
    parentSessionId,
    btwOrigin: origin,
    parentToolCallId,
    subagentType,
    depth: input?.depth,
  };
}
```

#### 编辑6（sessionMetadata.ts L148-150）：`deriveSessionRelationshipFromMetadata` 返回类型加 `'depth'`

old_string:
```typescript
export function deriveSessionRelationshipFromMetadata(
  metadata?: Pick<SessionMetadata, 'customMetadata' | 'relationship'> | null
): Pick<
  Session,
  'sessionKind' | 'parentSessionId' | 'btwOrigin' | 'parentToolCallId' | 'subagentType'
> {
```

new_string:
```typescript
export function deriveSessionRelationshipFromMetadata(
  metadata?: Pick<SessionMetadata, 'customMetadata' | 'relationship'> | null
): Pick<
  Session,
  'sessionKind' | 'parentSessionId' | 'btwOrigin' | 'parentToolCallId' | 'subagentType' | 'depth'
> {
```

#### 编辑3（sessionMetadata.ts）：两处 `normalizeSessionRelationship({...})` 调用加 `depth`

在 relationship 分支（L155-167）的 `normalizeSessionRelationship` 调用中添加 `depth: relationship?.depth`。

old_string:
```typescript
    return normalizeSessionRelationship({
      sessionKind: relationshipKind,
      parentSessionId: normalizeString(relationship?.parentSessionId) ?? undefined,
      parentToolCallId: normalizeString(relationship?.parentToolCallId),
      subagentType: normalizeString(relationship?.subagentType),
      btwOrigin: {
        requestId: normalizeString(relationship?.parentRequestId),
        parentSessionId: normalizeString(relationship?.parentSessionId),
        parentDialogTurnId: normalizeString(relationship?.parentDialogTurnId),
        parentTurnIndex: normalizeTurnIndex(relationship?.parentTurnIndex),
      },
    });
```

new_string:
```typescript
    return normalizeSessionRelationship({
      sessionKind: relationshipKind,
      parentSessionId: normalizeString(relationship?.parentSessionId) ?? undefined,
      parentToolCallId: normalizeString(relationship?.parentToolCallId),
      subagentType: normalizeString(relationship?.subagentType),
      depth: relationship?.depth,
      btwOrigin: {
        requestId: normalizeString(relationship?.parentRequestId),
        parentSessionId: normalizeString(relationship?.parentSessionId),
        parentDialogTurnId: normalizeString(relationship?.parentDialogTurnId),
        parentTurnIndex: normalizeTurnIndex(relationship?.parentTurnIndex),
      },
    });
```

在 customMetadata 分支（L178-190）的 `normalizeSessionRelationship` 调用中同样添加 `depth`：

old_string:
```typescript
  return normalizeSessionRelationship({
    sessionKind,
    parentSessionId: customMetadata?.parentSessionId ?? undefined,
    parentToolCallId: normalizeString(customMetadata?.parentToolCallId),
    subagentType: normalizeString(customMetadata?.subagentType),
    btwOrigin:
      sessionKind !== 'normal'
        ? {
            requestId: normalizeString(customMetadata?.parentRequestId),
            parentSessionId: normalizeString(customMetadata?.parentSessionId),
            parentDialogTurnId: normalizeString(customMetadata?.parentDialogTurnId),
            parentTurnIndex: normalizeTurnIndex(customMetadata?.parentTurnIndex),
          }
        : undefined,
  });
```

new_string:
```typescript
  return normalizeSessionRelationship({
    sessionKind,
    parentSessionId: customMetadata?.parentSessionId ?? undefined,
    parentToolCallId: normalizeString(customMetadata?.parentToolCallId),
    subagentType: normalizeString(customMetadata?.subagentType),
    depth: undefined,
    btwOrigin:
      sessionKind !== 'normal'
        ? {
            requestId: normalizeString(customMetadata?.parentRequestId),
            parentSessionId: normalizeString(customMetadata?.parentSessionId),
            parentDialogTurnId: normalizeString(customMetadata?.parentDialogTurnId),
            parentTurnIndex: normalizeTurnIndex(customMetadata?.parentTurnIndex),
          }
        : undefined,
  });
```

#### 验证命令

```bash
pnpm run type-check:web
```

---

### R006：前端 `Session` TS 类型加 `depth`/`children` 字段

**姬码锋执行任务**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\web-ui\src\flow_chat\types\flow-chat.ts`（或相应的 Session 类型定义文件） | 添加 `depth` 和 `children` 字段 |

#### 先读取

```
先找到 Session 类型定义文件并读取其当前字段。
```

> 搜索：`grep -n "interface Session"` 或 `export interface Session` 在 `flow_chat/types/` 目录。

#### 改动步骤

在 `Session` 接口中添加：
```typescript
  /** Depth in session tree (root = 0, child = parent_depth + 1). */
  depth?: number;
  /** Child session IDs (for tree rendering in session list). */
  children?: string[];
```

#### 验证命令

```bash
pnpm run type-check:web
```

---

### R009a：`task/schema.rs` description 加互引

**姬码锋执行任务**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\tools\implementations\task\schema.rs` | 1处编辑 |

#### 先读取

```
先 Read schema.rs L1-50 获取精确上下文。
```

#### 改动步骤

**编辑1**：在 `description` 字段的 description 文本末尾，添加对 `SessionControl` 和 `SessionMessage` 的互引提示。

当前代码（L7-L12）：
```rust
        properties.insert(
            "description".to_string(),
            json!({
                "type": "string",
                "description": "A short (3-5 word) description of the task"
            }),
        );
```

old_string:
```rust
                "description": "A short (3-5 word) description of the task"
```

new_string:
```rust
                "description": "A short (3-5 word) description of the task. Use SessionControl (list) to discover sessions and SessionMessage to communicate with them."
```

#### 验证命令

```bash
cargo check -p bitfun-core
```

---

### R009b：`session_control_tool.rs` description 加互引

**姬码锋执行任务**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\tools\implementations\session_control_tool.rs` | 1处编辑 |

#### 先读取

```
先 Read session_control_tool.rs L254-273 获取精确上下文。
```

#### 改动步骤

**编辑1**：在 `description()` 的 Actions 列表末尾添加 Task 和 SessionMessage/SessionHistory 互引。

old_string:
```rust
    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Manage persisted workspace-scoped agent sessions.

Actions:
- "create": Create a new session. You may optionally provide session_name and agent_type.
- "cancel": Cancel the target session's currently running dialog turn. This does not delete the session or clear any queued messages that may still run later.
- "delete": Delete an existing session by session_id.
- "list": List all sessions.

Arguments:
- "workspace": Absolute workspace path. Required for create and list. Ignored for cancel and delete.
- "session_name": Only used by create. Defaults to "New Session".
- "agent_type": Only used by create. Defaults to "agentic".
  - "agentic": Coding-focused agent for implementation, debugging, and code changes.
  - "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
  - "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc.
- "session_id": Required for cancel and delete."#
                .to_string(),
        )
    }
```

new_string:
```rust
    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Manage persisted workspace-scoped agent sessions.

Actions:
- "create": Create a new session. You may optionally provide session_name and agent_type.
- "cancel": Cancel the target session's currently running dialog turn. This does not delete the session or clear any queued messages that may still run later.
- "delete": Delete an existing session by session_id.
- "list": List all sessions. Sessions are displayed in a tree structure showing parent-child relationships (created via Task tool).

Related tools:
- Use Task (spawn) to launch subagents that appear as children in the session tree.
- Use SessionMessage to send messages to existing sessions.
- Use SessionHistory to export a session transcript.

Arguments:
- "workspace": Absolute workspace path. Required for create and list. Ignored for cancel and delete.
- "session_name": Only used by create. Defaults to "New Session".
- "agent_type": Only used by create. Defaults to "agentic".
  - "agentic": Coding-focused agent for implementation, debugging, and code changes.
  - "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
  - "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc.
- "session_id": Required for cancel and delete."#
                .to_string(),
        )
    }
```

#### 验证命令

```bash
cargo check -p bitfun-core
```

---

### R009c：`session_message_tool.rs` description 加互引

**姬码锋执行任务**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\tools\implementations\session_message_tool.rs` | 1处编辑 |

#### 先读取

```
先 Read session_message_tool.rs L280-295 获取精确上下文。
```

#### 改动步骤

**编辑1**：在 description 末尾添加互引提示。

old_string:
```rust
    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Asynchronously send a message to another agent session. When the target session finishes, its result is automatically sent back to you as a follow-up message.

Usage:
- Create a new session and send: omit "session_id", and provide "workspace", "session_name", "agent_type", and "message".
- Reusing an existing session: provide "session_id" and "message". You may omit "workspace"; the tool will resolve it from the target session when possible.

Allowed agent types when creating a session:
- "agentic": Coding-focused agent for implementation, debugging, and code changes.
- "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
- "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc.
"#
                .to_string(),
        )
    }
```

new_string:
```rust
    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Asynchronously send a message to another agent session. When the target session finishes, its result is automatically sent back to you as a follow-up message.

Usage:
- Create a new session and send: omit "session_id", and provide "workspace", "session_name", "agent_type", and "message".
- Reusing an existing session: provide "session_id" and "message". You may omit "workspace"; the tool will resolve it from the target session when possible.

Use SessionControl (list) to discover existing sessions before sending messages.
Use SessionHistory to export a transcript of any session.

Allowed agent types when creating a session:
- "agentic": Coding-focused agent for implementation, debugging, and code changes.
- "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
- "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc.
"#
                .to_string(),
        )
    }
```

#### 验证命令

```bash
cargo check -p bitfun-core
```

---

### R009d：`session_history_tool.rs` description 加互引

**姬码锋执行任务**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\tools\implementations\session_history_tool.rs` | 1处编辑 |

#### 先读取

```
先 Read session_history_tool.rs L58-101 获取精确上下文。
```

#### 改动步骤

**编辑1**：在 description 的 "Typical usage" 段落末尾添加对 SessionMessage 的引用。

old_string:
```rust
Typical usage:
- To review session history across a workspace, first use `SessionControl` to list the sessions in that workspace, then call this tool for the sessions you want to inspect.
- To inspect the latest state of a specific session, call this tool with `turns=["-1:"]` to export only the last turn.
```

new_string:
```rust
Typical usage:
- To review session history across a workspace, first use `SessionControl` to list the sessions in that workspace, then call this tool for the sessions you want to inspect.
- To inspect the latest state of a specific session, call this tool with `turns=["-1:"]` to export only the last turn.
- Use `SessionMessage` to send follow-up messages after reviewing a session's history.
```

#### 验证命令

```bash
cargo check -p bitfun-core
```

---

### R011：`session_control_tool.rs` Cancel handler 跳过 `ensure_session_exists` list 预检查

**姬码锋执行任务**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\tools\implementations\session_control_tool.rs` | 1处编辑 |

#### 先读取

```
先 Read session_control_tool.rs L434-460 获取精确上下文。
```

#### 改动步骤

**编辑1**：在 Cancel handler 中删除 `self.ensure_session_exists(...)` 调用（L455-456），允许取消任意 session（包括 Task subagent session），无需 list 预检查。

当前代码（L447-L456）：
```rust
                if self.current_workspace_session(context, &workspace.display_workspace)
                    == Some(session_id)
                {
                    return Err(BitFunError::tool(
                        "cannot cancel the current session from SessionControl".to_string(),
                    ));
                }

                self.ensure_session_exists(&runtime, &workspace, session_id)
                    .await?;
```

old_string:
```rust
                if self.current_workspace_session(context, &workspace.display_workspace)
                    == Some(session_id)
                {
                    return Err(BitFunError::tool(
                        "cannot cancel the current session from SessionControl".to_string(),
                    ));
                }

                self.ensure_session_exists(&runtime, &workspace, session_id)
                    .await?;
```

new_string:
```rust
                if self.current_workspace_session(context, &workspace.display_workspace)
                    == Some(session_id)
                {
                    return Err(BitFunError::tool(
                        "cannot cancel the current session from SessionControl".to_string(),
                    ));
                }

                // R-011: Skip list-based pre-check so subagent (Task) sessions can be cancelled.
                // The runtime's cancel_turn handles session-existence internally.
```

#### 验证命令

```bash
cargo check -p bitfun-core
cargo test -p bitfun-core -- session_control
```

---

## Wave 2（串行执行，依赖 Wave 1 全部完成）

### R003：coordinator.rs + session_control_tool.rs create/spawn 后注册 `SessionTreeManager`

**姬码锋执行任务**

⚠️ **必须 Wave 1 全部完成后才能开始此任务。**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\tools\implementations\session_control_tool.rs` | 1处编辑：Create handler 中 `persist_session_lineage` 之后注册 tree |
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\coordination\coordinator.rs` | 1处编辑：`persist_session_lineage` 调用之后注册 tree（隐藏 subagent create 路径） |

#### 先读取

```
先 Read session_control_tool.rs L394-412, coordinator.rs L5300-5320 获取精确上下文。
```

#### 背景

`SessionTreeManager` 是纯内存数据结构，已存在于 `bitfun_services_core::session::tree` 模块。`AgentRuntime` 已持有 `session_tree: Option<Arc<SessionTreeManager>>`。需要在使用 `persist_session_lineage` 写入关系后，同步注册到 `SessionTreeManager`。

#### 步骤1：session_control_tool.rs — Create handler 注册 tree

在 `persist_session_lineage` 调用之后，新增 `SessionTreeManager` 注册。

**文件**：`E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\tools\implementations\session_control_tool.rs`

在 L408-L412 的 `persist_session_lineage` 调用之后添加 tree 注册：

old_string:
```rust
                    let _ = coordinator
                        .session_manager
                        .persist_session_lineage(&created_session_id, relationship)
                        .await;
                }
```

new_string:
```rust
                    let _ = coordinator
                        .session_manager
                        .persist_session_lineage(&created_session_id, relationship.clone())
                        .await;

                    // R-003: Register in memory tree for UI rendering
                    if let Some(ref parent_id) = relationship.parent_session_id {
                        if let Some(tree) = coordinator.session_tree() {
                            let _ = tree.register_child(
                                parent_id,
                                &created_session_id,
                                relationship.depth.unwrap_or(1),
                            );
                        }
                    }
                }
```

> ⚠️ **实现注意**：需要给 `coordinator` 添加 `session_tree()` 访问器方法，返回 `Option<&SessionTreeManager>`。或者通过 runtime 的 `session_tree` 字段访问。

---

#### 步骤2：coordinator.rs — 隐藏 subagent create 路径注册 tree

在 `persist_session_lineage` 调用（L5301-L5311）之后添加 tree 注册。

**文件**：`E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\coordination\coordinator.rs`

old_string:
```rust
        if let Err(error) = self
            .session_manager
            .persist_session_lineage(
                &session_id,
                build_subagent_session_relationship(
                    subagent_parent_info.as_ref(),
                    &logical_agent_type,
                    continuation_policy,
                ),
            )
            .await
        {
            self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
                Some(session_id.clone()),
                prepared_session_created,
            )
            .await;
            return Err(error);
        }
```

new_string:
```rust
        let relationship = build_subagent_session_relationship(
            subagent_parent_info.as_ref(),
            &logical_agent_type,
            continuation_policy,
            subagent_parent_info.as_ref().and_then(|info| info.depth),
        );
        if let Err(error) = self
            .session_manager
            .persist_session_lineage(&session_id, relationship.clone())
            .await
        {
            self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
                Some(session_id.clone()),
                prepared_session_created,
            )
            .await;
            return Err(error);
        }

        // R-003: Register in memory tree for UI rendering
        if let Some(ref parent_id) = relationship.parent_session_id {
            if let Some(tree) = self.session_tree() {
                let _ = tree.register_child(
                    parent_id,
                    &session_id,
                    relationship.depth.unwrap_or(1),
                );
            }
        }
```

> ⚠️ 此处先 bind `relationship` 变量以复用（clone 给 persist 和 tree 注册）。

#### 验证命令

```bash
cargo check -p bitfun-core
cargo test -p bitfun-core
```

---

### R007：`session_control_tool.rs` list 输出树形 JSON

**姬码锋执行任务**

⚠️ **必须 R003 全部完成后才能开始此任务。**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\tools\implementations\session_control_tool.rs` | 2处编辑：List handler + `build_list_result_for_assistant` |

#### 先读取

```
先 Read session_control_tool.rs L577-618 获取 List handler 精确上下文。
先 Read session_control_tool.rs L208-245 获取 build_list_result_for_assistant 当前实现。
```

#### 步骤1：List handler 中构建树形 JSON（L596-602 之后）

在 List handler 中，获得 `sessions` 后、调用 `build_list_result_for_assistant` 之前，构建树形数据结构。

old_string:
```rust
                let current_session_id =
                    self.current_workspace_session(context, &workspace.display_workspace);
                let result_for_assistant = self.build_list_result_for_assistant(
                    &workspace.display_workspace,
                    &sessions,
                    current_session_id,
                );
```

new_string:
```rust
                let current_session_id =
                    self.current_workspace_session(context, &workspace.display_workspace);
                // R-007: Build tree structure from session relationships
                let tree_data = Self::build_session_tree_json(&sessions, current_session_id);
                let result_for_assistant = self.build_list_result_for_assistant(
                    &workspace.display_workspace,
                    &sessions,
                    current_session_id,
                    tree_data.as_deref(),
                );
```

#### 步骤2：新增 `build_session_tree_json` 辅助方法

在 `impl SessionControlTool` 块中（约 L207 之前）新增：

```rust
    /// Build a tree JSON from session summaries using parent_session_id relationships.
    /// Returns Some(serde_json::Value) if tree data is available, None for flat fallback.
    fn build_session_tree_json(
        sessions: &[AgentSessionSummary],
        current_session_id: Option<&str>,
    ) -> Option<serde_json::Value> {
        if sessions.is_empty() {
            return None;
        }
        // Collect session_id -> (name, agent_type, parent_id) mapping
        let session_map: std::collections::HashMap<&str, (&str, &str, Option<&str>)> = sessions
            .iter()
            .map(|s| {
                (
                    s.session_id.as_str(),
                    (s.session_name.as_str(), s.agent_type.as_str(), None::<&str>),
                )
            })
            .collect();
        // Build children map: parent_id -> Vec<child_id>
        // Note: AgentSessionSummary does not expose parent_session_id directly;
        // tree construction relies on SessionTreeManager (R003) being populated.
        // For now, output a flat list with a "tree" wrapper.
        let nodes: Vec<serde_json::Value> = sessions
            .iter()
            .map(|s| {
                json!({
                    "sessionId": s.session_id,
                    "sessionName": s.session_name,
                    "agentType": s.agent_type,
                    "createdAtMs": s.created_at_ms,
                    "lastActiveAtMs": s.last_active_at_ms,
                    "isCurrent": current_session_id == Some(s.session_id.as_str()),
                })
            })
            .collect();
        Some(json!({ "sessions": nodes }))
    }
```

> ⚠️ **注意**：`AgentSessionSummary` 当前不暴露 `parent_session_id`。完整树形需要 R003 的 `SessionTreeManager` 注册后才能通过 coordinator 访问。上述实现提供基础结构；若需完整树形，需在 `AgentSessionSummary` 中新增 `parent_session_id` 字段或通过 `coordinator.session_tree()` 查询。

#### 步骤3：修改 `build_list_result_for_assistant` 支持树形输出

修改方法签名和实现，接收可选的树形 JSON。

old_string:
```rust
    fn build_list_result_for_assistant(
        &self,
        workspace: &str,
        sessions: &[AgentSessionSummary],
        current_session_id: Option<&str>,
    ) -> String {
        if sessions.is_empty() {
            return format!("No sessions found in workspace '{}'.", workspace);
        }

        let mut lines = vec![format!(
            "Found {} session(s) in workspace '{}'",
            sessions.len(),
            workspace
        )];
        lines.push(String::new());
        if let Some(current_session_id) = current_session_id {
            lines.push(format!("Note: '{}' is your session_id", current_session_id));
            lines.push(String::new());
        }
        lines.push(
            "| session_id | session_name | agent_type | created_at | last_active_at |".to_string(),
        );
        lines.push("| --- | --- | --- | --- | --- |".to_string());
        for session in sessions {
            lines.push(format!(
                "| {} | {} | {} | {} | {} |",
                Self::escape_markdown_table_cell(&session.session_id),
                Self::escape_markdown_table_cell(&session.session_name),
                Self::escape_markdown_table_cell(&session.agent_type),
                Self::format_system_time(Self::system_time_from_epoch_ms(session.created_at_ms)),
                Self::format_system_time(Self::system_time_from_epoch_ms(
                    session.last_active_at_ms
                )),
            ));
        }
        lines.join("\n")
    }
```

new_string:
```rust
    fn build_list_result_for_assistant(
        &self,
        workspace: &str,
        sessions: &[AgentSessionSummary],
        current_session_id: Option<&str>,
        tree_json: Option<&serde_json::Value>,
    ) -> String {
        if sessions.is_empty() {
            return format!("No sessions found in workspace '{}'.", workspace);
        }

        let mut lines = vec![format!(
            "Found {} session(s) in workspace '{}'",
            sessions.len(),
            workspace
        )];
        if let Some(current_session_id) = current_session_id {
            lines.push(format!("Note: '{}' is your session_id", current_session_id));
        }

        // R-007: Output tree JSON when available, fallback to flat table
        if let Some(tree) = tree_json {
            lines.push(String::new());
            lines.push("Session tree:".to_string());
            lines.push(serde_json::to_string_pretty(tree).unwrap_or_default());
        } else {
            lines.push(String::new());
            lines.push(
                "| session_id | session_name | agent_type | created_at | last_active_at |".to_string(),
            );
            lines.push("| --- | --- | --- | --- | --- |".to_string());
            for session in sessions {
                lines.push(format!(
                    "| {} | {} | {} | {} | {} |",
                    Self::escape_markdown_table_cell(&session.session_id),
                    Self::escape_markdown_table_cell(&session.session_name),
                    Self::escape_markdown_table_cell(&session.agent_type),
                    Self::format_system_time(Self::system_time_from_epoch_ms(session.created_at_ms)),
                    Self::format_system_time(Self::system_time_from_epoch_ms(
                        session.last_active_at_ms
                    )),
                ));
            }
        }
        lines.join("\n")
    }
```

#### 验证命令

```bash
cargo check -p bitfun-core
cargo test -p bitfun-core -- session_control
```

---

## Wave 3（前端渲染，依赖 Wave 1 + Wave 2）

### R008：前端 session 列表组件树形渲染

**姬码锋执行任务**

⚠️ **必须 R006 和 R007 全部完成后才能开始此任务。**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\web-ui\src\app\components\NavPanel\sections\sessions\SessionsSection.tsx` | 多处编辑：session 排序 + 缩进渲染 |
| `E:\finance-trading\lvpa\software\taiji-quant\src\web-ui\src\app\components\NavPanel\sections\sessions\SessionsSection.scss` | 新增缩进样式 |

#### 先读取

```
先 Read SessionsSection.tsx L1-80, L143-170（props）, session 渲染部分；sessionOrdering.ts；SessionsSection.scss 全文。
```

#### 改动步骤

**编辑1（sessionOrdering.ts 或排序逻辑）**：调整 session 排序，使子 session 紧跟父 session 之后，按 depth 缩进排列。

当前逻辑在 `src/web-ui/src/flow_chat/utils/sessionOrdering.ts` 中，`sessionBelongsToWorkspaceNavRow` 返回排序权重。需要扩展为：先按父深度分组合并，再在组内排序。

具体编辑需要在实际代码审查后精确确定，大致思路：

```typescript
// 新增 treeOrder 函数：按 (rootId, depth, createdAt) 排序
// 子 sessions 排在父 session 之后，并按 depth 缩进
```

**编辑2（SessionsSection.tsx）**：渲染 session row 时根据 `depth` 添加左侧缩进。

```tsx
// 在 session row 的 style 或 className 中根据 depth 添加 paddingLeft
const indentStyle = session.depth ? { paddingLeft: `${session.depth * 16}px` } : undefined;
```

**编辑3（SessionsSection.scss）**：添加树形连接线样式（可选）。

```scss
.session-tree-child {
  border-left: 1px solid var(--border-color);
  margin-left: 8px;
}
```

#### 验证命令

```bash
pnpm run type-check:web
pnpm run build:web
```

#### 手动验证

启动 desktop app，创建几个 session（包括通过 Task tool 创建子 session），确认 session 列表：
1. 子 session 缩进显示在父 session 下方。
2. depth 层级正确。
3. session 排序合理（父子聚集）。

---

## Wave 4（P1 + 验证）

### R010：`agent_type` 合并

**姬码锋执行任务**

#### 背景

当前代码中存在多个 agent_type 相关字段：
- `SessionMetadata.agent_type` — 当前默认 mode
- `SessionMetadata.last_user_dialog_agent_type` — 最后存活的 user dialog turn mode
- `SessionMetadata.last_submitted_agent_type` — 最近提交 mode
- `AgentSessionSummary.agent_type` — 列表摘要
- `AgentSessionSummary.last_user_dialog_agent_type`
- `AgentSessionSummary.last_submitted_agent_type`

以及前端 `VALID_AGENT_TYPES` 常量集合（FlowChatStore.ts L69-78）。

合并目标：精简 agent_type 的语义，减少冗余字段。

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\services\services-core\src\session\types.rs` | 可能需要精简 `SessionMetadata` 字段 |
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\contracts\runtime-ports\src\lib.rs` | 可能需要精简 `AgentSessionSummary` |
| `E:\finance-trading\lvpa\software\taiji-quant\src\web-ui\src\flow_chat\store\FlowChatStore.ts` | `VALID_AGENT_TYPES` 可能需要同步 |
| 其他引用 `last_user_dialog_agent_type` / `last_submitted_agent_type` 的文件 | grep 后逐文件处理 |

#### 先读取

```
grep -rn "last_user_dialog_agent_type\|last_submitted_agent_type\|lastUserDialogAgentType\|lastSubmittedAgentType" 获取全量引用。
```

#### 改动步骤

> ⚠️ **此任务为 P1（优先级低），具体合并策略待需求进一步澄清。** 以下为框架性描述。

1. 确认合并目标：`last_user_dialog_agent_type` 和 `last_submitted_agent_type` 是否可合并为单一字段。
2. 统一后端 Rust struct + 前端 TS interface。
3. 更新所有引用点（预计 ~10 处）。
4. 确保前端 mode icon 显示逻辑不受影响。

#### 验证命令

```bash
cargo check --workspace
pnpm run type-check:web
```

---

### R012：Delete 级联

**姬码锋执行任务**

⚠️ **必须 R003 全部完成后才能开始此任务。**

#### 文件列表

| 文件 | 操作 |
|------|------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\tools\implementations\session_control_tool.rs` | 1处编辑：Delete handler 增加级联逻辑 |
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\services\services-core\src\session\tree.rs` | 使用已有的 `remove_subtree` 方法 |

#### 先读取

```
先 Read session_control_tool.rs L519-576, tree.rs L185-203 获取精确上下文。
```

#### 改动步骤

**编辑1**：在 Delete handler 中，删除 session 前先从 `SessionTreeManager` 获取所有子孙 session ID，然后级联删除。

在 `deletion_runtime.delete_session(...)` 调用之前（L551），添加级联逻辑：

old_string:
```rust
                deletion_runtime
                    .delete_session(AgentSessionDeleteRequest {
                        workspace_path: workspace.display_workspace.clone(),
                        session_id: session_id.to_string(),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;
```

new_string:
```rust
                // R-012: Cascade delete child sessions
                let child_ids: Vec<String> = coordinator
                    .session_tree()
                    .map(|tree| tree.get_descendants(session_id))
                    .unwrap_or_default();

                // Delete children first (reverse order: deepest first)
                for child_id in child_ids.iter() {
                    let _ = deletion_runtime
                        .delete_session(AgentSessionDeleteRequest {
                            workspace_path: workspace.display_workspace.clone(),
                            session_id: child_id.clone(),
                            remote_connection_id: workspace.remote_connection_id.clone(),
                            remote_ssh_host: workspace.remote_ssh_host.clone(),
                        })
                        .await;
                }

                deletion_runtime
                    .delete_session(AgentSessionDeleteRequest {
                        workspace_path: workspace.display_workspace.clone(),
                        session_id: session_id.to_string(),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;

                // R-012: Purge subtree from in-memory tree
                if let Some(tree) = coordinator.session_tree() {
                    tree.remove_subtree(session_id);
                }
```

> ⚠️ **实现注意**：`SessionTreeManager` 需要新增 `get_descendants` 方法（或使用已有的 `walk_ancestors` 方向相反的逻辑）。当前 `tree.rs` 有 `remove_subtree`（L185）可做子树清理，但缺一个公开的"获取所有后代"方法。需在 `tree.rs` 中新增：

```rust
/// 获取所有后代 session_id（BFS）
pub fn get_descendants(&self, session_id: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut queue: Vec<String> = self.get_children(session_id);
    while let Some(id) = queue.pop() {
        result.push(id.clone());
        for child in self.get_children(&id) {
            queue.push(child);
        }
    }
    result
}
```

#### 验证命令

```bash
cargo check -p bitfun-core
cargo test -p bitfun-services-core -- tree
cargo test -p bitfun-core -- session_control
```

---

## 全量验证（Wave 4 完成后）

### 验证命令（按顺序执行）

```bash
# 1. 全量编译检查
cargo check --workspace

# 2. 事件 crate 测试
cargo test -p bitfun-events

# 3. 核心 crate 测试
cargo test -p bitfun-core

# 4. agent-runtime 测试
cargo test -p bitfun-agent-runtime

# 5. services-core 测试（含 tree.rs）
cargo test -p bitfun-services-core

# 6. desktop crate 编译 + 测试
cargo check -p bitfun-desktop
cargo test -p bitfun-desktop

# 7. 前端类型检查
pnpm run type-check:web

# 8. 前端构建
pnpm run build:web
```

### 验收标准

| R-ID | 验收标准 |
|------|----------|
| R001 | `build_subagent_session_relationship` 的 `depth` 字段正确计算为 `parent_depth + 1`，测试通过 |
| R002 | Create handler 的 child_depth 从父 session metadata 继承而非硬编码 `1`，编译通过 |
| R004 | `SubagentTurnCompleted` 事件正确投影为前端事件，`cargo test -p bitfun-events` 通过 |
| R005 | `SessionRelationship` + `SessionMetadata` TS 类型有 `depth` 字段，`pnpm run type-check:web` 通过 |
| R006 | `Session` TS 类型有 `depth`/`children` 字段，前端类型检查通过 |
| R009a-d | 4 个 tool description 互引完整，编译通过 |
| R011 | Cancel handler 不再调用 `ensure_session_exists`，编译 + 测试通过 |
| R003 | `SessionTreeManager.register_child` 在 create/spawn 后正确调用，树内存结构正确 |
| R007 | list 输出包含树形 JSON，`result_for_assistant` 中有层次信息 |
| R008 | 前端 session 列表按树形缩进渲染，手动验证父子关系可见 |
| R010 | agent_type 字段合并完成，编译 + 类型检查通过 |
| R012 | Delete 级联删除子 session，tree 内存结构同步清理 |

---

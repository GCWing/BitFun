# R-004 设计方案：Session 控制面加固 + 树形拓扑 + 工具互引

> 派生自：R-004 需求矩阵（D1–D7）
> 设计原则：复用已有 SessionTreeManager / collect_hidden_subagent_cascade / delete_hidden_subagent_sessions_for_parent_turns 基础设施，最小改动补齐控制面缺口。

---

## 架构决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|---------|
| D1 | **depth 从 parent 继承+1，两入口统一** | `build_subagent_session_relationship()` 返回 `depth: None` 是 bug，SessionControl create 硬编码 `child_depth = 1u32` 同样错误。统一逻辑：从 parent 的 `SessionRelationship.depth` 读取，+1 写入 child。parent 无 relationship 时 depth=1。两入口（Task spawn + SessionControl create）走同一路径 | 各入口独立计算 depth（不一致，维护两套逻辑，SessionControl create 永远是深度1） |
| D2 | **SessionTreeManager 在 create/spawn 时注册** | 当前 `register_child()` 仅被单元测试和 `load_from_sessions()` 调用，生产环境中 create/spawn 路径未注册。在 `persist_session_lineage()` 成功后调用 `tree.register_child()`，保证内存树与持久化 lineage 同步 | 仅在 `load_from_sessions()` 批量加载（冷启动可工作，但运行时新建 session 不出现于树中） |
| D3 | **SubagentTurnCompleted 保留投影** | 当前 `frontend_projection.rs` 中 `AgenticEvent::SubagentTurnCompleted { .. } => None`，前端无法感知子 agent 完成。需恢复投影为 `agentic://subagent-turn-completed`，payload 含 `sessionId`, `subagentDialogTurnId`, `parentSessionId`, `status`, `outputText` | 不投影（前端只能通过轮询 dialog turn 变化感知，TaskTree 面板刷新不及时） |
| D4 | **list 返回树形 JSON** | SessionControl `list` 当前返回平铺 markdown 表格，Task `list` 返回平铺 JSON。改为 `build_tree(root_id, sessions)` 输出 `SessionTreeNode` 嵌套结构。session-less 场景（无 root session_id）返回 `Vec<SessionTreeNode>` 森林 | 维持平铺表格（agent 需自行推断父子关系，不可靠） |
| D5 | **Cancel 跳过 list 预检查** | `ensure_session_exists()` 调用 `list_sessions()` 但该接口过滤 subagent session（SessionKind::Subagent），导致 Task spawn 的子 agent 无法通过 SessionControl cancel。改为直接通过 coordinator 取消，不预先检查存在性 | 修改 `list_sessions` 不过滤 subagent（破坏现有语义，影响其他调用方） |
| D6 | **Delete 级联递归删除（方案B）** | 利用已有 `collect_hidden_subagent_cascade`（post-order 遍历）收集所有子孙 subagent session，按 post-order 逐个删除，保证子先于父。`delete_hidden_subagent_sessions_for_parent_turns` 已有完整实现可作参考 | 方案A：仅删除目标 session（留下孤儿 subagent，数据泄漏）；方案C：硬删除 + 手动级联（重复造轮子） |
| D7 | **四工具 description 互相引用** | SessionControl、Task、SessionMessage、SessionHistory 的 `description()` 末尾加 `## Related tools` 段，互相引用。agent 不会混淆：Task 负责 spawn/list/cancel 子 agent，SessionControl 负责 create/list/cancel/delete 持久 session，SessionMessage 负责跨 session 发消息，SessionHistory 负责导出 transcript | 不引用（agent 可能用 SessionControl cancel 处理 Task 子 agent，或反之） |

---

## 数据流

### D1: depth 继承链路

```
SessionControl create / Task spawn
  │
  ├─ 读取 parent SessionMetadata.relationship.depth
  │     ├─ Some(parent_depth) → child_depth = parent_depth + 1
  │     └─ None               → child_depth = 1
  │
  ├─ 构造 SessionRelationship { depth: Some(child_depth), ... }
  │
  └─ persist_session_lineage(child_session_id, relationship)
        │
        └─ apply_session_lineage() → metadata.relationship = relationship
              │
              └─ (D2) tree.register_child(parent_id, child_id, child_depth)
```

### D2: SessionTreeManager 注册链路

```
persist_session_lineage() 成功
  │
  ├─ 从 relationship 提取 parent_session_id + child_depth
  │
  └─ tree.register_child(parent_session_id, child_session_id, child_depth)
        │
        ├─ 循环检测（walk_ancestors）
        ├─ 深度上限检测（max_depth）
        ├─ edges: parent_id → [child_ids]
        └─ depths: child_id → depth
```

### D4: list 树形输出

```
SessionControl list / Task list
  │
  ├─ list_sessions(workspace) → Vec<AgentSessionSummary>
  │     │
  │     └─ 包含 SessionRelationship（parent_session_id, depth, kind）
  │
  ├─ tree.load_from_sessions(&sessions)   ← 冷加载所有关系
  │
  ├─ 定位 root session:
  │     ├─ 有当前 session_id → 以它为 root 构建单棵树
  │     └─ 无当前 session_id → 收集所有无 parent 的 session 为森林
  │
  └─ tree.build_tree(root_id, &sessions) → SessionTreeNode (嵌套)
        │
        └─ 序列化为 JSON 返回 agent
```

### D6: Delete 级联删除链路

```
SessionControl delete session_id=X
  │
  ├─ 收集级联 session:
  │     collect_hidden_subagent_cascade(
  │         all_sessions,
  │         parent_session_id=X,
  │         parent_dialog_turn_ids=所有 turn_ids_of_X
  │     ) → Vec<session_id> (post-order: 子孙在前，X 在最后)
  │
  ├─ 对每个 session_id（post-order）:
  │     ├─ cancel_active_turn(session_id)
  │     ├─ background_subagent_outcomes.delete_session_references(session_id)
  │     ├─ session_manager.delete_session(workspace, session_id)
  │     ├─ emit_event(SessionDeleted { session_id })
  │     └─ tree.remove_subtree(session_id)
  │
  └─ 返回 deleted_session_ids
```

### D5: Cancel 跳过 list 预检查

```
SessionControl cancel session_id=X
  │
  ├─ resolve_effective_workspace(Cancel, Some(X)) → workspace
  │
  ├─ 【旧】ensure_session_exists(&runtime, &workspace, X) → 调用 list_sessions() → 过滤掉 Subagent → 报错 NotFound
  │
  ├─ 【新】跳过 ensure_session_exists
  │     │
  │     └─ 直接 coordinator.cancel_turn(X, ...)
  │           │
  │           └─ 若 session 不存在 → cancel_turn 自身返回 NotFound（自然报错）
  │
  └─ 返回 cancel 结果
```

---

## 文件架构

| 文件 | R-ID | 操作 | 行数 | 变更说明 |
|------|------|------|------|---------|
| `assembly/core/.../coordinator.rs` | D1 | 修改 `build_subagent_session_relationship` | +8 | 接收 `parent_depth: Option<u32>` 参数，`depth: Some(parent_depth.map_or(1, \|d\| d + 1))` |
| `assembly/core/.../coordinator.rs` | D1 | 修改调用方 | +6 | `execute_hidden_subagent_internal` 等调用处从 parent session metadata 读取 depth 传入 |
| `assembly/core/.../session_control_tool.rs` | D1 | 修改 create action | +10 | 从 parent session metadata 读取 `relationship.depth`，`child_depth = parent_depth + 1` 替代硬编码 `1u32` |
| `assembly/core/.../coordinator.rs` | D2 | 在 `persist_session_lineage` 后注册 | +5 | 调用 `tree.register_child(parent_id, child_id, child_depth)`；若注册失败则 log::warn 不阻塞创建 |
| `assembly/core/.../session_control_tool.rs` | D2 | create 后注册 | +5 | `persist_session_lineage` 成功后调用 `tree.register_child()` |
| `contracts/events/src/frontend_projection.rs` | D3 | 恢复投影 | +15 | `SubagentTurnCompleted` match arm 从 `None` 改为 `Some(AgenticFrontendEvent::new("agentic://subagent-turn-completed", json!({...})))` |
| `contracts/events/src/frontend_projection.rs`（测试） | D3 | 新增投影测试 | +20 | 验证 `SubagentTurnCompleted` → `agentic://subagent-turn-completed` 映射，各 status 变体覆盖 |
| `assembly/core/.../session_control_tool.rs` | D4 | 重写 list 输出 | +40 | `build_list_result_for_assistant` 改为 `build_tree_result_for_assistant`，调用 `tree.build_tree()` 输出嵌套 JSON（带 `depth`、`children`）替代平铺 markdown 表格 |
| `assembly/core/.../task/mod.rs` | D4 | 重写 list action 输出 | +30 | Task `list` action 复用 `tree.build_tree()` 输出树形 JSON |
| `assembly/core/.../session_control_tool.rs` | D5 | 移除 cancel 预检查 | -8 | 删除 `self.ensure_session_exists(&runtime, &workspace, session_id).await?;`，依赖 coordinator 自然 NotFound |
| `assembly/core/.../coordinator.rs` | D6 | 新增 `delete_session_cascade` | +35 | 封装 `delete_hidden_subagent_sessions_for_parent_turns` 逻辑为公开方法 `delete_session_cascade`，级联删除目标及其所有子孙 subagent |
| `assembly/core/.../session_control_tool.rs` | D6 | delete action 走级联 | +10 | `delete` action 调用 `coordinator.delete_session_cascade()` 替代 `runtime.delete_session()` |
| `assembly/core/.../session_control_tool.rs` | D7 | 更新 description | +20 | 末尾加 `## Related tools` 段，引用 Task / SessionMessage / SessionHistory |
| `assembly/core/.../task/schema.rs` | D7 | 更新 description | +15 | `render_description` 末尾加 `## Related tools` 段，引用 SessionControl / SessionMessage / SessionHistory |
| `assembly/core/.../session_message_tool.rs` | D7 | 更新 description | +10 | `description()` 末尾加 `## Related tools` 段 |
| `assembly/core/.../session_history_tool.rs` | D7 | 更新 description | +10 | `description()` 末尾加 `## Related tools` 段 |
| `services/services-core/src/session/tree.rs` | D2, D4 | `build_tree` 支持森林 | +15 | 新增 `build_forest(sessions)` 返回 `Vec<SessionTreeNode>`，收集所有无 parent 的根节点构建多棵树 |

### 文件路径（绝对路径基准）

```
E:\finance-trading\lvpa\software\taiji-quant\src\crates\
  ├── assembly\core\src\agentic\coordination\coordinator.rs          ← D1, D2, D6
  ├── assembly\core\src\agentic\tools\implementations\
  │     ├── session_control_tool.rs                                   ← D1, D2, D4, D5, D6, D7
  │     ├── session_message_tool.rs                                   ← D7
  │     ├── session_history_tool.rs                                   ← D7
  │     └── task\
  │           ├── schema.rs                                           ← D7
  │           ├── mod.rs                                              ← D4
  │           └── execution.rs                                        ← D1
  ├── contracts\events\src\frontend_projection.rs                    ← D3
  └── services\services-core\src\session\tree.rs                     ← D2, D4
```

---

## 依赖图

```
                        ┌──────────────────────────┐
                        │   build_subagent_         │
                        │   session_relationship()  │
                        │   depth: parent+1 (D1)    │
                        └────────────┬─────────────┘
                                     │
              ┌──────────────────────┼──────────────────────┐
              │                      │                      │
    ┌─────────▼─────────┐  ┌────────▼────────┐  ┌─────────▼─────────┐
    │ coordinator.rs     │  │ session_control  │  │ task/execution.rs │
    │ persist_lineage    │  │ _tool.rs         │  │ spawn             │
    │ + tree.register (D2)│  │ create (D1,D2)   │  │ (D1)              │
    │ + delete_cascade(D6)│  │ list→tree (D4)   │  │ list→tree (D4)    │
    │                    │  │ cancel skip (D5)  │  │                   │
    │                    │  │ delete cascade(D6)│  │                   │
    └────────┬───────────┘  └────────┬─────────┘  └────────┬──────────┘
             │                       │                      │
             │              ┌────────▼─────────┐            │
             │              │ tree.rs           │            │
             │              │ register_child()  │◄───────────┘
             │              │ build_tree()      │
             │              │ build_forest() (新)│
             │              └────────┬─────────┘
             │                       │
             │              ┌────────▼──────────────┐
             │              │ frontend_projection.rs │
             │              │ SubagentTurnCompleted  │
             │              │ 恢复投影 (D3)           │
             │              └───────────────────────┘
             │
             └──────► collect_hidden_subagent_cascade (lineage.rs)
                          post-order 遍历 (D6)

工具互引 (D7): SessionControl ↔ Task ↔ SessionMessage ↔ SessionHistory
无新增 crate 依赖，无循环依赖。
```

---

## D6 级联删除详解

```
方案B 执行流程：

1. 定位目标 session 的 workspace
2. 加载 workspace 下所有 session metadata（包括 Subagent）
3. 收集目标 session 所有 dialog turn IDs
4. 调用 collect_hidden_subagent_cascade(all_meta, target_id, turn_ids)
   → 返回 post-order Vec<session_id>（子孙在前，目标在最后）
5. 对每个 session_id（post-order）：
   a. cancel_active_turn_for_session(session_id) — 取消正在运行的 turn
   b. background_subagent_outcomes.delete_session_references(session_id)
   c. session_manager.delete_session(workspace, session_id)
   d. emit_event(SessionDeleted { session_id })
   e. tree.remove_subtree(session_id) — 清理内存树
6. 返回 Vec<deleted_session_ids>

参考实现：
- delete_hidden_subagent_sessions_for_parent_turns (coordinator.rs:4377)
  已实现完整 post-order 级联删除流程，仅需适配入口参数
```

---

## D4 树形输出格式

```json
// 单棵树（有 root session_id）
{
  "sessionId": "root-abc",
  "sessionName": "My Session",
  "agentType": "agentic",
  "depth": 0,
  "status": "running",
  "children": [
    {
      "sessionId": "sub-1",
      "sessionName": "Explore: inspect parser",
      "agentType": "Explore",
      "depth": 1,
      "status": "completed",
      "children": []
    }
  ]
}

// 森林（无 root session_id）
[
  { "sessionId": "session-a", "depth": 0, "children": [...], ... },
  { "sessionId": "session-b", "depth": 0, "children": [], ... }
]
```

---

## 风险与降级

| 风险 | 等级 | 缓解措施 | 降级方案 |
|------|------|---------|---------|
| `tree.register_child()` 在 persist_lineage 后调用，若注册失败（循环/超深）不应阻塞创建 | 低 | `register_child` 返回 `Result`，用 `let _ = ...` + `log::warn!` 吞错误；lineage 已持久化，内存树缺失仅影响 list 树形展示 | 降级为 tree 不一致时下次 `load_from_sessions` 自动修复 |
| depth 继承读取 parent metadata 可能因 I/O 增加延迟 | 低 | `session_manager.get_session_metadata()` 通常命中内存缓存；depth 读取仅在 create/spawn 时发生一次，非热路径 | 若缓存未命中走磁盘读取，session 创建本身已有 I/O，额外一次 metadata 读取可忽略 |
| cancel 跳过 list 预检查后，不存在的 session 报错信息不如原来友好 | 低 | coordinator.cancel_turn 本身返回 `BitFunError::NotFound("Session not found: {id}")`，语义等价 | 维持原 ensure_session_exists 逻辑但改用不过滤 Subagent 的 list 接口 |
| 级联删除在大型 session 树下可能耗时长 | 中 | post-order 先删子再删父，每步都可独立回滚；删除操作本身轻量（session 级文件删除） | 若 session 树很大（>100），改为异步后台删除 + WebSocket 推送进度事件 |
| `build_tree` / `build_forest` 使用递归实现，超深树可能栈溢出 | 低 | SessionTreeManager.max_depth 默认 ≤ 64；`build_tree_impl` 已有 visited set 循环检测 | 改 `build_forest` 为迭代版（参考 `remove_subtree` 的栈式实现） |
| 四工具 description 互相引用可能让 agent 困惑 | 低 | 仅添加 `## Related tools` 段，不影响主 description 内容；引用简明（每工具一行） | 若 agent 仍选错工具，在各自 validate_input 中强化错误提示 |

---

## 不变式（Invariants）

1. **depth = parent_depth + 1** — 所有新建 session（Task spawn + SessionControl create）的 depth 严格从 parent 继承递增，parent 无 depth 时从 1 开始。
2. **内存树 = 持久化 lineage** — 每次 `persist_session_lineage()` 成功后立即 `tree.register_child()`，保证树与持久化一致。不一致时 `load_from_sessions` 可修复。
3. **SubagentTurnCompleted 必须前端可见** — `frontend_projection` 返回 `Some(AgenticFrontendEvent)`，前端 TaskTree 面板通过 EventBus 订阅实时刷新。
4. **list 输出满足 SessionTreeNode schema** — 含 `sessionId`, `depth`, `children` 字段；前端 R-002 递归 `walk()` 兼容。
5. **cancel 不因 subagent 类型被拒绝** — 跳过 `ensure_session_exists` 的 list 过滤逻辑，所有 session 类型均可被取消。
6. **delete 级联无残留** — post-order 删除保证子先于父，`tree.remove_subtree` 清理内存。即使中途失败，已删除的 session 不回滚（删除操作幂等）。
7. **四工具 description 不改变工具语义** — 仅追加 `## Related tools` 引用段，不修改 action/enum/参数定义。
8. **不新增 crate**，不修改 protocol schema，不破坏已有 `AgenticEvent` 序列化兼容。

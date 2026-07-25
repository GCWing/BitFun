# R-004: Session 层级树 + 军团通信链（Ultra 模式收尾）

## 需求来源

1. Session Create 建 L0 主对话，Task 建 L1→L2 逐级裂变，形成完整层级树。
2. SessionControl 系列工具（list/delete/history/SessionMessage）统一管理所有层级，list 按树形显示。
3. 工具说明写清楚让 agent 不会用错——Task 专用于裂变子 agent，SessionControl 专用于持久化管理。
4. 范围：ultra 模式收尾——一个对话处理任意复杂度任务，Task 机制驱动完整军团通信链。

## R-ID 矩阵

| R-ID | 描述 | 优先级 | 依赖 |
|------|------|--------|------|
| R001 | `build_subagent_session_relationship` 从 parent relationship 继承 depth+1 | P0 | - |
| R002 | SessionControl Create 也从 parent 继承 depth（当前硬编码 `Some(1)`） | P0 | R001 |
| R003 | Task spawn / SessionControl Create 后注册到 SessionTreeManager | P0 | R001, R002 |
| R004 | 前端投影保留 SubagentTurnCompleted（不再丢弃） | P0 | - |
| R005 | `deriveSessionRelationshipFromMetadata` 返回 depth | P0 | - |
| R006 | 前端 Session 类型加 depth/children | P0 | R005 |
| R007 | SessionControl list 输出树形 JSON（用 `SessionTreeManager.build_tree()`） | P0 | R003, R006 |
| R008 | 前端 session 列表渲染树形（缩进/展开/折叠） | P0 | R006, R007 |
| R009 | Task/SessionControl/SessionMessage/SessionHistory 四工具互引 description | P0 | - |
| R010 | `agent_type` 枚举集中化（11+ 处 → 1 处） | P1 | - |
| R011 | Cancel 支持子 session（跳过 list 预检查，与 Delete 对齐） | P0 | - |
| R012 | Delete 级联删除子 session（递归全删，post-order 先叶子后父） | P1 | R003 |
| R013 | `list_tasks` action（R007 树形落地后评估是否仍需） | P2 | R007 |

## 验收标准（逐 R-ID）

### R001: `build_subagent_session_relationship` 继承 depth+1

1. `coordinator.rs` 中 `build_subagent_session_relationship()` 不再硬编码 `depth: None`；改为读取 `parent_info` 对应 parent session 的 `SessionRelationship.depth`，计算 `child_depth = parent_depth.saturating_add(1)`。
2. 若 parent 无 relationship 或 depth 为 `None`，child_depth 默认为 1（即根直系子节点）。
3. 穿透 `session_manager` 查询 parent metadata 的 relationship 字段（session_manager 已有 `get_session_metadata` 或等价查询路径）。
4. `cargo check --workspace` 零错误；新增单元测试覆盖：parent depth=0→child=1、parent depth=2→child=3、parent 无 relationship→child=1。

### R002: SessionControl Create 继承 depth

1. `session_control_tool.rs` Create action（约 L394-412）中 `let child_depth = 1u32;` 替换为：先查询 parent session 的 `SessionRelationship.depth`，计算 `child_depth = parent_depth.unwrap_or(0).saturating_add(1)`。
2. 查询 parent session 的 relationship 使用 coordinator 已有的 `session_manager` 接口，不引入新的直接 DB 访问。
3. 与 R001 共享同一 depth 计算逻辑（抽取为 `compute_child_depth(parent_session_id)` 辅助函数，放在 `coordinator.rs` 或 `session_manager` 中，两处调用）。
4. `cargo check --workspace` 零错误；单元测试覆盖与 R001 相同的场景。

### R003: Task spawn / SessionControl Create 注册 SessionTreeManager

1. Task spawn 路径（`coordinator.rs` 中 `build_subagent_session_relationship` 调用点，约 L5305、L8664、L9453）在 relationship 写入后，调用 `session_tree_manager.register_child(parent_id, child_id, depth)`。
2. SessionControl Create 路径（`session_control_tool.rs` L394-412）在 `persist_session_lineage` 完成后，同样调用 `register_child`。
3. `SessionTreeManager` 实例通过 coordinator 访问（coordinator 已有 `session_tree_manager` 字段或可从 `runtime.rs` 获取）。
4. 注册失败时（cycle / max_depth exceeded）记录 `log::warn!` 但不阻塞 session 创建——session 已经持久化成功，树注册为辅助数据结构。
5. `cargo check --workspace` 零错误；集成测试验证：Task spawn 后 `session_tree_manager.get_children(parent_id)` 包含新 child。

### R004: 前端投影保留 SubagentTurnCompleted

1. 前端 EventQueue 消费者在收到 `AgenticEvent::SubagentTurnCompleted` 后，不再丢弃该事件；将其投影到对应的 session tree node 状态。
2. 投影逻辑：根据 `parent_session_id` 在 tree 中定位子节点，更新其 `status`（`Completed`→绿色勾，`Failed`/`Cancelled`→红色叉，`PartialTimeout`→黄色警告）。
3. 去重：以 `subagent_dialog_turn_id + status` 为去重键，避免重复渲染（与 R-003-004 的去重策略一致）。
4. 状态更新后触发 UI 局部刷新（仅更新受影响的 tree node，不重绘整棵树）。
5. 暂不处理 `output_text` 的展示（摘要文本展示延后到后续 UI 优化）。

### R005: `deriveSessionRelationshipFromMetadata` 返回 depth

1. `sessionMetadata.ts` 中 `deriveSessionRelationshipFromMetadata()` 返回值新增 `depth?: number` 字段。
2. 从 `metadata.relationship.depth` 读取 depth 值（该字段已在后端 `SessionRelationship` 中定义，`SessionTreeNode::build_tree()` 已在消费）。
3. `ResolvedSessionRelationship` 接口同步新增 `depth?: number`。
4. `normalizeSessionRelationship()` 和 `resolveSessionRelationship()` 透传 depth。
5. TypeScript 编译零错误（`pnpm run type-check:web` 通过）。

### R006: 前端 Session 类型加 depth/children

1. `Session` 接口（`flow-chat/types/flow-chat.ts` 或等价位置）新增字段：
   - `depth: number` — 当前节点在树中的深度（0 = 根）
   - `children: Session[]` — 子 session 列表（前端 walk 递归填充）
2. `SessionTreeNode` 类型（若独立于 `Session`）与后端 `SessionTreeNode` struct 字段对齐：`session_id`、`session_name`、`agent_type`、`depth`、`status`、`children`、`is_acp_external`。
3. `buildSessionMetadata()` 在构建 metadata 时写入 depth（从 `relationship.depth` 读取）。
4. 前端 `walk()` 递归函数已支持无限深度（R-002 已验证），本次仅确保 depth 字段正确透传。
5. TypeScript 编译零错误。

### R007: SessionControl list 输出树形 JSON

1. `session_control_tool.rs` List action（约 L577-616）在获取 `sessions` 列表后：
   - 调用 `session_tree_manager.load_from_sessions(&sessions)` 确保树关系已加载。
   - 识别根节点：没有 `parent_session_id` 或 parent 不在当前 workspace 列表中的 session。
   - 对每个根节点调用 `session_tree_manager.build_tree(root_id, &sessions)` 生成 `SessionTreeNode`。
   - 序列化整棵树为 JSON 作为 `result_for_assistant` 的主体输出。
2. 输出格式为紧凑但可读的 JSON 树，每节点包含：`session_id`、`session_name`、`agent_type`、`depth`、`status`、`children[]`。
3. 原有平铺 Markdown 表格输出作为 `result_for_assistant` 的 fallback（当 `session_tree_manager` 不可用时）。
4. `data` 字段（`json!()` 块）中的 `sessions` 保持原有平铺结构，新增 `tree` 字段携带树形数据供前端消费。
5. `cargo check --workspace` 零错误。

### R008: 前端 session 列表渲染树形

1. 前端 session 列表组件（`NavPanel` 或独立 SessionList 组件）改用树形渲染：
   - 根节点左对齐，子节点按 depth 缩进（每级缩进 16-20px）。
   - 有子节点的 session 显示展开/折叠箭头（默认展开）。
   - 点击箭头切换子节点可见性（纯 CSS/React state，不触发后端调用）。
2. 列表项显示：session 名称、agent_type 标签、status 图标（运行中/已完成/已归档）。
3. 当前 session 高亮（`current_session_id` 匹配）。
4. 树形数据来源：优先使用 R007 提供的 `tree` 字段；若不可用则回退到平铺列表。
5. 展开/折叠状态仅保存在组件本地 state，不持久化。
6. TypeScript 编译零错误；手动验证 3 层 session 树渲染正确。

### R009: 四工具互引 description

1. **Task 工具 description** 新增说明：
   - "Use Task to spawn a subagent that executes autonomously. The subagent runs in its own session (L1/L2 leaf)."
   - "To manage persisted sessions across workspaces, use SessionControl (create/list/delete)."
   - "To view dialog history of any session, use SessionHistory."
   - "To send a message into a session, use SessionMessage."
2. **SessionControl 工具 description** 新增说明：
   - "SessionControl manages persisted workspace-scoped sessions. It does NOT spawn subagents — use Task for that."
   - "Use SessionMessage to send prompts into sessions created by SessionControl."
   - "Use SessionHistory to read the dialog history of any session."
3. **SessionMessage 工具 description** 新增说明：
   - "Use SessionControl list to discover available sessions before sending a message."
   - "Use SessionHistory to review a session's dialog history."
4. **SessionHistory 工具 description** 新增说明：
   - "Use SessionControl list to discover available sessions."
   - "Use Task to spawn subagents; use SessionControl to manage sessions."
5. 四处 description 均使用 `long_description` 或 description 文本中的 `## Related Tools` 小节（与已有工具 description 风格一致）。
6. `cargo check --workspace` 零错误（工具注册宏在编译期校验）。

### R010: `agent_type` 枚举集中化

1. 全仓库搜索 `"agentic"` 字符串硬编码（不包括测试/注释/配置文件），识别所有重复定义点。
2. 在 `bitfun_core_types` 或等价共享 crate 中定义 `AgentType` 枚举（变体：`Agentic`、`Explore`、`FileFinder`、`Plan` 等，与 Task 工具的 `subagent_type` 对齐）。
3. 所有当前硬编码 `"agentic"` 的 11+ 处替换为 `AgentType::Agentic`（或 `AgentType::default()`）。
4. 提供 `AgentType::as_str()` 和 `FromStr` 实现，兼容已有字符串接口。
5. `cargo check --workspace` 零错误；`cargo test --workspace` 通过。

### R011: Cancel 支持子 session

1. `session_control_tool.rs` Cancel action（约 L434-569）移除 `ensure_session_exists` 的 list 预检查（当前 Cancel 需要 session 在 list 中可见；Delete 已跳过此检查）。
2. Cancel 改为：通过 `session_id` 直接构造 `AgentTurnCancellationRequest`，若 session 不存在则返回明确错误信息（而非静默失败）。
3. 子 session（由 Task spawn 创建的 L1/L2 session）同样可被 Cancel——它们与 SessionControl 创建的 session 使用相同的取消路径（`scheduler.cancel_turn` → `resolve_session_control_cancel_route`）。
4. Cancel 不需要 `workspace` 匹配检查（子 session 可能与父 session 不在同一 workspace list 中，但取消路径基于 session_id 全局唯一）。
5. `cargo check --workspace` 零错误。

### R012: Delete 级联删除子 session

1. Delete action 在删除目标 session 前：
   - 调用 `session_tree_manager.get_children(session_id)` 递归收集所有子孙 session。
   - 按 post-order（先叶子后父）依次调用 `runtime.delete_session()`。
2. 若任一子 session 删除失败：记录 `log::error!` 并继续删除其余 session（best-effort 级联）。最终报告删除成功数 / 失败数。
3. 父 session 在最后删除（确保子 session 先被清理）。
4. 删除完成后调用 `session_tree_manager.remove_subtree(session_id)` 清理内存树。
5. `result_for_assistant` 包含级联删除摘要（"Deleted session X and N child sessions"）。
6. `cargo check --workspace` 零错误；单元测试覆盖：3 层嵌套 session 级联删除后 tree 为空。

### R013: `list_tasks` action（延后评估）

1. 等待 R007 树形落地后评估：若 SessionControl list 树形输出已能满足 Task spawn 子 session 的可见性需求，则 `list_tasks` 不需要单独实现。
2. 若仍需：在 SessionControl 中新增 `list_tasks` action，仅列出 `SessionRelationshipKind::Subagent` 且 `parent_session_id == current_session_id` 的 session。
3. 输出格式与 list 一致（树形 JSON），但根节点固定为当前 session。

## 边界与约束

- **工作区**：仅 `E:\finance-trading\lvpa\software\taiji-quant`
- **不新增 crate**：所有修改在已有 crate 内完成
- **不改 protocol schema**：`SessionTreeNode` 已有定义，前端 `Session` 类型为追加字段
- **tree 为辅助数据结构**：`SessionTreeManager` 是纯内存结构，不持久化；数据源始终是 `SessionMetadata.relationship`
- **向后兼容**：平铺 list 输出保留为 fallback；已有 session 的 depth 缺失默认为 0
- **不涉及**：跨进程 session 同步、relay 模式下的远程 session 树

## 技术上下文

### 关键复用点

- `SessionTreeManager`（`services-core/src/session/tree.rs`）— `build_tree()`、`register_child()`、`remove_subtree()`、`load_from_sessions()` 已就绪
- `SessionMetadata.relationship.depth` — 字段已定义，`SessionTreeNode::build_tree()` 已在消费
- `persist_session_lineage()` — SessionControl Create 已在调用，Task spawn 路径亦可用
- `deriveSessionRelationshipFromMetadata()` — 前端已有，仅需追加 depth 透传
- 前端 `walk()` 递归 — 已支持无限深度（R-002 已验证）

### 修改文件清单（预期）

| 文件 | R-ID | 变更 |
|------|------|------|
| `src/crates/assembly/core/src/agentic/coordination/coordinator.rs` | R001, R003 | `build_subagent_session_relationship` 继承 depth；Task spawn 注册 SessionTreeManager |
| `src/crates/assembly/core/src/agentic/tools/implementations/session_control_tool.rs` | R002, R003, R007, R011, R012 | Create 继承 depth；注册 tree；list 树形化；Cancel 去预检查；Delete 级联 |
| `src/crates/services/services-core/src/session/tree.rs` | R003 | 暴露 `compute_child_depth()` 辅助（或放在 session_manager） |
| `src/web-ui/src/flow_chat/utils/sessionMetadata.ts` | R005 | `deriveSessionRelationshipFromMetadata` 返回 depth |
| `src/web-ui/src/flow_chat/types/flow-chat.ts`（或等价） | R006 | `Session` 类型加 `depth`/`children` |
| `src/web-ui/src/app/components/NavPanel/`（或等价） | R008 | 树形渲染 UI |
| `src/crates/assembly/core/src/agentic/tools/implementations/task_tool.rs` | R009 | description 互引 |
| `src/crates/assembly/core/src/agentic/tools/implementations/session_message_tool.rs` | R009 | description 互引 |
| `src/crates/assembly/core/src/agentic/tools/implementations/session_history_tool.rs` | R009 | description 互引 |
| `src/crates/contracts/core-types/src/agent_type.rs`（新增或等价） | R010 | `AgentType` 枚举集中化 |

> 注：具体文件路径以实际 `taiji-quant` 代码库结构为准，上表为基于当前代码分析的预期路径。

## 依赖关系图

```
R001 ──┬── R002 ──┬── R003 ──┬── R007 ── R013 (P2)
       │          │          │
       │          │          └── R012 (P1)
       │          │
       │          └── R007 (树形 JSON 需要 R003 注册的 tree 数据)
       │
       └── R005 ── R006 ──┬── R007 (前端类型需要 depth)
                          │
                          └── R008 (渲染需要 depth/children 类型)

R004 ── 独立（前端事件投影，无后端依赖）
R009 ── 独立（纯 description 文本变更）
R010 ── 独立（P1，可随时做）
R011 ── 独立（Cancel 逻辑修复，不依赖其他 R-ID）
```

### 可并行组

- **组 A（后端 depth）**：R001 + R002 + R003（建议同一 PR）
- **组 B（前端类型）**：R005 + R006（建议同一 PR）
- **组 C（前端 UI）**：R008（依赖组 B 完成）
- **组 D（list 树形）**：R007（依赖组 A + 组 B 完成）
- **独立**：R004、R009、R010、R011、R012 可各自独立 PR

## 已定决策

| 决策点 | 结论 |
|--------|------|
| R012 级联边界 | 方案 B：递归全删（post-order，先叶子后父），best-effort |
| history 入口 | 方向 B：SessionControl 不合并 history，保持 SessionHistory 分离 + 四工具互引 |
| 递归嵌套 | 前端 `walk()` 已支持无限深度（R-002 已验证） |
| tree 持久化 | 不持久化——`SessionTreeManager` 纯内存，启动时从 `SessionMetadata.relationship` 重建 |
| depth 缺失处理 | 默认为 0（根节点），不影响已有 session |

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| parent depth 查询引入额外 DB 读 | `session_manager` 已缓存 metadata，查询为内存操作 |
| SessionTreeManager 生命周期与 coordinator 绑定 | 使用 `Arc<SessionTreeManager>`，已在 `runtime.rs` 中作为长期持有实例 |
| 树形 list 输出 JSON 过大（大量 session） | 限制输出深度为 `max_depth`（默认 5），超深子树折叠为 `"children_truncated": true` |
| Cancel 去掉预检查后 session 不存在时错误信息模糊 | 显式返回 `"Session 'X' not found or already deleted"` |
| Delete 级联中途失败导致部分孤儿 session | best-effort 策略 + 返回详细删除计数，用户可手动清理 |
| `agent_type` 枚举化触及 11+ 处可能遗漏 | R010 用 `cargo check` 全工作区编译兜底，遗漏处编译失败 |
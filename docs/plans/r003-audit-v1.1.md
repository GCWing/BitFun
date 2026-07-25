# R-003 v1.1 重新审计报告

**审计日期**：2026-07-25
**审计范围**：r003-requirements.md (v1.1) / r003-design.md (v1.1) / r003-dispatch.md (v1.1)
**审计维度**：8 维（R-ID一致性 / 类型契约覆盖 / 依赖图正确性 / 侦察报告溯源 / 遗漏项 / 验收标准覆盖 / 容错设计 / 文件路径）
**对比基准**：r003-audit.md (v1.0)

---

## 审计结论

**PASS（带条件）**

v1.0 3 个致命缺口全部闭合。发现 **2 个新高优先级缺口**（字段名称不匹配、Cancelled 状态传导路径断裂）+ **5 个 v1.0 P1 缺口仍未闭合**。建议修复新高优先缺口后再执行。

---

## 一、致命缺口闭合验证

### G-01：R-003-005 已补入三份文档 → ✅ 闭合

| 文档 | v1.0 状态 | v1.1 状态 | 证据 |
|------|----------|----------|------|
| requirements | 已存在（基准） | 存在 | L18: R-003-005 行；L66-72: 4 条验收标准 |
| design | ❌ 缺失 | ✅ 存在 | L89-93: 文件架构表 5 行 R-003-005 测试条目；L175: 风险表新增 R-003-005 遗漏行（HIGH） |
| dispatch | ❌ 缺失 | ✅ 存在 | L17: 任务总览表 R-003-005 行；L690-777: Phase 3 后半含 6 个验证项 |

**判定**：闭合。

---

### G-02：dispatch 全文路径 taiji → taiji-quant → ✅ 闭合

逐文件验证：

| 文件 | 路径 | 状态 |
|------|------|------|
| agentic.rs | `E:\finance-trading\lvpa\software\taiji-quant\src\crates\contracts\events\src\agentic.rs` | ✅ |
| coordinator.rs | `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\coordination\coordinator.rs` | ✅ |
| scheduler.rs | `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\coordination\scheduler.rs` | ✅ |
| session.rs | `E:\finance-trading\lvpa\software\taiji-quant\src\crates\execution\agent-runtime\src\session.rs` | ✅ |
| agentic_api.rs | `E:\finance-trading\lvpa\software\taiji-quant\src\apps\desktop\src\api\agentic_api.rs` | ✅ |

Design 绝对路径基准表（L98-105）同步使用 `taiji-quant`。全 5/5 路径正确。

**判定**：闭合。

---

### G-03：status 从 String → SubagentCompletionStatus 枚举 → ✅ 闭合

| 文档 | v1.0 状态 | v1.1 状态 | 证据 |
|------|----------|----------|------|
| requirements | `SubagentCompletionStatus` 枚举 | 同，新增 `PartialTimeout` | L29: `status: SubagentCompletionStatus`（枚举：Completed / Failed / Cancelled / PartialTimeout） |
| design | `String` | `SubagentCompletionStatus` 枚举 | D2 决策: "其 status 字段使用 SubagentCompletionStatus 枚举而非裸 String" |
| dispatch | `String` | `SubagentCompletionStatus` 枚举 | Edit1 (L59-66): 枚举定义；Edit2 (L111): `status: SubagentCompletionStatus`；Edit5 (L213-227): snake_case 序列化测试 |

此外，Edit3b/4b 的状态映射从 `status_label()` 字符串匹配改为显式枚举转换：

```rust
// v1.0: status_str.to_string()  // 字符串
// v1.1: status  // SubagentCompletionStatus 枚举值
let status = match result.as_ref() {
    Ok(sr) => match sr.status {
        SubagentResultStatus::Completed => SubagentCompletionStatus::Completed,
        SubagentResultStatus::PartialTimeout => SubagentCompletionStatus::PartialTimeout,
    },
    Err(_) => SubagentCompletionStatus::Failed,
};
```

**判定**：闭合。（但内部 match 的非穷尽问题见下文 NEW-P1-02。）

---

## 二、8 维逐项复审

### 1. R-ID 一致性：PASS

三文档统一 5 个 R-ID（R-003-001 ~ R-003-005）。任务总览表、文件架构表、需求矩阵全部对齐。

### 2. 类型契约覆盖：⚠️ PASS（有警告）

v1.0 的核心矛盾（`status: String` vs `SubagentCompletionStatus` 枚举）已消除。但存在**字段名称跨文档不一致**：

| 字段 | requirements | design（数据流图） | dispatch（Edit2） | 一致性 |
|------|-------------|-------------------|-------------------|--------|
| `parent_session_id` | `String` | `String` | `String` | ✅ |
| `subagent_session_id` | **`subagent_session_id`** | **`session_id`** | **`session_id`** | ⚠️ 重命名 |
| `task_id` | **`task_id`** | **`subagent_dialog_turn_id`** | **`subagent_dialog_turn_id`** | ⚠️ 重命名 + 语义偏移 |
| `result_summary` | **`result_summary: String`** | **`output_text`** | **`output_text: Option<String>`** | ⚠️ 重命名 + 类型变化（Non-Optional → Optional） |
| `status` | `SubagentCompletionStatus` | `SubagentCompletionStatus` | `SubagentCompletionStatus` | ✅ |
| — | **无** | `parent_dialog_turn_id` | `parent_dialog_turn_id` | ✅（design 新增） |
| — | **无** | `parent_tool_call_id` | `parent_tool_call_id` | ✅（design 新增） |
| — | **无** | `agent_type` | `agent_type: Option<String>` | ✅（design 新增） |

**分析**：

1. `subagent_session_id` → `session_id`：rename 本身合理（在 `AgenticEvent` 上下文里 `session_id` 语义清晰）。但 requirements 仍用旧名，实现者若严格按 requirements 编码，会写出与 design/dispatch 不同的字段名。

2. `task_id` → `subagent_dialog_turn_id`：**语义偏移**。`task_id` 是业务层 task 标识，`subagent_dialog_turn_id` 是 session 层对话轮次标识。一个 task 可能跨多个 dialog turn，两者可能不等价。design/dispatch 用更细粒度的标识是正确的细化，但 requirements 未更新。

3. `result_summary: String` → `output_text: Option<String>`：双重变化。requirements 定义为**必填** `String`（含 ≤512 字符约束），dispatch 实现为 `Option<String>`（None 表示失败时无输出）。但 requirements 未说明失败场景下 `result_summary` 应取何值。

4. design/dispatch 新增 3 字段（`parent_dialog_turn_id`, `parent_tool_call_id`, `agent_type`）为合理的设计细化，已在 design 中记录，不算缺口。但 requirements 未同步。

**建议**：更新 requirements R-003-001 字段表，统一使用 design/dispatch 的字段名和类型。

> **NEW-P1-01**：requirements 字段名/类型与 design/dispatch 不一致（`subagent_session_id` vs `session_id`，`task_id` vs `subagent_dialog_turn_id`，`result_summary: String` vs `output_text: Option<String>`），可能导致实现者按 requirements 编码时产生字段名冲突。

### 3. 依赖图正确性：PASS

requirements 依赖链 → dispatch Phase 拓扑 → design 依赖图，三向一致：
- R-003-001 + R-003-003 并行（Phase 1）✅
- R-003-002 依赖 R-003-001（Phase 2 after Phase 1）✅
- R-003-004 依赖 Phase 1+2（Phase 3 前半）✅
- R-003-005 依赖 R-003-004（Phase 3 后半）✅
- design 依赖图 `events ← coordinator → scheduler, session, agentic_api` 方向正确 ✅
- 无循环依赖 ✅

v1.0 的 R-003-005 断裂已修复。

### 4. 侦察报告溯源：PASS（与前次相同）

4 份侦察报告对 4 个架构决策的支撑关系未变。D4 的行号定位属代码级精准定位，无需侦察报告显式支撑。

### 5. 遗漏项：⚠️ 侦察缺口仍未认领（同 v1.0）

来自 `task-vs-session-reconnaissance.md` 的 11 个 Task-Session 缺口（G-1 ~ G-11）和来自 `recursive-subagent-depth-limit-recon.md` 的 2 个高优 Bug + 1 个并发瓶颈，在 v1.1 中**仍未映射到任何 R-ID**，也**仍未在 design 中声明为后续跟进项**。

> 注：R-003 范围限定为"通信统一 + 上下文修复"，上述缺口属于独立功能增强，不映射到 R-003 是合理的。但 design 中应有一条"后续 R-ID 规划"行，否则这些已知缺口处于无人认领状态。

**建议**：在 design 末尾加一行"未纳入本次 R-003 的已知缺口"表，指向后续 R-ID。

### 6. 验收标准覆盖：⚠️ PASS（有缺口）

#### R-003-001（事件定义）：PASS

| # | 验收标准 | dispatch 覆盖 | 状态 |
|---|---------|--------------|------|
| 1 | 新增变体携带 5 字段 | Edit2 8 字段（字段名不同，见 NEW-P1-01） | ⚠️ 字段名不一致 |
| 2 | 同步路径 emit 事件 | Edit3b/4b `.complete()` 后 `enqueue()` | ✅ |
| 3 | Serialize/Deserialize derive | Edit5 序列化往返测试 2 个 | ✅ |
| 4 | `cargo check --workspace` + 单元测试 | Phase 1 验证: `cargo test -p bitfun-events` | ✅ |

#### R-003-002（dialog turn 注入）：⚠️ 3/6 未覆盖

| # | 验收标准 | dispatch 覆盖 | 状态 |
|---|---------|--------------|------|
| 1 | `reply_route` 指向父 session | `reply_route: None` | ❌ 仍为 None |
| 2 | metadata 标记 `source: "subagent_completion"` | `metadata: serde_json::Map::new()` | ❌ 仍为空 |
| 3 | 父 session dialog 历史可见子 agent 完成记录 | R-003-005 验证项中无此检查 | ❌ 未验证 |
| 4 | 上下文压缩后子 agent 结果不丢失 | 无验证 | ❌ 未覆盖 |
| 5 | `live_results` 向后兼容 | 无显式验证（但 design 不变式 2 声明，dispatch 未修改相关代码） | ⚠️ 设计保证 |
| 6 | `SubagentResultStatus → SubagentCompletionStatus` 显式转换 | Edit3b/4b 显式 match 转换 ✅ | ✅ |

> 注：验收标准 1/2/3/4 在 v1.0 中同样是未覆盖的，v1.1 中未修复。

#### R-003-003（128K→1M）：PASS

| # | 验收标准 | dispatch 覆盖 | 状态 |
|---|---------|--------------|------|
| 1 | coordinator.rs refresh 调用 | 编辑1 ✅ | ✅ |
| 2 | session.rs 从配置读取 | 编辑2 改默认值 `128128 → 1_048_576` | ⚠️ 语义偏差：改默认值 ≠ 从配置读取。但 design 将此定义为 L2 兜底，L1（refresh）才是读配置的路径。可接受。 |
| 3 | agentic_api.rs API 层刷新 | 编辑3 ✅ | ✅ |
| 4 | 验证 config.toml context_window=1048576 后新 session 值为 1M | R-003-005 验证项 1/2/3 覆盖 session/agentic_api/coordinator 三处 | ⚠️ 属单元级验证，非集成验证 |
| 5 | 已有 session 不受影响 | design 不变式 5 ✅ | ✅ |

#### R-003-004（后台通知链路）：PASS

| # | 验收标准 | dispatch 覆盖 | 状态 |
|---|---------|--------------|------|
| 1 | 后台 spawned task 完成→事件 + dialog turn | 编辑3b/4b 双闭包 ✅ | ✅ |
| 2 | 同步路径同样发射事件 | 编辑4b 直接执行路径同样处理 ✅ | ✅ |
| 3 | 前端 EventQueue 消费者刷新 + 去重 | 无前端代码变更（R-003 范围声明不改前端 UI） | ⚠️ 需求与范围声明矛盾 |
| 4 | EventQueue 引用通过 Arc clone | 编辑3a/4a ✅ | ✅ |

> 注：验收标准 3（前端消费者）在 requirements 中存在但 scope 声明"不改前端 UI"——需明确是否放入后续 R-ID。v1.0 已指出此矛盾（原 G-09），v1.1 未解决。

#### R-003-005（端到端验证）：⚠️ 覆盖不全

| requirements 验收项 | dispatch 对应验证 | 层级 | 状态 |
|---------------------|-------------------|------|------|
| 1. 端到端注入链路（Task spawn → complete → parent session 可见 `source:"subagent_completion"`） | 无对应 | 集成 | ❌ |
| 2. EventQueue 事件可达（父 session EventQueue 接收 SubagentTurnCompleted） | 验证 4（事件结构序列化） | 单元 | ⚠️ 仅结构验证，非端到端可达验证 |
| 3. 上下文窗口生效（新 session max_context_tokens ≥ 1_000_000） | 验证 1/2/3（三处默认值检查） | 单元 | ⚠️ 仅单元级，非 config.toml → session 集成验证 |
| 4. 取消状态正确传递（Cancelled 非 Failed） | 无对应 | 集成 | ❌ |

dispatch R-003-005 的 6 个验证项全部在单元测试层级（序列化测试、默认值测试、编译检查、手动代码审查），而 requirements 定义的是**行为级端到端验证**。层级不匹配。

**建议**：在 dispatch R-003-005 中新增 2 个手动集成验证步骤：(a) spawn task → wait → 检查父 session dialog 出现 `source: "subagent_completion"` 注入消息；(b) 取消子 agent → 检查 status = `Cancelled`。

### 7. 容错设计：⚠️ PASS（有新发现）

对照 design 风险与降级表：

| 风险 | 缓解措施 | dispatch v1.1 实现 | 状态 |
|------|---------|-------------------|------|
| spawned task 生命周期 | Arc clone | 编辑3a/4a 7 个 clone 变量 ✅ | ✅ |
| submit_dialog_turn panic | `let _ = ...await` | 错误用 `warn!` 记录；`let _ = event_queue.enqueue()` 静默吞错误 | ⚠️ 静默吞错误，无日志 |
| dialog turn 上下文膨胀 | ≤512 字符 | `output_text = result.as_ref().ok().map(\|sr\| sr.text.clone())` — 无截断 | ❌ |
| refresh 调用位置遗漏 | L2/L3 兜底 | L1+L2+L3 三层均修改 ✅ | ✅ |
| 已有 session 不受影响 | 仅新建时 refresh | ✅ | ✅ |
| R-003-005 集成测试遗漏（新增行） | HIGH，按验收标准逐条实现 | 仅单元级验证，缺端到端 | ❌ |

**新发现/持续风险**：

> **NEW-P1-02**：Cancelled 状态传导路径断裂。requirements R-003-002 验收标准 6 要求"`Cancelled` 正确传递为 `SubagentCompletionStatus::Cancelled` 而非 `Failed`"，但 dispatch edit3b/4b 的状态映射中，`SubagentResultStatus` 内层 match 仅处理 `Completed` 和 `PartialTimeout` 两个变体。如果 `SubagentResultStatus` 没有 `Cancelled` 变体（常见于 tokio::spawn 的 JoinHandle 返回 Err），取消的子 agent 会走 `Err(_) => Failed`，导致 `Cancelled` 被错误映射为 `Failed`。dispatch 中无 cancel token 检查逻辑，`Cancelled` → `SubagentCompletionStatus::Cancelled` 的传导路径在代码层面不存在。

> **NEW-P2-01**：`frontend_projection.rs` 变更在 design 中存在但 dispatch 缺失。design 文件架构表（L84）标注 `agentic.rs` 的变更说明第④项为"`frontend_projection.rs` 添加投影 match arm + 测试"，且绝对路径表（L100）列出 `frontend_projection.rs ← R-003-001, R-003-005`，design 测试条目（L90）分配了 +20 行给 `frontend_projection.rs`。但 dispatch 的 Phase 1（R-003-001）仅编辑 `agentic.rs` 一个文件，未提及 `frontend_projection.rs`。执行者若严格按 dispatch 操作，会遗漏前端事件投影映射。

**持续风险**：

| 风险 | v1.0 编号 | v1.1 状态 |
|------|----------|----------|
| `output_text` 无 ≤512 截断 | G-04 | ❌ 未修复 |
| `metadata` 未标记 `source: "subagent_completion"` | G-05 | ❌ 未修复 |
| `reply_route: None` | G-06 | ❌ 未修复 |
| `SubagentResultStatus` 非穷尽匹配 | G-07 | ❌ 未修复（且现在升级为 NEW-P1-02 的传导断裂） |
| 双闭包代码重复（~35 行 × 2） | G-11 | ❌ 未修复 |
| `AgentDialogPrependedReminder.text` 失败时为空字符串 | 低 | ❌ 未修复 |

### 8. 文件路径：✅ PASS

全 5/5 dispatch 路径指向 `taiji-quant`。design 绝对路径基准表同步正确。G-02 闭合。

---

## 三、新增矛盾与缺口登记

### 新增高优先级（P1）

| # | 缺口 | 等级 | 根因 | 修复方案 |
|---|------|------|------|---------|
| NEW-P1-01 | requirements 字段名与 design/dispatch 不一致：`subagent_session_id` vs `session_id`，`task_id` vs `subagent_dialog_turn_id`，`result_summary: String` vs `output_text: Option<String>` | **P1 高** | v1.1 修复 G-03 时未同步更新 requirements 字段表 | 更新 requirements R-003-001 字段表，统一使用 design/dispatch 的字段名和类型；或反之更新 design/dispatch 对齐 requirements |
| NEW-P1-02 | Cancelled 状态传导路径断裂：`SubagentResultStatus` match 无 `Cancelled` 臂，`Err(_) => Failed`，取消的 subagent 会被标记为 `Failed` 而非 `Cancelled` | **P1 高** | dispatch edit3b/4b 的状态映射缺少 cancel token 检查逻辑 | 方案 A：在 `complete()` 前检查 `cancel_token.is_cancelled()`，若已取消则 status = `Cancelled`；方案 B：确认 `SubagentResultStatus` 是否已有 `Cancelled` 变体，若有则加入 match 臂 |

### 新增中优先级（P2）

| # | 缺口 | 等级 | 根因 | 修复方案 |
|---|------|------|------|---------|
| NEW-P2-01 | `frontend_projection.rs` 变更在 design 中存在（+20 行测试）但 dispatch R-003-001 中未提及 | **P2 中** | design 文件架构表标注了投影映射 + 测试，但 dispatch 任务仅覆盖 `agentic.rs` | dispatch Phase 1 新增编辑 6：在 `frontend_projection.rs` 中添加 `SubagentTurnCompleted → agentic://subagent-turn-completed` 映射 |
| NEW-P2-02 | R-003-005 dispatch 验证项全部单元级，requirements 定义的行为级端到端验证（注入链路、取消状态）缺少对应执行步骤 | **P2 中** | dispatch 6 个验证项为序列化测试/编译检查/手动审查，无集成测试 | dispatch R-003-005 新增验证 7（端到端注入：spawn task → 检查父 session dialog `source:"subagent_completion"`）和验证 8（取消传导：取消子 agent → 检查 status=`Cancelled`） |

### 仍敞口的 v1.0 缺口

| v1.0 编号 | 描述 | v1.1 状态 |
|-----------|------|----------|
| G-04 | `output_text` 无 ≤512 字符截断 | ❌ 未修复 |
| G-05 | metadata 未标记 `source: "subagent_completion"` | ❌ 未修复 |
| G-06 | `reply_route: None` 未利用自动回复机制 | ❌ 未修复 |
| G-07 | `SubagentResultStatus` 非穷尽匹配（现升级为 NEW-P1-02） | ❌ |
| G-09 | R-003-004 验收标准 3（前端消费者）无实现 | ❌ 未解决（需明确是否放后续 R-ID） |
| G-10 | 上下文压缩后不丢失无验证 | ❌ 未修复 |
| G-11 | 双闭包代码重复 | ❌ 未修复 |
| G-12 | 侦察 Bug 1/2 未入回归测试 | ❌ 未修复 |

---

## 四、design 不变式 vs dispatch 合规

| 不变式 | v1.1 dispatch 合规 |
|--------|-------------------|
| 1. `complete()` 在 `emit_event()` 和 `submit_dialog_turn()` 之前 | ✅ 编辑3b/4b 先 `.complete()` 后事件+注入 |
| 2. `live_results` + `changes.notify_waiters()` 不变 | ✅ 未修改 |
| 3. 不删除/重命名已有 AgenticEvent 变体和字段 | ✅ 仅新增 |
| 4. 不新增 crate，不修改 protocol schema | ✅ |
| 5. 已有 session 的 `context_window` 不受影响 | ✅ L2/L3 兜底正确 |

全部 5 条不变式合规。

---

## 五、修复优先级建议

```
必须修复（阻塞执行）:
  NEW-P1-01 — 字段名三文档统一（避免实现者按 requirements 写出不兼容代码）
  NEW-P1-02 — Cancelled 传导路径补全（避免取消状态错误映射为 Failed）

建议修复（降低风险）:
  G-04 — output_text 截断
  G-05 — metadata source 标记
  G-06 — reply_route 或更新 requirements 移除该要求
  NEW-P2-01 — frontend_projection.rs 纳入 dispatch

可选修复（非阻塞）:
  G-09 — 前端消费者范围声明澄清
  G-10 — 上下文压缩回归测试
  G-11 — 双闭包提取公共函数
  G-12 — 侦察 Bug 纳入回归
  NEW-P2-02 — dispatch 增加集成验证步骤
```

---

## 附录：v1.0 vs v1.1 变更对照

| 缺口 | v1.0 | v1.1 | 变化 |
|------|------|------|------|
| G-01 (R-003-005 缺失) | design ❌ / dispatch ❌ | design ✅ / dispatch ✅ | **闭合** |
| G-02 (路径 taiji→taiji-quant) | 5/5 错误 | 5/5 正确 | **闭合** |
| G-03 (status enum vs String) | requirements enum / dispatch String | requirements enum / dispatch enum | **闭合** |
| G-04 (output_text 截断) | ❌ | ❌ | 未变 |
| G-05 (metadata source) | ❌ | ❌ | 未变 |
| G-06 (reply_route) | ❌ | ❌ | 未变 |
| G-07 (非穷尽匹配) | ❌ | ❌ | 未变（升级为 NEW-P1-02） |
| NEW-P1-01 (字段名不一致) | 不存在（v1.0 中字段名也不同但被 G-03 掩盖） | ⚠️ 新暴露 | **新增** |
| NEW-P1-02 (Cancelled 传导断裂) | 不存在（v1.0 无 Cancelled 要求） | ⚠️ 新暴露 | **新增** |
| NEW-P2-01 (frontend_projection.rs 缺失) | — | ⚠️ | **新增** |
| NEW-P2-02 (验证层级不匹配) | 部分 | ⚠️ | **新增** |

**净变化**：3 致命闭合，2 新高优暴露，4 原有高优未动。

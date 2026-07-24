# R-003 跨文档审计报告

**审计日期**：2026-07-25
**审计范围**：r003-requirements.md / r003-design.md / r003-dispatch.md
**审计维度**：8 维（R-ID一致性 / 类型契约覆盖 / 依赖图正确性 / 侦察报告溯源 / 遗漏项 / 验收标准 / 容错设计 / 文件路径）

---

## 审计结论

**FAIL**

共发现 **3 个致命（P0）缺口** + **5 个高（P1）缺口** + **4 个中（P2）缺口**。致命缺口集中在：R-003-005 完全丢失、dispatch 文件路径全部指向 taiji 而非 taiji-quant、需求与设计/派发的 AgenticEvent 字段契约不一致。

---

## 逐维度报告

### 1. R-ID 一致性

| 文档 | R-ID 列表 | 状态 |
|------|----------|------|
| requirements | R-003-001, R-003-002, R-003-003, R-003-004, R-003-005 | 基准（5 个） |
| design（文件架构表） | R-003-001, R-003-002, R-003-003, R-003-004 | ❌ 缺 R-003-005 |
| dispatch（任务总览表） | R-003-001, R-003-002, R-003-003, R-003-004 | ❌ 缺 R-003-005 |

**详细分析**：

- R-003-001：三文档一致（事件变体定义）。requirements 描述 5 字段，design/dispatch 扩展为 8 字段——属设计演进，详见维度 2。
- R-003-002：三文档一致（dialog turn 注入）。
- R-003-003：三文档一致（128K→1M 修复）。requirements 描述为 3 处修改，design/dispatch 同样 3 处。
- R-003-004：requirements（后台通知链路补齐 4 条验收标准）→ design（包含在 coordinator.rs 双闭包改造中）→ dispatch（Phase 3 全量验证）。表面上 dispatch Phase 3 标记为 R-003-004，但其内容仅含 7 条 build/test 命令，不含 requirements 中 R-003-004 的前端 EventQueue 消费者行为验收（验收标准 3）和去重逻辑验收。
- **R-003-005**：requirements 定义为 P0 依赖 R-003-001~R-003-004，含 3 类测试（单元/集成/回归）。design 文件架构表中无此行。dispatch 任务总览表中无此行。Phase 3 验证命令部分覆盖了单元测试（序列化测试、context_window 测试），但**完全缺失**：
  - 集成测试：父 session dialog 中出现 `source: "subagent_completion"` 记录
  - 集成测试：前端 EventQueue 接收到 `SubagentTurnCompleted` 事件
  - 集成测试：新建 session 的 `context_window` 为配置值（1M）
  - 回归测试：`live_results` + `changes.notify_waiters()` 行为不变
  - 回归测试：`cargo test --workspace` 全量通过确认
  - 前端类型检查：`pnpm run type-check:web`

**结论**：FAIL — R-003-005 在 design 和 dispatch 中丢失，且 dispatch Phase 3 的覆盖范围远小于 requirements 定义。

---

### 2. 类型契约覆盖

| 字段 | requirements (R-003-001) | design (D2 + 数据流) | dispatch (Phase 1 编辑1) | 一致性 |
|------|--------------------------|---------------------|--------------------------|--------|
| `parent_session_id` | `String` | `String` | `String` | ✅ |
| `subagent_session_id` | `String` | 重命名为 `session_id` | `session_id: String` | ⚠️ 重命名 |
| `task_id` | `String` | `subagent_dialog_turn_id: String` | `subagent_dialog_turn_id: String` | ⚠️ 重命名 + 语义偏移 |
| `result_summary` | `String`（≤512 字符） | `output_text: Option<String>` | `output_text: Option<String>` | ⚠️ 重命名 + 类型变化（非 Optional → Optional） |
| `status` | `SubagentCompletionStatus` 枚举（Completed/Failed/Cancelled） | `String`（"completed"/"partial_timeout"/"failed"） | `String` | ❌ 枚举→字符串，值集合不同 |
| — | 无 | `parent_dialog_turn_id: String` | `parent_dialog_turn_id: String` | ✅（design 新增） |
| — | 无 | `parent_tool_call_id: String` | `parent_tool_call_id: String` | ✅（design 新增） |
| — | 无 | `agent_type: Option<String>` | `agent_type: Option<String>` | ✅（design 新增） |

**关键差异**：

1. **状态枚举 vs 字符串**：requirements 定义 `SubagentCompletionStatus` 枚举含 `Cancelled` 变体；design/dispatch 使用 `String`，值集合为 `"completed"` / `"partial_timeout"` / `"failed"`——**缺少 `"cancelled"` 状态**。
2. **`task_id` → `subagent_dialog_turn_id`**：requirements 的 `task_id` 语义更接近业务层 task 标识；design/dispatch 使用 `subagent_dialog_turn_id` 更接近 session 层标识。两概念可能不等价——一个 task 可能跨多个 dialog turn。
3. **`result_summary` → `output_text`**：requirements 要求 ≤512 字符摘要；dispatch 中 `output_text` 直接取 `result.text`（完整输出），**无截断逻辑**。且 `output_text` 为 `Option<String>`，而 requirements 的 `result_summary` 为必填 `String`。
4. **design/dispatch 新增字段**（`parent_dialog_turn_id`, `parent_tool_call_id`, `agent_type`）：requirements 未定义但属于合理的设计细化——在 design 中已记录，不算缺口。

**结论**：FAIL — `status` 字段类型不一致（枚举 vs 字符串 + 值集合差异）且 `result_summary`/`output_text` 无截断 enforce。

---

### 3. 依赖图正确性

**requirements 依赖链**：
```
R-003-001 (无依赖) ──┐
                      ├──→ R-003-002 ──┐
R-003-003 (无依赖) ──┘                 ├──→ R-003-004 ──→ R-003-005
                                       │
                                       └──→ (直接依赖)
```

**dispatch Phase 拓扑**：
```
Phase 1（并行）: R-003-001 + R-003-003
    │
Phase 2（串行）: R-003-002（依赖 Phase 1）
    │
Phase 3（验证）: R-003-004（依赖 Phase 1 + Phase 2）
```

**分析**：
- R-003-001 和 R-003-003 无相互依赖，并行执行 ✅
- R-003-002 依赖 R-003-001（需事件类型先定义），串行正确 ✅
- R-003-003 与 R-003-001 无依赖关系，并行执行安全 ✅
- dispatch Phase 3 实际依赖 Phase 2（R-003-002），因为 R-003-004 的验证需 dialog turn 注入完成 ✅
- 无循环依赖 ✅
- **但 R-003-005 缺失**使依赖链不完整 ❌
- design 的依赖图中 `events ← coordinator → scheduler` + `coordinator → session, agentic_api` 方向正确，与代码实际引用关系一致 ✅

**结论**：PASS（拓扑正确）但有警告（R-003-005 断裂导致链不完整）。

---

### 4. 侦察报告溯源

已识别 4 份侦察报告（均位于 `E:\finance-trading\lvpa\software\taiji\reports\`）：

| 侦察报告 | R-003 架构决策追溯 |
|----------|-------------------|
| `task-vs-session-reconnaissance.md` | **D1**（复用 SessionMessage 注入路径）：报告维度四（通信）证明 `submit_dialog_turn` 7 层链路已就绪，Task 已有 `background_subagent_outcomes` 注册表。**D3**（coordinator spawned task 闭包改造）：报告定位了 `execute_hidden_subagent_internal` 调用点和 `background_subagent_outcomes.complete()` 位置。 |
| `recursive-subagent-depth-limit-recon.md` | **D3**（间接支撑）：报告验证了 `DelegationPolicy` 全链路（Task→coordinator→子代理）一致性，确认 spawned task 中生命周期无隐藏限制。 |
| `taiji-bitfun-comprehensive-duplication-audit.md` | **D1**（间接支撑）：审计结论"复用基础设施不重复造轮子"对齐 design 原则"不新建通信通道，复用已有 7 层注入链路"。 |
| `session-analysis-20260721-report.md` | 弱相关（会话创建路径分析），为 R-003-003 的 128K 修复提供背景但非直接溯源。 |

**未溯源决策**：
- **D4**（128K→1M 三层修复的具体位置 L1537-1546、L187、L1270）：行号选择基于代码检查（非侦察报告）。可接受——属代码层面的精准定位，侦察报告已证明 session 创建链路的整体结构。
- **D2 新增 8 字段的具体设计**（`parent_dialog_turn_id` 等 3 个额外字段）：侦察报告中 lineage 分析（task-vs-session-reconnaissance.md 维度一）提到了 `parent_dialog_turn_id` 和 `parent_tool_call_id`，但未显式建议将这些字段加入事件体。

**结论**：PASS — 4 个架构决策均有侦察报告支撑（D4 属代码级定位，合理；D2 字段设计有 lineage 分析背景）。

---

### 5. 遗漏项：侦察报告缺口未被 R-ID 覆盖

基于 4 份侦察报告，提取未被 R-003-001 ~ R-003-005 覆盖的缺口：

#### 5.1 来自 `task-vs-session-reconnaissance.md`

| 缺口编号 | 描述 | 严重度 | 是否被 R-003 覆盖 |
|----------|------|--------|-------------------|
| G-1 | Task 工具无 List action | **高** | ❌ 未覆盖 |
| G-2 | Task 工具无 Delete action | **中** | ❌ 未覆盖 |
| G-3 | SessionControl 不支持 Fork 模式 | **低** | ❌ 未覆盖 |
| G-4 | SessionControl 不支持复用已有 Subagent session | **低** | ❌ 未覆盖 |
| G-5 | SessionMessage 无前台同步模式 | **低** | ❌ 未覆盖 |
| G-6 | SessionMessage 无并发控制 | **低** | ❌ 未覆盖 |
| G-7 | 两系统均无后端转录导出工具 | **低** | ❌ 未覆盖 |
| G-8 | SessionControl 无级联收集 | **中** | ❌ 未覆盖 |
| G-9 | SessionControl 创建的 Standard 会话无超时/并发/回滚 | **中** | ❌ 未覆盖 |
| G-10 | SessionControl lineage 缺失 tool_call_id 等 | **低** | ❌ 未覆盖 |
| G-11 | Task lineage 缺失 depth 字段 | **低** | ❌ 未覆盖 |

> 注：R-003 范围限定为"Task-Session 通信统一 + 上下文修复"，上述缺口中的 G-1/G-2 属于独立功能增强，合理放在后续 R-ID 中。但 **design 和 requirements 均未声明这些缺口为后续跟进项**。

#### 5.2 来自 `recursive-subagent-depth-limit-recon.md`

| 缺口编号 | 描述 | 严重度 | 是否被 R-003 覆盖 |
|----------|------|--------|-------------------|
| Bug 1 | `delegation_policy_child_blocks_recursive_spawn_without_losing_depth` 测试失败 | **高** | ❌ 未覆盖 |
| Bug 2 | `call_impl_rejects_nested_subagent_at_max_depth` 测试失败 | **高** | ❌ 未覆盖 |
| Bug 3 | 注释过时（MAX_FISSION_DEPTH=5→10） | **低** | ❌ 未覆盖 |
| 并发瓶颈 | SubagentConcurrencyLimiter 默认 5 并发可能不足 | **中** | ❌ 未覆盖 |

> 注：Bug 1/2 是已有代码的测试兼容性问题，非 R-003 引入。但 R-003-005 的回归测试验收标准（`cargo test --workspace` 全量通过）如果执行，会发现这些失败。**dispatch Phase 3 未列出 `cargo test -p bitfun-runtime-ports`**，可能导致遗漏。

#### 5.3 来自 `taiji-bitfun-comprehensive-duplication-audit.md`

| 缺口编号 | 描述 | 严重度 | 是否被 R-003 覆盖 |
|----------|------|--------|-------------------|
| P0-事件基础设施自建 | taiji 自建事件系统 vs BitFun events crate | **致命** | ❌ 未覆盖（但 R-003 新增事件走的是 BitFun events crate，方向正确） |

> 注：审计报告的致命发现已在 R-003 的设计决策中隐含处理（使用 BitFun 原生 `AgenticEvent` 而非自建），但未显式引用审计结论。

**结论**：FAIL — 2 个高优先级 Bug（递归深度测试失败）未被 R-003-005 的测试命令覆盖，11 个 Task-Session 缺口无人认领（至少应在 design 中标注为后续 R-ID）。

---

### 6. 验收标准覆盖

逐条对照 requirements 验收标准 vs dispatch 验证步骤：

#### R-003-001

| # | 验收标准 | dispatch 覆盖 | 状态 |
|---|---------|--------------|------|
| 1 | 新增 5 字段变体 | 编辑1 插入 8 字段变体（字段集不同，见维度 2） | ⚠️ 字段不一致 |
| 2 | 同步路径 emit 事件 | 编辑3b/4b 在 `.complete()` 后 `emit_event()` | ✅ |
| 3 | Serialize/Deserialize derive | 编辑 4 添加序列化往返测试 | ✅ |
| 4 | `cargo check --workspace` + 单元测试 | Phase 1 验证: `cargo test -p bitfun-events` | ✅ |

#### R-003-002

| # | 验收标准 | dispatch 覆盖 | 状态 |
|---|---------|--------------|------|
| 1 | `complete()` 后调 `submit_dialog_turn`，注入 reply_route | 编辑3b/4b 调用 `submit_agent_dialog_turn_reject_if_busy`，但 `reply_route: None` | ❌ reply_route 为 None |
| 2 | 注入 content 含 role/status/key output，metadata 标记 `source: "subagent_completion"` | dispatch 中 `metadata: serde_json::Map::new()`（空 Map），`AgentSubmissionSource::AgentSession` | ❌ 无 source 标记 |
| 3 | 父 session dialog 历史可见子 agent 完成记录 | Phase 3 验证仅 `cargo test`，无集成测试 | ❌ 未验证 |
| 4 | 上下文压缩后子 agent 结果不丢失 | 无验证 | ❌ 未覆盖 |
| 5 | `live_results` 向后兼容 | 无显式验证 | ❌ 未覆盖 |

#### R-003-003

| # | 验收标准 | dispatch 覆盖 | 状态 |
|---|---------|--------------|------|
| 1 | coordinator.rs L1537-1546 加 refresh | 编辑1 L1569 插入 `refresh_session_context_window` | ✅（行号偏移属正常） |
| 2 | session.rs L187 从配置读取 | 编辑2 `128128 → 1_048_576`（改默认值，非读配置） | ⚠️ 语义偏差：改默认值 ≠ 从配置读取 |
| 3 | agentic_api.rs L1270 API 层刷新 | 编辑3 `unwrap_or(128128) → unwrap_or(1_048_576)` | ✅ |
| 4 | 验证 config.toml `context_window=1048576` 后新 session 值为 1M | 无集成测试 | ❌ 未覆盖 |
| 5 | 已有 session 不受影响 | design 不变式 5 声明，无测试 | ⚠️ 设计保证，可接受 |

#### R-003-004

| # | 验收标准 | dispatch 覆盖 | 状态 |
|---|---------|--------------|------|
| 1 | 后台 spawned task 完成→emit 事件 + submit_dialog_turn | 编辑3b/4b 双闭包均添加 ✅ | ✅ |
| 2 | 同步路径同样发射事件 | 同步路径（直接执行分支，编辑4b）同样处理 ✅ | ✅ |
| 3 | 前端 EventQueue 消费者刷新 + 去重 | 无前端代码变更，无验证 | ❌ 未覆盖 |
| 4 | `complete()` 不持有 EventQueue 问题通过 Arc clone 解决 | 编辑3a/4a clone `event_queue_for_spawn` ✅ | ✅ |

#### R-003-005

| # | 验收标准 | dispatch 覆盖 | 状态 |
|---|---------|--------------|------|
| 1 | 单元测试：序列化往返 | Phase 1 序列化测试 ✅ | 部分 |
| 2 | 单元测试：complete→submit_dialog_turn mock | 无 mock 测试 | ❌ |
| 3 | 单元测试：refresh 三处参数正确性 | 无专项测试 | ❌ |
| 4 | 集成测试：父 session dialog source 标记 | 无 | ❌ |
| 5 | 集成测试：前端 EventQueue 接收事件 | 无 | ❌ |
| 6 | 集成测试：新建 session context_window=1M | 无 | ❌ |
| 7 | 回归测试：live_results 行为不变 | 无 | ❌ |
| 8 | 回归测试：cargo test --workspace 全量通过 | Phase 3 包含 | 部分 |
| 9 | 回归测试：pnpm run type-check:web | Phase 3 包含 | ✅ |

**结论**：FAIL — R-003-002 的 5 条验收标准中 4 条未覆盖，R-003-005 的 9 条验收标准中 7 条未覆盖或仅部分覆盖。

---

### 7. 容错设计

对照 design 风险与降级表，检查 dispatch 实现：

| 风险 | 缓解措施 | dispatch 实现 | 状态 |
|------|---------|--------------|------|
| spawned task event_queue 生命周期 | Arc clone | 编辑3a/4a `event_queue_for_spawn` clone ✅ | ✅ |
| submit_dialog_turn panic | `let _ = ...await` 吞错误 | dispatch 中错误用 `warn!` 记录但 `let _ = event_queue.enqueue()` 吞错误 ✅ | ✅ |
| dialog turn 上下文膨胀 | ≤512 字符摘要 | dispatch 无截断逻辑，`output_text` 取完整 `result.text` | ❌ |
| refresh 调用位置遗漏 | L2/L3 兜底 | L1+L2+L3 三层均修改 ✅ | ✅ |
| 已有 session 不受影响 | 仅新建时 refresh | 正确 ✅ | ✅ |

**dispatch 中未在 design 风险表中记录的新风险**：

| 风险 | 位置 | 严重度 | 说明 |
|------|------|--------|------|
| `SubagentResultStatus` 非穷尽匹配 | dispatch 编辑3b/4b | **高** | match 仅处理 `Completed` 和 `PartialTimeout`，若上游新增变体（如 `Cancelled`），将编译失败或逻辑缺失 |
| `get_global_scheduler()` 返回 None | dispatch 编辑3b/4b | **低** | 有 `if let Some(scheduler)` 保护，安全降级——仅 emit 事件不注入 dialog turn |
| `submit_agent_dialog_turn_reject_if_busy` 返回 `Err` | dispatch 编辑3b/4b | **低** | 仅 `warn!` 日志，不重试。design 的降级方案（降级为仅 emit 事件）与此一致 |
| 双闭包代码重复 | dispatch 编辑3b+4b | **中** | 两处闭包中事件发射 + dialog turn 提交逻辑完全重复（~35 行 × 2），存在维护一致性风险 |
| `AgentDialogPrependedReminder.text` 使用 `unwrap_or_default()` | dispatch 编辑3b/4b | **低** | 当 `output_text` 为 None（失败时），reminder text 为空字符串，前端可能显示空白提醒 |

**结论**：FAIL — `output_text` 无 512 字符截断（design 缓解措施未落地），且 `SubagentResultStatus` 非穷尽匹配存在编译脆弱性。

---

### 8. 文件路径

| 设计要求路径（绝对路径基准） | dispatch 实际路径 | 一致性 |
|---------------------------|------------------|--------|
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\contracts\events\src\agentic.rs` | `E:\finance-trading\lvpa\software\taiji\src\crates\contracts\events\src\agentic.rs` | ❌ taiji ≠ taiji-quant |
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\coordination\coordinator.rs` | `E:\finance-trading\lvpa\software\taiji\src\crates\assembly\core\src\agentic\coordination\coordinator.rs` | ❌ |
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\assembly\core\src\agentic\coordination\scheduler.rs` | `E:\finance-trading\lvpa\software\taiji\src\crates\assembly\core\src\agentic\coordination\scheduler.rs` | ❌ |
| `E:\finance-trading\lvpa\software\taiji-quant\src\crates\execution\agent-runtime\src\session.rs` | `E:\finance-trading\lvpa\software\taiji\src\crates\execution\agent-runtime\src\session.rs` | ❌ |
| `E:\finance-trading\lvpa\software\taiji-quant\src\apps\desktop\src\api\agentic_api.rs` | `E:\finance-trading\lvpa\software\taiji\src\apps\desktop\src\api\agentic_api.rs` | ❌ |

**全 5 个文件路径均指向 `taiji` 而非 `taiji-quant`。** requirements 明确声明工作区为 `E:\finance-trading\lvpa\software\taiji-quant`。design 文件路径表中写的是相对路径（`contracts/events/src/agentic.rs`），但绝对路径基准也是 `taiji-quant`。dispatch 中所有绝对路径均使用了错误的根目录。

**结论**：FAIL — 5/5 文件路径错误，执行者若按 dispatch 路径操作将修改 taiji 仓库而非 taiji-quant 仓库。

---

## 缺口登记表

| # | 缺口 | 等级 | 根因 | 修复方案 | 所属 R-ID |
|---|------|------|------|---------|-----------|
| G-01 | R-003-005 在 design 和 dispatch 中完全丢失 | **P0 致命** | dispatch 任务总览表未列入，Phase 3 验证命令不足以覆盖集成/回归测试 | design 文件架构表新增 R-003-005 行；dispatch 新增 Phase 4「R-003-005 端到端集成验证」含 3 类测试用例 | R-003-005 |
| G-02 | dispatch 所有文件路径指向 `taiji` 而非 `taiji-quant` | **P0 致命** | 路径根目录错误 | 全局替换 `E:\finance-trading\lvpa\software\taiji\` → `E:\finance-trading\lvpa\software\taiji-quant\` | R-003-001~004 |
| G-03 | requirements `status: SubagentCompletionStatus` 枚举 vs design/dispatch `status: String` | **P0 致命** | requirements 定义的枚举类型在 design 阶段被降级为字符串 | 二选一：(A) 回退 requirements 对齐 design（推荐，字符串更灵活）；(B) 在 dispatch 中定义 `SubagentCompletionStatus` 枚举 | R-003-001 |
| G-04 | `output_text` 无 ≤512 字符截断 | **P1 高** | dispatch 直接取 `result.text` 未截断，design 风险表承诺的缓解措施未落地 | 编辑3b/4b 中 `output_text` 赋值时加 `chars().take(512).collect()` 或 `.truncate(512)` | R-003-002 |
| G-05 | R-003-002 验收标准 2：metadata 未标记 `source: "subagent_completion"` | **P1 高** | dispatch 中 `metadata: serde_json::Map::new()` 为空 | 在 `metadata` 中插入 `{"source": "subagent_completion"}` | R-003-002 |
| G-06 | R-003-002 验收标准 1：`reply_route: None` 未利用已有自动回复机制 | **P1 高** | dispatch 未构造 `reply_route`，与 requirements "复用 resolve_agent_session_reply_action() 自动回复机制" 矛盾 | 如 requirements 要求自动回复，需构造 `AgentSessionReplyRoute` 指向父 session；如 design 已重新评估无需自动回复，需更新 requirements | R-003-002 |
| G-07 | `SubagentResultStatus` 非穷尽匹配 | **P1 高** | dispatch match 仅处理 `Completed` / `PartialTimeout`，上游可能新增变体（如 `Cancelled`） | 加通配 arm `_ => "failed"` 或用 `#[non_exhaustive]` | R-003-002 |
| G-08 | R-003-005 集成测试全部缺失（父 session dialog source 标记、前端 EventQueue 事件接收、context_window 配置验证） | **P1 高** | dispatch Phase 3 仅含 build/test 命令，无集成测试步骤 | dispatch 新增 Phase 4 含 3 个集成测试用例的详细步骤 | R-003-005 |
| G-09 | R-003-004 验收标准 3（前端 EventQueue 消费者）无任何实现 | **P2 中** | R-003 范围声明"不改前端 UI"，但验收标准要求前端消费者行为 | 更新 requirements：明确前端消费者放到后续 R-ID，或将本次改为仅验证后端事件正确入队 | R-003-004 |
| G-10 | R-003-002 验收标准 4（上下文压缩后不丢失）无验证 | **P2 中** | 未列入任何 Phase | dispatch Phase 4 新增上下文压缩回归测试 | R-003-002 |
| G-11 | 双闭包代码重复（~35 行 × 2） | **P2 中** | 调度路径和直接执行路径两处闭包中事件+注入逻辑完全重复 | 提取 `emit_and_inject_subagent_completion()` 辅助函数，两处闭包各调一行 | R-003-002 |
| G-12 | 侦察报告 Bug 1/2（递归深度测试失败）未被 R-003-005 回归测试覆盖 | **P2 中** | dispatch Phase 3 未包含 `cargo test -p bitfun-runtime-ports` | Phase 3 验证命令新增 `cargo test -p bitfun-runtime-ports` | R-003-005 |

---

## 附录：design 不变式 vs dispatch 合规检查

| 不变式 | dispatch 合规 |
|--------|-------------|
| 1. `complete()` 在 `emit_event()` 和 `submit_dialog_turn()` 之前执行 | ✅ 编辑3b/4b 先 `.complete()` 后事件+注入 |
| 2. `live_results` + `changes.notify_waiters()` 保持不变 | ✅ 未修改相关代码 |
| 3. 不删除/重命名已有 `AgenticEvent` 变体和字段 | ✅ 仅新增变体 |
| 4. 不新增 crate，不修改 protocol schema | ✅ 无新增 crate |
| 5. 已有 session 的 `context_window` 不受影响 | ✅ L2/L3 兜底正确 |

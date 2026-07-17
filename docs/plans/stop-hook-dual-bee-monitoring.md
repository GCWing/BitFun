# Stop Hook + B01/C01 双蜂监控实现计划

## Background

### 当前状态

BitFun 只有 **post-tool-call** 级别的 hook：

- `SuccessfulToolPostCall`：记录 DeepReview 共享上下文
- `BehaviorGuard`：stale strategy 检测、Read-before-Edit、LionHeart 路径保护、PowerShell+中文检测

所有 hook 在 `call_with_tool_runtime_hooks()`（[tool_context_runtime.rs:190](src/crates/assembly/core/src/agentic/tools/tool_context_runtime.rs)）中触发，仅在单个工具调用成功后执行。

### 缺失

- **无 turn/session 级别的生命周期 hook**：Stop、SessionStart、SessionEnd
- **无法在回合结束后进行整体审查**：只能逐工具检查，看不到"本轮整体有没有跑偏"
- **B01 提示蜂和 C01 审查蜂只是 SKILL.md 中的提示词级模拟**，不是真正的代码级拦截

### 参考设计

1. **cc-haha `/goal`**（[goalState.ts](https://github.com/anthropics/claude-code/blob/main/src/goals/goalState.ts)）：`addSessionHook(context, threadId, 'Stop', '', hook)` — PromptHook 挂载 Stop 事件，每轮回合结束后用小 LLM 评估，返回 `{ok: true/false, reason}`
2. **用户 V10 蔷薇 Harness**：Plan→Do→Check→Act 监督链，"任何 agent 不能单独做决定，每个输出至少被另一个 agent 审查过"
3. **用户 V12 梦蝶军团**：Odysseus 军师作为被动观察者，悄悄话通道通信，用户对监督 agent 无感知

### 核心洞察

B01（提示蜂）和 C01（审查蜂）本质是**同一个 Stop hook 机制**的两个 handler，区别仅在于评估 prompt：

```
Stop 事件触发（每回合 agent 产出后）
  │
  ├─→ B01 handler: "本轮执行需要什么上下文？缺了吗？"
  │      └─ 缺 → 注入补充知识 → 下一轮
  │
  └─→ C01 handler: "本轮违反了哪条铁则？"
         └─ 违 → Abort + fix_instruction → 下一轮修正
```

双蜂挂同一个 Stop 事件，形成一个完整的"军师"模式——沉默观察、精准出手。

## Implementation Approach

### 架构变更

```
execution_engine.rs: execute_dialog_turn_impl()
  │
  ├─ [每 round 结束]
  │     │
  │     ├─ (现有) 检查 continue_loop / max_rounds
  │     │
  │     └─ (新增) run_stop_hooks(tool_name="__turn_end__", round_result, context)
  │              │
  │              ├─ RuntimeHookKind::Stop → B01 提示蜂 handler
  │              │     └─ 评估上下文完整性 → 注入补充提示
  │              │
  │              └─ RuntimeHookKind::Stop → C01 审查蜂 handler
  │                    └─ 评估铁则违规 → Abort / Continue
  │
  └─ [loop 继续或终止]
```

### 三层改动

| 层 | 文件 | 改动 |
|---|---|---|
| **契约层** | `execution/agent-runtime/src/post_call_hooks.rs` | 新增 `RuntimeHookKind::Stop`；扩展 `RuntimeHookRegistry` 支持 Stop hook；新增 `StopHookExecutor` trait |
| **引擎层** | `assembly/core/src/agentic/execution/execution_engine.rs` | 在 round 结束后调用 `run_stop_hooks()` |
| **实现层** | `assembly/core/src/agentic/tools/post_call_hooks.rs` | 实现 B01 context-check handler；实现 C01 iron-rule-check handler |

## File Change List

### 1. `src/crates/execution/agent-runtime/src/post_call_hooks.rs` — 契约层

**新增 `RuntimeHookKind::Stop`**：
```rust
#[non_exhaustive]
pub enum RuntimeHookKind {
    SuccessfulToolPostCall,
    DeepReviewSharedContextToolUse,
    BehaviorGuard,
    Stop,  // ← 新增
}
```

**新增 `StopHookContext`**：携带回合级别的审查信息
```rust
pub struct StopHookContext {
    pub session_id: String,
    pub turn_id: String,
    pub round_number: u32,
    pub tool_calls_in_round: Vec<ToolCallSummary>,  // 本轮所有工具调用
    pub assistant_message_summary: String,           // agent 本轮输出摘要
    pub file_reads_in_round: Vec<String>,            // 本轮读取的文件
    pub file_edits_in_round: Vec<String>,            // 本轮编辑的文件
}
```

**新增 `StopHookExecutor` trait**：
```rust
pub trait StopHookExecutor {
    fn context_guard(&mut self, ctx: &StopHookContext) -> HookResult;      // B01
    fn behavior_guard(&mut self, ctx: &StopHookContext) -> HookResult;     // C01
}
```

**新增 `run_stop_hooks()` 函数**：遍历注册的 Stop handler，收集 `HookResult`。

### 2. `src/crates/assembly/core/src/agentic/execution/execution_engine.rs` — 引擎层

在 `execute_dialog_turn_impl()` 的 round loop 中，每轮结束后插入：

```rust
// 现有：压缩上下文、检查 max_rounds、决定是否 continue
// ...

// 新增：Stop hook 注入点
if let Some(stop_hook) = &self.stop_hook_executor {
    let ctx = StopHookContext {
        session_id: ...,
        turn_id: ...,
        round_number: current_round,
        tool_calls_in_round: collect_tool_calls_this_round(),
        assistant_message_summary: summarize_assistant_output(),
        file_reads_in_round: session_file_reads(),
        file_edits_in_round: session_file_edits(),
    };
    let result = run_stop_hooks(&ctx, stop_hook);
    match result {
        HookResult::Continue => { /* 正常继续 */ }
        HookResult::Abort { reason, fix_instruction, .. } => {
            // 注入拦截消息到下一轮
            inject_guard_message(reason, fix_instruction);
        }
    }
}
```

### 3. `src/crates/assembly/core/src/agentic/tools/post_call_hooks.rs` — 实现层

**B01 提示蜂 handler（context_guard）**：
```rust
fn context_guard(&mut self, ctx: &StopHookContext) -> HookResult {
    // 1. 检查本轮是否读取了要编辑的文件 → 已在 per-tool FILE_READ_TRACKER 中检查
    // 2. 检查是否有现成工具可用但被忽略 → 提示
    // 3. 检查上下文是否足够（被编辑的文件是否读了足够的行数）
    // 4. 当前为 Should-level，不 Abort，只注入提示
    HookResult::Continue  // 暂不拦截，仅记录
}
```

**C01 审查蜂 handler（behavior_guard）**：
```rust
fn behavior_guard(&mut self, ctx: &StopHookContext) -> HookResult {
    // 复用 per-tool 级别的 FILE_READ_TRACKER + STALE_TRACKER 数据
    // 从回合级别做综合判断：

    // 1. Read-before-Edit 聚合检查：本轮所有 Edit 的文件，是否都在本轮或之前 Read 过
    for edit in &ctx.file_edits_in_round {
        if !ctx.file_reads_in_round.iter().any(|r| normalize_path(r) == normalize_path(edit)) {
            // 已在 per-tool 级别拦截，此处做回合级聚合报告
        }
    }

    // 2. 重复工具调用检测：同一工具连续 N 轮都出现 → 可能陷入循环
    //    （比 per-tool 的 3 次连续调用更宏观）

    // 3. 全局扫描检查：agent 是否在钻牛角尖（心智模型1）
    //    例如：连续 3 轮都只调用同一个工具类型

    HookResult::Continue  // 本期先建立框架，具体规则后续迭代
}
```

### 4. `src/crates/execution/agent-runtime/src/runtime.rs` — 注册

在 `AgentRuntime` 构建时注册 Stop hook executor：
```rust
runtime_hook_registry
    .register(RuntimeHookPlan::new("stop-bee-guard", RuntimeHookKind::Stop)
        .with_order(200)
        .with_timeout_millis(5_000));
```

## 实施步骤

### Phase 1：契约层（最小改动，先跑通链路）

1. `post_call_hooks.rs`（execution 层）：加 `RuntimeHookKind::Stop`、`StopHookContext`、`StopHookExecutor` trait、`run_stop_hooks()`
2. 跑 `cargo check` 确认编译

### Phase 2：引擎层（挂载注入点）

3. `execution_engine.rs`：在 round loop 结束后构造 `StopHookContext`、调用 `run_stop_hooks()`
4. 跑 `cargo check` + `cargo test -p bitfun-agent-runtime --test post_call_hook_execution_contracts`

### Phase 3：实现层（双蜂 handler）

5. `post_call_hooks.rs`（assembly 层）：实现 B01 context_guard + C01 behavior_guard
6. 跑完整 `cargo test -p bitfun-core`

### Phase 4：集成验证

7. 端到端手动验证：开启 Team 模式，确认 Stop hook 在每轮后触发
8. 跑标准验证套件：`cargo check --workspace` + `pnpm run type-check:web`

## 与已有代码的关系

- **不修改** `tool_pipeline.rs`（per-tool 级别 hook 保持不变）
- **不修改** 现有的 `BehaviorGuard`（per-tool 级别继续生效）
- **新增的 Stop hook 是回合级别的补充**，不是替代
- Per-tool hook 和 Stop hook 形成两层防御：逐工具检查 + 回合结束后全局审查

## 后续扩展方向（本期不做）

- `SessionStart` / `SessionEnd` hook：session 生命周期级审查
- PromptHook 类型：用小 LLM（如 Haiku/DeepSeek-Flash）评估条件，替代纯 Rust 规则
- 自动挂载 C01 审查蜂 session：Stop hook 检测到严重违规时，自动 spawn 独立审查 session

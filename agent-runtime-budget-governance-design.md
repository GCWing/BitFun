# 设计文档：LLM 输出截断检测、恢复与预算治理

> 日期：2026-04-27  
> 状态：Draft  
> 范围：所有使用 LLM 的 Agent 场景，不限于深度审核  
> 目标：消除静默截断，建立结构化截断信号，并逐步引入请求预算治理

---

## 背景与问题

### 事件

2026-04-27 的会话中，模型（glm-5.1）在执行工具调用时突然停止输出。日志显示：

- 输入 token：约 100,068
- 输出 token：仅 64-261
- 无 `finish_reason`
- 无 ERROR/WARN 日志
- 应用无任何用户提示

用户看到的是“模型突然停止”，系统没有说明是网络问题、模型截断、工具调用失败，还是任务完成。

### 初步根因

长上下文挤占了输出 token 预算，模型没有足够空间完成工具调用参数 JSON。当前 `stream_processor` 在 `TimedStreamItem::End` 分支直接把流结束视为正常结束，未检查：

- provider 是否报告 `length` / `max_tokens` 类 finish reason；
- 是否存在 pending tool call；
- pending tool call 的 JSON 是否完整；
- 是否需要向上游标记 partial recovery；
- 是否需要向用户显示明确提示。

这类问题不是单纯网络错误，同请求自动重试通常无法解决，甚至会放大资源消耗。

---

## 设计目标

1. **不再静默失败**：任何可识别的输出截断都必须进入结构化恢复路径，并向用户呈现原因。
2. **机器可判定、用户可理解**：系统内部使用结构化 kind/status；UI 显示本地化、人类可读提示。
3. **安全优先**：半截工具调用不得自动执行；语法修复仅用于完成事件链路和诊断。
4. **避免无意义重试**：区分网络中断、watchdog timeout、provider max tokens、工具参数截断等情况。
5. **逐步引入预算治理**：先止血，再从请求前预算、任务拆分、输出落盘等方向降低截断发生率。
6. **保持平台边界**：产品逻辑位于 core / ai-adapters；前端只通过事件和 adapter 消费结构化状态；UI 组件不直接依赖 Tauri。
7. **不得新增用户感知中止**：新增检测、预检、压缩、拦截只能减少原本会静默失败、突然断掉或不可恢复的场景；若某项治理动作会让原本可正常完成的对话变成用户可感知阻断，则该动作默认无效，必须改为后台恢复、轻提示或不实现。

### 产品体验约束

普通对话和 Deep Review 共享同一条体验原则：**把不可控断裂变成可恢复连续体验，而不是用更早的阻断换取技术上更安全的失败**。

- 预算预检不能因为低置信估算直接阻断请求；优先触发本地上下文压缩、摘要化、输出落盘或任务内部拆分。
- 压缩动作如果耗时短，应作为后台治理，不打断用户；如果可能长耗时，必须进入可见的进度状态，避免用户看到长时间无输出。
- 错误呈现应表达“模型或外部服务出现预期外中断，BitFun 已保留当前结果并尝试恢复/引导下一步”，避免把部分成功渲染成 fatal error。
- 工具拦截应最小化：只读工具不因同批其他工具截断而关联拦截；写操作优先按依赖关系和副作用级别做最小拦截。
- 任何新增动作的验收标准都要包含用户感知指标：中止次数不增加、可恢复提示更明确、有效输出不丢失。

---

## 非目标

- 不在 Phase 1 止血阶段重写 Deep Review 的完整调度系统；runtime-bounded scheduling 作为 Phase 2 后续治理能力分阶段引入。
- 不自动“续写”工具调用参数。
- 不把所有 provider 行为强行统一成同一个文本错误消息。
- 不因为截断检测而执行语义不可靠的工具调用。
- 不用字符串包含关系作为长期架构契约。

---

## 与 Context Reliability Architecture 的关系

根目录 `context-reliability-architecture.md` 是 **Context Reliability Architecture**，负责回答“上下文中哪些事实可信、哪些事实必须跨压缩保留、长任务如何可审计和可恢复”。本文是 **runtime 截断、预算、调度与大输出治理方案**，负责回答“模型输出或工具/子任务结果即将超出预算、已经截断、或被 gateway 限流时，runtime 如何检测、排队、恢复、落盘和呈现”。

因此两篇文档不合并为一篇总方案。合并会让基础上下文架构和运行时控制面混在一起，反而降低可维护性。正确关系是：`context-reliability-architecture.md` 提供信任、证据、压缩契约和 context profile；本文消费这些能力，并把截断恢复、scheduler、artifact、spill、large write 等运行时事件回写为可审计事实。

### 与 `deep-review-design.md` 已实现基线的关系

本轮对照按 `deep-review-design.md` 已实现处理：Deep Review 已拥有 Strategy Engine、Architecture / Frontend reviewer、predictive timeout、dynamic concurrency policy、partial result capture、retry budget、Judge overlap handling、strategy directive / model plumbing 和 continuation / remediation 基础能力。本文不重新实现这些 Deep Review 专项能力。

本文只在已实现基线之上补三类增量：

1. **统一 runtime 调度**：Deep Review 的 `DeepReviewConcurrencyPolicy` / `max_parallel_instances` 作为 reviewer policy 输入和上限；真实执行顺序、gateway permit、queue、retry backoff、parent cancellation cleanup 由 `SubagentScheduler` 和 AI request limiter 负责。
2. **统一事件语义**：Deep Review 已有 `partial_timeout` / retry / timeout 结果需要映射到通用 scheduler event。对外统一为 `completed_with_partial`、`timed_out`、`retry_waiting`、`failed` 等状态，避免前端和 judge 同时消费两套状态机。
3. **统一证据与预算治理**：Deep Review 已有 partial output 和 launch manifest；本文只补 artifact 落盘、reviewer output budget、gateway-keyed limiter、token/byte/diff line 预算和跨场景观测字段。

因此，任何实现若让 `reviewTeamService.ts` / Deep Review orchestrator 自己做一套 batching + retry，同时 runtime scheduler 再做一套 queue + retry，都视为无效实现。Deep Review policy 可以决定“哪些 reviewer、什么 scope、什么超时/重试预算”；runtime 只能有一个最终调度器决定“何时运行、是否重试、如何释放 permit”。

### 重复实现禁止与收敛规则

新增实现必须先归类到已有 owner。若出现相似状态、相似重试、相似 artifact、相似 manifest、相似 budget policy 或相似 UI 状态，默认采用 adapter / projection / event mapping 收敛到现有 owner，不新增平行模块。只有现有 owner 无法覆盖跨场景需求，并且文档写明迁移和回滚路径时，才允许新增通用层。

| 相似能力 / 场景 | 唯一 owner | 允许的收敛动作 | 禁止动作 |
|---|---|---|---|
| Deep Review reviewer 并发、subagent 容量、gateway 压力 | `SubagentScheduler` + AI request limiter | `DeepReviewConcurrencyPolicy` / `max_parallel_instances` 只投影为 scheduler cap 和 policy input | frontend / Deep Review orchestrator / runtime 各自维护 queue 或 permit |
| Deep Review retry、provider overload retry、gateway backoff | scheduler retry classifier | Deep Review retry budget 投影为 classifier 可用预算和上限 | orchestrator 和 scheduler 对同一 reviewer 各自重试 |
| `partial_timeout`、partial recovery、subagent 终态 | scheduler state + `PartialRecoveryKind` 边界 | raw status 按 evidence 映射为 `completed_with_partial` / `timed_out` / `retry_waiting` | UI / Judge / ledger 同时把 raw status 和 normalized state 当主状态 |
| Deep Review launch manifest、Work Packet、reviewer role / scope | Deep Review canonical manifest + Work Packet projection | 从已实现 manifest 生成 Work Packet projection，保留 `model_id` / `prompt_directive` / scope / retry budget | 在 context 或 runtime 中复制一套 reviewer role schema |
| PR URL / 最近提交 / patch / 大 diff 的 review evidence | Deep Review source resolver + runtime artifact storage | 父任务一次生成 source-agnostic `ReviewEvidencePack`，按 reviewer scope 投影为 artifact slice | 每个 subagent 自行重复拉取或重建同一份完整变更证据 |
| artifact、spill-to-file、大文件写入 manifest | runtime artifact / session storage | Evidence Ledger 记录 artifact ref、hash、status、sensitivity、next action | ledger / context 架构创建第二套文件存储或回灌全文 |
| token / byte / diff line / context budget 阈值 | runtime budget module + model profile | Deep Review strategy、context health、tool metadata 作为估算输入 | 各功能点硬编码自己的预算阈值和阻断规则 |
| context compaction、microcompact、emergency truncation | `ContextMutationKind` + Compaction Contract | runtime 发 mutation event，context 架构记录事实和保留契约 | 把 `ContextCompacted` 放入 `PartialRecoveryKind` 或由 partial recovery 执行输入侧删除 |
| 用户可见 waiting / retrying / partial / failed 状态 | normalized runtime event contract | Deep Review UI、普通对话 UI、诊断面板消费同一事件 | 各 UI 自定义不可互通的状态字符串 |

| 主题 | `context-reliability-architecture.md` 负责 | 本文负责 | 集成边界 |
|---|---|---|---|
| 信任与优先级 | `ContextTrustLevel`、`MessageSemanticKind`、prompt markup escaping | 不重新定义信任等级 | recovery / scheduler / artifact 事件进入上下文前必须带来源和语义类型 |
| 输入侧上下文变化 | Compaction Contract、Evidence Ledger、Context Health | `ContextMutationKind` 事件、预算预检触发和用户呈现 | 压缩质量与事实保留由上下文架构保障；本文只决定何时触发、如何节流、如何展示 |
| 输出侧截断恢复 | 不作为主语义 | `PartialRecoveryKind`、`ProviderFinishReason`、`ToolArgumentStatus` | 输出截断不得写成 context compaction；context mutation 不进入 partial recovery |
| subagent 任务契约 | Work Packet 定义目标、范围、权限、输入 artifact、输出 schema | `SubagentScheduler` 决定排队、gateway permit、retry、timeout、状态事件 | Work Packet 描述“该做什么”；scheduler 控制“什么时候、以什么容量、失败后如何处理” |
| Deep Review | reviewer/judge 证据契约和 partial evidence 进入 Evidence Ledger | reviewer token/byte/diff line 分片、runtime-bounded scheduling、ReviewJudge 状态输入 | policy 决定 reviewer 与输入范围；runtime 决定实际执行顺序和 gateway 压力控制 |
| 大输出与大文件写入 | Evidence Ledger 记录 artifact 事实、hash、状态和验证结果 | spill-to-file、subagent artifact、Large File Write Protocol 生产 artifact | 上下文中只注入摘要和引用；完整内容保存在 session artifact，不回灌全文 |
| 普通对话体验 | `conversation` profile 保持近期用户意图，少自动 subagent | 预算预检不得因低置信估算阻断；短耗时治理后台化 | 两篇文档共同约束：新增治理不能让原本可完成的普通对话变得更不流畅 |

### 交叉方案待确认项

以下不是当前文档冲突，而是两篇方案在同一问题上的不同层次。竞品最新公开实现显示：Codex 已将自动 compaction 作为 agent loop 的一部分，Claude Code / Claude Agent SDK 强调 subagent 独立上下文、并行和工具限制，OpenCode 用 `mode: subagent`、hidden agent 和 `permission.task` 控制 subagent 可见性与调用权限。基于这些事实，当前建议如下，最终产品取舍仍需用户确认。

| 议题 | 可能分歧 | 竞品参考后的建议 | 需要确认 |
|---|---|---|---|
| 自动 compaction 默认开启时机 | 上下文架构希望提高压缩可靠性；本文要求不能新增用户感知中止 | Phase 1 只记录 `ContextMutationKind`；Phase 2 先 observe-only，再允许短耗时后台 compaction；长耗时或有损 mutation 必须展示进度/诊断 | Phase 2B 是否允许对普通对话默认启用短耗时后台 compaction |
| Work Packet 是否泛化为 TaskTool 通用 schema | Deep Review 已有 launch manifest / strategy directive；上下文架构倾向通用任务契约 | 将已实现 Deep Review manifest 作为 Work Packet 投影的第一版；通用 TaskTool schema 放到 Advanced runtime quality，不进入近期默认范围 | Work Packet 泛化是否作为 Phase 3/4 之后的独立方案 |
| Context Health 是否可见 | 上下文架构需要 health score；本文担心额外 UI 状态影响流畅性 | P0/P1 内部 telemetry；用户只看到 action-oriented 状态，例如“正在整理上下文以继续”或“需要拆分任务”，不显示原始分数 | 是否把 raw health score 暴露给高级诊断面板 |
| subagent 并行承诺 | Deep Review 已有 dynamic concurrency policy，但它不应成为第二套 runtime scheduler | 按 Claude/OpenCode 的权限与隔离方向保留 subagent；Deep Review policy 只给上限和意图，BitFun runtime 做 bounded scheduling | Review Team UI 文案是否明确显示 queued / running / retrying |
| artifact 与 Evidence Ledger 所有权 | 两篇文档都提到保留证据 | 本文负责创建 artifact、manifest、spill；Evidence Ledger 只记录事实、hash、路径、状态和验证结果 | session artifact 的长期保留策略和 ledger 持久化位置 |

参考来源：Claude Code subagents、Claude Agent SDK subagents、OpenCode agents、OpenAI Codex agent loop / auto compaction。文档落地时以官方文档的当前行为为准；若竞品后续变化，只影响默认策略的保守程度，不改变本文的边界划分。

---

## 现有能力审计

### 已有基础设施

| 能力 | 位置 | 当前状态 | 评价 |
|---|---|---|---|
| Deep Review Strategy Engine | `deep-review-design.md`、`deep_review_policy.rs`、`reviewTeamService.ts`、`DeepReviewService.ts` | 按已实现基线处理：risk classification、predictive timeout、dynamic concurrency policy、partial result capture、retry budget、role strategy directive | 作为 Deep Review policy / manifest 输入；不再由本文重复实现 |
| 文件数拆分策略 | `src/crates/core/src/agentic/deep_review_policy.rs` | `should_split_files()` + `same_role_instance_count()` 已实现，并可被 Strategy Engine 调整 | 可作为快速路径，但仍需 token/byte/diff line 预算补强 |
| Deep Review 文件拆分 Prompt | `src/crates/core/src/agentic/agents/prompts/deep_review_agent.md` | 已提示按阈值拆分，并包含 reviewer role / strategy directive | prompt 只表达调度意图；真实执行顺序仍由 runtime scheduler 决定 |
| subagent 并行执行 | `task_tool.rs` + `coordinator.execute_subagent()` | 已实现独立 session、上下文隔离和 Deep Review partial result 基础 | 可复用，但应进入统一 `SubagentScheduler` 事件与 permit 路径 |
| subagent 并发控制 | Deep Review dynamic concurrency policy + `SubagentConcurrencyLimiter` | Deep Review 可计算 `max_parallel_instances` / retry budget / timeout policy | 只能作为 policy 上限；不得与 gateway request limiter 形成双调度器 |
| 部分恢复 | `stream_processor.rs`、Deep Review partial result capture | 已能在错误/timeout 后保留部分输出，Deep Review 可得到 partial reviewer evidence | 缺口是跨普通对话、subagent 和 UI 的结构化 kind / state 统一 |
| 工具调用聚合 | `src/crates/ai-adapters/src/tool_call_accumulator.rs` | 能聚合参数并做简单 JSON 修复 | 应在此层增强 JSON 完整性判断 |
| 前端错误呈现 | `src/web-ui/src/shared/ai-errors/aiErrorPresenter.ts` | 已有错误分类框架 | 缺少 `output_truncated` 分类和事件接入 |

### 关键缺口

| 缺口 | 说明 | 影响 |
|---|---|---|
| 截断信号不结构化 | 当前主要依赖 `partial_recovery_reason: Option<String>` | 后端重试、前端展示都容易被文案变化破坏 |
| `TimedStreamItem::End` 过早视为正常 | 未检查 pending tool call 或 provider max tokens | 截断被当成正常完成 |
| 工具调用完整性接口缺失 | `PendingToolCall` 没有 `arguments_closed` 字段 | 不能按原设计直接实现，需要 accumulator 提供语义方法 |
| partial recovery 重试过宽 | `round_executor` 对任意 partial recovery 都可能重试 | token 截断场景会重复失败 |
| 前端事件类型不完整 | `DialogTurnCompletedEvent` 类型未显式声明 recovery 字段 | UI 无法稳定消费截断状态 |
| Deep Review 预算维度仍需统一 | 已有 Strategy Engine 和文件拆分，但 token/byte/diff line 与共享上下文预算需要进入统一 runtime 预算模型 | 大文件、大 diff 仍可能打满上下文；共享上下文重复注入会系统性低估预算 |
| subagent 调度不可观测 | 当前主要看到模型等待或最终失败，缺少 queued / retrying / throttled 状态 | 用户无法区分“模型在思考”“排队等待容量”“因 gateway overload 重试” |
| gateway 并发与 Deep Review 并发策略混在一起 | Deep Review `max_parallel_instances` / `ai.subagent_max_concurrency` 控制 reviewer / hidden session 上限，但 provider / vLLM 实际并发可能更低 | 多 reviewer 可能因为瞬时 burst 被拒绝，而不是任务本身失败；若再叠加 runtime retry，可能形成双重试 |
| Deep Review 状态名未进入统一事件契约 | Deep Review 可能已有 `partial_timeout` / retry / timeout 状态，runtime 方案使用 `completed_with_partial` / `retry_waiting` / `timed_out` | Judge、前端和日志可能看到两套语义，导致重复提示或错误降级 |

---

## 竞品与行业经验

公开 issue 和社区反馈显示，Claude Code、OpenAI Codex CLI、Cursor、Cline/Roo Code、LangChain/LangGraph 等工具都遇到过相似问题：

1. **大输出不能只截断**：用户需要知道完整内容在哪里、哪些内容被省略、模型实际看到了什么。
2. **工具输出和模型输出都需要预算**：terminal output、read_file、大文件、subagent 返回都可能撑爆上下文。
3. **结构化错误优于文本匹配**：parser、agent runtime、UI 应消费机器可读状态，而不是日志文案。
4. **spill-to-file 是常见方向**：大输出保存在本地文件，只把摘要和路径放进上下文。
5. **任务拆分需要按 token/byte/diff line**：只按文件数量拆分不足以控制上下文风险。
6. **subagent 并行需要 runtime 调度**：prompt 可以请求并行，但真实运行顺序、gateway permit、重试和 timeout 应由 runtime 控制。

BitFun 当前方案应吸收这些经验：先修复静默截断，再建立统一输出预算与可恢复链路。

---

## 核心设计

### 1. 引入结构化恢复类型

在 core 的 stream/round result 层增加机器可读字段，保留原有人类可读 reason。

建议枚举：

```rust
pub enum PartialRecoveryKind {
    StreamInterrupted,
    WatchdogTimeout,
    OutputTruncated,
    ToolArgumentsIncomplete,
    ProviderMaxTokens,
    ToolOutputBudgetExceeded,
    RateLimited,
    UnknownPartial,
}
```

`StreamResult` 保留：

```rust
pub partial_recovery_reason: Option<String>,
pub partial_recovery_kind: Option<PartialRecoveryKind>,
```

设计原则：

- `partial_recovery_kind` 给系统判断；
- `partial_recovery_reason` 给日志和诊断；
- 不再用 `reason.contains("truncated")` 作为架构契约；
- 若短期需要兼容旧字段，可继续透传 reason，但新逻辑必须优先使用 kind。
- `ContextCompacted` 不放入 `PartialRecoveryKind`。上下文压缩、microcompact、emergency truncation 属于输入侧上下文变更，不是模型输出流的 partial recovery，应使用独立的 `ContextBudgetEvent` / `ContextMutationKind` 记录。

---

### 2. 在 `ai-adapters` 层暴露工具调用完整性接口

原设计中的 `arguments_closed` 字段不存在，不应让 core 层窥探 accumulator 内部结构。

在 `PendingToolCalls` 上增加语义方法：

```rust
impl PendingToolCalls {
    pub fn has_pending_payload(&self) -> bool;
    pub fn has_incomplete_json_payload(&self) -> bool;
    pub fn pending_payload_summary(&self) -> PendingToolCallSummary;
}
```

其中：

- `has_pending_payload()` 判断是否仍有未 finalize 的工具调用；
- `has_incomplete_json_payload()` 基于 `raw_arguments` 的 JSON parse / boundary 检测判断是否可能未闭合；
- `pending_payload_summary()` 用于日志，不暴露敏感参数全文。

JSON 修复仍放在 `tool_call_accumulator.rs` 内部，例如增强 `PendingToolCall::parse_arguments()`：

1. 先尝试原始 JSON parse；
2. 保留现有“删除一个多余右花括号”的修复；
3. 不做“补齐缺失括号/引号后继续执行”的自动修复；
4. 修复后必须再次 `serde_json::from_str` 验证；
5. 修复失败时 finalize 为 `is_error = true`。

工具参数必须携带可信状态，而不是只靠 `is_error`：

```rust
pub enum ToolArgumentStatus {
    Complete,          // 原始 JSON 完整，可按现有权限策略执行
    RepairedTrusted,   // 仅限删除单个多余右花括号等不改变语义的保守修复
    RepairedUntrusted, // 语法补齐等可能改变语义的修复，不得自动执行
    Incomplete,        // 明确未闭合或无法解析，不得自动执行
}
```

重要约束：

- 只有 `Complete` 和少数经白名单证明不改变语义的 `RepairedTrusted` 可进入现有工具执行路径；
- `RepairedUntrusted` / `Incomplete` 必须转为结构化错误事件和诊断，不得自动执行；
- 修复用于完成事件链路、保存诊断、向用户提示；
- core 消费 `FinalizedToolCall.argument_status`、`is_error` 和 recovery kind，不实现 JSON 修复细节。

---

### 3. 在 `stream_processor` 中检测流结束时的截断

当前行为：

```rust
TimedStreamItem::End => {
    debug!("Stream ended normally (no more data)");
    break;
}
```

#### 3a. provider finish reason 归一化

不同 provider 对 finish reason 的命名和语义不一致（`stop` / `end_turn` / `stop_sequence` / `length` / `max_tokens` / `tool_use`）。为避免 core 层面对 provider 差异，在 `ai-adapters` 层增加归一化：

```rust
pub enum ProviderFinishReason {
    Stop,          // 模型正常结束
    Length,        // 模型达到 max output tokens
    ToolUse,       // 模型请求执行工具
    ContentFilter, // 内容被安全过滤中断
    Unknown(String), // 无法识别的原始值，保留用于诊断
}
```

每个 ai-adapter 在解析 provider response 时，将原始 finish reason 映射到此枚举。`Unknown` 变体保留原始字符串，不触发截断检测。

`StreamContext` 在 stream 结束时读取归一化后的 `ProviderFinishReason`，而非原始字符串。

#### 3b. End 分支截断检测

```rust
TimedStreamItem::End => {
    if ctx.pending_tool_calls.has_incomplete_json_payload() {
        ctx.force_finish_pending_tool_calls();
        ctx.partial_recovery_kind = Some(PartialRecoveryKind::ToolArgumentsIncomplete);
        ctx.partial_recovery_reason = Some(
            "Model output ended while tool arguments were incomplete".to_string()
        );
        warn!(
            "Stream ended with incomplete tool arguments: session_id={}, round_id={}, summary={:?}",
            ctx.session_id,
            ctx.round_id,
            ctx.pending_tool_calls.pending_payload_summary()
        );
    } else if ctx.provider_metadata_indicates_max_tokens() {
        ctx.partial_recovery_kind = Some(PartialRecoveryKind::ProviderMaxTokens);
        ctx.partial_recovery_reason = Some(
            "Model output stopped because the provider reported max token completion".to_string()
        );
        warn!(
            "Stream ended due to provider max tokens: session_id={}, round_id={}",
            ctx.session_id,
            ctx.round_id
        );
    } else {
        debug!("Stream ended normally (no more data)");
    }
    break;
}
```

检测信号分层：

| 信号 | 强度 | 处理 |
|---|---|---|
| provider finish reason 是 `length` / `max_tokens` | 强 | `ProviderMaxTokens` |
| stream end 时 pending tool call JSON 不完整 | 强 | `ToolArgumentsIncomplete` |
| stream error 且已有有效输出 | 中 | `StreamInterrupted` |
| watchdog timeout 且已有有效输出 | 中 | `WatchdogTimeout` |
| 无 finish reason 但无 pending tool call | 弱 | 记录 debug，不单独判定截断 |

不建议仅因“无 finish_reason”判定截断，因为 provider 行为不一致。

---

### 4. 修正 `round_executor` 重试策略

当前 `round_executor` 对 partial recovery 的重试过宽：任何 `partial_recovery_reason.is_some()` 都可能重试。

目标策略：

| Recovery kind | 是否自动重试同请求 | 原因 |
|---|---|---|
| `StreamInterrupted` 且无有效输出 | 可以 | 可能是瞬态网络问题 |
| `WatchdogTimeout` 且无有效输出 | 可以 | 可能是瞬态卡顿 |
| `StreamInterrupted` 且已有有效输出 | 谨慎，不默认重试 | 重试可能造成重复输出或重复工具调用 |
| `ToolArgumentsIncomplete` | 不重试 | 同请求大概率再次截断，且工具调用不安全 |
| `ProviderMaxTokens` | 不重试同请求 | 需要缩短上下文、增加输出预算或拆分任务 |
| `ToolOutputBudgetExceeded` | 不重试同请求 | 应落盘/摘要，而不是重复执行 |
| `RateLimited` 且无有效输出 | 交给调度器/限流器 backoff 后重试 | 顶层 round 不做即时同请求重试，但 subagent scheduler 可在 retry budget 内排队重试 |
| `RateLimited` 且已有有效输出 | 不默认重试 | 保留 partial output，避免重复文本或重复工具调用 |
| `UnknownPartial` | 不默认重试 | 保守处理，提示用户 |

`ContextMutationKind`（microcompact / compression / emergency truncation）不进入上述 retry 表。它属于请求前上下文治理事件，不是模型输出流恢复原因；处理方式应是记录预算变化、展示诊断提示，并在仍然超预算时拆分任务或要求用户缩小范围。

伪代码：

```rust
if is_partial_recovery && attempt_index < max_attempts - 1 {
    match result.partial_recovery_kind {
        Some(PartialRecoveryKind::StreamInterrupted) | Some(PartialRecoveryKind::WatchdogTimeout)
            if !result.has_effective_output => retry_with_backoff(),
        Some(PartialRecoveryKind::RateLimited) if !result.has_effective_output && is_scheduler_owned => {
            retry_via_scheduler_backoff()
        }
        Some(PartialRecoveryKind::ToolArgumentsIncomplete)
        | Some(PartialRecoveryKind::ProviderMaxTokens)
        | Some(PartialRecoveryKind::ToolOutputBudgetExceeded)
        | Some(PartialRecoveryKind::RateLimited) => finish_with_partial_recovery(),
        _ => finish_with_partial_recovery(),
    }
}
```

同时保留现有 `is_transient_network_error()`，但它只用于网络错误类，不用于 token 截断类。

---

### 5. 事件传播与前端展示

后端 `DialogTurnCompleted` 事件增加结构化字段：

```rust
partial_recovery_kind: Option<PartialRecoveryKind>,
partial_recovery_reason: Option<String>,
```

前端对应类型补齐：

```ts
export type PartialRecoveryKind =
  | 'stream_interrupted'
  | 'watchdog_timeout'
  | 'output_truncated'
  | 'tool_arguments_incomplete'
  | 'provider_max_tokens'
  | 'tool_output_budget_exceeded'
  | 'rate_limited'
  | 'unknown_partial';

export type ContextMutationKind =
  | 'microcompact'
  | 'model_compression'
  | 'structured_fallback_compression'
  | 'emergency_truncation';

export interface DialogTurnCompletedEvent {
  session_id: string;
  turn_id: string;
  turn_index: number;
  files_changed: number;
  lines_added: number;
  lines_removed: number;
  timestamp: number;
  partial_recovery_kind?: PartialRecoveryKind;
  partial_recovery_reason?: string;
}
```

`aiErrorPresenter.ts` 增加分类：

```ts
type AiErrorCategory = ... | 'output_truncated';
```

UI 提示按 kind 区分：

| Kind | 用户提示 |
|---|---|
| `tool_arguments_incomplete` | 模型在生成工具调用参数时被截断，该工具未执行。建议拆分任务或减少上下文后重试。 |
| `provider_max_tokens` | 模型输出达到上限。建议缩短请求、压缩上下文或拆分任务。 |
| `output_truncated` | 模型输出可能不完整。建议缩短请求或新建会话继续。 |
| `stream_interrupted` | 模型响应流中断，已保留部分输出。可根据内容决定是否重试。 |
| `watchdog_timeout` | 模型响应长时间无新内容，已停止等待并保留部分输出。 |
| `rate_limited` | 模型输出因限流被中断，请稍后重试。 |
| `consecutive_truncation` | 连续多次截断，建议拆分任务或减少上下文。 |

展示位置：

- active session：在消息流末尾显示轻量提示条；
- inactive session：保留现有未读/中断状态，同时进入会话后显示具体提示；
- Deep Review 父任务：若子 reviewer 截断，父任务摘要中列出被截断的 reviewer 角色和文件组。

---

### 6. Deep Review 预算治理

`deep-review-design.md` 已实现后，Deep Review 已具备 Strategy Engine：risk classification、predictive timeout、dynamic concurrency policy、partial result capture、retry budget、Architecture / Frontend reviewer、Judge overlap handling 和 strategy directive / model plumbing。本文不再规划这些 Deep Review 专项能力，而是把它们接入统一预算与 runtime 调度。

剩余缺口是：Deep Review 的已有策略主要控制 reviewer 角色、scope、timeout、retry 上限和策略强度；它仍需要与统一 token/byte/diff line 预算、session artifact、gateway permit 和 scheduler state 对齐。

新增预算输入：

- file count；
- total bytes；
- diff line count；
- estimated input tokens；
- per-file max bytes；
- model context window；
- reserved output tokens；
- reviewer prompt 固定成本；
- tool schema 成本。

目标拆分逻辑：

```text
if estimated_input_tokens + reserved_output_tokens > safe_context_budget:
    split by token/bytes/diff lines first
else if file_count > reviewer_file_split_threshold:
    split by file count
else:
    keep current grouping
```

设计约束：

- Deep Review 已有 risk classification / strategy level / file splitting 结果保留，作为预算拆分的输入；
- 文件数阈值保留，作为简单场景的快速路径；
- token/byte/diff line 是更高优先级，用于补足 Strategy Engine 对单文件巨型 diff 和共享上下文重复注入的低估；
- 单个超大文件应单独分组，必要时提示用户缩小范围；
- 共享上下文必须计入每个 reviewer group 的预算，因为每个 reviewer 都会实际看到这些内容；
- 共享上下文本身超预算时，不重复注入全文，改为接口签名 / 类型声明 / import 图摘要；
- Deep Review policy 决定需要哪些 reviewer、每个 reviewer 的输入范围和角色约束；runtime scheduler 决定真实执行顺序、排队、重试和 gateway 压力控制；
- Deep Review `max_parallel_instances` / retry budget / predictive timeout 只作为 policy 上限和运行预算输入，不得直接绕过 runtime scheduler；
- prompt / manifest 不再表达“无条件并行启动所有 reviewer”，而应表达“调度所有 required reviewers，runtime 会按 gateway 容量有界执行”；
- 已有 `partial_timeout` / timeout / retry 状态必须映射到统一 scheduler event：有可用 evidence 时为 `completed_with_partial`，无可用 evidence 时为 `timed_out`，等待重试时为 `retry_waiting`；
- judge 聚合时也需要限制 reviewer 返回大小；
- ReviewJudge 输入必须包含每个 reviewer 的 `queued` / `retried` / `timed_out` / `failed` / `completed_with_partial` 状态和 partial evidence。

#### 6.1 Review Evidence Pack：source-agnostic 一次取证，多 reviewer 复用

`/DeepReview` 的输入来源不能被固化为本地 Git range。用户可能输入最近 X 个提交、branch/range/pathspec、PR URL、远端 compare 链接、上传的 patch artifact、当前 working tree、选中文件列表或混合上下文。方案的目标不是“优化 Git diff”，而是将任意 review source 先解析为可复用的标准化证据包。

当用户要求 Deep Review 大规模变更时，不能让每个 reviewer 在 subagent 中各自重新拉取、查询或重建同一份完整变更证据。重复取证会把 reviewer 的 `run_timeout_seconds` 消耗在 I/O、网络/API、命令输出和解析上，而不是消耗在分析上；在大仓库、远端 PR、Windows 环境或自托管代码平台下，这会直接放大 timeout 和上下文超限概率。

新增 source-agnostic `ReviewEvidencePack`，由 Deep Review 父任务在 launch preflight 阶段通过 `ReviewEvidenceSourceResolver` 和对应 `ReviewEvidenceProvider` 一次性生成，并落盘为 session artifact。subagent 默认只消费 pack 的 slice、source metadata 和必要源码读取权限。

第一批 source provider：

| source kind | 输入示例 | 取证方式 | 备注 |
|---|---|---|---|
| `local_git_range` | 最近 X 个提交、本地 branch/range/pathspec | 本地 Git provider 生成 normalized change set | Git 是 provider，不是 pack 抽象本身 |
| `pull_request_url` | GitHub / GitCode / GitLab 等 PR URL | 平台 provider 拉取 PR files、patch、commit metadata、review base/head | 优先用 API / connector；必要时 fallback 到远端 patch |
| `working_tree` | 审核当前未提交改动 | workspace provider 读取 working tree diff 和文件快照 | stale 判断依赖 working tree state hash |
| `patch_artifact` | 用户提供 patch / diff 文件 | patch parser provider 解析变更和文件索引 | 适合离线或无仓库场景 |
| `explicit_files` | 用户选择文件或目录做深度审核 | filesystem provider 生成文件快照和范围索引 | 没有 diff 时应标记为 snapshot review |

建议 pack 内容：

```text
pack_id
source_kind
source_locator
source_provider
source_revision
source_fingerprint
generated_at
collector_events
pack_hash
file_index
change_index
rename_map
diff_stat
per_file_change_artifacts
per_file_size_and_diff_lines
cross_file_context_summary
staleness_policy
```

执行方式：

1. 父任务解析 `/DeepReview` 输入，识别 source kind、source locator 和权限需求；
2. 对应 source provider 一次性收集证据，生成 normalized file index、change index、diff/stat、rename map、per-file change artifact 和摘要；
3. Deep Review Strategy Engine 继续决定 reviewer role / scope / strategy level；
4. Work Packet projection 为每个 reviewer 写入 `review_evidence_pack_id`、`evidence_slice`、`source_kind`、`allowed_source_paths`、`allowed_source_queries` 和 `forbidden_full_evidence_reconstruction`；
5. reviewer 默认从 artifact slice 读取证据，只在需要确认局部上下文时读取源码、小范围 hunk、单文件 patch、PR 文件详情或特定 symbol；
6. 若 pack stale、缺失或 scope 不足，reviewer 请求父任务刷新或扩展 pack，而不是自行重新拉取或重建完整 source evidence。

允许的 fallback：

- pack 生成失败时，Deep Review 可以退回当前行为，但必须记录 `evidence_pack_failed` 和失败 provider，并将 reviewer timeout 调整为包含取证成本；
- 小规模 source 可以内联 pack summary，不必落盘每个 per-file artifact；
- reviewer 可执行局部只读查询，例如读取某个文件当前内容、查询特定 symbol、请求某个 PR 文件详情、或验证一个小范围 hunk，但不能默认重建完整 source；
- runtime 可加只读 source-result cache 作为保护网，键为 `source_kind + source_locator + source_fingerprint + query args`，但 cache 不是主要设计，主要设计仍是显式 pack；
- 对需要网络或平台 API 的 source provider，失败分类必须区分 auth / quota / not found / network / provider unavailable，避免把 PR URL 失败误判为 reviewer 分析失败。

可观测字段：

- `review_evidence_pack_id`
- `source_kind`
- `source_provider`
- `source_locator_hash`
- `source_fingerprint`
- `pack_generation_ms`
- `source_collection_count`
- `pack_bytes`
- `pack_hash`
- `file_count`
- `diff_line_count`
- `artifact_slice_count`
- `cache_hit`
- `reviewer_full_evidence_reconstruction_count`
- `pack_stale`

门禁：

- 本地提交范围、PR URL、working tree、patch artifact fixture 都必须走同一 pack contract；
- 同一 source fingerprint 下的完整证据收集只能由父任务 preflight 执行一次；
- 4-5 个 reviewer 运行时，subagent 不得各自重复拉取或重建全部文件变更；
- reviewer timeout 不应包含父任务 pack generation 时间；pack generation 若长耗时，应以“正在准备审核证据”展示；
- pack stale 时必须阻止继续使用旧证据，或记录 stale 诊断并请求用户确认；
- pack artifact 必须复用 session artifact 权限、敏感内容检测、hash 校验和清理策略。

---

### 7. SubagentScheduler 与 Gateway Request Limiter

subagent 调度不能只依赖静态并发 cap，也不能由 Deep Review 的 Strategy Engine 单独承担。Deep Review 已有 dynamic concurrency policy，可以决定 reviewer policy 上限；但本地或自托管 vLLM gateway 可能只允许 1-4 个并发 streaming request，真实容量必须由 runtime 在 AI request 边界观察和控制。因此需要将 subagent 派发升级为 core runtime 拥有的、可观测的、自适应队列。

目标路径：

```text
TaskTool
  -> ConversationCoordinator
  -> SubagentScheduler
  -> Hidden subagent session
  -> ExecutionEngine
  -> AI request limiter
  -> Provider adapter / model gateway
```

边界：

- `SubagentScheduler` 控制 hidden session 的生命周期、公平排队、重试状态和事件；
- AI request limiter 控制实际 provider / gateway / model group 的 streaming request permit；
- Deep Review policy 控制 reviewer 角色、文件范围、超时预算和最多同角色实例数；
- Deep Review Strategy Engine 的 `max_parallel_instances`、predictive timeout、retry budget 是 scheduler 输入，不是最终执行器；
- prompt 可以请求并行，runtime 决定安全可执行的真实并发。

调度状态：

```text
accepted
queued
waiting_for_capacity
running
retry_waiting
completed
completed_with_partial
failed
cancelled
timed_out
```

关键转移：

- `accepted -> queued`：Task call 合法并被 policy 接受；
- `queued -> waiting_for_capacity`：任务可运行，但 subagent slot 或 gateway permit 不足；
- `waiting_for_capacity -> running`：scheduler 授予 subagent slot，AI request limiter 授予 gateway permit；
- `running -> completed`：subagent 正常完成；
- `running -> completed_with_partial`：已有有效输出但未完整完成，应保留 evidence；
- `running -> retry_waiting`：无有效输出前遇到 transient gateway overload；
- `retry_waiting -> queued`：backoff 到期后重新排队；
- `queued/running -> cancelled`：父 session 或用户取消；
- `queued/running -> failed`：不可重试错误或 retry budget 耗尽。

动态并发：

```text
configured_max = ai.subagent_max_concurrency 或 review-team policy 上限
effective_max = runtime-adjusted value, clamped to [1, configured_max]
```

控制策略：

1. 初始 `effective_max = configured_max`；
2. 遇到 gateway concurrency / rate-limit overload 时快速降低；
3. 连续成功窗口后缓慢升高；
4. 永远不低于 1，保证最低 forward progress；
5. 优先按 gateway key 维护计数，gateway key 可来自 model config 的 `concurrency_key`，否则用 `provider + base_url` 或 normalized request URL。

重试分类：

- 可重试：HTTP 429、gateway concurrency saturation、queue full、server busy、capacity exceeded、无有效输出前 connection reset / stream closed；
- 不可重试：Deep Review policy violation、invalid subagent type、missing workspace、auth/quota/billing/model-not-found/invalid key、用户取消、tool permission denial；
- 条件重试：read-only subagent 有 partial output 时保留 partial evidence，通常不直接重跑全文；write-capable subagent 只有在执行历史证明没有 state-changing tool 后才允许重试。

timeout 语义：

- `queue_timeout_seconds`：等待 scheduler capacity 的最长时间；
- `run_timeout_seconds`：subagent 开始运行后的最长 active execution 时间；
- `stream_idle_timeout_secs`：模型 stream chunk 之间的最大静默时间；
- `parent_deadline`：父任务端到端 deadline。

对 Deep Review，已实现的 predictive timeout / `effective_timeout_seconds` 应映射为 `run_timeout_seconds`，也就是 active review time；queue waiting 单独计量，避免本地低并发 gateway 让 reviewer 在尚未开始前就超时。

状态兼容：

- Deep Review 已有 `partial_timeout` / `PartialTimeout` 状态时，若存在有效 reviewer evidence，对外映射为 `completed_with_partial`；
- 若 timeout 前没有足够 evidence，对外映射为 `timed_out`；
- Deep Review retry budget 触发重试等待时，对外映射为 `retry_waiting`；
- 原始 Deep Review 状态可保留在 diagnostics 字段，不作为 UI / Judge 的主判定状态。

UI 与日志：

- UI 区分 model thinking、queue waiting、retry backoff、running、completed_with_partial；
- Deep Review summary 可显示“runtime 使用有界队列继续审核，因为当前 model gateway 限制并发”；
- 日志使用英文，无 emoji，记录 permit acquire/release、effective concurrency changed、retry classified、retry budget exhausted、partial output preserved；
- 关键事件需要包含 `scheduler_state`、`queued_ms`、`run_ms`、`retry_count`、`gateway_key`、`configured_max`、`effective_max`、`retry_classification`。

配置形状保持三层拆分：

```json
{
  "ai": {
    "subagent_max_concurrency": 4,
    "gateway_concurrency": {
      "default": {
        "max_concurrent_requests": 4,
        "adaptive": true,
        "min_concurrent_requests": 1
      }
    },
    "review_teams": {
      "default": {
        "max_parallel_reviewers": 3,
        "reviewer_queue_timeout_seconds": 300,
        "reviewer_retry_budget": 1
      }
    }
  }
}
```

精确字段名可在实现阶段调整，但必须保留三层语义：global subagent execution capacity、gateway/model request capacity、Deep Review reviewer policy。

禁止重复实现：

- `reviewTeamService.ts` 可以生成 reviewer manifest、strategy directive、model_id、scope 和 policy 上限，但不能另建与 `SubagentScheduler` 并行的 runtime queue；
- Deep Review orchestrator 可以声明 retry budget 和降级策略，但不能绕过 scheduler retry classifier 自主重复发起同一批 reviewer；
- provider rate-limit / gateway overload 只能在 AI request limiter / provider diagnostics 层归一化，不能由前端仅凭 cached rate-limit 状态决定最终执行并发；
- Judge 和 UI 必须消费统一 scheduler state，Deep Review 原始状态只作为诊断补充。

---

### 8. 统一输出预算与 spill-to-file（后续阶段）

为了对齐行业实践，后续应建立统一输出预算策略，覆盖：

- 模型输出；
- 工具输出；
- terminal stdout/stderr；
- read file 输出；
- subagent 返回；
- Deep Review reviewer 报告；
- judge 聚合输入。

建议策略：

1. 每类输出有 `max_inline_bytes` / `max_inline_tokens`；
2. 超限内容落盘到 session 目录；
3. 上下文中只注入摘要、统计和文件引用；
4. UI 提供“打开完整内容”；
5. 日志记录 spill 文件路径、原始大小、摘要大小；
6. 对敏感内容保持本地存储，不上传额外远端。

通用 spill-to-file 不是 Phase 1 必须完成，但 **subagent / Deep Review reviewer 返回的落盘引用是 Phase 1 必须完成的专项能力**。原因是当前 hidden subagent 成功后会清理子 session，若只做硬截断而不保留完整输出引用，reviewer 证据会永久丢失。

---

### 9. 大文件写入协议（Large File Write Protocol）

竞品和社区问题显示，Claude、OpenCode 等工具在多轮长对话后，如果让模型通过普通 tool call JSON 一次性写入约 1k 行以上文件内容，容易出现输出截断、上下文超限或工具参数不完整。根因不是单纯上下文压缩不足，而是把“大内容传输”放进了“对话上下文 / 工具参数通道”。

因此，大文件写入应作为统一预算治理的上游预防机制，而不是截断后的恢复手段。目标是：**模型负责规划和校验，文件内容通过 session-backed writer 分块落盘，上下文只保留 manifest、摘要、hash 和引用**。

建议协议：

```text
start_file_write(path, mode, expected_sections, intent)
append_file_chunk(write_id, section_id, sequence, content_chunk, chunk_hash)
finish_file_write(write_id, expected_hash, validation_plan)
abort_file_write(write_id, reason)
```

上下文中只注入轻量结果：

```text
file: src/foo.ts
status: written
sections: imports, types, service, tests
bytes: 48231
hash: abc123
preview: first/last lines or structural summary
artifact_ref: .bitfun/sessions/{session_id}/artifacts/writes/{write_id}
```

设计约束：

1. **优先 patch/hunk，不重写全文**：修改既有大文件时，默认生成 edit plan 和小范围 patch；只有新建大文件或确实需要全量重生成时进入大文件写入协议。
2. **事务化写入**：所有 chunk 先写临时文件，`finish_file_write` 校验 hash、section manifest 和基础格式后再原子替换目标文件。
3. **上下文不回灌全文**：tool result 不返回完整文件内容，只返回进度、摘要、hash、行数、artifact 引用和必要预览。
4. **按需读取**：后续修改通过 range/symbol 读取相关片段，不自动把完整文件重新塞回对话。
5. **可恢复**：写入中断时保留 write manifest，下一轮可继续 append、重新校验或 abort。
6. **产品进度可见**：大文件写入表现为“正在创建文件 / 已写入 N 个 section / 正在校验 / 已完成”，而不是模型长时间无输出。
7. **平台边界**：协议和事务逻辑属于 core / tool layer；桌面或前端只通过 adapter 展示进度和结果，不直接操作文件。

触发条件建议：

- 单次 `write_file` / `replace_file` content 预计超过 `large_write_inline_threshold`；
- 模型计划新建或重写超过约 800-1000 行文件；
- 工具参数 JSON 中 `content` 字段预计会接近输出 token 预算；
- 多轮长对话后上下文已接近预算，且下一步是大文件生成。

若协议尚未实现，不应鼓励模型通过普通工具参数生成大文件全文；应提示模型先输出文件结构计划或拆成小范围 patch。

---

## 分阶段实施计划

### 落地原则与阶段门禁

本方案不能作为一次性大改上线。每个阶段必须满足以下可维护、可测试、可回滚要求后才能进入下一阶段：

1. **先观测，后干预**：凡是可能改变用户路径的能力，先以 observe-only 模式记录结构化事件和指标，再开启真实拦截、压缩或落盘。
2. **feature flag 分层**：按能力独立开关，而不是一个总开关。例如 `partial_recovery_kind_enabled`、`tool_argument_status_enforced`、`budget_precheck_observe_only`、`spill_to_file_enabled`、`large_file_write_protocol_enabled`。
3. **事件契约稳定**：后端新增字段使用 optional / enum unknown fallback，前端必须兼容旧事件；系统逻辑不得依赖本地化文案。
4. **测试夹具先行**：每个阶段先补可复现 fixture，再实现行为。fixture 覆盖 stream chunks、provider finish reason、半截 tool JSON、大输出、预算估算、大文件 chunk。
5. **观测字段固定**：所有阶段至少记录 `session_id`、`turn_id`、`round_id`、`model_id`、`recovery_kind` / `budget_state` / `tool_argument_status`、估算 token、实际大小、动作耗时和是否用户可见。
6. **默认可降级**：任何阶段发现误判、过度提示、频繁压缩或写入异常时，应能降级为记录诊断 + 保留旧行为，而不是影响主路径。
7. **用户感知指标必须不倒退**：新增动作上线后，普通对话的可感知中止数、不透明等待次数、不必要确认次数不得增加。

### 可落地阶段总览

| 阶段 | 目标 | 默认上线形态 | 可维护性要求 | 可测试性要求 | 进入下一阶段门槛 |
|---|---|---|---|---|---|
| Phase 0：基线与夹具 | 不改行为，先固化当前失败模式 | observe-only / test-only | 建立统一 fixture 目录和事件快照格式；文档化 `ai.subagent_max_concurrency` | 可复现半截 JSON、max_tokens 文本截断、stream error、subagent 大返回、gateway burst failure、1k+ 行写文件 | fixture 能稳定失败于旧逻辑；Review Team manifest 不再承诺无条件并行 |
| Phase 1A：结构化截断与调度信号 | 引入 `PartialRecoveryKind`、`ProviderFinishReason`、`has_effective_output`、scheduler state event | 默认仅记录 kind/state，不改变执行策略 | enum unknown fallback；reason 只用于诊断；scheduler state 事件 optional | adapter finish reason 映射、stream end/error、scheduler state transition 单测 | 无误判正常 stop；Unknown 不触发截断；queued/running/retry/completed 事件可序列化 |
| Phase 1B：工具参数安全 | 引入 `ToolArgumentStatus` 和截断批次标记 | 默认拒绝 `Incomplete` / `RepairedUntrusted`，read-only 白名单可继续 | tool mutability metadata 有默认值和审计入口 | 半截 JSON、保守修复、read-only/mutating 分级测试 | 不可信参数不会进入 tool pipeline；只读工具不关联拦截 |
| Phase 1C：用户呈现与恢复/调度事件 | 前端消费 recovery/context mutation/scheduler 事件 | 轻提示 + 诊断入口；queue/retry 状态可区分 | i18n key 稳定，UI 不依赖后端英文文案 | event serialization、locale、消息流快照、waiting/retrying 展示测试 | 部分成功不显示 fatal；无有效输出才进入错误态；用户能区分排队与模型思考 |
| Phase 1D：subagent 专项落盘 | 解决 hidden subagent 输出丢失和父 session overflow | subagent 超限时先落盘再注入摘要 | artifact 生命周期和 session 删除绑定 | 超大 subagent result、artifact 清理、父事件上浮测试 | 不存在“只硬截断不落盘”的路径 |
| Phase 2A：预算观测与 gateway limiter | 建立预算估算、模型 profile、gateway-keyed request permits | observe-only + static permit cap，不做自适应降级 | budget module 独立；gateway key 可配置；stream 结束/错误/取消时释放 permit | tokenizer/启发式估算、actual token 校准、permit acquire/release 测试 | 估算误差可观测；低置信不参与阻断；permit 不泄漏 |
| Phase 2B：本地恢复与 adaptive concurrency | Soft/HardOverBudget 分层、压缩节流、effective concurrency 调整 | Soft 后台恢复；Hard 恢复后仍超限才引导用户；overload 时降低 effective max | 冷却窗口、收益阈值、耗时记录、adaptive policy 独立可调 | 压缩频率、收益阈值、长耗时进度、gateway overload 分类测试 | 不增加普通对话低置信中止；effective max 不低于 1 且不过度振荡 |
| Phase 2C：Deep Review Strategy Engine 接入 runtime control plane | 将已实现 Strategy Engine 的 timeout/concurrency/retry/partial 状态接入 token/byte/diff line 预算、source-agnostic Review Evidence Pack 和 runtime scheduler | 只影响 Deep Review 调度、状态映射、取证复用和 reviewer 状态汇总 | Deep Review policy 只给上限；runtime scheduler 是唯一执行调度器；完整 source evidence 由父任务 preflight 通过 provider 一次生成 | 大 diff / PR URL / patch fixture、共享上下文计预算、Review Evidence Pack 去重、gateway concurrency=2 排队完成测试、状态映射快照 | 大文件/大 diff 不再只按文件数拆分；4-5 reviewers 不因 burst launch 失败；subagent 不重复重建完整 evidence；无双调度/双重试 |
| Phase 3：统一输出预算 | 工具输出、terminal、read_file、subagent 泛化 spill | 超限才 spill，普通输出直显 | output budget policy 集中配置，summary formatter 可扩展 | spill 权限、LRU、敏感检测、摘要 fixture | 大输出不回灌全文，敏感内容不进摘要 |
| Phase 4：大文件写入协议 | 避免 1k+ 行文件走普通 tool JSON | 大文件/高风险上下文触发，小文件仍直接写 | writer 状态机、manifest、事务提交独立模块 | chunk sequence/hash、abort/continue、原子提交、range 后续修改测试 | 无事务/无校验路径不能写目标文件 |

### 分阶段维护与测试资产

| 资产 | 所属阶段 | 维护方式 | 测试方式 |
|---|---|---|---|
| Stream fixture | Phase 0-1 | 保存 provider 原始 chunk、finish reason、error event 的最小样本 | Rust 单测 + adapter 映射测试 |
| Tool argument fixture | Phase 0-1B | 覆盖完整 JSON、半截 JSON、尾部多余右花括号、语义不可信补齐 | `tool_call_accumulator` 单测 + pipeline 拒绝测试 |
| Event snapshot | Phase 1A-1C | 对 `DialogTurnCompletedEvent` / context mutation event 做稳定快照 | 后端序列化测试 + 前端 event handler 测试 |
| Scheduler fixture | Phase 1A / Phase 2 | 覆盖 accepted、queued、waiting_for_capacity、running、retry_waiting、completed_with_partial、failed、cancelled、timed_out | state transition、gateway permit、retry classifier 单测 |
| Budget fixture | Phase 2 | 覆盖 CJK、代码、长日志、大 diff、不同 model profile | 估算误差测试 + observe-only 指标回放 |
| Deep Review compatibility fixture | Phase 2C | 覆盖已实现 Strategy Engine 的 `max_parallel_instances`、predictive timeout、partial timeout、retry budget 与 scheduler state 映射 | 无双调度、无双重试；`partial_timeout` 正确映射为 `completed_with_partial` 或 `timed_out` |
| Review Evidence Pack fixture | Phase 2C | 覆盖 local git range、PR URL、working tree、patch artifact、rename、pathspec、pack stale、4-5 reviewers 共享取证 | 父任务完整 source evidence 收集只执行一次；reviewer 消费 artifact slice；`reviewer_full_evidence_reconstruction_count = 0` |
| Artifact fixture | Phase 1D / Phase 3 | 覆盖 subagent result、terminal output、read_file output、敏感内容 | 落盘权限、LRU、REDACTED 摘要测试 |
| Large write fixture | Phase 4 | 覆盖 1k+ 行新文件、既有大文件小改、chunk 缺失/重复/乱序 | writer 状态机、hash 校验、原子提交、range 修改测试 |

阶段推进时，先补资产，再接行为。没有对应 fixture 的行为不进入默认路径。

### 运行期可观测与回滚

| 能力 | 必备指标 / 日志字段 | 告警或回滚信号 | 回滚方式 |
|---|---|---|---|
| 结构化截断信号 | `recovery_kind`、provider raw finish reason、`has_effective_output`、是否用户可见 | 正常 stop 被标记为截断；Unknown finish reason 激增 | 关闭 enforcement，仅保留 reason 诊断 |
| 工具参数安全 | `tool_argument_status`、tool mutability、是否执行、拒绝原因 | read-only tool 被误拦截；mutating tool 拒绝率异常升高 | 降级为只拒绝 `Incomplete`，保留 mutability 诊断 |
| 前端恢复提示 | recovery kind、展示级别、用户是否继续、是否打开诊断 | 截断提示曝光明显超过实际 recovery 数；用户继续率下降 | 降级为诊断区展示，不打断消息流 |
| subagent scheduler | scheduler state、queued_ms、run_ms、retry_count、parent_session_id、cancelled_by_parent | queued/running 状态不闭合；父 session 取消后仍有 queued task | 关闭 scheduler enforcement，回退到静态 `SubagentConcurrencyLimiter` |
| gateway request limiter | gateway_key、configured_max、effective_max、permit acquired/released、overload classification | permit 泄漏；effective_max 振荡；fast cloud provider 被错误降速 | 关闭 adaptive concurrency，保留 configured static cap |
| subagent retry | retry_classification、retry_budget、effective_output_seen、state_changing_tool_seen | auth/quota/model-not-found 被重试；write-capable subagent 变更后被重试 | 关闭 retry，仅保留 queue 和 partial evidence |
| Review Evidence Pack | `review_evidence_pack_id`、`source_kind`、`source_provider`、`source_locator_hash`、`source_fingerprint`、`pack_generation_ms`、`source_collection_count`、`pack_hash`、`file_count`、`diff_line_count`、`artifact_slice_count`、`reviewer_full_evidence_reconstruction_count`、`pack_stale` | reviewer 重复重建完整 evidence；pack stale 仍被使用；pack 生成耗时长但无进度 | 禁用 pack enforcement，退回当前行为但记录重复取证诊断 |
| subagent artifact | result bytes、inline bytes、artifact path hash、清理状态 | artifact 创建失败；父 session 注入摘要失败 | 回退为中止 subagent 返回注入，保留子 session 不清理 |
| 预算预检 | estimated tokens、actual tokens、confidence、budget state、治理动作耗时 | SoftOverBudget 导致用户可见等待增加；压缩频率过高 | 切回 observe-only，不阻断不压缩 |
| context mutation | tokens before/after、summary source、耗时、有损标记 | 长耗时 mutation 无进度；收益低于阈值仍频繁触发 | 关闭自动 mutation，仅保留手动/诊断 |
| spill-to-file | original bytes、inline bytes、summary kind、sensitive flag、cleanup result | spill 失败、磁盘占用超限、敏感摘要泄漏 | 关闭泛化 spill，仅保留 Phase 1D subagent 专项 artifact |
| 大文件写入协议 | write_id、section、sequence、chunk hash、final hash、commit status | open write session 泄漏；hash mismatch；abort/continue 失败 | 禁用协议入口，回退为小文件直接写入 + 大文件 patch/hunk 提示 |

所有回滚都必须保持“有效输出不丢失”：关闭新策略时，宁可回到旧行为或 observe-only，也不能删除 artifact、丢弃 partial result 或把未完成写入显示为成功。

### Phase 1：止血修复

目标：消除静默截断，避免无意义重试。

| 内容 | 主要文件 | 验证 |
|---|---|---|
| 增加 `PartialRecoveryKind` + `ProviderFinishReason` | core execution types / event types / ai-adapters stream types | Rust 单元测试 |
| `PendingToolCalls` 暴露完整性方法 + `ToolArgumentStatus` | `tool_call_accumulator.rs` | JSON 完整/不完整/保守修复测试 |
| `TimedStreamItem::End` 检测 pending tool call + 纯文本截断 | `stream_processor.rs` | 模拟 stream end + 半截 JSON / 纯文本截断 |
| `TimedStreamItem::Error` 分支截断 kind 判断 | `stream_processor.rs` | 模拟 stream error + watchdog/timeout/rate-limit |
| 修正 partial recovery 重试策略 + `has_effective_output` 定义 | `round_executor.rs` | 截断不重试、网络类仍可重试、重试耗尽行为 |
| 连续截断计数器 `TruncationGuard` | `round_executor.rs` 或 session 层 | 连续截断超阈值强制终止 |
| Task tool 返回大小检查 + subagent 输出落盘引用 + recovery kind 上浮 | `task_tool.rs` / coordinator / session storage | subagent 返回超限时父 session 收到摘要和本地完整输出引用 |
| ai-adapter finish reason 归一化 | 各 adapter stream handler | 各 provider finish reason 映射测试 |
| Context 预算事件建模（不作为 partial recovery） | `execution_engine.rs` / event 定义 | microcompact / compression / emergency truncation 事件不会被误判为模型输出截断 |
| subagent scheduler state event | coordinator / scheduler event 定义 | accepted / queued / running / retry / completed / failed / cancelled 序列化测试 |
| 事件透传 recovery kind | `execution_engine.rs` / event 定义 | 事件序列化测试 |
| 前端展示截断与调度提示 | `snapshot.ts`、`EventHandlerModule.ts`、`aiErrorPresenter.ts`、相关消息组件和 locales | 前端单测 + waiting / retrying / completed_with_partial 手动验证 |

### Phase 2：预算预检与 Deep Review 拆分

目标：降低普通对话和大审核任务的截断率，同时避免因为低置信预算估算新增用户感知阻断。

| 内容 | 主要文件 | 验证 |
|---|---|---|
| 请求前预算预检分层（Near / Soft / Hard） | `execution_engine.rs` 或相邻 budget module | 低置信不阻断、高置信恢复后仍超限才阻断 |
| 本地上下文恢复优先 | context compression / microcompact 路径 | SoftOverBudget 后台恢复、HardOverBudget 恢复后重估 |
| 压缩冷却窗口和收益阈值 | context budget policy | 连续压缩不会频繁触发 |
| 长耗时 context mutation 进度事件 | event 定义 / frontend handler | 长耗时压缩有可见进度，短耗时不打断 |
| gateway-keyed AI request limiter | AI client boundary / provider adapter | permit acquire/release 覆盖 stream finish/error/cancel |
| provider overload retry classifier | provider diagnostics normalization | 429 / capacity / queue full 可重试，auth/quota/model-not-found 不重试 |
| adaptive effective concurrency | scheduler / gateway concurrency policy | overload 降低、成功窗口升高、最低保持 1、不振荡 |
| 估算文件组 token/byte/diff line | `deep_review_policy.rs` 或相邻预算模块 | 单元测试 |
| 分片策略加入预算维度 | Deep Review policy / Strategy Engine 输入 | 大 diff fixture、共享上下文计预算测试 |
| Review Evidence Pack preflight | Deep Review source resolver / provider / session artifact / Work Packet projection | 任意 source fingerprint 只完整收集一次；reviewer 只读取 slice artifact；pack stale 检测 |
| Deep Review Strategy Engine 接入 runtime scheduler | `reviewTeamService.ts` / `DeepReviewService.ts` / coordinator / scheduler | `max_parallel_instances` 只作为上限；无双调度；gateway concurrency=2 时 4-5 reviewers 排队完成 |
| reviewer queue/run/idle timeout 映射 | Deep Review policy / settings / scheduler | predictive timeout 映射为 `run_timeout_seconds`；queue waiting 不消耗 active review timeout |
| reviewer retry budget 接入 retry classifier | Deep Review policy / scheduler | 无双重试；auth/quota/model-not-found 不重试；gateway overload 可进入 `retry_waiting` |
| reviewer 返回大小限制 | subagent / review result 聚合路径 | 超大报告测试 |
| 子 reviewer 截断和调度状态上浮到父任务 | Deep Review service / UI | `partial_timeout` -> `completed_with_partial` / `timed_out` 映射；queued / retried / failed 展示测试 |

### Phase 3：统一输出预算

目标：覆盖工具输出、terminal 输出、subagent 返回等更广泛场景。

| 内容 | 主要文件 | 验证 |
|---|---|---|
| 定义 output budget policy | core service / tool layer | 单元测试 |
| 大输出 spill-to-file | tool implementations / session storage | 文件落盘测试 |
| 上下文注入摘要而非全文 | tool result formatter / agent context builder | 集成测试 |
| UI 打开完整输出 | web-ui adapter + message components | 前端测试 |

### Phase 4：大文件写入协议

目标：避免约 1k 行以上的大文件内容通过普通对话文本或单次 tool call JSON 传输，从源头降低长对话后的上下文超限和工具参数截断。

| 内容 | 主要文件 | 验证 |
|---|---|---|
| 定义 large file write policy 和触发阈值 | core tool policy / agent context builder | 阈值和模型预算单元测试 |
| 增加 session-backed writer manifest | tool layer / session storage | start / append / finish / abort 单元测试 |
| chunk 写入临时文件并原子提交 | filesystem service / tool implementation | hash mismatch、顺序错乱、abort 恢复测试 |
| tool result 只返回摘要、hash、artifact ref | tool result formatter / event types | 大文件写入不回灌全文的集成测试 |
| 大文件优先 patch/hunk 修改 | file edit tool / patch tool routing | 既有大文件小改动不触发全文重写 |
| 前端展示大文件写入进度 | web-ui adapter + message components | 创建、写入 section、校验、完成状态测试 |

---

## 风险与缓解

### 原有风险（设计内已识别）

| # | 风险 | 可能性 | 影响 | 解决办法 | 是否可根治 | 阶段 |
|---|---|---|---|---|---|---|
| R0-1 | 把正常 stream end 误判为截断 | 中 | 中 | 只把强信号判为截断；无 finish reason 单独不判截断 | 是，强信号策略可完全避免误判 | Phase 1 |
| R0-2 | provider finish reason 不统一 | 高 | 中 | 每个 ai-adapter 负责将 provider 原始 finish reason 归一化为 `Option<ProviderFinishReason>` 枚举（`Stop` / `Length` / `ToolUse` / `ContentFilter` / `Unknown`）。无法识别的 finish reason 归为 `Unknown`，不触发截断检测 | 否，新 provider 或 provider 行为变更可能引入新的 `Unknown`，但归一化层确保不会误判 | Phase 1 |
| R0-3 | JSON 修复生成语义错误参数 | 中 | 高 | 引入 `ToolArgumentStatus`；只有原始完整 JSON 和白名单内不改变语义的保守修复可执行；语法补齐类修复不实现自动执行 | 是，但前提是执行路径强制检查 status；否则该修改无效 | Phase 1 |
| R0-4 | 字符串 reason 被误用为逻辑判断 | 高 | 中 | 引入 `PartialRecoveryKind`，系统逻辑只消费 kind；reason 仅用于日志和诊断 | 是，kind 枚举可完全替代字符串判断 | Phase 1 |
| R0-5 | 前端提示过多打扰用户 | 中 | 低 | 按严重度展示：普通 partial recovery 轻提示；短耗时后台治理只进诊断；长耗时治理显示进度；fatal error 仅用于无有效输出且不可恢复 | 是，分层展示可避免把恢复过程渲染成额外打扰 | Phase 1 |
| R0-6 | Deep Review 拆分过细导致成本上升 | 中 | 中 | 并发限制（`SubagentConcurrencyLimiter`）、最大同角色实例数、预算阈值共同控制 | 是，三项约束联合可限制成本上限 | Phase 2 |
| R0-7 | spill-to-file 暴露敏感内容路径 | 低 | 中 | 见 R12 完整方案（文件权限 + 敏感检测 + REDACTED + UI 警告） | 是，三层防护可解决 | Phase 3 |

### 竞品对比发现的新风险与解决办法

#### R1：模型自主重试截断工具导致 doom loop（高风险）

**来源**：Opencode #18108 — `finishReason: length` + `repairToolCall` 修复后模型重新提交，无限循环。

**场景**：round_executor 不重试，但下一轮模型看到截断工具的 `is_error` 结果后，自主决定"换个方式再试"，再次触发同类工具调用，再次截断。

**解决办法**：

在 `round_executor` 或 `session` 层增加 **连续截断计数器**：

```rust
struct TruncationGuard {
    consecutive_truncation_count: u32,
    max_consecutive_truncations: u32, // 默认 3
}
```

每次 round 结束时：

- 若 `partial_recovery_kind` 是截断类（`ToolArgumentsIncomplete` / `ProviderMaxTokens`），计数器 +1；
- 若 round 正常完成，计数器归零；
- 若计数器 >= `max_consecutive_truncations`，强制终止当前 turn，向用户提示"连续多次截断，建议拆分任务或缩短上下文"。

此计数器是 **session 级别** 的，跨 turn 生效，防止模型在多个 turn 中反复尝试同一截断路径。

**阶段**：Phase 1（与 A5 同步实施）。

---

#### R2：subagent 截断导致父 session context overflow（高风险）

**来源**：Claude Code #23463 — subagent results silently overflow context, causing unrecoverable session crash。

**场景**：Task tool 启动的 subagent 截断后返回了部分结果，部分结果仍然很大，注入父 session context 后导致父 session 也 overflow。

**解决办法**：

在 Task tool 返回结果给父 session 之前，增加 **返回大小检查 + 本地落盘引用**：

```rust
const MAX_SUBAGENT_RETURN_BYTES: usize = 32 * 1024; // 32 KiB

if result_bytes > MAX_SUBAGENT_RETURN_BYTES {
    // 完整结果先落盘到父 session 可访问的 artifact/spill 目录
    let artifact = persist_subagent_output(parent_session_id, subagent_session_id, result);
    // 父 session 只注入摘要、统计和本地 artifact 引用
    result = summarize_with_artifact_ref(artifact, MAX_SUBAGENT_RETURN_BYTES);
    result.truncation_kind = Some(TruncationKind::SubagentOutputBudgetExceeded);
}
```

Phase 1 不允许只做硬截断。原因是当前 hidden subagent 成功后会删除子 session；如果不先落盘，完整 reviewer 输出会永久丢失。通用 spill-to-file 可留到 Phase 3，但 subagent / Deep Review reviewer 返回必须在 Phase 1 先具备专项落盘能力。

同时在 subagent 的 turn completed 事件中，将 `partial_recovery_kind` 上浮到父 session 的事件流：

```rust
// 在 task_tool 的结果聚合逻辑中
if subagent_result.partial_recovery_kind.is_some() {
    parent_event.subagent_truncation = Some(SubagentTruncationInfo {
        subagent_session_id,
        role: subagent_role,
        recovery_kind: subagent_result.partial_recovery_kind,
    });
}
```

**阶段**：Phase 1 做大小检查、父 session artifact 落盘、摘要注入和 recovery kind 上浮；Phase 3 再泛化为统一输出预算。

---

#### R3：多 tool call 并行，部分截断（中风险）

**来源**：Claude Code #19143 — streaming tool-use arguments truncated by premature stop。

**场景**：模型同时输出 3 个 tool call，第 3 个被截断，前 2 个已完整。若直接执行前 2 个完整 tool call，可能造成半套副作用；若全部标记失败，又会丢失已经完整的只读观察结果。

**解决办法**：

在 `force_finish_pending_tool_calls()` 中区分完整性，但 **不要默认执行截断批次中的完整 mutating tool call**：

```rust
fn force_finish_pending_tool_calls(&mut self) {
    for tc in &mut self.pending_tool_calls {
        if tc.is_json_complete() {
            // JSON 完整，但本批次已发生截断，只标记为 CompleteWithinTruncatedBatch
            tc.finalize_complete_within_truncated_batch();
        } else {
            // JSON 不完整，标记为 Incomplete，不执行
            tc.finalize_as_incomplete();
        }
    }
}
```

执行策略：

1. 截断批次内的 `Incomplete` / `RepairedUntrusted` 一律不执行；
2. 截断批次内的完整 read-only tool 不因同批其他工具截断而关联拦截，可按白名单继续执行；
3. mutating tool 不再简单“一刀切”拦截，而是按工具元数据做最小拦截：
   - `read_only`：继续执行；
   - `idempotent_local_mutation`：参数完整、无依赖未完成工具、可记录 operation id 防重复时允许执行；
   - `destructive_or_external_side_effect`：删除、提交、推送、外部网络写入、任意 shell 写操作等默认延后；
   - `unknown_mutability`：按 mutating 高风险处理，不自动执行。
4. 被延后的 mutating tool 不应直接把用户卡在错误态；系统应把完整参数、截断批次信息和延后原因返回给模型，优先让下一轮自动重新规划或补发缺失动作；
5. 只有在无法自动恢复，且继续执行可能产生不可逆副作用时，才向用户显示需要确认或缩小任务的提示。

**阶段**：Phase 1。

---

#### R4：stream error 携带截断信号（中风险）

**来源**：Opencode #12233 — `StreamIdleTimeoutError` 导致 infinite retry loop。

**场景**：某些 provider 在输出截断时不发送正常 End，而是发送 error event（如 `StreamIdleTimeoutError`、`ContentFilterError`）。当前设计只在 `TimedStreamItem::End` 分支检测，会遗漏这类截断。

**解决办法**：

在 `TimedStreamItem::Error` 分支也增加截断检测：

```rust
TimedStreamItem::Error(err) => {
    // 现有逻辑：标记 partial_recovery_reason
    // 新增：如果已有有效输出且 error 类型可恢复，设置对应的 recovery kind
    if ctx.has_effective_output() {
        if is_stream_idle_timeout(&err) {
            ctx.partial_recovery_kind = Some(PartialRecoveryKind::WatchdogTimeout);
        } else if is_stream_interrupted(&err) {
            ctx.partial_recovery_kind = Some(PartialRecoveryKind::StreamInterrupted);
        } else if is_rate_limit_error(&err) {
            ctx.partial_recovery_kind = Some(PartialRecoveryKind::RateLimited);
        }
    }
    // ... 现有 error 处理逻辑
}
```

同时在 `PartialRecoveryKind` 枚举中补充 `RateLimited` variant。

**阶段**：Phase 1。

---

#### R5：纯文本回答被截断，无 tool call（中风险）

**来源**：所有竞品均未良好处理。

**场景**：模型输出纯文本回答（无 tool call），被 max_tokens 截断。当前设计只检测 pending tool call，不检测纯文本截断。

**解决办法**：

在 `TimedStreamItem::End` 分支增加纯文本截断检测：

```rust
// 在 ToolArgumentsIncomplete 和 ProviderMaxTokens 检测之后
else if ctx.provider_metadata_indicates_max_tokens() && ctx.has_text_output() {
    ctx.partial_recovery_kind = Some(PartialRecoveryKind::OutputTruncated);
    ctx.partial_recovery_reason = Some(
        "Model text output was truncated by provider max tokens".to_string()
    );
}
```

注意：`ProviderMaxTokens` 和 `OutputTruncated` 的区分——前者是 provider 明确报告的，后者是推断的。优先使用 provider 信号。

对纯文本截断，用户提示为："模型输出可能不完整。建议缩短请求或新建会话继续。"

**阶段**：Phase 1。

---

#### R6：`has_effective_output` 定义模糊（中风险）

**来源**：A5 重试策略依赖此判断，但文档未精确定义。

**场景**：网络中断时，stream 可能已收到部分 text chunk 但没有完整 tool call。此时 `has_effective_output` 的判断直接影响是否重试。

**解决办法**：

明确定义：

```rust
fn has_effective_output(&self) -> bool {
    // 有非空文本输出
    let has_text = self.assistant_text.as_ref().map_or(false, |t| !t.is_empty());
    // 有至少一个非 error 的 finalized tool call
    let has_valid_tool = self.tool_calls.iter().any(|tc| !tc.is_error);
    has_text || has_valid_tool
}
```

关键语义：

- 空文本 + 空 tool call = 无有效输出 → 可安全重试（网络类）；
- 有文本 + 空 tool call = 有有效输出 → 谨重重试（重试可能产生重复文本）；
- 有 error tool call = 有有效输出 → 不重试（截断类）。

**阶段**：Phase 1（与 A5 同步实施）。

---

#### R7：前后端版本兼容（低风险）

**来源**：A6 新增事件字段，旧版前端/后端不识别。

**场景**：滚动发布时，新版后端发送含 `partial_recovery_kind` 的事件，旧版前端忽略该字段；旧版后端发送不含该字段的事件，新版前端需要处理 `undefined`。

**解决办法**：

- 后端：`partial_recovery_kind` 和 `partial_recovery_reason` 均为 `Option<T>`，序列化时 `None` 不输出字段（serde 默认行为），旧版前端不受影响；
- 前端：所有新字段声明为 optional（`partial_recovery_kind?: ...`），`undefined` 时按"无截断"处理；
- 不需要版本协商，JSON 容忍性已足够。

**阶段**：Phase 1（设计约束，无需额外代码）。

---

#### R8：JSON 完整性检测性能与误判（中风险）

**来源**：A2 每次调用 `has_incomplete_json_payload()` 做 full parse；原方案试图仅用 open brace/bracket count 避免 parse。

**场景**：`raw_arguments` 可能很大（几万字符），stream end 时做 full parse 有延迟。但如果只用简单括号计数，又可能被字符串内 `{}`、转义引号、snapshot 覆盖等情况误导，把不完整 JSON 判成完整，从而触发工具执行。

**解决办法**：

Phase 1 不采用“只读 open brace/bracket count、不 re-parse”的方案。该方案会把性能问题转化为执行安全问题，标记为无效。

Phase 1 的有效方案：

1. `has_incomplete_json_payload()` 在 stream end / finish boundary 做一次 `serde_json::from_str`，以 parse 结果作为最终完整性判断；
2. 可增加轻量 boundary hint（brace/bracket、字符串状态）作为快速判定“明显不完整”的早退路径；
3. 任何 hint 判定为完整后，仍必须执行 `serde_json::from_str` 验证；
4. 如需优化为增量检测，必须实现 JSON lexical state machine（跟踪字符串、转义、对象/数组栈和 snapshot reset），并保留 parse 作为最终校验；
5. 检测摘要只记录长度、边界状态、错误类别，不记录完整参数。

```rust
struct PendingToolCall {
    // ... 现有字段
    json_boundary_hint: JsonBoundaryHint,
}
```

**阶段**：Phase 1。

---

#### R9：token 估算准确性不足（低风险）

**来源**：A8 预算治理依赖 token 估算，但 char/4 粗估误差可能 2-3x。

**场景**：中文/日文等 CJK 字符 1 char ≈ 2-3 tokens，char/4 严重低估；代码中大量缩进/空行可能高估。

**解决办法**：

采用 **分层估算策略**：

1. **精确模式**：若模型有已知 tokenizer（如 tiktoken for GPT 系列），使用 tokenizer 计算；
2. **启发式模式**：否则使用 `chars * 0.6`（比 char/4 更保守，对 CJK 和代码都更安全）；
3. **安全系数**：估算结果乘以 1.2（20% 安全余量），用于预算判断；
4. **置信度分层**：估算来源、模型 profile、新旧 actual token 偏差共同决定 confidence；低置信估算只能触发后台治理或诊断，不能阻断普通对话；
5. **日志记录**：每次估算后记录 estimated vs actual（如果后续有 actual token count），用于校准。

Phase 2 初期使用启发式模式 + 安全系数；后续可按模型引入精确 tokenizer。

**阶段**：Phase 2。

---

#### R10：Deep Review 拆分后 reviewer 之间有交叉依赖（中风险）

**来源**：A8 预算治理按 token/byte 拆分，但文件间可能有语义依赖。

**场景**：文件 A 的 review 依赖文件 B 的类型定义，拆分后 reviewer A 看不到文件 B 的信息，review 质量下降。

**解决办法**：

在拆分逻辑中保留 **共享上下文**：

```rust
struct ReviewerGroup {
    primary_files: Vec<FileRef>,
    shared_context_files: Vec<FileRef>, // 类型定义、接口、公共模块
}
```

拆分时：

1. 识别"共享依赖文件"（被多个 primary file import 的文件）；
2. 将共享依赖文件加入每个 reviewer group 的 `shared_context_files`；
3. 共享文件必须计入每个 reviewer group 的预算（因为每个 reviewer 都会看到）；
4. 在 reviewer prompt 中明确标注哪些是共享上下文、哪些是主要审核目标。

预算公式：

```text
group_input_tokens =
    primary_files_tokens
  + shared_context_tokens
  + reviewer_prompt_tokens
  + tool_schema_tokens
  + reserved_output_tokens
```

若 `group_input_tokens > safe_context_budget`，不得继续重复注入共享全文。应按优先级降级：

1. 将共享文件替换为接口签名 / 类型声明 / import 图摘要；
2. 仍超预算时，进一步缩小 primary file group；
3. 仍超预算时，提示用户缩小审核范围，而不是发送高风险请求。

**阶段**：Phase 2。

---

#### R11：spill 文件生命周期与磁盘累积（低风险）

**来源**：A10 长期运行 session 产生大量 spill 文件。

**场景**：用户长时间使用同一 session，多次触发 spill，磁盘占用持续增长。

**解决办法**：

1. spill 文件保存在 `.bitfun/sessions/{session_id}/spill/` 目录；
2. session 删除时一并清理 spill 目录（复用现有 session 清理逻辑）；
3. 单个 session 的 spill 目录大小上限：默认 100 MiB，超限后最旧文件被 LRU 淘汰；
4. 全局 spill 目录总大小上限：默认 1 GiB，超限后按 session 最后活跃时间淘汰；
5. 日志记录 spill 文件创建、大小和淘汰事件。

**阶段**：Phase 3。

---

#### R12：spill 文件包含敏感信息（中风险）

**来源**：Codex #14206 讨论；terminal 输出可能含 API key、环境变量值。

**场景**：工具输出包含 `AWS_SECRET_ACCESS_KEY=xxx`，spill 到文件后，文件权限不当可能泄露。

**解决办法**：

1. spill 文件创建时设置权限为仅当前用户可读（Unix: 0600，Windows: 仅当前用户 ACL）；
2. 在 spill 前对输出做 **敏感模式检测**（正则匹配常见 secret pattern：API key、token、password、private key）；
3. 检测到敏感内容时：
   - 仍然 spill（保留完整内容供诊断）；
   - 在摘要中用 `[REDACTED]` 替换敏感值；
   - 日志中标记 `spill_file_contains_sensitive_content = true`；
   - UI 打开 spill 文件时显示警告"该输出可能包含敏感信息"。
4. 不上传 spill 文件到远端（已有约束）。

**阶段**：Phase 3。

---

#### R13：摘要质量不足（低风险）

**来源**：A10 spill-to-file 的摘要策略。

**场景**："前 N 行 + 统计 + 文件引用"是最简摘要，但某些场景需要语义摘要（如"该命令输出了 500 行编译错误，主要涉及 3 个模块"），生成语义摘要需要额外 LLM 调用。

**是否可解决**：可解决，但需要分层。

**解决办法**：

采用 **分层摘要策略**，不用 LLM 生成语义摘要：

1. **结构化摘要**（Phase 3 默认）：前 N 行 + 行数/字节数统计 + 文件引用。对于模型判断"是否需要查看完整内容"已足够。
2. **启发式摘要**（Phase 3 同步实现）：对已知格式做轻量结构化提取，不调用 LLM：
   - 编译错误：提取 error/warning 行 + 涉及文件列表 + 错误数统计；
   - test 输出：提取 pass/fail/skip 计数 + 失败用例名；
   - git diff stat：复用 diff stat 输出（已有）；
   - 其他：退回结构化摘要。
3. **LLM 语义摘要**（不实施）：额外 LLM 调用增加延迟、成本和截断风险（摘要本身也可能被截断），且引入循环依赖（截断治理依赖 LLM 调用，LLM 调用又可能截断）。**不纳入本设计**。

**判断**：Phase 3 实现结构化摘要 + 启发式摘要，不实现 LLM 语义摘要。启发式摘要已覆盖最常见的工具输出类型，无需 LLM 调用，不引入额外截断风险。

---

#### R14：预算随模型切换动态变化（低风险）

**来源**：A9 统一预算策略。

**场景**：用户在 session 中切换模型（Claude 200K → GPT-4o 128K → glm 128K），预算应随之调整。

**解决办法**：

预算策略从 `model_profile` 读取，不硬编码：

```rust
struct ModelBudgetProfile {
    context_window_tokens: u32,
    max_output_tokens: u32,
    safe_input_ratio: f32, // 默认 0.7，即输入不超过 70% context window
}
```

每次模型切换时重新计算预算。`ModelBudgetProfile` 按 model id 从配置加载，未知模型使用保守默认值（128K context, 4K output, 0.6 safe ratio）。

**阶段**：Phase 2（Deep Review 预算治理时同步实施）。

---

#### R15：预算耗尽前无预警（低风险）

**来源**：A9 统一预算策略。

**场景**：等到截断才提示用户，用户已经浪费了一轮 LLM 调用。

**解决办法**：

在请求发送前做 **预算预检**，但预检的产品目标是减少用户感知中止，不是提前制造阻断。估算逻辑不能保证完全准确，因此必须区分置信度和动作级别：

```rust
fn check_budget_before_request(&self, estimate: BudgetEstimate, model: &ModelId) -> BudgetCheckResult {
    let profile = self.get_model_budget(model);
    let thresholds = profile.budget_thresholds_for(estimate.confidence);
    if estimate.input_tokens > thresholds.hard_over_budget {
        BudgetCheckResult::HardOverBudget { estimate, profile }
    } else if estimate.input_tokens > thresholds.soft_over_budget {
        BudgetCheckResult::SoftOverBudget { estimate, profile }
    } else if estimate.input_tokens > thresholds.near_budget {
        BudgetCheckResult::NearBudget { estimate, profile }
    } else {
        BudgetCheckResult::WithinBudget
    }
}
```

- `HardOverBudget`：先执行本地确定性治理动作（microcompact / compression / 输出摘要化 / spill-to-file / Deep Review 分片）；治理后重新估算，仍高置信超限时才不发送同一个大请求，并返回可恢复状态；
- `SoftOverBudget`：不直接阻断。优先在后台执行轻量压缩或摘要化；若压缩耗时可控，用户无需感知；若可能长耗时，进入“正在整理上下文以继续”的进度状态；
- `NearBudget`：记录 warn 日志，不干预执行；
- `WithinBudget`：正常执行。

为避免频繁上下文压缩，增加节流与收益判断：

1. 同一 session 在短时间窗口内只能触发一次重压缩；窗口内再次接近预算时优先复用已有摘要或 spill 引用；
2. 压缩前估算可回收 token，若预计收益低于阈值（例如 < 10% context window），不触发重压缩；
3. 压缩后记录 tokens before/after、耗时和摘要来源，用于后续校准；
4. 如果连续压缩仍不能降低预算风险，则说明任务本身需要拆分，此时进入自动拆分或用户引导，而不是反复压缩。

准确性约束：

- token 估算只能作为治理触发信号，不能作为低置信阻断依据；
- 阻断必须满足“高置信超限 + 本地恢复动作已尝试 + 仍无法安全发送”三个条件；
- `NearBudget` / `SoftOverBudget` / `HardOverBudget` 阈值必须来自 model profile、估算来源和历史误差校准；文档中的百分比只能作为示意，不得实现为全局固定魔数；
- 如果无法证明新预检能减少用户感知中止或异常数量，则该预检只能记录诊断，不进入默认执行路径。

注意：软提示只能作为辅助信息，不能作为 OverBudget 的主要解决措施。单纯在 prompt 中注入警告会进一步挤占上下文，而且弱模型可能无法自我拆分，因此该做法不能单独进入实现。

**阶段**：Phase 2。

---

#### R16：重试次数耗尽后截断类 partial recovery 的行为未定义（中风险）

**来源**：A5 重试策略。

**场景**：`attempt_index == max_attempts - 1` 时，截断类 partial recovery 的处理未明确：是静默完成？还是标记为 error？

**解决办法**：

明确定义：重试次数耗尽时，截断类 partial recovery 的行为与"不重试"一致——**finish with partial recovery**：

- round 正常结束（不是 error）；
- `partial_recovery_kind` 和 `partial_recovery_reason` 被保留在 round result 中；
- 事件正常传播到前端；
- 前端显示截断提示。

不标记为 error 的原因：round 确实产生了部分有效输出（文本、已完成的 tool call），标记为 error 会丢失这些有效内容。

**阶段**：Phase 1。

---

#### R17：context compaction 截断历史导致 agent 丢失关键上下文（中风险）

**来源**：Opencode #8089, #18037。

**场景**：auto-compaction 删除历史消息以腾出空间，但可能删除了 agent 当前任务依赖的关键上下文（如用户指令、文件结构、之前工具调用的结果）。

**是否可解决**：当前 BitFun 已有 context compression、microcompact 和 emergency truncation 这类输入侧上下文治理能力，因此该风险在当前产品中成立。它不应作为模型输出截断处理，但必须被纳入上下文预算事件和信任元数据治理。

**解决办法**：

1. `ContextCompacted` 不放入 `PartialRecoveryKind`，避免把输入侧上下文变更误判为输出流恢复；
2. 增加独立事件 / 状态：

```rust
pub enum ContextMutationKind {
    Microcompact,
    ModelCompression,
    StructuredFallbackCompression,
    EmergencyTruncation,
}
```

3. 每次上下文变更记录：`kind`、tokens before/after、被清理的 turn 范围、是否存在模型摘要、summary source、是否触发 emergency truncation；
4. emergency truncation 必须进入用户可见提示或诊断区域，因为它是有损删除，不应只写日志；
5. 用户指令、系统 prompt、当前任务描述、压缩边界摘要需要明确保护策略；如果某种上下文治理无法满足这些保护条件，则该治理模式不得发布或默认开启；
6. 按耗时区分展示策略：
   - 短耗时压缩：作为后台治理，仅在诊断区记录，不打断消息流；
   - 长耗时压缩：进入可见进度状态，例如“正在整理上下文以继续”，避免用户看到长时间无输出；
   - 有损 emergency truncation：必须显示轻量提示，说明 BitFun 已尽量保留关键上下文，并提供查看诊断的入口。

**判断**：当前不实现 `PartialRecoveryKind::ContextCompacted`；改为实现输入侧 `ContextMutationKind` 事件。若无法给 emergency truncation 提供用户可见提示和诊断证据，则不要扩大自动上下文删除能力。

---

#### R18：i18n key 未定义，英文文案缺失（低风险）

**来源**：A7 前端提示。

**场景**：文档列出了中文提示文案，但没有定义 i18n key 和英文对应。

**解决办法**：

定义 i18n key 结构：

```text
ai_error.output_truncated.tool_arguments_incomplete
ai_error.output_truncated.provider_max_tokens
ai_error.output_truncated.stream_interrupted
ai_error.output_truncated.watchdog_timeout
ai_error.output_truncated.rate_limited
ai_error.output_truncated.consecutive_truncation
ai_error.context.preparing_to_continue
ai_error.context.reorganized
ai_error.context.emergency_truncated
```

英文文案：

| Key | English |
|---|---|
| `tool_arguments_incomplete` | The model stopped while preparing an action. BitFun kept the safe parts and will avoid running incomplete actions. |
| `provider_max_tokens` | The model reached its output limit. BitFun preserved the current answer and can continue with a smaller next step. |
| `stream_interrupted` | The model response was interrupted. Partial output has been preserved. |
| `watchdog_timeout` | The model stopped responding for a while. Partial output has been preserved. |
| `rate_limited` | The provider paused this response due to rate limits. BitFun preserved the current state so you can continue shortly. |
| `consecutive_truncation` | This task is repeatedly exceeding the model output limit. BitFun preserved the current result and needs a smaller next step. |
| `preparing_to_continue` | BitFun is organizing the context so the model can continue. |
| `reorganized` | BitFun reorganized earlier context to keep this conversation within the model limit. |
| `emergency_truncated` | BitFun had to trim older context to keep the task running. Key instructions were preserved where possible. |

中文文案应遵循同一语气：明确问题来自模型输出限制、provider 中断或上下文预算，而不是用户操作错误；同时说明 BitFun 已保留有效结果、正在恢复或会引导用户更好地继续。避免使用“失败”“错误”“无法处理”作为主文案，除非确实没有可恢复内容。

**阶段**：Phase 1。

---

#### R19：大文件 chunk 顺序错乱或内容缺失（中风险）

**来源**：Large File Write Protocol。

**场景**：模型或工具层分多次 append 大文件内容时，chunk sequence 错乱、重复、遗漏，最终文件看似写入成功但内容不完整。

**解决办法**：

1. 每个 write session 维护 manifest：`write_id`、`expected_sections`、`next_sequence`、chunk hash、累计 bytes；
2. `append_file_chunk` 必须校验 sequence 单调递增，重复 sequence 只能在 hash 完全一致时幂等接受；
3. `finish_file_write` 校验 expected hash、section 完整性和基础格式；
4. hash 或 section 不匹配时不提交目标文件，只保留临时 artifact 和恢复建议。

**阶段**：Phase 4。

---

#### R20：半成品文件被误认为成功（高风险）

**来源**：Large File Write Protocol。

**场景**：写入中断后临时文件已经存在，如果系统或模型把它当作目标文件完成状态，后续构建/测试会基于半成品继续。

**解决办法**：

1. 所有大文件写入先进入 session temp path，不直接覆盖目标文件；
2. 只有 `finish_file_write` 校验成功后才原子替换目标文件；
3. turn 结束时若存在 open write session，必须生成 `FileWriteIncomplete` 事件；
4. UI 展示“写入未完成，可继续/放弃”，而不是显示成功。

**阶段**：Phase 4。

---

#### R21：后续修改读不到完整上下文（中风险）

**来源**：Large File Write Protocol。

**场景**：上下文只保存 manifest 和摘要，模型后续想修改刚生成的大文件时，如果不读取相关 range/symbol，可能凭摘要误改。

**解决办法**：

1. manifest 中记录 section line ranges、symbol index 或结构化摘要；
2. 后续修改大文件时，agent context builder 注入 manifest，并提示模型按需读取 range/symbol；
3. patch tool 在修改前可自动读取目标 range，避免模型凭记忆修改；
4. 对跨 section 修改，要求先读取相关 sections 再生成 patch。

**阶段**：Phase 4。

---

#### R22：大文件写入 artifact 泄漏或磁盘累积（中风险）

**来源**：Large File Write Protocol 与 spill-to-file 共用 session artifact。

**场景**：大文件临时 chunk、manifest、失败 artifact 长期留在 `.bitfun/sessions` 下，可能占用磁盘或包含敏感内容。

**解决办法**：

1. large write artifact 复用 R11/R12 的权限、敏感检测、session 删除清理、LRU 淘汰策略；
2. abort 或 finish 后清理不再需要的 chunk，只保留必要 manifest 和最终 artifact 引用；
3. 日志只记录路径、大小、hash 和敏感标记，不记录完整内容；
4. UI 打开 artifact 时沿用敏感内容警告。

**阶段**：Phase 4。

---

#### R23：gateway overload 误分类导致错误重试或错误降速（中风险）

**来源**：Agent Runtime Subagent Scheduling Plan。

**场景**：provider 返回 auth、quota、billing、model-not-found 等不可重试错误，但 runtime 将其误判为 capacity / overload，导致无意义排队、重试或降低 effective concurrency。

**解决办法**：

1. provider adapter 保留 raw diagnostics，并先分类不可重试错误；
2. 只有 HTTP 429、capacity exceeded、queue full、server busy、concurrency saturation、无有效输出前 transient network failure 进入 retryable；
3. `Unknown` provider error 不默认重试，只进入诊断；
4. retry classifier 有 fixture 覆盖 vLLM overload、OpenAI-compatible 429、auth/quota/model-not-found。

**阶段**：Phase 2。

---

#### R24：adaptive concurrency 振荡（中风险）

**来源**：Agent Runtime Subagent Scheduling Plan。

**场景**：provider 错误噪声较大，runtime 一会儿降低 effective max、一会儿升高，导致 reviewer 执行节奏不稳定。

**解决办法**：

1. 降低 effective max 要快，升高必须慢，并基于连续成功窗口；
2. effective max 永远不低于 1；
3. 每个 gateway key 独立维护计数，避免一个慢 gateway 影响其他 provider；
4. 若振荡超过阈值，关闭 adaptive，仅使用 configured static cap。

**阶段**：Phase 2。

---

#### R25：queue timeout 掩盖真实 provider 故障（中风险）

**来源**：Agent Runtime Subagent Scheduling Plan。

**场景**：所有 reviewer 都在排队或 retry_waiting，但真实原因是 auth/quota/model 配置错误。若 UI 只显示“等待容量”，用户会误以为稍后会恢复。

**解决办法**：

1. queue timeout 只用于等待 scheduler capacity，不用于吞掉 provider 非重试错误；
2. auth/quota/model-not-found/invalid key 必须立即进入设置或诊断提示；
3. retry budget 耗尽后显示最终 provider diagnostics 和已保留 partial evidence；
4. Deep Review summary 区分 queued execution、gateway overload、provider configuration error。

**阶段**：Phase 2。

---

#### R26：write-capable subagent 重试产生副作用（高风险）

**来源**：Agent Runtime Subagent Scheduling Plan。

**场景**：写型 subagent 在执行过文件修改、shell 写操作、提交或外部写入后失败。如果 runtime 自动重试，可能造成重复修改或不可逆副作用。

**解决办法**：

1. subagent retry policy 必须读取 tool history 的 mutability 结果；
2. write-capable subagent 只有在确认没有 state-changing tool 执行时才允许自动重试；
3. read-only reviewer 可使用更宽松 retry budget，但 partial output 仍应优先保留而不是盲目重跑；
4. 无法证明安全时，不重试，返回 `completed_with_partial` 或 failed with evidence。

**阶段**：Phase 2。

---

#### R27：父 session 取消后 queued subagent 未清理（中风险）

**来源**：Agent Runtime Subagent Scheduling Plan。

**场景**：用户取消父任务或 session 关闭后，queued / retry_waiting subagent 仍在后台等待 permit，之后意外运行并污染事件流或资源。

**解决办法**：

1. scheduler state 必须绑定 parent session / parent turn cancellation token；
2. parent cancelled 后，所有 queued / waiting_for_capacity / retry_waiting task 进入 `cancelled`；
3. running subagent 尽力取消 stream，并释放 gateway permit；
4. 取消路径必须有 permit release 和 artifact cleanup 测试。

**阶段**：Phase 1 / Phase 2。

---

#### R28：Deep Review Strategy Engine 与 runtime scheduler 双调度（高风险）

**来源**：`deep-review-design.md` 已实现后的交叉对照。

**场景**：Deep Review 的 `DeepReviewConcurrencyPolicy` / frontend batching 已经控制 reviewer 分批启动，runtime `SubagentScheduler` 又再次排队和限流。两套调度叠加后，用户可能看到重复等待、队列状态不一致，甚至某些 reviewer 永远无法被调度。

**解决办法**：

1. Deep Review policy 只输出 reviewer 列表、scope、`max_parallel_instances`、timeout / retry budget 等约束；
2. `SubagentScheduler` 是唯一实际执行调度器，负责 queue、permit、retry_waiting、cancel cleanup；
3. `reviewTeamService.ts` / `DeepReviewService.ts` 不直接根据 cached rate-limit 状态决定最终执行并发，只能把 provider/model hints 写入 manifest；
4. 增加 Deep Review compatibility fixture，验证同一 reviewer 只进入一个 scheduler state machine。

**阶段**：Phase 2C。

---

#### R29：Deep Review retry budget 与 scheduler retry classifier 双重试（高风险）

**来源**：`deep-review-design.md` 已实现后的交叉对照。

**场景**：Deep Review orchestrator 根据每角色 retry budget 重发 reviewer，scheduler 又根据 provider overload 做自动 retry，导致同一 reviewer 被重复执行，成本和时间放大，甚至重复产生 conflicting evidence。

**解决办法**：

1. Deep Review retry budget 只定义“最多允许 retry 的策略上限”；
2. 是否 retry 必须经过 scheduler retry classifier；
3. retry event 必须带 `retry_owner = scheduler`，Deep Review orchestrator 只能请求 retry，不能绕过 scheduler 自行重发；
4. 对已有 partial evidence 的 read-only reviewer，默认进入 `completed_with_partial`，除非 classifier 判断 retry 更有收益。

**阶段**：Phase 2C。

---

#### R30：Deep Review partial timeout 与通用 partial recovery 状态冲突（中风险）

**来源**：`deep-review-design.md` 已实现后的状态映射。

**场景**：Deep Review 已有 `partial_timeout` / `PartialTimeout`，而 runtime 方案使用 `completed_with_partial` / `timed_out`。如果 UI、Judge、日志同时消费两套状态，可能把有用 partial evidence 显示成失败，或把无 evidence timeout 显示成部分成功。

**解决办法**：

1. 统一对外 scheduler state：有有效 evidence 的 `partial_timeout` 映射为 `completed_with_partial`；
2. 无有效 evidence 的 timeout 映射为 `timed_out`；
3. Deep Review 原始状态保留在 diagnostics / raw_status，不作为主判断字段；
4. ReviewJudge 输入只消费统一状态和 partial evidence，不解析原始错误文案。

**阶段**：Phase 1 / Phase 2C。

---

#### R31：Deep Review 大 source 重复取证导致 reviewer 超时（中风险）

**来源**：最近 X 个提交、PR URL、working tree、patch artifact、超大 diff Deep Review 的实际体验问题。

**场景**：每个 reviewer 在 subagent 内自行重新拉取或重建完整变更证据，例如本地提交范围、远端 PR 文件列表、patch 内容、文件快照和统计信息。4-5 个 reviewer 会重复读取同一批变更，导致大量时间消耗在命令执行、网络/API、输出传输和解析上，而不是实际分析；大仓库、远端 PR、Windows 环境或自托管代码平台下尤其容易触发 timeout。

**解决办法**：

1. Deep Review 父任务在 launch preflight 阶段通过 source resolver 和 provider 一次生成 source-agnostic `ReviewEvidencePack`，并落盘到 session artifact；
2. Work Packet projection 只给 reviewer 对应的 `evidence_slice`、artifact ref、允许读取的源码路径和必要局部查询权限；
3. reviewer 默认禁止重复重建完整 source evidence；需要更多上下文时请求父任务扩展 pack，或执行小范围只读查询；
4. pack generation 时间单独记录和展示，不计入 reviewer active run timeout；
5. pack 绑定 `source_kind` / `source_locator_hash` / `source_fingerprint` / `pack_hash`，stale 时必须刷新或让用户确认。

**阶段**：Phase 2C。

---

### 方案变动的周边影响评估

本轮风险复核后，方案从“检测截断并提示”收紧为“检测截断、隔离不可信工具调用、保留完整证据、再提示”。主要影响如下：

| 变动 | 影响范围 | 正面影响 | 用户负面感知风险与约束 |
|---|---|---|---|
| `ContextCompacted` 从 `PartialRecoveryKind` 移除，改为 `ContextMutationKind` | core execution events、transport、前端诊断展示 | 避免把输入侧上下文压缩误当作模型输出中断；能单独审计 microcompact / compression / emergency truncation | 短耗时压缩不打断消息流；长耗时压缩必须显示进度；只有有损 emergency truncation 显示轻量提示 |
| `ToolArgumentStatus` 替代单纯 `is_error` 判断 | ai-adapters accumulator、core tool call model、tool pipeline | 能明确区分完整 JSON、保守修复、不可信修复和不完整参数，阻止补齐式修复产生副作用 | 不应把部分成功渲染成 fatal error；提示语义是“已保留安全部分并继续恢复” |
| 截断批次工具最小化拦截 | stream_processor、round_executor、tool pipeline、前端 tool card | 防止半批工具执行造成不可逆副作用，同时保留只读观察结果 | read-only tool 不关联拦截；低风险本地幂等写操作满足条件可继续；高风险写操作延后并优先自动重新规划 |
| subagent / reviewer 输出 Phase 1 即落盘 | task_tool、coordinator、session storage、Deep Review UI | 避免 hidden subagent 清理后丢失完整 reviewer 证据；父 session 只吃摘要和引用，降低 overflow 风险 | 普通对话中 artifact 只在输出确实超大时使用；阈值不能过低，避免用户觉得内容被过早折叠 |
| `SubagentScheduler` + Gateway Request Limiter | coordinator、AI client boundary、provider adapter、Deep Review UI | 本地/弱并发 gateway 下 reviewer 会排队完成而不是 burst failure；用户能看到 waiting/retrying/running | Deep Review 可能更慢；UI 必须解释为受控排队执行，而不是系统卡住或审核变差；已实现 Strategy Engine 的并发策略只能作为上限 |
| Deep Review 已实现状态映射到统一 scheduler event | DeepReviewService、ReviewJudge 输入、前端 review UI、日志 | 避免 `partial_timeout` / retry / timeout 与通用状态重复解释；Judge 只处理一套状态 | 若映射错误，会把可用 partial evidence 渲染成失败，或把无 evidence timeout 渲染成部分成功 |
| Review Evidence Pack | Deep Review source resolver、source provider、session artifact、Work Packet、scheduler timeout | 避免每个 reviewer 重复重建完整 source evidence，让 timeout 留给分析而不是取证 | 大 source 任务启动前会多一个“准备审核证据”阶段；长耗时必须显示进度，pack stale 必须刷新或确认 |
| OverBudget 从软提示改为本地恢复优先 | execution_engine、Deep Review 分片、前端恢复提示 | 不把“已经超预算”的问题继续交给弱模型自救，降低重复截断概率 | 低置信超限不得阻断；先压缩/摘要/spill 并重估；只有高置信超限且恢复失败才引导用户缩小范围 |
| 共享上下文计入每个 reviewer 预算 | deep_review_policy、reviewTeamService、DeepReviewService | 避免系统性低估 reviewer 输入 token | 拆分后的 reviewer 数量或摘要需求可能增加，需要和并发/成本上限一起调参 |
| 大文件写入协议 | tool layer、session storage、filesystem service、前端进度展示 | 避免 1k+ 行文件内容通过普通 tool call JSON 撑爆上下文；写入可恢复、可校验、可原子提交 | 用户会看到文件生成进度而不是长时间流式文本；必须避免进度过碎或把小文件也强行协议化 |

这些变动不会改变仓库的平台边界：判断和治理仍在 core / ai-adapters / transport 层完成，前端只消费结构化事件和本地化文案。

---

### 普通对话的用户负面感知复核

以下复核覆盖 Deep Review 以外的正常 Flow Chat 场景。原则上，新增机制只能把原本会突然断掉、静默失败或不可恢复的场景变成可恢复体验；不能让原本可完成的请求更容易被用户感知为中止。

| 场景 | 潜在负面感知 | 方案约束 |
|---|---|---|
| 预算预检 | 用户刚发送消息就被要求缩小范围，感觉比旧逻辑更早失败 | 只有 `HardOverBudget` 且本地恢复失败才阻断；`SoftOverBudget` 走后台压缩/摘要，不直接打断 |
| 频繁压缩 | 对话中反复出现“整理上下文”，感觉系统卡顿 | 加压缩冷却窗口、收益阈值和复用摘要；连续压缩无效时转为拆分任务，不反复压缩 |
| token 估算误差 | 本来可能成功的请求被误判为超预算 | 估算只作为治理触发信号，不作为低置信阻断依据；记录估算与实际差异用于校准 |
| 工具批次截断 | 只读观察结果被误拦截，用户看到不必要失败 | read-only tool 不关联拦截，完整只读结果继续返回给模型和用户 |
| 写操作延后 | 用户觉得“模型明明已经决定改文件，却没有动” | 使用工具 mutability 元数据做最小拦截；低风险本地幂等写操作可继续，高风险写操作延后后优先自动重新规划 |
| 错误文案 | 截断提示像 fatal error，削弱信任 | 文案强调模型/provider/预算限制、有效结果已保留、BitFun 正在恢复或引导继续 |
| 后台长耗时治理 | 用户看到长时间无输出，以为卡死 | 短耗时后台静默；长耗时显示进度状态；有损删除提供诊断入口 |
| subagent 排队执行 | 本地模型下 Deep Review 变慢，用户误以为 reviewer 没启动 | UI 明确区分 queued / waiting_for_capacity / running / retry_waiting，并显示 configured/effective concurrency |
| artifact / spill | 普通输出被过早折叠，阅读成本上升 | 只对真正超大输出启用；普通规模仍直接展示；摘要和引用必须同时存在 |
| 大文件写入协议 | 用户期待模型直接吐完整文件，但看到分段写入/校验状态 | 只在大文件或高风险上下文触发；进度状态要聚合为 section 级别，完成后提供文件路径、摘要和验证结果 |

若某项机制在普通对话中增加了用户感知中止、等待不透明、或不必要确认次数，则该机制不能进入默认路径，只能作为诊断或显式高级选项。

---

### 高风险项实施硬门槛

以下项目只有满足硬门槛才进入实现；否则必须从实施计划中移除，并在 UI / 日志中保留现有保守行为。

| 项 | 必须满足的解决措施 | 不满足时的处理 |
|---|---|---|
| R0-3 JSON 修复生成语义错误参数 | `ToolArgumentStatus` 贯穿 accumulator、core tool call model、tool pipeline；pipeline 在执行前强制拒绝 `RepairedUntrusted` / `Incomplete` | 不实现补齐式 JSON 修复；只保留现有删除单个多余右花括号的保守修复，并保持 parse 失败为 `is_error=true` |
| R1 模型自主重试 doom loop | session / turn 层有连续截断计数器；超过阈值强制结束 turn，并给用户明确恢复建议 | 不实现模型自主“继续尝试”提示；截断后只完成 partial recovery，不鼓励模型继续同路径 |
| R2 subagent 返回 overflow / 证据丢失 | 完整 subagent 输出先落盘到父 session artifact；父 session 只注入摘要、统计、引用；session 删除时能清理 artifact | 不实现“只硬截断 subagent 返回”的 Phase 1 方案，避免 hidden subagent 清理后永久丢失 reviewer 证据 |
| R3 多 tool call 部分截断 | tool pipeline 能识别截断批次和 tool mutability；read-only tool 不关联拦截；mutating tool 按幂等/破坏性/外部副作用分级 | 不实现“一刀切拦截完整工具”或“完整者全部正常执行”的策略；缺少 mutability 元数据时按高风险保守处理 |
| R15 预算预检导致提前阻断 | 预检具备 `HardOverBudget` / `SoftOverBudget` 分层、本地恢复动作、压缩节流、估算校准；低置信不阻断 | 不把预算预检接入默认阻断路径，只记录诊断或后台轻量治理 |
| R17 emergency truncation / context mutation | context mutation 事件记录 tokens before/after、删除范围、summary source、耗时，并按短耗时/长耗时/有损删除决定展示 | 不扩大自动上下文删除能力；`ContextCompacted` 不进入 partial recovery；长耗时压缩不能静默造成长时间无输出 |
| R19-R22 大文件写入协议 | 具备 manifest、chunk sequence/hash 校验、temp file、finish 原子提交、abort/continue 恢复和 artifact 清理 | 不把大文件写入协议接入默认写文件路径；仍使用小文件直接写入和 patch/hunk 修改 |
| R23-R31 subagent / Deep Review 调度与取证复用 | 具备 retry classifier、gateway-keyed permit、adaptive 回滚、write-capable retry 安全检查、parent cancellation cleanup、Deep Review Strategy Engine 兼容映射、Review Evidence Pack 去重 | 不启用自适应并发和自动重试；不启用 pack enforcement；Deep Review 只保留已实现 policy，上层仅记录 queue / status / 重复取证诊断 |
| 重复实现与 owner 漂移 | 新增模块前必须完成“唯一 owner”检查：相似状态、重试、artifact、manifest、budget policy、UI 状态只能有一个行为 owner；其它层只能 projection / adapter / event mapping | 不新增平行模块；若现有 owner 不足，只能先提交迁移方案和回滚路径，不进入默认实现 |

---

### 风险处置汇总

| # | 风险 | 等级 | 处置 | 阶段 |
|---|---|---|---|---|
| R1 | 模型自主重试 doom loop | 高 | 连续截断计数器，超阈值强制终止 turn | Phase 1 |
| R2 | subagent 截断致父 session overflow / 证据丢失 | 高 | Phase 1 必须先落盘完整 subagent 输出，父 session 只注入摘要和引用；否则不实现硬截断 | Phase 1 |
| R3 | 多 tool call 部分截断 | 中 | 完整性仍记录；read-only tool 不关联拦截；mutating tool 按幂等/破坏性/外部副作用分级做最小拦截 | Phase 1 |
| R4 | stream error 携带截断信号 | 中 | Error 分支增加截断 kind 判断，补充 RateLimited variant | Phase 1 |
| R5 | 纯文本截断无 tool call | 中 | End 分支增加纯文本 + max_tokens 检测 | Phase 1 |
| R6 | has_effective_output 定义模糊 | 中 | 明确定义：有非空文本或有非 error tool call | Phase 1 |
| R7 | 前后端版本兼容 | 低 | Option<T> + optional 字段，JSON 容忍 | Phase 1 |
| R8 | JSON 完整性检测性能与误判 | 中 | 不采用“只看括号计数不 re-parse”；parse 是最终完整性校验 | Phase 1 |
| R9 | token 估算准确性 | 低 | 启发式 chars*0.6 + 1.2x 安全系数 + 日志校准 | Phase 2 |
| R10 | reviewer 交叉依赖 | 中 | 保留共享上下文，但必须计入每个 reviewer 预算；超预算改为接口/类型摘要 | Phase 2 |
| R11 | spill 文件磁盘累积 | 低 | session 清理 + 单 session/全局大小上限 + LRU 淘汰 | Phase 3 |
| R12 | spill 文件含敏感信息 | 中 | 文件权限 + 敏感模式检测 + 摘要 REDACTED + UI 警告 | Phase 3 |
| R13 | 摘要质量不足 | 低 | 结构化摘要 + 启发式摘要（编译错误/test输出等已知格式），不使用 LLM 语义摘要 | Phase 3 |
| R14 | 预算随模型切换变化 | 低 | ModelBudgetProfile 从配置加载，切换时重算 | Phase 2 |
| R15 | 预算耗尽前无预警 | 低 | 请求前预算预检；SoftOverBudget 后台恢复优先；HardOverBudget 也必须先压缩/摘要/spill 并重估，仍超限才引导用户缩小范围 | Phase 2 |
| R16 | 重试耗尽后行为未定义 | 中 | finish with partial recovery，保留有效输出 | Phase 1 |
| R17 | context compaction / emergency truncation 截断历史 | 中 | 当前成立；使用独立 `ContextMutationKind` 事件和用户可见诊断，不放入 `PartialRecoveryKind` | Phase 1 |
| R18 | i18n key 和英文文案缺失 | 低 | 定义 key 结构 + 中英文文案 | Phase 1 |
| R19 | 大文件 chunk 顺序错乱或内容缺失 | 中 | write manifest + sequence/hash 校验 + finish 完整性校验 | Phase 4 |
| R20 | 半成品文件被误认为成功 | 高 | temp file 写入 + finish 原子提交 + open write session 事件 | Phase 4 |
| R21 | 后续修改读不到完整上下文 | 中 | manifest 记录 section/range，后续修改按需读取 range/symbol | Phase 4 |
| R22 | 大文件 artifact 泄漏或磁盘累积 | 中 | 复用 spill 权限/敏感检测/清理/LRU，finish/abort 后清理 chunk | Phase 4 |
| R23 | gateway overload 误分类 | 中 | 不可重试错误优先分类；Unknown 不默认重试；retry classifier fixture | Phase 2 |
| R24 | adaptive concurrency 振荡 | 中 | 快降慢升、按 gateway key 独立计数、振荡时回退静态 cap | Phase 2 |
| R25 | queue timeout 掩盖真实 provider 故障 | 中 | queue timeout 与 provider error 分离；auth/quota/model 错误立即提示诊断 | Phase 2 |
| R26 | write-capable subagent 重试产生副作用 | 高 | 读取 tool mutability history；无法证明无状态变更则不自动重试 | Phase 2 |
| R27 | 父 session 取消后 queued subagent 未清理 | 中 | parent cancellation token 绑定 scheduler state；取消时释放 permit 和清理 artifact | Phase 1 / Phase 2 |
| R28 | Deep Review Strategy Engine 与 runtime scheduler 双调度 | 高 | Deep Review policy 只输出上限和约束；`SubagentScheduler` 是唯一执行调度器 | Phase 2C |
| R29 | Deep Review retry budget 与 scheduler retry classifier 双重试 | 高 | retry budget 只定义上限；实际 retry 必须由 scheduler classifier 判定并记录 owner | Phase 2C |
| R30 | `partial_timeout` 与通用 partial recovery 状态冲突 | 中 | `partial_timeout` 按 evidence 映射为 `completed_with_partial` 或 `timed_out`，raw status 仅诊断 | Phase 1 / Phase 2C |
| R31 | Deep Review 大 source 重复取证导致 reviewer 超时 | 中 | 父任务 preflight 一次生成 source-agnostic `ReviewEvidencePack`，reviewer 只消费 artifact slice；重复完整 evidence 重建进入诊断/门禁 | Phase 2C |

---

## 验收标准

### Phase 0 验收

1. 已建立 stream fixture，覆盖正常 stop、provider max_tokens、无 finish reason、stream error、watchdog timeout。
2. 已建立 tool argument fixture，覆盖完整 JSON、半截 JSON、保守修复、不可置信补齐。
3. 已建立 budget fixture，覆盖 CJK、代码、大 diff、长日志和不同 model profile。
4. 已建立 artifact / large write fixture，覆盖超大 subagent result、敏感输出、1k+ 行新文件和 chunk 异常。
5. 已建立 scheduler fixture，覆盖 gateway concurrency=2、4-5 reviewers burst、retryable overload、non-retryable auth/quota/model errors。
6. 已建立 Deep Review compatibility fixture，覆盖已实现 Strategy Engine 的 `max_parallel_instances`、predictive timeout、`partial_timeout`、retry budget 与 runtime scheduler 的映射关系。
7. 已建立 Review Evidence Pack fixture，覆盖 local git range、PR URL、working tree、patch artifact、rename、pathspec、pack stale 和多 reviewer 共享取证。
8. `ai.subagent_max_concurrency` 和 Deep Review `max_parallel_instances` 在设置和 manifest 中被描述为 runtime bounded scheduling 的上限，而不是无条件并行承诺。
9. 旧逻辑在关键 fixture 上能稳定复现静默截断、父 session overflow、gateway burst failure、双调度风险、重复全量 diff 或大文件写入失败，作为后续回归基线。
10. 所有后续阶段新增行为都有独立 feature flag 和 observe-only 开关。

### Phase 1 验收

1. stream end 时如果存在未完成工具参数，系统标记结构化恢复类型，而不是静默完成。
2. 工具参数截断不会触发同请求自动重试。
3. 网络类无有效输出错误仍保持可重试行为。
4. 半截工具调用不会自动执行。
5. 前端 active session 消息流末尾显示明确截断提示。
6. 前端事件类型显式包含 recovery kind / reason。
7. 日志使用英文，无 emoji，包含 session_id、round_id 和不泄露全文参数的 summary。
8. **连续截断超过 3 次时强制终止 turn 并提示用户**（R1）。
9. **Task tool 返回前检查结果大小；超限时完整输出先落盘到父 session artifact，父 session 只注入摘要和引用**（R2）。
10. **多 tool call 并行时，不完整者标记为 `Incomplete`；read-only tool 不关联拦截；mutating tool 依赖 mutability 元数据做最小拦截**（R3）。
11. **stream error 分支能识别 WatchdogTimeout / StreamInterrupted / RateLimited**（R4）。
12. **纯文本截断能被检测并提示**（R5）。
13. **`has_effective_output` 有明确定义和单测**（R6）。
14. **重试耗尽时 finish with partial recovery，保留有效输出**（R16）。
15. **i18n key 和中英文文案已定义**（R18）。
16. **每个 ai-adapter 将 provider 原始 finish reason 归一化为 `ProviderFinishReason` 枚举**（R0-2）。
17. **`ProviderFinishReason::Unknown` 不触发截断检测，保留原始值用于诊断**（R0-2）。
18. **`PendingToolCalls` 使用 parse 作为 JSON 完整性最终校验；简单括号计数只能作为 early hint，不能单独决定可执行性**（R8）。
19. **subagent 截断的 recovery kind 上浮到父 session 事件流**（R2）。
20. **microcompact / compression / emergency truncation 通过独立 context mutation 事件记录，不进入 `PartialRecoveryKind`**（R17）。
21. **subagent scheduler state event 能表达 accepted / queued / waiting_for_capacity / running / retry_waiting / completed / completed_with_partial / failed / cancelled / timed_out**。
22. **父 session 取消会取消 queued / waiting_for_capacity / retry_waiting subagent，并释放 running subagent 的 gateway permit**（R27）。

### Phase 2 验收

1. Deep Review 分片策略支持 token/byte/diff line 预算。
2. 单个超大文件或超大 diff 会被识别并隔离处理。
3. 子 reviewer 截断能在父任务中可见。
4. **token 估算使用启发式 + 安全系数，日志记录估算值**（R9）。
5. **拆分时保留共享上下文文件，但共享上下文计入每个 reviewer group 预算；超预算使用接口/类型摘要**（R10）。
6. **模型切换时预算自动重算**（R14）。
7. **请求前预算预检；SoftOverBudget 不阻断；HardOverBudget 先执行压缩/摘要/spill 并重估，仍超预算才不发送同一大请求**（R15）。
8. **预算治理记录压缩耗时、tokens before/after、估算与实际差异；连续压缩必须有冷却窗口和收益阈值**（R15 / R17）。
9. **普通对话的默认路径不得因低置信预算估算新增用户可感知中止**（产品体验约束）。
10. **gateway-keyed AI request limiter 在 stream finish / error / cancel 时都释放 permit**。
11. **retry classifier 能区分 retryable overload 和 non-retryable auth/quota/model-not-found**（R23 / R25）。
12. **adaptive effective concurrency 遇 overload 快速降低，成功窗口后缓慢升高，且不低于 1**（R24）。
13. **write-capable subagent 只有在 tool history 证明无 state-changing tool 后才可自动重试**（R26）。
14. **Deep Review 在 gateway concurrency=2 时可通过排队完成 4-5 reviewers，而不是 burst failure**。
15. **ReviewJudge 输入包含 queued / retried / timed_out / failed / completed_with_partial 状态和 partial evidence**。
16. **Deep Review 已实现 `partial_timeout` 状态按 evidence 映射为 `completed_with_partial` 或 `timed_out`，UI / Judge 不直接解析 raw status**（R30）。
17. **Deep Review retry budget 不会绕过 scheduler retry classifier；同一 reviewer 不会被 orchestrator 和 scheduler 双重 retry**（R29）。
18. **Deep Review dynamic concurrency policy 不会绕过 `SubagentScheduler`；同一 reviewer 不会同时进入两套 queue**（R28）。
19. **大 source Deep Review 由父任务生成 source-agnostic `ReviewEvidencePack`；4-5 个 reviewer 不重复重建完整 source evidence；pack stale 会刷新或提示确认**（R31）。

### Phase 3 验收

1. 大工具输出不会直接塞爆上下文。
2. 超限输出可落盘，UI 能打开完整内容。
3. 上下文中注入的是摘要和引用，而不是不可控全文。
4. **spill 目录有大小上限和 LRU 淘汰**（R11）。
5. **spill 文件有正确权限，敏感内容在摘要中 REDACTED**（R12）。
6. **启发式摘要能提取编译错误文件列表、test pass/fail 计数、diff stat**（R13）。

### Phase 4 验收

1. 模型尝试写入超过阈值的大文件全文时，不通过普通 tool call JSON 直接传输完整内容。
2. `start_file_write` / `append_file_chunk` / `finish_file_write` / `abort_file_write` 有完整状态机和单元测试。
3. chunk sequence 错乱、重复、hash mismatch 时不会提交目标文件。
4. 未完成 write session 在 turn 结束时产生 `FileWriteIncomplete` 事件，UI 不显示成功。
5. `finish_file_write` 成功后才原子替换目标文件，并记录最终 hash、bytes、section manifest。
6. 上下文只注入 manifest、摘要、hash、artifact ref 和必要 preview，不注入完整文件内容。
7. 后续修改大文件时优先 patch/hunk，并能按 range/symbol 读取相关内容。
8. 大文件写入进度以 section 级别展示，避免长时间无输出，也避免 chunk 级别噪音。
9. large write artifact 复用 spill 的权限、敏感检测、session 清理和 LRU 策略。

### 跨阶段门禁

1. 每个阶段必须提供最小回滚开关，回滚后不删除用户已有输出、artifact 或写入 manifest。
2. 每个阶段必须有一组 deterministic fixture，不能只依赖真实 provider 手动复现。
3. 每个阶段必须记录可观测字段，能区分“检测到问题”“执行了治理动作”“用户可见提示”三类事件。
4. 每个阶段必须证明普通对话默认路径没有因为低置信估算、过度压缩、过重提示或不必要确认而增加用户感知中止。
5. 任一阶段如果达不到门禁，只能停留在 observe-only 或诊断能力，不进入默认行为。

---

## 建议验证命令

根据仓库规则，相关改动最少应覆盖：

- 前端改动：`pnpm run lint:web && pnpm run type-check:web && pnpm --dir src/web-ui run test:run`
- core / ai-adapters 改动：`cargo check --workspace && cargo test --workspace`
- Deep Review 行为改动：`cargo test -p bitfun-core deep_review -- --nocapture`
- subagent scheduler / gateway limiter 改动：scheduler state transition、permit acquire/release、retry classifier、gateway concurrency=2 Deep Review fixture

执行前可根据改动范围选择最小验证集。若本设计仅修改文档，不需要运行上述命令。

---

## 最终合理性与先进性复核

本轮复核的标准不是“能不能做”，而是：在 BitFun 当前架构、已有能力、用户体验约束和竞品方向下，是否是当前阶段的最佳路径。结论如下：

| 方案项 | 判定 | 为什么是当前最佳路径 | 被否定的次优路径 |
|---|---|---|---|
| `PartialRecoveryKind` + `ProviderFinishReason` | 保留 | 结构化状态是跨后端重试、前端提示、日志诊断的最小稳定契约；provider 差异留在 adapter 层，符合平台边界 | 继续依赖 `reason.contains(...)` 或 provider 原始字符串 |
| `ToolArgumentStatus` | 保留 | 工具参数完整性是执行安全边界，必须显式区分完整、保守修复、不可信修复和不完整；只靠 `is_error` 无法表达可信度 | 自动补齐 JSON 后执行，或把所有修复都当成 error 丢弃 |
| 截断批次的最小化工具拦截 | 保留 | read-only 工具不关联拦截可以保持流畅；mutating 工具按幂等/破坏性/未知分级，兼顾安全和体验 | 截断批次全部失败，或完整工具全部执行 |
| `RateLimited` 分层处理 | 修正后保留 | 顶层 round 不即时重复请求，避免重复输出；scheduler-owned subagent 在无有效输出时可 backoff 重试，符合 gateway overload 场景 | 一概不重试，或一概自动重试 |
| `ContextMutationKind` 独立建模 | 保留 | context compression / microcompact / emergency truncation 是输入侧治理，不是输出 partial recovery；独立事件能解释“为什么模型记忆变化” | 把 `ContextCompacted` 塞进 `PartialRecoveryKind` |
| 与 `context-reliability-architecture.md` 保持双文档边界 | 保留 | Context Reliability Architecture 是信任、证据、压缩契约和 context profile 的基础设施；本文是截断、预算、scheduler、artifact 和大文件写入的运行时控制面，双文档边界比全文合并更可维护 | 强行合并成一篇总纲，或在两篇文档中重复定义同一类事件 |
| 与 `deep-review-design.md` 已实现基线保持增量关系 | 保留 | Deep Review Strategy Engine 已解决角色、prompt、timeout、partial、retry、concurrency policy 等专项问题；本文只补 runtime scheduler、gateway limiter、统一事件、artifact 和预算，避免重复建设 | 在本文重新实现 Deep Review batching、retry 或 partial capture |
| Deep Review token/byte/diff line 预算 | 保留 | 文件数只能控制 reviewer 数量，不能控制上下文；预算拆分是减少大 diff 截断的必要条件 | 继续只按文件数拆分 |
| Review Evidence Pack | 新增保留 | PR URL、最近提交、working tree、patch artifact 或超大 diff 的取证是所有 reviewer 的共享输入，应由父任务通过 source provider 一次生成并落盘复用；这能把 reviewer timeout 留给分析，而不是重复拉取/解析证据 | 每个 reviewer 自行重复重建完整 source evidence，或只靠 prompt 要求 reviewer 少跑命令 |
| `SubagentScheduler` + gateway limiter | 保留 | 这是从 Deep Review policy 并发上限升级到 runtime-bounded scheduling 的关键；能让低并发 vLLM gateway 排队成功而不是 burst failure | 只调低 `ai.subagent_max_concurrency`，只靠 Deep Review batching，或让 prompt 自己少并发 |
| adaptive effective concurrency | 保留但必须可回滚 | 本地和云 gateway 能力差异大，静态配置不能覆盖；快降慢升、按 gateway key 隔离、最低 1 是当前最佳控制面 | 全局固定并发，或无边界自适应 |
| subagent retry classifier | 保留 | read-only reviewer、write-capable worker 和 Deep Review retry budget 风险不同，必须按错误类别、有效输出、tool mutability 决定是否执行 retry | 所有 subagent 统一重试策略，或 Deep Review orchestrator 与 scheduler 双重 retry |
| Deep Review partial 状态映射 | 保留 | 已实现 `partial_timeout` 是 Deep Review 内部状态；统一映射为 `completed_with_partial` / `timed_out` 才能让 UI、Judge、ledger 共用一套契约 | 前端/Judge 同时解析 raw `partial_timeout` 和通用 scheduler state |
| subagent / reviewer 输出专项落盘 | 保留 | hidden subagent 成功后会清理子 session，Phase 1 若不先落盘就会永久丢 evidence；这是比通用 spill 更早的必要能力 | 只硬截断返回，或等 Phase 3 通用 spill 后再处理 |
| `OverBudget` 本地恢复优先 | 修正后保留 | 低置信估算只触发后台治理；高置信超限也先压缩/摘要/spill 并重估，符合“不新增用户感知中止”原则 | 固定阈值直接阻断，或只在 prompt 里软提醒模型自救 |
| spill-to-file + 结构化/启发式摘要 | 保留 | 大输出不应回灌全文；启发式摘要覆盖 test/log/diff 等高频场景且不引入额外 LLM 调用 | LLM 语义摘要，或只截断不留完整引用 |
| Large File Write Protocol | 保留 | 1k+ 行文件写入不应走普通对话文本或单个巨大 tool JSON；事务化 chunk + manifest + 原子提交是源头预防 | 继续让模型一次性生成完整文件内容，或只靠上下文压缩兜底 |
| 分阶段 observe-only / feature flag / fixture 体系 | 保留 | 方案跨 core、adapter、frontend、storage，必须先观测再干预；没有 fixture 和回滚会让“治理能力”本身变成新风险 | 一次性大改上线，或只靠手工复现 |

先进性判断：

1. 与 Codex 类产品的方向一致：并行任务需要隔离执行、实时进度和可验证证据，而不是只返回最终答案。
2. 与 Claude Code / OpenCode 的方向一致：subagent 应有独立上下文、工具权限、模式/permission profile，并对 read-only 与 write-capable 工作采用不同安全策略。
3. 相比“简单可行”的上下文压缩或降低并发，本方案更接近 runtime control plane：结构化状态、队列、gateway permit、artifact、预算、事务写入、UI 可观测性彼此闭环。
4. 当前不把 persisted scheduler state、LLM 语义摘要、全自动写型 subagent 重试放进近期默认范围，是合理克制；这些方向有价值，但不是当前风险/收益最优点。

最终修正结论：原方案整体合理且具备先进性；本轮仅将 `RateLimited` 和预算阈值从简单规则修正为分层策略，避免它们退化成“可行但不最佳”的实现。

---

## 最终判断

本设计运行在 `context-reliability-architecture.md` 定义的 Context Reliability Architecture 之上。上游架构提供信任边界、Evidence Ledger、Compaction Contract、Context Health 和 Work Packet；本文将原先的“截断检测与提示”扩展为六层运行时治理：

1. **止血层**：检测工具参数截断，阻止静默成功和无意义重试；
2. **隔离层**：用 `ToolArgumentStatus`、tool mutability 元数据和最小化拦截策略阻止不可信工具调用自动执行；
3. **证据层**：subagent / reviewer 输出先落盘，再向父 session 注入摘要和引用；
4. **调度层**：用 `SubagentScheduler` 和 gateway request limiter 将 subagent / reviewer 从 burst launch 改为 runtime-bounded scheduling；
5. **治理层**：用预算估算、本地上下文恢复、分片、context mutation 事件和 spill-to-file 降低截断发生率；
6. **预防层**：用大文件写入协议避免 1k+ 行文件内容进入普通对话文本或单次 tool call JSON。

风险处置结论：

- **可根治或可硬约束规避的**（R0-1, R0-3, R0-4, R0-5, R0-6, R0-7, R1-R16, R18-R27）：通过结构化类型、执行前拒绝、计数器、大小检查、artifact 落盘、runtime queue、gateway permit、事务化写入等机制解决；
- **可缓解但不可根治的**（R0-2）：provider 行为不一致是外部因素，通过归一化层 + `Unknown` 兜底确保不误判；
- **需单独建模的**（R17）：上下文压缩 / 裁剪不是输出流 partial recovery，必须通过独立 context mutation 事件和诊断展示治理。

产品结论：方案不接受“用更多用户可感知中止换取更早失败”。新增动作必须减少静默失败、突然断掉、不可恢复和不透明等待；如果在普通对话中增加低置信阻断、反复压缩、过重错误提示或不必要确认，则该动作不能进入默认路径。

以下修改若无法满足硬门槛，则不得实现：补齐式 JSON 修复后自动执行、截断批次内 mutating tool 无分级自动执行或一刀切拦截、低置信 OverBudget 直接阻断、只硬截断 subagent 返回而不落盘、无 retry classifier 的 subagent 自动重试、write-capable subagent 状态变更后自动重试、无事务/无校验的大文件 chunk 写入、把 `ContextCompacted` 放进 `PartialRecoveryKind`、为已有 owner 可覆盖的相似能力新增平行实现、允许多个 Deep Review reviewer 对同一 source fingerprint 重复重建完整 evidence。这些约束优先级高于阶段计划。

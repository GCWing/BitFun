# Deep Review 已完成状态

## 目的

本文档把当前 Deep Review 相关设计和阶段文档中已经完成的内容合并为一份独立状态说明。它只记录已经实现或带防护边界实现的行为，把后续计划和延期事项放到 `docs/deep-review-pending-plan.zh-CN.md` 中，避免后续再从多份阶段文档中拼接状态。

## 来源文档

本次合并覆盖以下 Deep Review 文档和本地伴随设计副本：

| 来源 | 在本合并文档中的作用 |
|---|---|
| `docs/deep-review-design.md` | 原始策略引擎、Architecture Reviewer、Frontend Reviewer、Prompt 职责边界和当前实现状态。 |
| `docs/deep-review-phase2-plan.md` | Phase 2 中策略、并发、重试、缓存、Token 预算和报告可靠性状态。 |
| `docs/deep-review-phase2-addendum.md` | 事实模型、状态措辞、风险登记、已完成轮次和延期边界。 |
| `docs/deep-review-phase3-followup-plan.md` | 最新产品决策，以及 Phase 3 中诊断、设置、provider queue、retry controls、cost-aware scope 与 evidence pack 的实现边界。 |
| `docs/deep-review-architecture-refactor-plan.md` | 架构重构目标和模块边界；该文档中的重构仍属于待完成事项。 |
| `docs/deep-review-nondeepreview-impact-inventory.md` | 已记录的共享运行时影响规则，后续工作仍必须遵守。 |
| `docs/superpowers/plans/2026-05-09-deep-review-phase3-execution-plan.md` | Phase 3 的轮次执行状态和验证历史。 |
| `docs/superpowers/specs/deep-review-design.md`、`docs/superpowers/plans/deep-review-phase2-plan.md` | 本地伴随副本，已检查无新的不同边界。 |

## 状态措辞

| 措辞 | 含义 |
|---|---|
| 已实现 | 运行时代码中已有确定行为，并有聚焦验证。 |
| 已实现并带防护 | 行为已经存在，但被设置、预算、用户控制或可信元数据明确约束。 |
| 安全网 | 运行时能阻止或报告不安全状态，但还不是完整的顺滑产品体验。 |
| Prompt 引导 | Manifest 或 Prompt 要求 orchestrator 执行，但弱模型仍可能遗漏或顺序错误。 |
| 产品决策延期 | 需要隐私、保留、删除、UX 或产品规则确认后才能实现。 |
| 待实现 | 设计已明确，但代码尚未落地。 |

本文只列出已实现或已实现并带防护的内容。待实现和延期内容见 `docs/deep-review-pending-plan.zh-CN.md`。

## 当前运行时形态

Deep Review 仍是 Prompt 驱动的 5 阶段 orchestrator：

1. 识别审核范围。
2. 并行启动 specialist reviewer。
3. 顺序执行 judge/quality gate。
4. 通过 `submit_code_review` 合成最终报告。
5. 可选通过普通编辑工具进行修复。

当前审核团队包括：

| Reviewer | 运行时状态 | 范围 |
|---|---|---|
| `ReviewBusinessLogic` | 始终启用 | 正确性、业务规则、数据和状态迁移。 |
| `ReviewPerformance` | 始终启用 | 运行时热点路径、昂贵计算、payload 成本。 |
| `ReviewSecurity` | 始终启用 | 可利用的信任边界、认证授权、数据处理和安全风险。 |
| `ReviewArchitecture` | 始终启用 | 分层边界、依赖方向、API 契约形态、可维护性。 |
| `ReviewFrontend` | 条件启用 | React、UI 状态、i18n、可访问性、前后端契约漂移和前端平台边界。 |
| `ReviewJudge` | 顺序执行 | 去重、处理重叠、验证证据、合成报告。 |
| 自定义 review agent | 可选 | 必须满足最小审核工具契约。 |

Deep Review 仍保持 source-agnostic。Git 变更、本地 workspace 变更或未来来源都应通过 target manifest 描述，而不是绑定为 Git-only 抽象。

## 已完成 Reviewer 与 Prompt 工作

### 核心角色扩展

- `ReviewArchitecture` 已作为始终启用的核心 reviewer 实现。
- `ReviewFrontend` 已作为前端聚焦 reviewer 实现，并支持条件启用。
- Architecture 和 Frontend reviewer 都已有独立 prompt。
- 既有 reviewer prompt 已收窄职责以减少重叠：
  - Business Logic 不再负责 UI 状态或层级边界分析。
  - Performance 不再负责 React render 优化。
  - Security 聚焦可利用风险，而不是一般结构边界。
- Judge prompt 已覆盖 Architecture/Business Logic、Architecture/Security、Frontend/Performance、Frontend/Business Logic 的重叠处理。
- DeepReview orchestrator prompt 包含角色级策略强化和 Frontend 策略指令。

### 角色元数据、可见性与国际化

- 后端提供的 reviewer definition 是前端团队解析和 review-agent 可见性的运行时来源。
- 前端 fallback metadata 仅作为降级安全网。
- Settings 和 Agents 页面已包含 Architecture、Frontend reviewer 的国际化名称。
- Agents 页面 Code Review Team 卡片已调整，避免 reviewer tag 裁切，并使用更紧凑的角色摘要。
- Hidden review-agent metadata 由静态非 review hidden id 与后端提供的 review-agent hidden id 动态合成。

## 已完成目标分类与条件派发

- `ReviewFrontend` 已从固定存在改为条件执行。
- 条件启用由 target/domain classification 和 reviewer applicability registry 驱动。
- 当前分类器支持 frontend UI、frontend style、frontend i18n、frontend contract、desktop contract、backend core、API layer、transport 等 domain tags。
- 兼容的 `hasFrontendFiles()` 仍从 frontend 相关 tag 派生。
- 同一 registry 可用于未来条件 reviewer。
- 自定义 review subagent 只有在有效且适用时才会进入 manifest；无效 agent 会以可解释方式呈现，而不是静默消失。

## 已完成自定义 Review-Agent 契约

- 最小有效 review-agent 工具集已集中定义为：
  - `GetFileDiff`
  - `Read`
- 缺少必需工具会报告为 `invalid_tooling`。
- 缺少 `Grep`、`Glob`、`LS` 等推荐调查工具时，只视为审核质量降级，不视为无效配置。
- UI 和运行时共用同一契约定义，避免创建/编辑 UI 与 Review Team 执行规则漂移。
- 无效或跳过的 reviewer 会进入 manifest/report metadata，用户能看到原因。

## 已完成策略与风险元数据

- 后端已有 `ChangeRiskFactors` 和 `auto_select_strategy()` 纯策略 helper。
- Launch manifest 会记录：
  - 前端推荐；
  - 后端兼容推荐；
  - 用户显式 override；
  - 最终策略；
  - mismatch 状态；
  - mismatch 严重度。
- 后端评分仅作为 advisory/mismatch-warning metadata。
- 后端评分不会覆盖用户选择策略、扩展 reviewer roster 或改变 Token/并发成本。
- `max_cyclomatic_complexity_delta` 仍明确为 `not_measured`；权威自动策略选择未实现。

## 已完成预测超时与部分结果捕获

- Launch manifest 会记录目标文件数和 diff 行数。
- Reviewer 和 judge 的有效超时由策略和目标大小派生。
- TaskTool 在启动 Deep Review reviewer subagent 时会遵守 manifest policy。
- `SubagentResultStatus::PartialTimeout` 已存在。
- 如果超时 subagent 在 grace period 内返回可用 final message，coordinator 可以保留该最终消息。
- 当前限制明确：不会在 grace window 之外重建任意 stream fragment。

## 已完成并发与队列基础

### 运行时执行

- `DeepReviewConcurrencyPolicy` 解析已实现。
- TaskTool 会针对本地 reviewer cap 饱和进行有界等待。
- 队列等待时间与 reviewer 执行时间分离。
- 本地 cap 等待超时可变为 `CapacitySkipped`。
- 本轮有效并发会在本地 capacity skip 和显式 provider transient-capacity reviewer failure 后降低。
- 成功 reviewer 观察可谨慎恢复有效 cap。
- Capacity skip 会进入最终报告可靠性信号。

### 用户可见队列控制

- 后端队列状态事件契约已存在。
- Flow Chat action bar 已有紧凑队列提示。
- 本地 cap 队列支持后端绑定的：
  - pause；
  - continue；
  - cancel；
  - optional-extra skip。
- 启动时已有 active-session concurrency warning，避免 Deep Review 静默争用繁忙用户 session。
- 恢复动作包括下次降低速度运行和打开 Review settings。

### 当前边界

- 当前队列自动化范围很窄，面向 Deep Review reviewer。
- 本地 cap 等待有界、可见、可暂停、可继续、可取消，并可跳过 optional-extra reviewer。
- 显式 provider transient-capacity reviewer failure 会进入短 provider queue，并在最终 `capacity_skipped` 前对同一 reviewer 做一次有界 reattempt。
- Provider queue/retry/success 计数和 provider reason counts 都是聚合 diagnostics。
- Provider queue time 与 reviewer runtime timeout 分离。
- 完整后端 batch/stagger scheduling 尚未实现；当前已增加窄范围 launch-batch 重叠保护：前一 reviewer batch 仍活跃时，后续 batch 会以 `launch_batch_blocked` 原因进入可见队列。
- 用户可见 effective-cap override controls 尚未实现。
- Deep Review queueing 不是全局 subagent queueing。

## 已完成重试防护

- Retry budget tracking 已存在。
- Reviewer timeout retry guidance 已存在。
- Retry guidance 会优先使用有效 manifest policy。
- TaskTool structured retry admission 已存在。
- Retry reviewer Task 必须包含结构化 coverage 并通过运行时检查：
  - `retry: true`；
  - source packet/status 信息；
  - 可重试 source status；
  - 缩小后的 retry scope；
  - 更低 timeout；
  - 可用 retry budget。
- 接受的 retry Task 会获得有界 retry-scope prompt block。
- 缺少 coverage、scope 未缩小、source status 不可重试、timeout 未降低、budget 耗尽都会被拒绝。
- 用户可见显式 retry action 已支持结构化 unresolved Deep Review slices。
- Manual retry 只允许 `partial_timeout` 或 transient `capacity_skipped`，并且必须有显式缩小的 retry scope。
- 有界 automatic retry admission 受 Review Team opt-in、结构化 coverage、缩小 scope、更低 timeout、retry budget 和 elapsed guard 保护。
- 有界 automatic retry 默认关闭。
- 后端自动 redispatch scheduling 仍延期；`auto_retry` 只是给后端所有的 retry caller 使用的准入路径，不是通用 scheduler。

## 已完成增量缓存边界

- Per-session `DeepReviewIncrementalCache` primitive 已存在。
- Session metadata 包含 cache field。
- 既有 persistence 会保留 cache field。
- TaskTool 可以通过解析出的 `packet_id` 命中同一 session 的缓存。
- `submit_code_review` 可以把已完成 reviewer 输出写回 per-session cache。
- 读写路径都按 work-packet `packet_id` 对齐。
- 报告可靠性信号可展示 cache hit/miss。
- 当前 cache 不具备独立于 session metadata 的保留周期。
- 删除或清理 session metadata 会移除该 cache。
- Project-level 或 cross-session cache 未实现。

## 已完成 Packet Metadata 与报告可靠性

- `submit_code_review` 已有 packet metadata fallback。
- 缺失 reviewer `packet_id` 时，能在可能情况下从 manifest 推断。
- 低置信度 fallback metadata 会被标记。
- 最终报告可靠性信号覆盖：
  - partial timeout；
  - retry guidance；
  - skipped reviewer；
  - capacity/concurrency limit；
  - cache hit/miss；
  - token-budget tradeoff。
- Report/export utilities 会摘要或折叠密集可靠性细节。
- 标准 Code Review 不应在没有 Deep Review context 时获得 Deep Review-only packet/cache/queue 信号。

## 已完成共享上下文测量

- Deep Review reviewer 的 `Read` 和 `GetFileDiff` 调用会按 parent turn、reviewer type、tool name、normalized path、call count、reviewer count 测量。
- 测量不包含内容。
- 测量不存储源码、diff、tool output、model output 或 provider raw body。
- 最终 Deep Review submission 可输出一次 aggregate debug diagnostics。
- 报告中不包含原始 shared-context diagnostics。
- 跨 subagent 的 programmatic tool-result reuse 未实现。

## 已完成 Token 与上下文预算防护

- Launch manifest 包含按模式配置的 heuristic prompt-byte threshold。
- Manifest 包含 estimated max reviewer prompt bytes。
- Summary-first full-scope metadata 已存在。
- File split 和 max-file 风格 guardrail 已存在。
- Summary-first 行为仍保持每个 assigned file 可见，不能静默从 coverage metadata 中隐藏文件。
- Hard prompt-byte clipping 和 byte-accurate enforcement 仍延期。

## 已完成 Cost-Aware Scope 与 Shared Evidence Pack

- Launch manifest 现在包含 `DeepReviewScopeProfile`。
- `quick` 映射为 `high_risk_only`，只做高风险优先覆盖。
- `normal` 映射为 `risk_expanded`，覆盖变更文件和一跳高风险上下文。
- `deep` 映射为 `full_depth`，保持完整深度意图。
- Reduced-depth run 会在报告可靠性信号和导出中标记 `reduced_scope`，不得声称 full-depth coverage。
- Changed-file coverage metadata 不会因为 reduced-depth 而被移除。
- Launch manifest 现在包含 metadata-first `DeepReviewEvidencePack` v1。
- Evidence pack 包含 version、source label、changed file metadata、diff stats、domain/risk tags、packet ids、hunk hints、contract hints、budget 和 privacy boundary。
- Evidence pack 明确为 `metadata_only`，不包含 source text、full diff、model output、provider raw body 或 full file contents。
- Reviewer 和 judge prompt 都要求把 hunk/contract hints 当作 orientation，报告 finding 前必须通过 `GetFileDiff`、`Read`、`Grep` 或 read-only `Git` 确认。
- Rust manifest parser 会校验 evidence pack version、source、size limits、privacy boundary 和 forbidden content keys。
- Runtime diagnostics 新增 content-free duplicate discovery savings candidate count，用于后续观察 evidence pack 是否减少重复发现成本。
- Report/export 只展示 evidence pack 的聚合计数、source 和 privacy boundary，不展开内容型 payload。
- Programmatic full tool-result cache 仍未实现。

## 已完成 Consent、Recovery 与 Settings UX

### 首次运行与启动体验

- Deep Review consent dialog 已包含紧凑 pre-review summary：
  - 文件数；
  - 风险区域；
  - 选中策略；
  - optional reviewer 数；
  - summary-first 状态；
  - 有跳过 reviewer 时显示提醒。
- 弹框信息已精简为关键提醒。
- 用户可见文案已国际化。
- 密集 lineup/cost card 仍延期。

### Action Bar 与恢复

- Deep Review action bar 已支持 interruption/recovery 状态。
- 手动取消会保留 parent summary，而不是把所有 stop 都当作完整审核丢失。
- 可恢复时 continue/resume controls 保持可见。
- Diagnostics copy actions 会保留原始诊断元数据，同时保持用户文案国际化。

### Review Capacity 与 Retry Settings

- Default Review Team config 存储：
  - `max_parallel_reviewers`；
  - `max_queue_wait_seconds`；
  - `allow_provider_capacity_queue`；
  - `allow_bounded_auto_retry`；
  - `auto_retry_elapsed_guard_seconds`。
- 默认值保持保守：
  - 4 个并行 reviewer；
  - 60 秒最大队列等待；
  - provider capacity queue 由策略允许，并限制为一次短 reattempt；
  - bounded automatic retry 默认关闭；
  - elapsed guard 为 180 秒。
- 控件属于 Review Team settings。
- 不修改全局 `ai.subagent_max_concurrency`。

## 已完成自适应运行时支持

- Context health snapshot 已存在，用于降级长任务 session。
- Model capability profile 已存在，用于弱模型处理。
- Runtime policy 可以基于模型能力和 session health 调整 context profile。
- 这是防护层，不替代用户选择的审核策略。

## 已完成 Compression Contract 集成

- `CompressionContract` 和从 `EvidenceLedgerSummary` 的转换已完成。
- Compressor prompt 已注入 contract 内容。
- 当前不需要更多 Deep Review 实现。

## 已完成非 DeepReview 影响文档化

共享影响清单已记录，后续仍必须遵守：

| 共享区域 | 已完成边界 |
|---|---|
| `TaskTool` | Deep Review queue、retry、packet、cache 逻辑必须在显式 Deep Review context 检查后执行。 |
| `tool_pipeline.rs` | 重复 `Read`/`GetFileDiff` 测量受 Deep Review gate 保护，且不含内容。 |
| `CodeReviewTool` | Deep Review 报告增强由 Deep Review context gate 保护。 |
| `bitfun-events` | 当前 Deep Review queue event 稳定且为领域特定；是否替换成通用事件是未来产品/API 决策。 |
| Session metadata | Deep Review cache 是 per-session 的，非 DeepReview session 不存在。 |
| Review action bar | Queue/recovery panels 只在 Deep Review 状态下渲染。 |
| Report utilities | Manifest/cache/token-budget section 可选且受 Deep Review gate 保护。 |
| Review settings | Review Team capacity settings 标记为 review-scoped，不是全局 subagent concurrency。 |

## 来源文档中记录的验证历史

来源文档记录了聚焦验证和 release gate，包括：

- `cargo test -p bitfun-core deep_review -- --nocapture`
- `cargo check --workspace`
- `cargo test --workspace`
- `pnpm run lint:web`
- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run test:run`
- `reviewTeamService`、Deep Review action bar/store、queue events、report utilities 的聚焦前端测试；
- runtime diagnostics、cache、retry admission、queue/capacity、report reliability 的聚焦 Rust 测试。

最新合并验证已经执行完整 Rust gate：`cargo test -p bitfun-events deep_review_queue_state_event_serializes_stable_contract -- --nocapture`、`cargo test -p bitfun-core deep_review -- --nocapture`、`cargo check --workspace` 和 `cargo test --workspace` 均已通过。真实 Deep Review 手工 smoke / E2E 仍应作为发布前产品验收单独执行。

## 已完成边界总结

Deep Review 已从纯 Prompt 概念推进为带运行时防护的能力，包含：

- 始终启用 Architecture reviewer；
- 条件启用 Frontend reviewer；
- 后端提供团队定义；
- 数据驱动 reviewer applicability；
- 可解释的自定义 reviewer 校验；
- advisory strategy metadata；
- predictive timeout；
- partial-timeout final-message capture；
- local-cap queue controls；
- 有界 provider-capacity queue 和一次 reattempt；
- structured retry admission；
- 面向结构化 unresolved slices 的显式 manual retry action；
- 带防护的 `auto_retry` admission，且 Review Team 设置默认关闭；
- per-session packet cache；
- packet fallback；
- report reliability signals；
- content-free duplicate-tool diagnostics；
- cost-aware reduced-depth scope profiles；
- metadata-only shared evidence packs；
- content-free duplicate discovery savings diagnostics；
- 紧凑 evidence-pack report/export summary；
- compact launch summary；
- review-scoped capacity/retry settings。

当前已完成边界明确不包含 backend-owned automatic redispatch scheduler、完整 backend batch/stagger scheduling、project-level cache、hard byte clipping、programmatic shared tool-result reuse、global subagent scheduling 和进一步的大型架构重构。当前 launch-batch 保护只是重叠安全网，不是完整 dispatcher。

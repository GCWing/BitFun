# Deep Review 待完成计划与架构重构

## 目的

本文档把所有 Deep Review 待完成和延期事项合并为一份可执行计划，并纳入新的架构重构设计。除非具体产品 checkpoint 明确批准，后续实现不得改变当前行为。

已完成行为基线见 `docs/deep-review-completed-status.zh-CN.md`。

## 来源文档

本计划合并以下文档中的待完成内容：

- `docs/deep-review-design.md`
- `docs/deep-review-phase2-plan.md`
- `docs/deep-review-phase2-addendum.md`
- `docs/deep-review-phase3-followup-plan.md`
- `docs/deep-review-architecture-refactor-plan.md`
- `docs/deep-review-nondeepreview-impact-inventory.md`
- `docs/superpowers/plans/2026-05-09-deep-review-phase3-execution-plan.md`
- `docs/superpowers/specs/` 和 `docs/superpowers/plans/` 下的本地伴随 Deep Review 设计副本。

## 产品与架构边界

未来实现必须保持在以下边界内：

1. Deep Review 仍是 Prompt 驱动，并由确定性运行时防护补强。未经单独设计批准，不替换为后端 DAG scheduler。
2. Deep Review queueing 不能意外变成全局 subagent queueing。
3. Deep Review 不能静默消耗正常用户 session 的并发。如果 session 已繁忙，应提示、暂停或要求用户手动继续。
4. 队列等待时间不能算入 reviewer runtime timeout。
5. Provider capacity queueing 必须短、可见、有界、可暂停、可取消。
6. 自动 retry 默认保持手动；只有用户显式 opt-in 后，才允许小范围有界自动 retry。
7. 自动 retry 绝不能形成无限循环。
8. Quick/default 审核可以降低广度，但不能从 coverage metadata 中隐藏变更文件。
9. Diagnostics 必须低频且不含内容。
10. Project-level review cache 在 retention、deletion、invalidation、user visibility 规则确认前保持延期。
11. 重构轮次必须保持现有行为，除非具体 behavior-change checkpoint 获得批准。

## 剩余功能计划

### Round 1：短 Provider Capacity Queue

状态：已实现并带防护。继续保持 Deep Review-scoped，不能升级为通用 provider/adaptive scheduler。

目标：当 provider 返回窄分类的 transient capacity error 时，Deep Review 应短暂等待并重试一次，然后再报告 `capacity_skipped`。

范围：

- 仅以下错误可 queue：
  - provider rate limit；
  - provider concurrency limit；
  - 显式 `Retry-After`；
  - temporary overload/capacity pressure。
- 以下错误必须快速失败：
  - authentication；
  - billing/quota exhaustion；
  - invalid model；
  - policy violation；
  - user cancellation；
  - invalid reviewer tooling；
  - deterministic validation errors。
- 等待时间受 `min(Retry-After, max_queue_wait_seconds)` 限制。
- 如果用户未 pause/cancel，等待后只 reattempt reviewer 一次。
- 使用 provider-specific reason 发送既有 queue-state event。
- 记录 aggregate diagnostics counters：
  - provider queue count；
  - provider retry count；
  - provider retry success count；
  - final capacity skip count。
- 队列等待时间与 reviewer runtime timeout 分离。
- 复用紧凑 action-bar queue notice 和控制。

风险：

| 风险 | 影响 | 缓解 |
|---|---|---|
| Provider wait 看起来像卡住 | 用户可能以为审核冻结。 | 显示队列提示、elapsed queue time、pause/continue/cancel 控制。 |
| 错误分类过宽 | Auth/quota/model 错误可能无意义等待。 | 窄分类器和 fail-fast 测试。 |
| Deep Review 挤占活跃 session | Queue 恢复时可能抢占正常工作 capacity。 | 保留 active-session warning 和手动 pause/continue。 |
| Retry 拉长整体审核 | 慢模型下 capacity wait 成本变高。 | 只短暂 reattempt 一次，受 max queue wait 和 diagnostics 约束。 |

验证：

- Rust 测试覆盖可 queue 与不可 queue provider error。
- Rust 测试覆盖 queue expiry、queue success、pause、cancel、diagnostics counters。
- 前端测试覆盖 provider queue notice、本地化 reason text、queue-state update。
- 既有 `cargo test -p bitfun-core deep_review -- --nocapture`。

退出标准：

- Provider queue 可见且有界。
- Queue time 与 timeout 分离。
- 非 transient error 快速失败。
- Provider queue 过期时最终报告保持诚实。

### Round 2：显式 Retry Action 与有界自动 Retry 偏好

状态：已实现并带防护；backend-owned automatic redispatch scheduling 仍延期。

目标：为 unresolved reviewer slice 提供清晰 retry action，同时只有用户显式 opt-in 后才允许小范围自动 retry。

范围：

- 从 report metadata 中提取 retryable unresolved packets：
  - source packet id；
  - reviewer id；
  - source status；
  - covered files；
  - unresolved files；
  - retry timeout。
- 可 retry source：
  - `partial_timeout`；
  - transient `capacity_skipped`。
- 不可 retry source：
  - auth；
  - quota/billing；
  - invalid model；
  - policy；
  - invalid tooling；
  - validation；
  - cancellation；
  - non-transient capacity skip。
- 在 action bar 添加显式 retry unresolved slice 按钮。
- 添加显式 opt-in 操作：之后允许 bounded automatic retries。
- 通过 Review Team settings 持久化 opt-in。
- Bounded automatic retry 默认关闭。
- Auto retry 仅在以下条件全部满足时运行：
  - preference enabled；
  - source status 可 retry；
  - retry coverage 是结构化的；
  - retry scope 非空且小于原 scope；
  - role/packet retry budget 仍可用；
  - elapsed guard 仍可用；
  - timeout 小于 source task timeout。
- 稳定 suppression reasons：
  - `preference_disabled`；
  - `budget_exhausted`；
  - `scope_not_reduced`；
  - `elapsed_guard_exceeded`；
  - `non_retryable_status`；
  - `non_transient_error`；
  - `missing_coverage`。

风险：

| 风险 | 影响 | 缓解 |
|---|---|---|
| Retry 无限循环 | Token 和时间成本暴涨。 | Role budget、packet budget、scope 缩小、elapsed guard、一次只处理一个 slice。 |
| Retry 重复坏上下文 | 大范围 retry 可能重复失败。 | 必须有结构化 coverage 且 scope 缩小。 |
| 用户失去控制 | 隐式 retry 会像不可预期自动化。 | 默认手动；opt-in 明确且可在 settings 关闭。 |
| 报告变嘈杂 | Retry metadata 可能挤占 findings。 | Retry 控件保持紧凑，只在必要时展示 unresolved status。 |

验证：

- 前端 report parser 测试 retryable 和 non-retryable slices。
- Store/action-bar 测试 manual retry、disabled state、opt-in action。
- DeepReviewService 测试 retry launch metadata。
- Rust 测试 retry admission、suppression reasons、budget guards。
- Lint、type-check、聚焦 web tests、Rust Deep Review tests。

退出标准：

- Manual retry 可用于结构化 unresolved slices。
- Automatic retry 默认关闭。
- Opt-in 后的 automatic retry 有界且不会循环。

### Round 3：成本感知审核范围

状态：已实现并带防护；后续只保留验证、观测和文案收口。

目标：在大变更或慢模型场景下降低审核时间和 Token 占用，让 quick/default 优先关注高风险证据，并保持 `deep` 为 full-depth 选项。

Scope profile：

```ts
type DeepReviewScopeProfile = {
  reviewDepth: 'high_risk_only' | 'risk_expanded' | 'full_depth';
  riskFocusTags: string[];
  maxDependencyHops: number | 'policy_limited';
  optionalReviewerPolicy: 'risk_matched_only' | 'configured' | 'full';
  allowBroadToolExploration: boolean;
  coverageExpectation: string;
};
```

策略映射：

| 策略 | 审核深度 | 依赖上下文 | Optional reviewers | 探索范围 |
|---|---|---|---|---|
| `quick` | `high_risk_only` | 变更文件和直接高风险契约 | 仅 risk-matched | 关闭 broad exploration |
| `normal` | `risk_expanded` | 变更文件加一跳高风险上下文 | 已配置但仍受 applicability gate | 有限 |
| `deep` | `full_depth` | 策略限制内的广泛上下文 | configured/full 行为 | 允许 |

Quick/default 中仍必须覆盖的高风险类别：

- security；
- data loss；
- migrations；
- authentication/authorization；
- cross-boundary API contracts；
- concurrency；
- persistence；
- configuration changes；
- platform boundary violations。

风险：

| 风险 | 影响 | 缓解 |
|---|---|---|
| 降低深度导致漏掉低风险回归 | Quick/default 用广度换速度。 | 明确标记 coverage，并提供加深路径。 |
| Judge 过度信任 | high-risk-only pass 不是 full review。 | Judge prompt 和 report metadata 必须保留 `reviewDepth` 与 `coverageExpectation`。 |
| Optional reviewers 看似意外消失 | 用户可能期待配置的 reviewer 都运行。 | 显示 applicability/risk-match metadata 和 skipped reasons。 |
| 策略变成隐藏 override | 运行时可能改变用户意图。 | Scope profile 只收窄深度，不秘密改变选中策略。 |

验证：

- Manifest tests 覆盖三种 depth profile。
- 测试 reduced-depth manifest 仍保留 changed-file coverage metadata。
- 更新 reviewer 和 judge prompts。
- Report reliability tests 覆盖 reduced-depth wording。
- Rust Deep Review tests 确认 report schema 兼容。

退出标准：

- Quick 是 high-risk-only。
- Normal 是 risk-expanded。
- Deep 仍是 full-depth。
- 报告不会把 reduced-depth run 声称为 full coverage。

### Round 4：Shared Evidence Pack

状态：已实现并带防护；programmatic full tool-result cache 仍延期。

目标：让 reviewers 从紧凑共享事实开始，减少重复发现相同文件、hunk、contract hint 所消耗的时间和 Token。

建议 manifest 形态：

```ts
type DeepReviewEvidencePack = {
  version: 1;
  source: 'target_manifest';
  changedFiles: string[];
  diffStat: {
    fileCount: number;
    totalChangedLines?: number;
    lineCountSource: 'unknown' | 'diff_stat' | 'estimated';
  };
  domainTags: string[];
  riskFocusTags: string[];
  packetIds: string[];
  hunkHints: Array<{
    filePath: string;
    changedLineCount: number;
    lineCountSource: 'unknown' | 'diff_stat' | 'estimated';
  }>;
  contractHints: Array<{
    kind: 'i18n_key' | 'tauri_command' | 'api_contract' | 'config_key';
    filePath: string;
    source: 'path_classifier';
  }>;
  budget: {
    maxChangedFiles: number;
    maxHunkHints: number;
    maxContractHints: number;
    omittedChangedFileCount: number;
    omittedHunkHintCount: number;
    omittedContractHintCount: number;
  };
  privacy: {
    content: 'metadata_only';
    excludes: [
      'source_text',
      'full_diff',
      'model_output',
      'provider_raw_body',
      'full_file_contents',
    ];
  };
};
```

规则：

- 不包含完整源码。
- 不包含完整 diff。
- 不包含 reviewer output。
- 不包含 provider raw response body。
- 保持 source-agnostic。
- 优先使用 metadata、hunk ranges、domain tags、risk tags、packet ids、cheap contract hints。
- `Read` 和 `GetFileDiff` 只用于确认或补缺上下文。
- Programmatic cross-subagent `Read` output reuse 在 duplicate-tool diagnostics 证明成本较高前继续延期。

风险：

| 风险 | 影响 | 缓解 |
|---|---|---|
| Evidence pack 变成隐藏大上下文 | 会复现 Token/隐私问题。 | 默认 metadata-first 且不含内容。 |
| Hints 过期 | Reviewer 可能依赖旧元数据。 | 从同一 manifest 输入中一次性派生，并包含 source labels。 |
| Tool-result reuse 改变 reviewer 隔离性 | 共享 full reads 可能泄漏或冻结上下文。 | 先只做 evidence，full tool-result reuse 延期。 |
| Contract extraction 过重 | 重静态分析会拖慢启动。 | 仅从已有 changed files/name 做廉价提取。 |

验证：

- Manifest tests 覆盖 evidence pack structure、source label、size budget 和 privacy boundary。
- 测试 pack 不存储完整源码、完整 diff、model output、provider raw body 或 full file contents。
- Prompt tests 或轻量断言证明 reviewer 会先使用 evidence，但必须用工具确认 stale hints。
- Diagnostics 保留 content-free duplicate measurement，并新增可节省重复 discovery 的聚合候选计数。

退出标准：

- Reviewer 收到紧凑 shared evidence。
- Diagnostics 中不存储 source/diff/model output。
- 减少重复发现，但不改变工具语义。

### Round 5：文档对齐与 Release Gate

状态：provider queue、retry controls、cost-aware scope 和 evidence pack 完成后的 active release gate。

目标：每个功能闭环后保持文档与代码一致。

动作：

- 只有验证通过后才更新状态措辞。
- Provider queue 只有在可见有界行为验证后才能标记为已实现。
- Retry controls 只有在 manual retry 和 opt-in guard 存在后才能标记为已实现。
- Project-level cache 保持延期。
- Programmatic shared context cache 在测量证明必要前保持延期。
- Hard prompt-byte clipping 保持延期。
- 扫描过期完成声明。

验证：

```powershell
rg -n "project-level cache.*implement[[:alpha:]]*ed|automatic retry.*compl[[:alpha:]]*ete|provider/adaptive queue.*compl[[:alpha:]]*ete|hard prompt.*compl[[:alpha:]]*ete|global.*concurrency.*auto[[:alpha:]]*matic" docs/deep-review-design.md docs/deep-review-phase2-plan.md docs/deep-review-phase2-addendum.md docs/deep-review-phase3-followup-plan.md
cargo test -p bitfun-core deep_review -- --nocapture
cargo check --workspace --exclude bitfun-cli
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/web-ui run test:run
git diff --check
```

退出标准：

- 文档能区分已实现、带防护实现、Prompt 引导、延期和待实现行为。
- 文档声明不超过代码事实。
- 新用户可见字符串都有 locale 覆盖。
- Queue/retry/cache/token 功能没有引入隐藏困惑或未记录隐私风险。

## 延期产品决策

### Project-Level Review Cache

状态：产品决策延期。

实现前必须有单独计划定义：

- retention duration；
- 文件 rename、model、strategy、roster、prompt 变化下的 invalidation；
- deletion behavior；
- 用户可见性和管理 UI；
- reviewer outputs 是否可以持久化到 session metadata 之外；
- 对 source summaries 和 security findings 的隐私评审。

当前边界：仅 per-session cache。

### Programmatic Shared Tool-Result Cache

状态：等待真实测量证明必要。

当前边界：

- prompt-level reuse guidance；
- 不含内容的重复 `Read`/`GetFileDiff` 测量；
- 最终 aggregate diagnostics。

在真实运行证明重复成本较高且单独语义/隐私计划获批前，不拦截和复用完整 tool result。

### Hard Prompt-Byte Clipping

状态：延期。

当前边界：

- heuristic prompt-byte estimate；
- summary-first full-scope metadata；
- file splitting/max-file guardrails。

除非每个被省略或降级的文件都明确出现在 coverage/reliability metadata 中，否则不得 hard-clip reviewer coverage。

### Backend DAG Scheduler

状态：延期。

当前边界：

- prompt-driven orchestration；
- TaskTool hard guardrails；
- local-cap queue controls；
- Deep Review-scoped 短 provider capacity queue。

当前计划中不把 orchestrator 替换为确定性后端 workflow engine。

### 权威运行时策略选择

状态：延期。

当前边界：

- 仅 advisory/mismatch-warning metadata。

在 measured complexity signal 和产品批准前，后端风险评分不得覆盖用户选择策略。

## 架构重构计划

状态：部分实现。后端 Deep Review 模块与前端 review-team facade 已抽取；剩余 refactor follow-up 默认不允许行为变化，除非明确列出并获得批准。

### 重构目标

1. 尽可能把 Deep Review 特定逻辑从宽泛共享文件中迁出。
2. 分离通用 subagent runtime primitive 与 Deep Review policy adapter。
3. 除非作为产品决策评审，否则保持标准 subagent 行为稳定。
4. 减少超大文件和重复定义。
5. 在重构轮次中保持现有 Deep Review 行为。
6. 保持依赖无环且位置选择可预测。
7. 明确并测试所有非 DeepReview 影响。
8. 保持前后端 Deep Review 边界清晰。
9. 避免引入新的性能、质量或安全风险。

### 当前重构压力

| 区域 | 当前压力 |
|---|---|
| `src/crates/core/src/agentic/deep_review_policy.rs` | 角色/团队定义、manifest 解析、execution policy、并发、queue controls、effective cap learning、retry、diagnostics、shared-context measurement、cache 和 tests 都在同一文件。 |
| `src/crates/core/src/agentic/tools/implementations/task_tool.rs` | 通用 Task tool 中混入 Deep Review capacity wait、retry admission、packet/cache lookup、provider capacity skip、queue event 和 tests。 |
| `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs` | 标准 Code Review 与 Deep Review report 逻辑混合，包括 packet fallback、reliability、diagnostics、cache write-through。 |
| `src/crates/core/src/agentic/tools/pipeline/tool_pipeline.rs` | 通用 tool pipeline 携带 Deep Review context propagation 和 duplicate read/diff measurement。 |
| `src/crates/events/src/agentic.rs` | 共享 event crate 中包含 Deep Review queue event payload。 |
| `src/web-ui/src/shared/services/reviewTeamService.ts` | Config、backend definition、validation、strategy、risk、manifest、work packets、cache plan、token budget、prompt block 都在一个大文件。 |
| `src/web-ui/src/flow_chat/services/DeepReviewService.ts` | Slash parsing、target resolution、stats、runtime signals、launch cleanup、child-session launch 耦合。 |
| `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx` | Queue controls、recovery、remediation、diagnostics、review actions 集中且密集。 |
| `src/web-ui/src/flow_chat/utils/codeReviewReport.ts` | Report normalization、reliability notices、manifest rendering、markdown export 一起增长。 |

### 目标后端结构

在 core 中创建 Deep Review 子系统：

```text
src/crates/core/src/agentic/deep_review/
  mod.rs
  constants.rs
  team_definition.rs
  manifest.rs
  execution_policy.rs
  concurrency_policy.rs
  queue.rs
  retry.rs
  diagnostics.rs
  shared_context.rs
  incremental_cache.rs
  report.rs
  task_adapter.rs
  tests/
```

职责：

| 模块 | 职责 |
|---|---|
| `constants.rs` | Agent type constants 和 role families。 |
| `team_definition.rs` | Default review team definition 和 strategy profile data。 |
| `manifest.rs` | `deep_review_run_manifest`、packet lookup、strategy/concurrency/cache/token fields 的 typed accessors。 |
| `execution_policy.rs` | Timeouts、file split thresholds、retry limit config、risk helper。 |
| `concurrency_policy.rs` | Configured cap 与 effective-cap calculations。 |
| `queue.rs` | Queue state、queue controls、capacity classification、local/provider queue decisions。 |
| `retry.rs` | Structured retry coverage validation、retry prompt block、retry budget helpers。 |
| `diagnostics.rs` | Aggregate runtime diagnostics 和低频 final logging data。 |
| `shared_context.rs` | Duplicate `Read`/`GetFileDiff` measurement 和未来 evidence metadata helpers。 |
| `incremental_cache.rs` | Per-session packet cache model 和 serialization。 |
| `report.rs` | `CodeReviewTool` 使用的 Deep Review-specific reliability signals 和 packet metadata helpers。 |
| `task_adapter.rs` | Deep Review-specific TaskTool orchestration hooks。 |

`deep_review_policy.rs` 在迁移期间应变成 compatibility facade，导入更新完成后再缩小为 re-export 或移除。

### 通用 Subagent Runtime 边界

只有在 Deep Review 抽取稳定后，才引入通用 runtime 区域：

```text
src/crates/core/src/agentic/subagent_runtime/
  mod.rs
  capacity.rs
  queue_state.rs
  retry_admission.rs
```

规则：

- 未证明通用前，不把行为移动到这里。
- 通用模块不得 import Deep Review 模块。
- Deep Review adapter 可以 import 通用模块。
- Provider-capacity auto queueing 不得在本重构中变成全局 subagent 行为。

可通用候选：

- capacity acquisition/release guard；
- 与 Deep Review label 无关的 queue state shape；
- queue wait 与 running time 的 timeout separation；
- bounded retry admission primitives。

### 后端工具 Facade

保持公开 tool entrypoint 稳定：

- `TaskTool` 仍是注册的 Task tool。
- `CodeReviewTool` 仍是注册的 report submission tool。

把功能特定逻辑移动到 adapter 后：

```rust
let deep_review_context = deep_review::manifest::Context::from_tool_context(context);
deep_review::task_adapter::prepare_launch(...);
deep_review::retry::validate_retry(...);
deep_review::queue::wait_for_reviewer_capacity(...);
```

```rust
deep_review::report::fill_packet_metadata(...);
deep_review::report::fill_reliability_signals(...);
deep_review::incremental_cache::persist_completed_packets(...);
deep_review::diagnostics::log_final_snapshot(...);
```

必要防护：没有 Deep Review context 时，普通 Task 和标准 Code Review 行为必须不变。

### 目标前端 Review-Team 结构

把 `reviewTeamService.ts` 拆为目录，并保留 compatibility facade：

```text
src/web-ui/src/shared/services/review-team/
  index.ts
  types.ts
  defaults.ts
  config.ts
  backendDefinition.ts
  strategy.ts
  targetClassifier.ts
  subagentCapabilities.ts
  manifestBuilder.ts
  workPackets.ts
  tokenBudget.ts
  risk.ts
  promptBlock.ts
  cachePlan.ts
  preReviewSummary.ts
```

保持以下 import path 可用：

```text
src/web-ui/src/shared/services/reviewTeamService.ts
```

旧文件应变成从 `./review-team` 导出的 facade。

依赖规则：

- `types.ts` 依赖应很轻，不 import 实现模块。
- `config.ts` 可 import config APIs。
- `backendDefinition.ts` 可 import agent APIs。
- `manifestBuilder.ts` 可 import pure helpers。
- Pure helpers 不得 import `manifestBuilder.ts`。
- Flow Chat launch modules 应优先 import facade，除非有清晰边界理由。

### 目标 Flow Chat Deep Review 结构

拆分 launch、action-bar、report 职责：

```text
src/web-ui/src/flow_chat/deep-review/
  launch/
    commandParser.ts
    targetResolver.ts
    launchPrompt.ts
    launchSession.ts
    launchErrors.ts
  action-bar/
    CapacityQueueNotice.tsx
    InterruptionRecoveryPanel.tsx
    RemediationControls.tsx
    ReviewActionHeader.tsx
  report/
    reliabilityNotices.ts
    manifestSections.ts
    markdown.ts
```

继续保留当前 public exports：

- `DeepReviewService.ts`
- `DeepReviewActionBar.tsx`
- `codeReviewReport.ts`

### 重构执行轮次

#### Refactor Round 0：Baseline 与 Guardrails

动作：

- 记录当前超大文件行数。
- 运行聚焦 Deep Review tests。
- 确认 non-DeepReview impact inventory 当前有效。
- 不改变行为。

验证：

- `pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts src/flow_chat/components/btw/DeepReviewActionBar.test.tsx src/flow_chat/utils/codeReviewReport.test.ts`
- `cargo test -p bitfun-core deep_review -- --nocapture`

#### Refactor Round 1：后端 Deep Review 模块抽取

动作：

- 创建 `src/crates/core/src/agentic/deep_review/`。
- 先移动 constants 和 team definitions。
- 移动 execution policy 和 strategy helpers。
- 逐个移动 concurrency、queue、diagnostics、shared context、retry、cache。
- `deep_review_policy.rs` 保持 compatibility facade。

验证：

- Rust Deep Review tests。
- `rg -n "deep_review_policy::" src/crates/core/src` 确认 import 都是有意保留。

允许行为变化：无。

#### Refactor Round 2：TaskTool Adapter 抽取

动作：

- 添加 `deep_review::task_adapter`。
- 把 Deep Review context detection、packet id resolution、cache lookup、retry validation、retry prompt preparation、queue/capacity calls 移入 adapter。
- 添加或保留非 DeepReview Task 回归测试。

验证：

- Deep Review TaskTool tests。
- 非 DeepReview Task 测试，证明没有 Deep Review context 时不会进入 queue/retry/cache 路径。

允许行为变化：无。

#### Refactor Round 3：CodeReviewTool Report Adapter

动作：

- 添加 `deep_review::report`。
- 移动 packet metadata fallback、reliability signals、token budget notes、diagnostics logging、incremental cache write-through。
- 添加或保留标准 Code Review 回归测试。

验证：

- Deep Review report tests。
- 标准 Code Review 测试，证明 Deep Review metadata 不会出现在 Deep Review 外。

允许行为变化：无。

#### Refactor Round 4：Event 与 Tool Pipeline 收敛

动作：

- 保持当前 Deep Review queue event contract 稳定。
- 把 payload conversion helpers 移入 Deep Review modules。
- 用小 hook/helper 替代 `tool_pipeline.rs` 中内联 Deep Review context propagation。
- 保持 duplicate read/diff measurement 受 Deep Review gate 保护。

验证：

- Queue event serialization tests。
- Tool pipeline 非 DeepReview 回归测试。

允许行为变化：无。

延期行为变化：

- 把 `DeepReviewQueueStateChanged` 替换为通用 `SubagentQueueStateChanged` event。

#### Refactor Round 5：前端 Review Team 拆分

动作：

- 创建 `src/web-ui/src/shared/services/review-team/`。
- 先移动 types。
- 再移动 pure helpers：strategy、risk、work packets、token budget、cache plan、pre-review summary。
- 分别移动 config persistence 和 backend definition loading。
- `reviewTeamService.ts` 保持 facade。

验证：

- `pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts`
- `pnpm run type-check:web`

允许行为变化：无。

#### Refactor Round 6：Flow Chat Deep Review 拆分

动作：

- 拆分 command parsing、target resolution、manifest runtime signals、launch cleanup、child-session launch。
- 拆分 queue notice、interruption recovery、remediation controls、review action layout。
- 拆分 reliability notices、manifest markdown、report normalization。

验证：

- `pnpm --dir src/web-ui run test:run -- src/flow_chat/services/DeepReviewService.test.ts src/flow_chat/components/btw/DeepReviewActionBar.test.tsx src/flow_chat/utils/codeReviewReport.test.ts`
- `pnpm run lint:web`
- `pnpm run type-check:web`

允许行为变化：无。

#### Refactor Round 7：文档、注释与 Ownership 清理

动作：

- 为边界不明显的新 Rust `deep_review` 模块添加 module-level docs。
- 只为 facade 和边界模块添加简洁 TypeScript file header。
- 抽取后移除重复 constants 和状态措辞。
- 仅当文件 ownership 变化时更新文档。

验证：

- `rg -n "TODO|TBD|temporary|copy of|duplicate" src/crates/core/src/agentic/deep_review src/web-ui/src/shared/services/review-team`
- 聚焦前端和 Rust Deep Review tests。

允许行为变化：无。

### 重构行为变化 Checkpoints

出现以下情况必须停止并询问：

1. 把 Deep Review queue 行为应用到所有 subagents。
2. 让普通 subagents 的 provider transient errors 自动 queue。
3. 用通用 subagent queue event 替换 Deep Review-specific queue event。
4. 把 retry 从结构化 model/user-issued retry 改为后端自动 redispatch。
5. 让后端风险评分覆盖 user/team strategy。
6. 把 review cache 持久化到 session metadata 之外。
7. Hard-clipping prompt bytes 或从 coverage metadata 隐藏文件。
8. 超出 cost-aware plan 改变 quick/normal/deep 语义。

## 非 DeepReview 影响要求

后续轮次必须保持以下规则：

1. 通用 subagent runtime modules 不得 import Deep Review modules。
2. Deep Review adapters 可以 import 通用 runtime modules。
3. Shared tools 只有在显式 context gate 后才能调用 Deep Review adapters。
4. 标准 Code Review 必须在没有 Deep Review manifest 时继续工作。
5. Deep Review queue time 不能变成全局 subagent timeout rule。
6. Provider capacity queueing 在产品批准前保持 Deep Review-scoped。
7. Diagnostics 必须 aggregate-only 且不含内容。

必须保留或新增的回归测试：

- 没有 `deep_review_run_manifest` 的普通 Task 不应用 Deep Review queue controls。
- 普通 Task retry 不要求 Deep Review `retry_coverage`。
- 标准 Code Review submission 不发出 Deep Review packet/cache/queue metadata。
- Deep Review queue events 按当前稳定形态序列化。
- Tool pipeline duplicate-read measurement 忽略非 DeepReview `Read` 和 `GetFileDiff` 调用。
- 标准 Code Review action bar 不渲染 Deep Review queue controls。
- Deep Review queue controls 只在 Deep Review 状态下渲染。
- 标准 Code Review markdown export 不包含 Deep Review manifest/cache sections。
- Review settings 文案区分 Review Team max reviewers 与 global subagent concurrency。

## 跨领域 UX 与国际化要求

- 每个新用户可见字符串必须覆盖 `en-US`、`zh-CN`、`zh-TW`。
- 优先使用既有 action bar 和 Review settings，不新增不必要 modal。
- Queue 和 retry notice 保持紧凑。
- 默认不展示密集 Token/cost 内部细节。
- Reliability 与 coverage 解释使用默认折叠细节。
- 保持主题兼容和紧凑布局稳定。
- 除非能帮助用户做决策，不增加解释实现内部逻辑的可见文本。

## 性能与隐私要求

- Diagnostics 必须低频。
- Runtime logs 必须为英文且无 emoji。
- Diagnostics 不得记录或存储源码、完整 diff、reviewer output、provider raw body 或完整文件内容。
- Shared evidence 必须紧凑且 metadata-first。
- Queue 和 retry 自动化必须受 settings 与 budgets 约束。
- 大变更成本削减必须通过 coverage metadata 保持透明。

## 最终 Release Gate

完成任一待实现功能批次或重构批次后运行：

```powershell
cargo test -p bitfun-core deep_review -- --nocapture
cargo check --workspace --exclude bitfun-cli
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/web-ui run test:run
git diff --check
```

如果触及 backend、desktop API 或 Tauri adapters，还要按 `AGENTS.md` 运行相邻 desktop/backend 验证。

## 停止条件

出现以下情况必须停止并重新评审设计：

- 修复需要把全局 `ai.subagent_max_concurrency` 作为 Deep Review 常规恢复路径；
- diagnostics 需要存储源码、diff、reviewer output、provider raw body 或完整文件内容；
- provider queue 需要对同一 reviewer packet 自动 reattempt 超过一次；
- auto retry 需要不小于原 packet 的 scope；
- quick/default 成本削减会隐藏 changed files，而不是标记 reduced-depth coverage；
- duplicate-call diagnostics 证明必要前，就需要共享完整 `Read` output cache；
- UI 变化需要新增页面或 modal，而 action bar/Review settings 足以承载；
- 重构影响 Deep Review 之外行为，但未经过确认 checkpoint。

## 预期最终状态

全部待完成功能和重构结束后：

- Provider transient queue 短、可见、有界、可控制。
- Retry 默认由用户显式触发，opt-in 后的自动 retry 仍有界。
- Quick/default 通过风险聚焦降低时间和 Token 成本，`deep` 保持 full-depth。
- Reviewers 从紧凑 shared evidence pack 开始。
- Project-level cache、完整 tool-result reuse、hard prompt clipping、DAG scheduling 仍保持延期，除非单独批准。
- Deep Review 后端逻辑位于独立模块树。
- Shared TaskTool、CodeReviewTool、tool pipeline、event code 只包含薄 Deep Review hooks。
- 前端 review-team 和 Flow Chat Deep Review 代码按职责拆分，并保留稳定 facade。
- 非 DeepReview 行为有聚焦回归测试覆盖。
- 文档和代码状态保持一致。

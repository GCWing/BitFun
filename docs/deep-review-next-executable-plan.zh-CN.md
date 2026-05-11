# Deep Review 后续整体重构与功能补齐执行计划

## 文档定位

本文整合以下四份现有文档，形成后续可执行的粗粒度计划：

- `docs/deep-review-completed-status.md`
- `docs/deep-review-pending-plan.md`
- `docs/deep-review-architecture-refactor-plan.md`
- `docs/deep-review-nondeepreview-impact-inventory.md`

当前版本把 Deep Review 的后续交付压缩为两个阶段：阶段 1 是本 PR 的最终发布收口，阶段 2 是有触发条件的产品化后续。下方旧 M1-M4 内容保留为追溯证据和边界索引，不再代表新增扩张路线；后续拆分必须服从本节的发布闭环和阶段边界。

## 当前基线

Deep Review 当前已经不是纯 prompt 概念，而是带运行时护栏的 prompt-driven 5 阶段编排：

1. 识别审查范围。
2. 并行启动专业 reviewer。
3. 顺序执行 judge / quality gate。
4. 通过 `submit_code_review` 综合最终报告。
5. 需要修复时回到普通编辑工具链。

已经完成并可作为后续依赖的能力包括：

- Architecture reviewer 常驻启用。
- Frontend reviewer 基于目标分类条件启用。
- 后端 reviewer definition 作为前端 Review Team 解析的运行时来源。
- 自定义 review-agent 最小工具契约集中化。
- advisory strategy metadata、mismatch warning、predictive timeout、partial timeout final-message capture。
- Deep Review 本地并发上限队列、pause / continue / cancel / optional-extra skip 控制。
- 结构化 retry admission 和 retry guardrails。
- per-session incremental cache 和 packet metadata fallback。
- report reliability signals、content-free duplicate `Read` / `GetFileDiff` diagnostics。
- compact consent summary、action-bar recovery、Review Team capacity / retry settings。
- provider transient capacity 短队列与一次性重试，且保持 Deep Review scoped。
- 用户可见 explicit retry action，以及默认关闭、opt-in 后才可通过 guard 的 bounded auto retry admission。
- quick / normal / deep cost-aware scope profile，且 reduced-depth report 不声称 full coverage。
- metadata-first shared evidence pack，且不存储 source text、full diff、reviewer output、provider raw body 或 full file contents。
- 前端 review-team、Flow Chat launch/action-bar/report 的 no-behavior helper split 与 ownership 文档。

当前明确尚未完成或仍延期的边界包括：

- shared runtime containment 已落地第一层 no-behavior primitive：`agentic/subagent_runtime/queue_timing.rs` 负责 queue wait / pause 计时，Deep Review adapter 复用它；后续 capacity guard、queue state、retry admission 仍必须证明 generic 后再迁移。
- backend-owned automatic retry redispatch scheduler 仍未实现，并按产品设计独立延期；当前 `auto_retry` 只是 admission guard，不是自动调度器。
- 完整 backend batch / stagger scheduler 仍未实现；当前已补 P0-a 批次重叠保护：TaskTool 会在前序 `launch_batch` 仍有活跃 reviewer 时把后续批次放入可见 capacity queue，并给出 `launch_batch_blocked` 原因。
- Deep Review queue liveness P0 已在本 PR 收口：当同一次 Deep Review 仍有 active reviewer 时，本地排队 reviewer 不再按固定短窗口过期；只有无 active reviewer 且仍无法取得 admission 时才触发 queue-expired。完整 backend batch / stagger scheduler 仍按独立产品设计延期。
- user-facing effective-cap override controls 仍未实现，除非后续产品决策明确纳入，否则不应在本计划中顺手实现。
- project-level review cache、programmatic full tool-result cache、hard prompt-byte clipping、backend DAG scheduler、authoritative runtime strategy selection 继续保持延期。

## 最终 PR 判断与两阶段执行计划

从资深架构、功能和产品视角，本 PR 可以作为当前 Deep Review 发布线的最终收口 PR，前提是阶段 1 的验证门禁通过。当前代码与文档已经覆盖 Deep Review 的核心用户体验闭环：带范围说明和成本意识的启动、可见 reviewer 队列和进度、诚实的报告可靠性 / scope / evidence 表达、带决策确认的修复入口，以及异常场景下的 retry、continue、cancel、run slower 和 settings 恢复路径。这个闭环不依赖新增大页面或隐藏自动化。

产品策略闭环定义为：用户明确启动和范围 -> 系统展示 reviewer 等待 / 运行 / 跳过原因 -> 产出带可靠性和覆盖边界的报告 / 导出 -> 用户选择是否修复并确认待决策项 -> 异常时给出同层级的继续、取消、降并发、重试或设置入口。当前 PR 到此为止，不继续把 Deep Review 扩张成后台 DAG scheduler、project-level cache、全局 subagent queue、隐藏自动重派发、静默 prompt clipping 或 raw effective-cap 控制面。

阶段 1，本 PR 收口：

- 完成基于 `origin/yanzhn/deep_tmp` 的 rebase 与冲突清理，保留近期非 Deep Review UI 文案和样式语义。
- 修正 Deep Review 本地 reviewer queue liveness：同一次 Deep Review 内仍有 active reviewer 时继续等待；释放后再 admission；取消 / optional skip 仍可立即结束。
- 对齐 zh-CN / zh-TW locale、action bar queue reason、remediation decision gate 和 subagent duration copy，避免 UI 冲突造成旧文案回退。
- 更新 completed / pending / next plan 文档，把已实现能力、延期项和发布门禁标记到最新代码状态。
- 通过 Rust queue liveness、Deep Review、Web UI focused/full suite、locale JSON、conflict-marker 和 diff-check 验证后，强制推送 `yanzhn/deepreview`。

阶段 2，仅在阶段 1 合入后按触发条件启动：

- 如果真实使用或 telemetry 显示 provider burst / reviewer 等待仍是主要痛点，再单独设计轻量 backend batch / stagger：先定默认 batch cadence、等待文案、取消/跳过语义和失败恢复顺序，再实现。
- 如果自动恢复收益被验证，再设计默认关闭、显式 opt-in 的后台补跑；不得在本 PR 隐式启用自动重派发。
- 若没有新的失败证据，阶段 2 不启动，Deep Review 维持当前 prompt-driven 编排和可见恢复闭环。

发布可用性门禁：

- 正常场景：compact consent summary、queue/progress、final report/export、remediation action bar 与 decision gate 可连贯完成一次 Deep Review。
- 异常场景：capacity queue 的 pause/continue/cancel/skip optional/run slower/settings、partial retry、reduced scope reliability、fail-fast non-transient 错误和可复制诊断都有可见路径。
- 功能漂移 guardrail：普通 Task、标准 Code Review、普通 action bar、report export、非 Deep Review locale 和 session usage UI 不被 Deep Review 冲突覆盖。
- 回滚边界：若 queue liveness 或 action-bar UI 出现严重回归，可回滚本 PR 新增的 release-close commit，不需要回滚已在前序提交拆分出来的 helper/facade 结构。

## 总体原则

后续工作必须遵守以下原则：

1. Deep Review 继续保持 prompt-driven 编排，并通过确定性 guardrails 限制风险；不要在本计划内改成 backend DAG scheduler。
2. Deep Review queueing 不能意外升级为全局 subagent queueing。
3. Deep Review 内部并发上限只作用于同一次 Deep Review 的 reviewer / judge 排队、批次和启动顺序；不得把当前主 session 中的普通 subagent、其他 session、或其他 session 内部 subagent 计入 Deep Review reviewer cap。主流程繁忙只能作为风险提示或失败诊断信号，不应阻止 Deep Review 入队，也不应缩短 Deep Review 队列等待。
4. Provider capacity queue 只处理短暂、可恢复、窄分类的容量问题；auth、quota、billing、invalid model、policy、validation、user cancellation 必须 fail fast。
5. Queue wait time 不计入 reviewer runtime timeout，也不能变成全局 subagent timeout 规则。当同一次 Deep Review 中仍有任意 reviewer 正在执行或可执行时，排队 reviewer 不应因为固定短等待窗口被跳过；running reviewer 自身的 run timeout 才是主要兜底。
6. Automatic retry 默认关闭，只能在用户明确 opt-in 后按预算、scope、elapsed guard 执行。
7. Quick / normal 可以降低深度，但不能从 coverage metadata 中隐藏 changed files。
8. Diagnostics 必须低频、聚合、content-free；不得记录 source text、full diff、reviewer output、provider raw body 或 full file contents。
9. Project-level review cache、programmatic full tool-result cache、hard prompt-byte clipping、authoritative runtime strategy selection 都保持延期，除非另有产品决策。
10. 架构重构默认不改变行为。任何行为改变必须在对应里程碑中显式列出并通过发布门禁。
11. 任何 shared area 变更都要用 non-DeepReview regression tests 证明普通 Task、标准 Code Review、普通 action bar 和 report export 不受影响。
12. 新 UI 优先使用现有 action bar 和 Review settings surface；需要新页面或新 modal 时必须停下重新确认设计。

## 当前维护压力

已核对的高风险文件体量如下，后续重构应优先降低这些文件的职责密度：

| 文件 | 当前行数 | 主要问题 |
|---|---:|---|
| `src/crates/core/src/agentic/deep_review_policy.rs` | 1669 | 已是 Deep Review subsystem facade，但仍有 global tracker、config loading、legacy tests 与兼容导出压力。 |
| `src/crates/core/src/agentic/tools/implementations/task_tool.rs` | 2479 | 通用 TaskTool 中仍有 Deep Review queue / retry / cache / packet adapter 调用和测试压力。 |
| `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs` | 1627 | 标准 Code Review 与 Deep Review report enrichments 已有 guard，但仍在同一工具文件。 |
| `src/crates/core/src/agentic/tools/pipeline/tool_pipeline.rs` | 1485 | 通用 tool pipeline 仍承担 Deep Review context propagation 和 duplicate measurement hook。 |
| `src/web-ui/src/shared/services/reviewTeamService.ts` / `src/web-ui/src/shared/services/review-team/` | 2 / split modules | old import path 已是 facade；新增逻辑应进入窄 helper 或 `review-team/index.ts` 的明确 orchestration 边界。 |
| `src/web-ui/src/flow_chat/services/DeepReviewService.ts` / `src/web-ui/src/flow_chat/deep-review/launch/DeepReviewService.ts` | 2 / 338 | old import path 已是 facade；launch 编排已拆出 command parser、target resolver、launch prompt 和 launch errors。 |
| `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx` / `src/web-ui/src/flow_chat/deep-review/action-bar/DeepReviewActionBar.tsx` | 5 / 785 | old import path 已是 facade；capacity queue notice、partial-results panel、recovery-plan preview、header、formatting、remediation selection、action controls 和 diagnostics builder 已拆出；剩余主体主要是 action bar 状态编排与回调 glue。 |
| `src/web-ui/src/flow_chat/utils/codeReviewReport.ts` / `src/web-ui/src/flow_chat/deep-review/report/codeReviewReport.ts` | 2 / 423 | old import path 已是 facade；report reliability、manifest sections、section normalization 和 markdown export 已拆出。 |

## 子计划执行规则

每个重大里程碑必须拆成多个可独立验证的小计划执行。除非计划明确允许行为变化，否则默认是 no-behavior-change。每个子计划建议对应一个 PR；如果实现中发现一个子计划仍需要同时修改过多 shared entrypoint，应继续拆分。

每个子计划都必须满足：

- **单一责任：** 只完成一个边界清晰的模块迁移、功能补齐或发布收口动作。
- **稳定入口：** 保持现有 public API、tool name、Tauri command、event contract、import path 可用，除非该子计划明确声明迁移策略。
- **测试先行：** shared runtime 或行为变更必须先补 regression / failing test，再改实现。
- **非 Deep Review 保护：** 触碰 `TaskTool`、`CodeReviewTool`、tool pipeline、event、action bar、report utilities、settings 时，必须带 non-DeepReview regression。
- **文案完整：** 新增用户可见文案时，同一子计划必须补齐 `en-US`、`zh-CN`、`zh-TW`。
- **隐私边界：** diagnostics、logs、evidence pack、markdown export 不得存储 source text、full diff、reviewer output、provider raw body 或 full file contents。
- **文档同步：** 子计划改变实现状态、ownership 或延期边界时，同一 PR 必须同步本文和对应源文档。

## 里程碑 1：无行为变化的架构隔离与回归基线

**目标：** 先把 Deep Review 作为子系统隔离出来，降低后续功能补齐的改动风险。该里程碑默认不改变用户可见行为，但完成后应可独立发布。

**发布价值：** 普通 Code Agent、普通 Task、标准 Code Review 不变；Deep Review 内部职责边界清晰，后续 provider queue、retry、scope profile、evidence pack 可以在小模块内实现和测试。

### 关键步骤

1. 建立基线。
   - 在 PR 描述中记录上述高风险文件行数。
   - 先运行 focused Deep Review 测试，确认改动前行为。
   - 补齐或确认 non-DeepReview regression tests，覆盖普通 Task、标准 Code Review、普通 action bar、标准 report export。

2. 创建后端 Deep Review 子系统目录。
   - 新建 `src/crates/core/src/agentic/deep_review/`。
   - 目标模块：
     - `constants.rs`
     - `team_definition.rs`
     - `manifest.rs`
     - `execution_policy.rs`
     - `concurrency_policy.rs`
     - `queue.rs`
     - `retry.rs`
     - `diagnostics.rs`
     - `shared_context.rs`
     - `incremental_cache.rs`
     - `report.rs`
     - `task_adapter.rs`
   - `src/crates/core/src/agentic/deep_review_policy.rs` 在迁移期间保留为 compatibility facade，后续只做 re-export 或被移除。

3. 后端迁移顺序。
   - 先移动 constants、role family、default team definition。
   - 再移动 manifest typed accessors、packet lookup、strategy / execution policy。
   - 再移动 concurrency、queue、retry、diagnostics、shared-context measurement、incremental cache。
   - 最后将 TaskTool 和 CodeReviewTool 中的 Deep Review 逻辑收敛到 `task_adapter.rs` 和 `report.rs`。

4. 保持工具入口稳定。
   - `TaskTool` 仍然是已注册 Task 工具。
   - `CodeReviewTool` 仍然是已注册 review submission 工具。
   - shared tool 只能在明确 Deep Review context 存在时调用 Deep Review adapter。

5. containment shared runtime。
   - `tool_pipeline.rs` 中的 Deep Review context propagation 和 duplicate measurement 抽成小 hook/helper。
   - `src/crates/events/src/agentic.rs` 中现有 `DeepReviewQueueStateChanged` contract 保持稳定。
   - 不在该里程碑中引入 generic `SubagentQueueStateChanged`。
   - 已引入 `src/crates/core/src/agentic/subagent_runtime/` 的第一层通用 primitive：`queue_timing.rs`，仅负责 queue wait、pause / continue 与毫秒快照，不包含 Deep Review reason、event、reviewer、retry 或 provider 语义。
   - 后续 generic candidates 仍限于 capacity acquisition / release guard、queue state shape、bounded retry admission primitives；每一项都必须先有跨场景复用证据和 non-DeepReview regression。
   - `agentic/subagent_runtime/*` 不得 import Deep Review modules；Deep Review adapters 可以 import generic runtime modules。
   - Deep Review provider-capacity auto queueing 不得因为抽象 generic runtime 而变成普通 subagent 行为。
   - `agentic/deep_review/*` 不得依赖 desktop、Tauri、frontend-specific concepts。
   - UI components 不得直接调用 Tauri APIs，仍通过 adapter / infrastructure layer。

6. 前端 Review Team 服务拆分。
   - 新建 `src/web-ui/src/shared/services/review-team/`。
   - 当前已落地模块：
     - `index.ts`
     - `types.ts`
     - `defaults.ts`
     - `strategy.ts`
     - `pathMetadata.ts`
     - `manifestMembers.ts`
     - `workPackets.ts`
     - `tokenBudget.ts`
     - `risk.ts`
     - `scopeProfile.ts`
     - `evidencePack.ts`
     - `cachePlan.ts`
     - `preReviewSummary.ts`
     - `promptBlock.ts`
   - `src/web-ui/src/shared/services/reviewTeamService.ts` 已保留为 facade，确保现有 import path 不变。
   - `index.ts` 保留 side-effectful config/backend loading、default team assembly 和 final manifest assembly；纯 helper 不得反向 import `index.ts`。

7. Flow Chat Deep Review 拆分。
   - 新建 `src/web-ui/src/flow_chat/deep-review/launch/`，承载 command parsing、target resolution、launch prompt、child-session launch、launch errors。
   - 新建 `src/web-ui/src/flow_chat/deep-review/action-bar/`，承载 queue notice、partial-results panel、recovery-plan preview、remediation controls、review action header。
   - 新建 `src/web-ui/src/flow_chat/deep-review/report/`，承载 reliability notices、manifest sections、markdown export。
   - `DeepReviewService.ts`、`DeepReviewActionBar.tsx`、`codeReviewReport.ts` 保留 public facade。

### 可执行计划拆分

| 子计划 | 目标 | 主要文件 | 行为变化 | 最小验证 | 退出标准 |
|---|---|---|---|---|---|
| M1-P0 Baseline guardrails | 记录基线并补齐重构前 regression | `docs/deep-review-next-executable-plan.zh-CN.md`、现有 Deep Review tests | 无 | `cargo test -p bitfun-core deep_review -- --nocapture`；focused web tests | 关键大文件行数、shared impact、non-DeepReview regression 缺口被记录。 |
| M1-P1 Backend constants/team extraction | 迁移 constants、role family、default team definition | `src/crates/core/src/agentic/deep_review/{mod.rs,constants.rs,team_definition.rs}`、`deep_review_policy.rs` | 无 | Rust Deep Review tests | `deep_review_policy.rs` facade re-export 生效；agent registry / reviewer visibility 不变。 |
| M1-P2 Manifest and execution policy extraction | 迁移 manifest parser、execution policy、strategy helpers | `deep_review/{manifest.rs,execution_policy.rs}`、`deep_review_policy.rs`、`task_tool.rs` imports | 无 | Rust policy / TaskTool focused tests | typed accessors 集中；JSON field name 不再散落新增。 |
| M1-P3 Queue/retry/cache/diagnostics extraction | 迁移 concurrency、queue、retry、diagnostics、shared context、incremental cache | `deep_review/{concurrency_policy.rs,queue.rs,retry.rs,diagnostics.rs,shared_context.rs,incremental_cache.rs}` | 无 | Rust Deep Review tests；queue/capacity tests | moved tests 随模块迁移；`pub(crate)` 优先，避免扩大 API。 |
| M1-P4 TaskTool adapter | 将 Deep Review TaskTool 分支收敛到 adapter | `deep_review/task_adapter.rs`、`task_tool.rs` | 无 | Deep Review TaskTool tests；普通 Task regression | `task_tool.rs` 只保留 context-gated adapter 调用；普通 Task 不进入 queue/retry/cache。 |
| M1-P5 CodeReviewTool report adapter | 隔离 Deep Review report enrichments | `deep_review/report.rs`、`code_review_tool.rs` | 无 | Deep Review report tests；标准 Code Review regression | 标准 Code Review 不出现 packet/cache/queue reliability signals。 |
| M1-P6 Event and tool-pipeline containment | 收敛 event conversion 与 duplicate measurement hook | `tool_pipeline.rs`、`tools/framework.rs`、`src/crates/events/src/agentic.rs` | 无 | event serialization test；tool pipeline non-DeepReview test | `DeepReviewQueueStateChanged` contract 稳定；measurement 仍 Deep Review-gated。 |
| M1-P7 Frontend review-team facade split | 已完成：拆分 `reviewTeamService.ts` 并抽离 review-team 纯 helper | `src/web-ui/src/shared/services/review-team/*`、`reviewTeamService.ts` | 无 | `reviewTeamService.test.ts`；`type-check:web` | old facade 只 re-export；pure helpers 无 API adapter 和反向 facade import。 |
| M1-P8 Flow Chat split | 已完成：拆分 launch、action bar、report helpers 与 action-bar remediation/diagnostics cleanup | `src/web-ui/src/flow_chat/deep-review/*`、`DeepReviewService.ts`、`DeepReviewActionBar.tsx`、`codeReviewReport.ts` | 无 | focused Flow Chat tests；lint；type-check | public exports 不变；标准 Code Review action bar 不受影响；主 action-bar 保持状态编排，不再承载 remediation list、action button row 或 diagnostics 文本生成。 |
| M1-P9 Ownership cleanup | 已完成：清理重复定义风险、补 Flow Chat/Rust 模块 ownership 说明、更新文档 | new modules、Deep Review docs、`agentic/deep_review/*` module docs、`agentic/subagent_runtime/*` | 无 | placeholder scan；focused web tests；`git diff --check` | ownership 清晰；source docs 只说明边界，不声称新行为。 |

#### M1 子计划执行卡片

**M1-P0 Baseline guardrails**

- 前置检查：确认当前分支和工作区状态，记录上述高风险文件行数。
- 改动范围：只允许更新计划文档、PR 说明草稿或新增缺失 regression test；不移动生产代码。
- 必须验证：运行 Deep Review focused Rust 测试和现有 web focused tests；如果无法运行，记录原因和替代静态检查。
- 禁止事项：不得在该子计划里开始模块拆分。

**M1-P1 Backend constants/team extraction**

- 前置检查：先用 `rg -n "DEEP_REVIEW_AGENT_TYPE|default_review_team_definition|ReviewTeamDefinition"` 确认引用点。
- 改动范围：只移动 constants、role family、default team definition 和相关纯数据结构。
- 必须验证：agent registry、review specialist agents、Deep Review focused Rust tests。
- 禁止事项：不得移动 queue、retry、cache、diagnostics 或 TaskTool 运行逻辑。

**M1-P2 Manifest and execution policy extraction**

- 前置检查：列出 `deep_review_run_manifest`、`workPackets`、`executionPolicy`、strategy helper 的读写点。
- 改动范围：只迁移 manifest typed accessors、execution policy、strategy helpers 和对应 tests。
- 必须验证：policy parsing tests、TaskTool manifest policy tests、Deep Review focused Rust tests。
- 禁止事项：不得改变 manifest JSON 字段名、timeout 计算结果或 strategy authority。

**M1-P3 Queue/retry/cache/diagnostics extraction**

- 前置检查：按 queue、retry、diagnostics、shared_context、incremental_cache 建立迁移清单。
- 内部提交顺序：
  - M1-P3a：抽离 `diagnostics.rs` 与只读 runtime diagnostics 数据结构，不迁移 budget tracker。
  - M1-P3b：抽离 `concurrency_policy.rs` 与 effective concurrency state，不迁移 provider queue control。
  - M1-P3c：抽离 `queue.rs` 的 capacity classifier、reviewer queue state、queue control tracker，不新增 short queue 行为。
  - M1-P3d：抽离 `shared_context.rs` 与 `incremental_cache.rs`，保持现有 cache fingerprint / packet key 语义。
  - M1-P3e：抽离 budget/retry admission helpers 到 `retry.rs` 或 `budget.rs`，旧 facade 只保留 compatibility wrapper。
- 改动范围：每次只移动一个内部 slice；优先 `pub(crate)`，只在现有调用需要时扩大可见性。
- 必须验证：capacity queue tests、retry admission tests、cache tests、diagnostics tests。
- 禁止事项：不得新增 provider short queue 行为，不得改变默认 concurrency/retry/cache 语义。

**M1-P4 TaskTool adapter**

- 前置检查：先补或确认普通 Task 不走 Deep Review queue/retry/cache 的 regression。
- 改动范围：将 Deep Review context detection、manifest/cache lookup、retry validation、queue/capacity calls 移到 `task_adapter.rs`。
- 必须验证：Deep Review TaskTool tests、普通 Task regression、Rust Deep Review focused tests。
- 禁止事项：不得改 `TaskTool` schema、tool name、普通 subagent execution path。

**M1-P5 CodeReviewTool report adapter**

- 前置检查：先补或确认标准 Code Review 不出现 Deep Review metadata 的 regression。
- 改动范围：将 packet fallback、reliability signals、runtime diagnostics、cache write-through 移到 `report.rs`。
- 必须验证：Deep Review report tests、标准 Code Review report regression。
- 禁止事项：不得改变 `submit_code_review` schema 对普通 Code Review 的默认值。

**M1-P6 Event and tool-pipeline containment**

- 前置检查：记录 `DeepReviewQueueStateChanged` 事件序列化快照和 tool pipeline context propagation 点。
- 改动范围：移动 event payload conversion helper 和 duplicate measurement hook；事件 enum 名称不变。
- 必须验证：`cargo test -p bitfun-events deep_review_queue_state_event_serializes_stable_contract -- --nocapture`、tool pipeline non-DeepReview regression。
- 禁止事项：不得引入 generic queue event，不得让 duplicate measurement 记录非 Deep Review tool call。

**M1-P7 Frontend review-team facade split**

- 前置检查：用 `rg -n "reviewTeamService"` 列出 import 点；先创建 `review-team/` 目录和 `index.ts`。
- 当前状态：`reviewTeamService.ts` 已成为 facade；`types/defaults/strategy/pathMetadata/manifestMembers/workPackets/tokenBudget/risk/scopeProfile/evidencePack/cachePlan/preReviewSummary/promptBlock` 已按责任拆出。
- 保留边界：`index.ts` 继续承载 config/backend loading、default team assembly 和 final manifest assembly；后续如果继续拆 side-effect adapter，必须保持相同 facade tests 且不改变行为。
- 必须验证：`reviewTeamService.test.ts`、`reviewTeamLocaleCompleteness.test.ts`、`pnpm run type-check:web`。
- 禁止事项：old facade 不得新增业务逻辑；pure helpers 不得反向 import `index.ts` 或调用 API adapter。

**M1-P8 Flow Chat split**

- 前置检查：先记录 `DeepReviewService.ts`、`DeepReviewActionBar.tsx`、`codeReviewReport.ts` 当前 public exports。
- 当前状态：old import paths 均保留为 facade；`launch/commandParser.ts`、`launch/targetResolver.ts`、`launch/launchPrompt.ts`、`launch/launchErrors.ts`、`action-bar/CapacityQueueNotice.tsx`、`action-bar/PartialResultsPanel.tsx`、`action-bar/RecoveryPlanPreview.tsx`、`action-bar/ReviewActionHeader.tsx`、`action-bar/RemediationSelectionPanel.tsx`、`action-bar/ReviewActionControls.tsx`、`action-bar/interruptionDiagnostics.ts`、`action-bar/actionBarFormatting.ts`、`report/manifestSections.ts`、`report/reliabilityNotices.ts`、`report/reportSections.ts`、`report/markdown.ts` 已落地。
- 改动范围：按 launch -> action-bar -> report 三组拆分；每组应保持独立可验证。当前低风险 action-bar cleanup 已完成，后续只有在新行为导致密度重新升高时才继续做 no-behavior component move。
- 必须验证：`DeepReviewService.test.ts`、`DeepReviewActionBar.test.tsx`、`codeReviewReport.test.ts`、lint、type-check。
- 禁止事项：不得改变 UI 文案、布局行为、standard review remediation flow。

**M1-P9 Ownership cleanup**

- 前置检查：确认 M1-P1 到 M1-P8 均已合入并验证。
- 当前状态：`src/web-ui/src/flow_chat/deep-review/README.md` 已补充 launch/action-bar/report ownership、facade 保留规则、Deep Review gating、隐私边界和 focused verification；Rust 侧已补 `deep_review` 与 `subagent_runtime` module docs，说明 generic runtime、Deep Review product policy、cache/diagnostics/privacy、report honesty 与 tool-context gating 的边界；placeholder scan 没有发现真实 TODO/TBD 残留，命中项仅为业务词 `temporary_overload` 和文档里的 duplicate measurement 表述。
- cleanup 拆解：
  - facade cleanup：确认旧 import path 只 re-export 或保留极薄 compatibility adapter，不新增业务逻辑。
  - Rust docs cleanup：给职责不明显的 module 加 `//!` 说明，标明哪些是 Deep Review policy、哪些是 generic runtime primitive。
  - duplicated wording cleanup：只清真实重复常量、重复导出或过时状态文案，不清业务词和测试语义。
  - ownership docs cleanup：同步 README / 架构文档中的 owner、禁止事项、最小验证和延期边界。
  - verification cleanup：执行 placeholder scan、diff check 和对应 focused tests，确保 cleanup 没有行为变化。
- 改动范围：删除重复导出、补模块级说明、更新 ownership 文档；不得把 cleanup 扩大成调度、retry、cache 或 UI 行为改动。
- 必须验证：placeholder scan、focused Rust/web tests、`git diff --check`。
- 禁止事项：不得借 cleanup 引入行为变化。

### 测试要求

最小 focused verification：

```powershell
cargo test -p bitfun-core deep_review -- --nocapture
cargo test -p bitfun-events deep_review_queue_state_event_serializes_stable_contract -- --nocapture
pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts src/flow_chat/services/DeepReviewService.test.ts src/flow_chat/components/btw/DeepReviewActionBar.test.tsx src/flow_chat/utils/codeReviewReport.test.ts src/flow_chat/utils/deepReviewQueueStateEvents.test.ts
pnpm run type-check:web
```

完成整个里程碑后运行：

```powershell
cargo check --workspace
cargo test --workspace
pnpm run lint:web
pnpm --dir src/web-ui run test:run
git diff --check
```

### 退出标准

- `deep_review_policy.rs` 不再承载大部分业务实现，只保留兼容导出或很薄的 facade。
- `TaskTool`、`CodeReviewTool`、`tool_pipeline.rs` 中 Deep Review 逻辑只以 context-gated hook / adapter 形式存在。
- `reviewTeamService.ts`、`DeepReviewService.ts`、`DeepReviewActionBar.tsx`、`codeReviewReport.ts` 的原 import path 继续可用。
- facade 文件只做 re-export 或极薄 compatibility adapter，不继续承载新业务逻辑。
- 至少有回归测试证明普通 Task 不进入 Deep Review queue/retry/cache；标准 Code Review 不出现 Deep Review-only packet/cache/queue reliability metadata。
- 用户可见行为没有变化。

## 里程碑 2：Provider 短队列与显式 Retry 功能补齐

**目标：** 补齐当前已设计但未实现的 Deep Review provider capacity queue 和 retry action，让不完整 reviewer slice 有可见、可控、可恢复的产品路径。该里程碑包含用户可见行为变化，必须按发布标准收口。

**发布价值：** 短暂 provider capacity 问题不再直接损失 reviewer；用户能看到队列状态、取消或继续等待，并能对结构化未完成 slice 发起显式 retry。自动 retry 仍默认关闭。

### 产品语义修正：Deep Review 队列活性

本里程碑后续优化必须先修正队列语义，再继续扩大 provider queue / retry 能力。产品结论如下：

- Deep Review reviewer queue 是同一次 Deep Review 内部的执行节奏控制，不是全局 subagent 公平调度器。
- `max_parallel_reviewers = 3` 的含义是“同一次 Deep Review 最多 3 个 reviewer 同时运行”，不表示当前主 chat、其他 session 或其他 session 内部 subagent 也要占用这 3 个名额。
- 当同一次 Deep Review 中已有任意 reviewer 正在运行时，后续 reviewer 的排队等待是有活性的：最差情况是正在运行的 reviewer 进入自身 `reviewer_timeout_seconds`、失败或被用户取消，然后释放队列。此时不应再用 60 秒这类固定短窗口跳过队列中的 reviewer。
- 队列超时只有在“当前没有任何 Deep Review reviewer 正在运行或可执行，但队列仍无法取得执行通路”时才有产品意义。该场景通常代表主运行时 subagent permit、provider gateway、模型配置或内部 guard 出现不可恢复阻塞，应转成明确错误和建议操作。
- Provider transient queue 与 Deep Review local reviewer queue 应拆成两个概念：provider queue 可以短时、有上限地等待 `Retry-After` / rate limit；local reviewer queue 应按 Deep Review 活性等待，不把每个排队 reviewer 当成独立的一分钟倒计时。
- UI 必须展示阻塞原因、正在运行的 reviewer、排队 reviewer 列表、已等待时间、以及可操作建议。若 queue expiry 发生，错误提示必须说明“当前没有可执行 Deep Review 通路”，而不是泛化成“审核员排队超时”。

### 关键步骤

1. 实现 provider transient capacity classifier。
   - queueable：
     - provider rate limit
     - provider concurrency limit
     - explicit `Retry-After`
     - temporary overload / capacity pressure
   - fail fast：
     - authentication
     - billing / quota exhaustion
     - invalid model
     - policy violation
     - user cancellation
     - invalid reviewer tooling
     - deterministic validation error

2. 拆分 Deep Review local queue 与 provider transient queue。
   - Local reviewer queue：只按同一次 Deep Review 内部 active reviewer / launch batch / judge sequencing 判断是否可运行；不统计普通主流程 subagent、其他 session 或其他 session 内部 subagent。
   - Local reviewer queue：只要同一次 Deep Review 仍有 active reviewer，排队 reviewer 应持续等待，并在 active reviewer 释放后继续尝试。
   - Local reviewer queue：只有 `active_reviewer_count == 0` 且连续无法取得 Deep Review 可执行通路时，才进入整体队列超时 / fail-fast 分支。
   - Provider transient queue：等待上限仍可为 `min(Retry-After, provider_queue_wait_seconds)`，用于处理 rate limit、provider concurrency limit、temporary overload。
   - Queue wait time 与 reviewer runtime timeout 分离；排队耗时不消耗 reviewer active runtime。
   - 用户未 pause / cancel 时才发起一次 provider reattempt。
   - 复用现有 Deep Review queue-state event contract，但 event payload 必须足以区分 `local_waiting_for_reviewer`、`provider_waiting_for_capacity`、`no_executable_path_timeout`。
   - provider queue 仍然只作用于 Deep Review reviewer execution。

3. 补齐 runtime diagnostics 与 report reliability。
   - 记录 provider queue count。
   - 记录 provider retry count。
   - 记录 provider retry success count。
   - 记录 final capacity skip count。
   - report 中保留 capacity skip / retry / queue expiry 的诚实可靠性提示。

4. 补齐前端 queue notice。
   - 在 action bar 中显示 provider queue reason、elapsed wait、pause / continue / cancel。
   - reason 文案必须覆盖 `en-US`、`zh-CN`、`zh-TW`。
   - notice 保持 compact，不新增大 modal。

5. 实现 explicit retry action。
   - 从 report metadata 中提取 retryable unresolved slices：
     - source packet id
     - reviewer id
     - source status
     - covered files
     - unresolved files
     - retry timeout
   - retryable source status：
     - `partial_timeout`
     - transient `capacity_skipped`
   - non-retryable source status：
     - auth
     - quota / billing
     - invalid model
     - policy
     - invalid tooling
     - validation
     - cancellation
     - non-transient capacity skip

6. 实现 opt-in bounded auto retry preference。
   - 默认关闭。
   - 在 Review Team settings 中持久化。
   - 保持当前保守默认值时必须区分语义：`max_parallel_reviewers = 4` 只约束同一次 Deep Review 内部 reviewer；`max_queue_wait_seconds = 60` 若继续存在，只能作为 provider transient queue 的短等待上限，不能作为 local reviewer queue 的逐元素超时。
   - `allow_provider_capacity_queue` 可以作为策略开关存在，但 provider runtime queue 必须仍受可见、有上限、可取消的行为约束。
   - Review Team capacity / retry settings 不得改变全局 `ai.subagent_max_concurrency`，也不得把全局或跨 session subagent 活跃数计入 Deep Review reviewer cap。
   - 只有满足全部条件才允许自动 retry：
     - 用户已启用 preference。
     - source status 可 retry。
     - retry coverage 结构化。
     - retry scope 非空且小于原 scope。
     - role / packet budget 未耗尽。
     - elapsed guard 未超过。
     - retry timeout 低于 source task timeout。
   - suppression reason 使用稳定枚举：
     - `preference_disabled`
     - `budget_exhausted`
     - `scope_not_reduced`
     - `elapsed_guard_exceeded`
     - `non_retryable_status`
     - `non_transient_error`
     - `missing_coverage`

### 可执行计划拆分

| 子计划 | 目标 | 主要文件 | 行为变化 | 最小验证 | 退出标准 |
|---|---|---|---|---|---|
| M2-P0 Queue liveness semantics | 已完成：修正 Deep Review local queue 活性语义，取消“每个排队 reviewer 固定短超时”的产品口径 | `deep_review/task_adapter.rs`、`task_tool.rs` queue tests、action-bar queue helpers | 是，排队等待更符合 Deep Review 完整性 | Rust queue liveness tests；action bar queue state tests | active reviewer 存在时排队 reviewer 不因固定短窗口被跳过；active reviewer 释放后继续 admission；无 active reviewer 且无可执行通路才触发 queue-expired。 |
| M2-P1 Provider classifier and diagnostics | 收窄 provider transient classifier，补齐 diagnostics counters | `deep_review/queue.rs` 或当前 `deep_review_policy.rs`、`task_tool.rs` tests | 小，错误分类更明确 | Rust tests for queueable / fail-fast errors | auth/quota/model/policy/cancel/validation fail fast；rate/concurrency/retry-after/overload queueable。 |
| M2-P2 Provider transient queue runtime | 实现 provider capacity wait + one reattempt，并与 local reviewer queue 分离 | `deep_review/queue.rs`、`task_adapter.rs` / `task_tool.rs` | 是，Deep Review reviewer 失败恢复改变 | Rust provider retry-after/success/expiry/pause/cancel tests | provider queue 有短上限；local queue 活性等待；queue wait 不计入 runtime timeout；普通 subagent 不受影响。 |
| M2-P3 Queue event and action-bar reason surface | 展示 provider queue reason、elapsed wait、controls | `DeepReviewActionBar.tsx` 或 split components、`deepReviewQueueStateEvents.ts`、locales | 是，用户可见 queue notice | action bar tests；queue event tests；locale completeness | pause/continue/cancel 可用；provider reason localized；不新增大 modal。 |
| M2-P4 Manual retry extraction and launch | 已完成：从 report metadata 提取 retryable unresolved slices，增加显式 retry action | `codeReviewReport.ts`、`DeepReviewActionBar.tsx`、action-bar store、locales | 是，新增手动 retry | report parser tests；action-bar tests；locale completeness | 只对 `partial_timeout` / transient `capacity_skipped` 启用；非 retryable source 禁用且有原因。 |
| M2-P5 Bounded auto retry preference | 已完成：实现 opt-in 设置读取、suppression reasons、runtime guards；未实现后台自动 redispatch scheduler | `reviewTeamService.ts` / `review-team/config.ts`、`task_tool.rs`、`deep_review/concurrency_policy.rs`、`budget.rs` | 是，但默认关闭 | Rust retry admission tests；settings tests；locale tests | 默认 false；预算、scope、elapsed guard、lower timeout 全部生效；不会循环。 |
| M2-P6 Reliability and docs close | 已完成：更新 report reliability、文档状态和 non-DeepReview inventory | Deep Review docs | 无新增行为 | web report/settings tests；stale claim scan；最终 release gate 已执行 | final report 对 queue/retry 诚实；源文档只在验证后标记 implemented。 |

M2 的实现顺序必须固定为 P0 -> P1 -> P2 -> P3 -> P4 -> P5 -> P6。不要在 P2 同时做 manual retry，也不要在 P4 顺手启用 auto retry。

#### M2 子计划执行卡片

**M2-P0 Queue liveness semantics**

- 产品结论：这是 P0，且已在本 PR 收口；固定短窗口不能代表 Deep Review 内部 reviewer 排队的完整性承诺。
- 前置检查：已确认当前 Deep Review active reviewer 计数按 `parent_dialog_turn_id` 隔离；`deepReviewCapacityGuard.ts` 仍只作为启动前提示，不成为阻塞 Deep Review 的 hard gate。
- 改动范围：Deep Review local queue admission / timeout 判断；既有 queue event reason、action bar 展示和 Review Team settings 文案保持可解释，不额外扩张 UI surface。
- 必须验证：
  - `max_parallel_instances = 1` 时，第二个 reviewer 在第一个 reviewer active 期间持续等待；第一个 reviewer 释放后第二个 reviewer 进入 running。
  - `launch_batch_blocked` 场景中，只要前序 batch 仍有 active reviewer，后续 batch 不因固定 60 秒窗口跳过。
  - `active_reviewer_count == 0` 且无法取得 Deep Review 可执行通路时，队列进入 `queue_expired` 或等价稳定原因，并给出可操作错误。
  - 当前主 session 已有普通 Task subagent、其他 session 有 subagent、或其他 session 内部有多个 subagent 时，不计入 Deep Review reviewer cap。
  - 普通 Task / 标准 Code Review 不获得 Deep Review queue 行为。
- 禁止事项：不得把 Deep Review queue 变成全局 subagent queue；不得取消 reviewer runtime timeout；不得让队列无限等待而没有 watchdog / no-executable-path 诊断。

**M2-P1 Provider classifier and diagnostics**

- 前置检查：确认当前 provider capacity failures 只会转为 `capacity_skipped`，还没有 runtime short queue。
- 改动范围：只调整 provider error classifier、diagnostics counter 数据结构和对应 tests。
- 必须验证：queueable/fail-fast classifier tests、diagnostics snapshot tests。
- 禁止事项：不得启动 reattempt，不得改前端 action bar。

**M2-P2 Provider transient queue runtime**

- 前置检查：先完成 M2-P0，确保 local reviewer queue 和 provider transient queue 已拆分。
- 改动范围：只实现 Deep Review reviewer 的 provider queue wait 和一次 reattempt。
- 必须验证：`Retry-After`、provider queue expiry、pause/cancel、one reattempt；queue wait 不进入 reviewer runtime timeout；retry 后成功更新 diagnostics；retry 后失败进入 `capacity_skipped`。
- 禁止事项：不得让 provider queue 对普通 subagent 生效；不得超过一次自动 reattempt；不得把 provider queue 的短上限套到 local reviewer queue。

**M2-P3 Queue event and action-bar reason surface**

- 前置检查：确认 transport adapters 和 `AgentAPI.ts` 已能传递 queue reason / elapsed / max wait。
- 改动范围：只补 provider reason UI、elapsed wait、pause/continue/cancel 显示和 locale。
- 必须验证：`DeepReviewActionBar.test.tsx`、`deepReviewQueueStateEvents.test.ts`、locale completeness。
- 禁止事项：不得新增大 modal 或新页面；不得改变 remediation controls。

**M2-P4 Manual retry extraction and launch**

- 前置检查：先定义 retryable unresolved slice parser 的输入/输出，并写 parser tests。
- 改动范围：report parser、action bar explicit retry button、action-bar state、retry launch prompt metadata。
- 必须验证：retryable/non-retryable parser tests、button disabled-state tests、retry launch metadata tests。
- 禁止事项：不得启用 auto retry；不得 retry 非结构化 slice。

**M2-P5 Bounded auto retry preference**

- 前置检查：确认 manual retry 已完成，并确认 Review Team settings 默认值仍是 4/60/false/180。
- 改动范围：settings opt-in、runtime admission guard、suppression reason reporting。
- 必须验证：settings tests、Rust retry admission/suppression tests、locale tests。
- 禁止事项：不得改变默认值；不得绕过 reduced scope、budget、elapsed guard、lower timeout。
- 当前边界：`auto_retry` 只是 backend-owned retry caller 的 admission guard；后台自动 redispatch scheduler 仍未实现。

**M2-P6 Reliability and docs close**

- 前置检查：确认 P1-P5 行为都已经通过 focused tests。
- 改动范围：report reliability wording、status docs、non-DeepReview impact inventory。
- 必须验证：Rust report tests、web report tests、stale claim scan、`git diff --check`。
- 禁止事项：不得把 project-level cache、global subagent queue、backend DAG scheduler 标记为完成。

### 测试要求

Rust：

```powershell
cargo test -p bitfun-core deep_review -- --nocapture
```

需要覆盖：

- queueable 与 non-queueable provider errors。
- local queue liveness：active reviewer 存在时排队 reviewer 持续等待；active reviewer 释放后排队 reviewer 继续运行。
- no-executable-path expiry：没有任何 active reviewer 且仍无法取得 Deep Review 可执行通路时，整体队列超时并给出稳定 reason。
- provider queue expiry、queue success、pause、continue、cancel。
- provider retry success 与最终 `capacity_skipped`。
- retry admission、suppression reasons、budget guards。
- 主 session / 其他 session / 其他普通 subagent 不计入 Deep Review reviewer cap。
- non-DeepReview Task 不进入 provider queue 或 local reviewer queue。

Frontend：

```powershell
pnpm --dir src/web-ui run test:run -- src/flow_chat/components/btw/DeepReviewActionBar.test.tsx src/flow_chat/services/DeepReviewService.test.ts src/flow_chat/utils/codeReviewReport.test.ts src/flow_chat/utils/deepReviewQueueStateEvents.test.ts src/shared/services/reviewTeamLocaleCompleteness.test.ts
pnpm run lint:web
pnpm run type-check:web
```

### 退出标准

- Provider queue 可见、短时、有上限、可 pause / continue / cancel。
- Local reviewer queue 可见、有活性、可 pause / continue / cancel；active reviewer 存在时不因固定短等待窗口跳过排队 reviewer。
- 只有当前 Deep Review 没有任何 active reviewer 且仍无法取得可执行通路时，才允许整体队列超时，并必须给出明确错误原因与建议操作。
- 非瞬态错误 fail fast，不进入等待。
- Retry action 只对结构化 unresolved slice 启用。
- Automatic retry 默认关闭；启用后也不能无限循环。
- 当前主 session、多 session 或普通 subagent 的并发不影响 Deep Review 内部 reviewer cap；相关拥塞只作为失败诊断和用户提示，不作为 Deep Review 入队阻断条件。
- Final report 对 retry、capacity skip、queue expiry 的状态表述不夸大覆盖率。
- 普通 subagent 和标准 Code Review 行为不变。

## 里程碑 3：Cost-Aware Scope 与 Shared Evidence Pack

**目标：** 降低大变更或慢模型下的 Deep Review 时间和 token 成本，同时保持 coverage metadata 透明。该里程碑是体验与成本优化，但必须通过 schema、prompt、report 和 diagnostics 的一致性测试后才能发布。

**发布价值：** `quick` 更快且明确是 high-risk-only；`normal` 做 risk-expanded；`deep` 保持 full-depth。Reviewer 启动时获得 compact shared evidence，减少重复发现成本，但不共享完整源码或完整 diff。

### 关键步骤

1. 增加 scope profile schema。

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

2. 建立 strategy mapping。

| Strategy | Review depth | Dependency context | Optional reviewers | Exploration |
|---|---|---|---|---|
| `quick` | `high_risk_only` | changed files + direct high-risk contracts | risk-matched only | broad exploration off |
| `normal` | `risk_expanded` | changed files + one-hop high-risk context | configured but applicability-gated | limited |
| `deep` | `full_depth` | policy-limited broad context | configured / full behavior | allowed |

3. 固化 high-risk categories。
   - security
   - data loss
   - migrations
   - authentication / authorization
   - cross-boundary API contracts
   - concurrency
   - persistence
   - configuration changes
   - platform boundary violations

4. 更新 manifest builder 与 Rust manifest parser。
   - TypeScript 负责 UX defaults、manifest construction、prompt block。
   - Rust 负责 runtime enforcement、queue safety、retry admission、final trust boundary。
   - JSON field name 必须集中在 builder / parser 中，不允许散落 hand-read。

5. 更新 reviewer 和 judge prompt。
   - Reviewer 必须理解 reduced-depth 不是 full coverage。
   - Judge 必须保留 `reviewDepth` 和 `coverageExpectation`，不能把 quick / normal 报告写成 full-depth。

6. 增加 report reliability 与 markdown export 表达。
   - reduced-depth 以 collapsed / compact 形式展示。
   - 所有 changed files 仍在 coverage metadata 中可见。
   - skipped optional reviewer 必须有 risk-match / applicability reason。

7. 实现 metadata-first shared evidence pack。

```ts
type DeepReviewEvidencePack = {
  version: 1;
  changedFiles: string[];
  diffStat: {
    fileCount: number;
    totalChangedLines: number;
  };
  domainTags: string[];
  riskFocusTags: string[];
  packetIds: string[];
  hunkHints: Array<{
    filePath: string;
    changedLineCount: number;
  }>;
  contractHints: Array<{
    kind: 'i18n_key' | 'tauri_command' | 'api_contract' | 'config_key';
    value: string;
    filePath: string;
    source: 'manifest' | 'target_classifier' | 'cheap_static_hint';
  }>;
};
```

8. Evidence pack 隐私与成本约束。
   - 必须包含 schema version，首版固定为 `version: 1`。
   - 必须设置最大字节数和每类 hint 的最大条目数；超过上限时只截断 hints，不隐藏 changed files coverage。
   - 不包含 full source text。
   - 不包含 full diff text。
   - 不包含 reviewer output。
   - 不包含 provider raw response body。
   - 不包含 full tool result cache。
   - 只使用 changed files、domain tags、risk tags、packet ids、hunk hints、cheap contract hints。
   - Evidence pack 必须从同一份 launch manifest 输入派生，并用 source label 标记 hint 来源。
   - 如果 hints 可能陈旧，reviewer prompt 必须要求通过 `Read` / `GetFileDiff` 确认，而不能把 hint 当成事实。
   - `Read` 和 `GetFileDiff` 仍由 reviewer 按需调用确认。

9. 用 diagnostics 评估收益。
   - 继续使用 content-free duplicate tool-use measurement。
   - 在真实运行中比较 shared evidence pack 前后的重复读取趋势。
   - 只有测量证明必要且另有产品/隐私计划时，才考虑 programmatic full tool-result cache。

### 可执行计划拆分

| 子计划 | 目标 | 主要文件 | 行为变化 | 最小验证 | 退出标准 |
|---|---|---|---|---|---|
| M3-P1 Scope profile schema | 定义 `DeepReviewScopeProfile`、strategy mapping、manifest 字段 | `reviewTeamService.ts` / `review-team/types.ts`、`manifestBuilder.ts` | 是，manifest 增字段 | `reviewTeamService.test.ts`；type-check | quick/normal/deep profile 生成稳定；changed files coverage 不丢。 |
| M3-P2 Rust parser and report reliability | Rust 解析 scope profile 并生成 reduced-depth reliability signals | `deep_review/manifest.rs`、`deep_review/report.rs`、`code_review_tool.rs` | 是，report metadata 增强 | Rust Deep Review schema/report tests | quick/normal 不声称 full coverage；旧 manifest 兼容。 |
| M3-P3 Prompt alignment | 更新 orchestrator/reviewer/judge prompt，要求 reduced-depth 诚实表达 | `src/crates/core/src/agentic/agents/prompts/*`、frontend prompt block | 是，模型行为约束改变 | prompt snapshot-light assertions 或 focused prompt tests | reviewer/judge 都看到 `reviewDepth` / `coverageExpectation`；不隐藏 skipped / reduced files。 |
| M3-P4 Frontend report and launch UX | 在 launch summary/report/export 中表达 reduced-depth 和 deepen path | `DeepReviewConsentDialog.tsx`、`codeReviewReport.ts`、locales | 是，用户可见 | component/report tests；locale completeness | quick 显示 high-risk-only；normal 显示 risk-expanded；不新增大 modal。 |
| M3-P5 Evidence pack schema and builder | 实现 metadata-first `DeepReviewEvidencePack` v1 | `review-team/manifestBuilder.ts` 或 `reviewTeamService.ts`、`targetClassifier.ts` | 是，manifest 增 evidence pack | web manifest tests；privacy assertions | pack 有 version/size budget/source label；不含 source/diff/model output。 |
| M3-P6 Evidence pack Rust validation and prompt injection | Rust parser 校验 evidence pack，prompt 指导 reviewer 从 evidence 起步 | `deep_review/manifest.rs`、`task_adapter.rs`、prompts | 是，reviewer 上下文改变 | Rust parser tests；Deep Review prompt tests | stale hints 必须要求工具确认；full tool-result cache 仍未实现。 |
| M3-P7 Diagnostics comparison | 保留 duplicate measurement 并增加可观测收益说明 | `deep_review/diagnostics.rs`、`shared_context.rs`、report utilities | 小，diagnostics/report 增说明 | diagnostics tests；report tests | duplicate discovery 可观测；不记录内容。 |

M3 不允许把现有 `shared_context_cache` 直接改名为 evidence pack。旧 cache/prompt guidance 可以继续存在，但新 evidence pack 必须是独立、metadata-first、可验证的 manifest 字段。

#### M3 子计划执行卡片

**M3-P1 Scope profile schema**

- 前置检查：确认当前代码没有 `reviewDepth` / `DeepReviewScopeProfile` / `coverageExpectation` 字段。
- 改动范围：TypeScript types、strategy mapping、manifest builder 输出。
- 必须验证：quick/normal/deep manifest tests、changed-file coverage metadata tests、type-check。
- 禁止事项：不得改变 reviewer roster 以外的 queue/retry behavior。

**M3-P2 Rust parser and report reliability**

- 前置检查：先写旧 manifest 兼容测试和 reduced-depth reliability 测试。
- 改动范围：Rust manifest parser、report reliability signal generation。
- 必须验证：Rust schema/report tests、Deep Review focused tests。
- 禁止事项：不得因为缺少 scope profile 拒绝旧 Deep Review manifest。

**M3-P3 Prompt alignment**

- 前置检查：列出 DeepReview orchestrator、reviewer、judge prompt 文件和 frontend prompt block 注入点。
- 改动范围：只改 prompt / prompt block，使模型理解 reduced-depth 和 coverage expectation。
- 必须验证：prompt block tests 或 snapshot-light assertions。
- 禁止事项：不得把 quick 说成 full review；不得隐藏 skipped / reduced files。

**M3-P4 Frontend report and launch UX**

- 前置检查：确认 launch consent、report notice、markdown export 的现有文案和 locale keys。
- 改动范围：compact launch summary、report reliability notice、markdown export wording、deepen path copy。
- 必须验证：component tests、report tests、locale completeness。
- 禁止事项：不得新增大 modal 或展示 dense token/cost internals。

**M3-P5 Evidence pack schema and builder**

- 前置检查：先写 privacy assertion，证明 evidence pack 不包含 source/diff/model output。
- 改动范围：new evidence pack builder、manifest field、version/size budget/source labels。
- 必须验证：web manifest tests、privacy assertions、type-check。
- 禁止事项：不得复用 full `Read` / `GetFileDiff` output；不得从 coverage metadata 移除 changed files。

**M3-P6 Evidence pack Rust validation and prompt injection**

- 前置检查：写 parser tests 覆盖 version、size limit、source label、stale hint confirmation rule。
- 改动范围：Rust parser、prompt injection、reviewer task context。
- 必须验证：Rust parser tests、Deep Review prompt tests、focused Deep Review tests。
- 禁止事项：不得实现 programmatic full tool-result cache。

**M3-P7 Diagnostics comparison**

- 前置检查：确认 existing duplicate measurement 仍 content-free。
- 改动范围：diagnostics aggregate、report/export compact summary、docs note。
- 必须验证：diagnostics tests、report tests、privacy scan。
- 禁止事项：不得记录 file contents、diff text、tool result body 或 provider body。
- 实现细化：runtime diagnostics 只允许记录聚合计数，例如 total duplicate discovery calls、duplicate context count、可节省重复读取候选数；不得记录重复读取的内容或 tool result body。
- 实现细化：report/export 只允许展示 evidence pack 的 source、privacy boundary、changed file / hunk hint / contract hint / packet id 计数和 omitted metadata 计数；不得展开 `privacy.excludes`、source text、full diff、reviewer output 或 provider raw body。
- 验收口径：M3-P7 完成后，应能在不读取任何内容型 payload 的情况下回答“evidence pack 是否减少了重复 discovery 的候选空间”，但不能声称已经实现跨 subagent 的 full tool-result cache。

### 测试要求

Rust：

```powershell
cargo test -p bitfun-core deep_review -- --nocapture
```

需要覆盖：

- 三种 scope profile 的 manifest parsing。
- reduced-depth 仍保留 changed-file coverage metadata。
- evidence pack 不存储 full source / full diff / model output。
- report schema 与已有 Deep Review submission 兼容。

Frontend：

```powershell
pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts src/flow_chat/services/DeepReviewService.test.ts src/flow_chat/utils/codeReviewReport.test.ts src/shared/services/reviewTeamLocaleCompleteness.test.ts
pnpm run lint:web
pnpm run type-check:web
```

### 退出标准

- `quick` 明确为 high-risk-only。
- `normal` 明确为 risk-expanded。
- `deep` 保持 full-depth。
- Report 不把 reduced-depth 误写成 full coverage。
- Evidence pack compact、metadata-first、source-agnostic、content-free。
- Duplicate discovery 成本应下降或至少可被 diagnostics 观测。

## 里程碑 4：发布收口、文档对齐与长期延期边界

**目标：** 将前 3 个里程碑的实现收口到发布标准，并同步状态文档，避免文档声称超过代码行为。该里程碑可以作为最终 release hardening，也可以在每个功能里程碑末尾重复执行其门禁。

**发布价值：** 用户看到的是一致、可解释、可恢复的 Deep Review；维护者看到的是清晰模块边界、明确 deferred items 和完整回归矩阵。

### 关键步骤

1. 状态文档 reconciliation。
   - `docs/deep-review-completed-status.md` 只标记已实现且已验证的行为。
   - `docs/deep-review-pending-plan.md` 移除已完成项或改为后续优化项。
   - `docs/deep-review-architecture-refactor-plan.md` 更新模块 ownership 和 facade 状态。
   - `docs/deep-review-nondeepreview-impact-inventory.md` 记录 touched shared files、行为是否变化、对应 regression test。

2. 文案与 i18n 收口。
   - 所有新用户可见字符串覆盖 `en-US`、`zh-CN`、`zh-TW`。
   - Queue / retry / reduced-depth / evidence-pack 相关文案保持 compact。
   - 不展示实现内部细节，除非用户需要据此做选择。

3. 隐私与日志审计。
   - 后端日志 English-only 且无 emojis。
   - Frontend logging 使用既有 logger 约定。
   - Diagnostics 不包含 source、diff、reviewer output、provider raw body、full file contents。
   - Evidence pack 和 report export 中的 reliability details 默认 compact 或 collapsed。

4. Non-DeepReview regression matrix。
   - 普通 Task 无 `deep_review_run_manifest` 时不应用 Deep Review queue controls。
   - 普通 Task retry 不要求 Deep Review `retry_coverage`。
   - 标准 Code Review submission 不出现 Deep Review packet/cache/queue metadata。
   - Deep Review queue event serialization 维持现有 stable shape。
   - Tool pipeline duplicate-read measurement 忽略 non-DeepReview `Read` / `GetFileDiff`。
   - 标准 Code Review action bar 不显示 Deep Review queue controls。
   - Deep Review queue controls 只在 Deep Review state 下渲染。
   - 标准 Code Review markdown export 不包含 Deep Review manifest/cache sections。
   - Review settings 文案区分 Review Team max reviewers 与全局 subagent concurrency。

5. 发布门禁。

```powershell
rg -n "project-level cache.*implement[[:alpha:]]*ed|auto retry.*compl[[:alpha:]]*ete|provider/adaptive queue.*compl[[:alpha:]]*ete|hard prompt.*compl[[:alpha:]]*ete|global.*concurrency.*auto[[:alpha:]]*matic" docs/deep-review-design.md docs/deep-review-phase2-plan.md docs/deep-review-phase2-addendum.md docs/deep-review-phase3-followup-plan.md docs/deep-review-pending-plan.md
cargo test -p bitfun-core deep_review -- --nocapture
cargo test -p bitfun-events deep_review_queue_state_event_serializes_stable_contract -- --nocapture
cargo check --workspace
cargo test --workspace
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/web-ui run test:run
git diff --check
```

如果触碰 desktop integration、Tauri command、browser/computer-use 或 desktop-only 行为，还必须补跑：

```powershell
cargo check -p bitfun-desktop
cargo test -p bitfun-desktop
```

如果覆盖桌面 smoke / functional flow，还应按 `AGENTS.md` 要求执行最近的 E2E spec 或：

```powershell
pnpm run e2e:test:l0
```

### 可执行计划拆分

| 子计划 | 目标 | 主要文件 | 行为变化 | 最小验证 | 退出标准 |
|---|---|---|---|---|---|
| M4-P1 Status wording reconciliation | 对齐 implemented / guarded / pending / deferred 状态 | `docs/deep-review-completed-status.md`、`docs/deep-review-pending-plan.md` | 无 | stale claim `rg` scan | 文档声明不超过代码行为。 |
| M4-P2 Architecture ownership update | 更新 ownership、facade、module layout 状态 | `docs/deep-review-architecture-refactor-plan.md`、本文 | 无 | docs diff review；focused tests list remains current | ownership 与实际文件树一致。 |
| M4-P3 Non-DeepReview impact inventory update | 记录 touched shared files 与 regression evidence | `docs/deep-review-nondeepreview-impact-inventory.md` | 无 | listed regression commands | 每个 shared file 都有对应 non-DeepReview regression。 |
| M4-P4 I18n and UX audit | 检查 queue/retry/reduced-depth/evidence 文案 | `src/web-ui/src/locales/*`、相关 components/tests | 无 | locale completeness tests；lint/type-check | 新用户文案三语覆盖，界面不新增未批准 modal/page。 |
| M4-P5 Privacy/log/export audit | 检查 diagnostics/logs/evidence/export 内容边界 | Rust diagnostics/report、frontend report/export utilities | 无 | privacy assertions；report tests；log grep | 无 source/diff/output/raw body/full file contents 泄漏。 |
| M4-P6 Final release gate | 执行最终命令矩阵并记录结果 | docs / PR description | 无 | 本节发布门禁全部命令 | 全部命令通过；失败项必须在同一里程碑内修复或回滚。 |

M4 可以在每个功能里程碑末尾部分执行，但最终发布前必须完整执行 P1 到 P6。不要把 M4 当成“只改文档”的收尾，它是发布阻断项清零阶段。

#### M4 子计划执行卡片

**M4-P1 Status wording reconciliation**

- 前置检查：对比代码、测试结果和 `docs/deep-review-completed-status.md`。
- 改动范围：只更新 implemented / guarded / pending / deferred 状态。
- 必须验证：stale claim scan。
- 禁止事项：不得在验证前把 pending 功能标记为 implemented。

**M4-P2 Architecture ownership update**

- 前置检查：列出实际新模块树、facade 文件和仍保留的 shared hooks。
- 改动范围：更新 architecture refactor docs 和本文 ownership 描述。
- 必须验证：docs diff review、focused test list 和实际文件路径一致。
- 禁止事项：不得声明已经不存在但代码仍在使用的 facade。

**M4-P3 Non-DeepReview impact inventory update**

- 前置检查：从 git diff 列出 touched shared files。
- 改动范围：只更新 shared area、行为变化与 regression evidence。
- 必须验证：每个 touched shared file 至少对应一个 regression command。
- 禁止事项：不得遗漏 `TaskTool`、`CodeReviewTool`、tool pipeline、event、action bar、report utilities、settings。

**M4-P4 I18n and UX audit**

- 前置检查：列出新增 locale keys 和 UI surfaces。
- 改动范围：locale files、compact wording、existing surface alignment。
- 必须验证：locale completeness tests、lint、type-check。
- 禁止事项：不得新增未批准 modal/page；不得显示实现内部细节。

**M4-P5 Privacy/log/export audit**

- 前置检查：搜索 diagnostics/log/export/evidence pack 输出路径。
- 改动范围：privacy assertions、log wording、export filtering。
- 必须验证：privacy tests、report/export tests、log grep。
- 禁止事项：不得输出 source text、full diff、reviewer output、provider raw body、full file contents。

**M4-P6 Final release gate**

- 前置检查：确认 M4-P1 到 M4-P5 已完成。
- 改动范围：只修复 release-gate failures；失败无法快速修复时回滚对应子计划。
- 必须验证：本节完整发布门禁。
- 禁止事项：不得跳过失败命令，不得以“文档已更新”替代代码验证。

### 退出标准

- 四份源文档与代码行为一致。
- 没有 stale completion claim。
- 没有新用户可见字符串缺失 locale。
- 没有 Deep Review-only 行为泄露到普通 Task、标准 Code Review 或普通 action bar。
- 所有新 queue / retry / scope / evidence 行为都有可复现测试。
- 所有 deferred product decisions 仍明确延期，没有被实现为隐式行为。
- 发布阻断项必须为零：
  - Deep Review-only queue / retry / packet / cache 信号泄露到标准 Code Review。
  - Diagnostics、logs、evidence pack 或 export 中出现 source text、full diff、reviewer output、provider raw body 或 full file contents。
  - queue / retry 新文案缺少任一 locale。
  - provider queue 对普通 subagent 生效。
  - reduced-depth report 声称 full coverage。

## 延期事项

以下事项不属于本 PR 阶段 1 的实现范围。每项都必须独立形成产品/架构决策，不能作为 cleanup 或 refactor 的顺手行为合入。

| 延期项 | 当前边界 | 后续收益 | 主要风险 | 需要单独批准的内容 |
|---|---|---|---|---|
| Backend-owned automatic retry redispatch scheduler | 当前 `auto_retry` 只是 admission guard；没有后台自动 caller、排队器或重新派发循环。 | 可自动恢复短暂 capacity / timeout 导致的未完成 slice，减少用户手动重试成本。 | 隐式自动化削弱用户控制；重试循环、成本失控、过期上下文、重复 report 状态、与 pause/cancel 冲突。 | 触发时机、用户可见状态、取消/暂停、预算、最大轮次、失败归因、report 合并规则。 |
| Project-level review cache | 只保留 per-session cache。 | 跨会话重复审查更快，适合大型项目反复迭代。 | stale finding、隐私/安全发现持久化、删除权、模型/roster/策略变化导致误复用。 | retention、invalidation、deletion、user visibility、privacy review。 |
| Programmatic full tool-result cache | 只做 prompt-level reuse guidance 和 content-free duplicate measurement。 | 显著减少重复 `Read` / `GetFileDiff` 调用和 token/IO 成本。 | 复用陈旧文件内容、泄漏 source/diff、缓存膨胀、工具语义被绕过。 | full result reuse 语义、权限/隐私、失效策略、内容大小上限、审计方式。 |
| Hard prompt-byte clipping | 只做 heuristic estimate、summary-first metadata、file split / max-file guardrails。 | 对超大 diff 有硬成本上限，降低 provider timeout 风险。 | 隐藏 coverage 缺口、遗漏关键文件、用户误以为 full review、prompt/report 不一致。 | clipped / omitted 文件如何在 coverage metadata、prompt、report、export 中诚实表达。 |
| Backend DAG scheduler | 保持 prompt-driven orchestration + TaskTool guardrails。 | 编排更确定，可统一 queue、retry、失败恢复和可观测性。 | 大规模架构迁移；模型灵活性下降；中断/恢复/用户控制复杂；旧 session 兼容难。 | 整体编排模型、失败恢复、用户控制、迁移策略和兼容策略。 |
| Authoritative runtime strategy selection | backend scoring 只做 advisory / mismatch warning。 | 自动选择更合适成本/深度，减少用户配置错误。 | 覆盖用户意图、评分不可解释、不同项目误判、测试矩阵扩大。 | 复杂度度量、策略覆盖权、用户 override 规则、解释 UI 和回退策略。 |
| Generic global subagent queue | Deep Review queue 只作用于 Deep Review；当前 generic runtime 仅有 queue timing primitive。 | 所有 hidden subagent 可以共享公平并发、统一等待语义和 capacity telemetry。 | 普通 subagent 被意外延迟；事件 contract/settings 破坏兼容；Deep Review policy 泄露到通用 agent。 | 普通 subagent 产品语义、事件 contract、settings、兼容性和 non-DeepReview regression。 |
| Backend batch / stagger scheduling | 完整 scheduler 延后；已实现 P0-a 批次重叠保护与 `launch_batch_blocked` 可见队列原因。 | 降低弱 orchestrator 误启动后续批次的风险；等待原因更可解释。完整 scheduler 仍可进一步降低 provider 瞬时并发尖峰。 | 当前保护只处理“前序批次仍活跃”的重叠，不负责自动生成完整 batch DAG；完整 scheduler 仍有 wall-clock 变长、失败顺序和 queue controls 交叠风险。 | 是否继续做 backend-owned batch/stagger dispatcher；默认 batch cadence、stagger 秒数、取消/跳过语义和失败恢复顺序。 |
| User-facing effective-cap override controls | 不在本计划内实现。 | 高级用户可按机器/provider 能力调节并发，降低卡顿或提高吞吐。 | 与 Review Team capacity settings 重复；过高值造成 provider/本机压力；过低值导致体验变慢；effective learning 被用户误解。 | 用户控制模型、设置入口、默认值保护、上限、解释文案、与 learned effective cap 的关系。 |

### 待产品决策项的产品方案建议

以下优先级从独立产品设计视角排序：优先解决用户可感知的失败恢复和等待体验，再处理成本优化，最后考虑高爆炸半径的编排重构。推荐方案都不是当前 cleanup 范围，只有在单独产品确认后才进入实现。

| 产品决策项 | 推荐优先级 | 推荐方案 | 主要收益 | 主要劣势 / 风险 | 决策建议 |
|---|---:|---|---|---|---|
| Deep Review queue liveness semantics | P0 | 已在本 PR 完成：Deep Review 内部排队不再按每个 reviewer 固定短超时；当同一次 Deep Review 仍有 active reviewer 时持续等待；active reviewer 释放后继续 admission。 | 避免弱模型/低并发下大量 reviewer 被错误跳过，保持 Deep Review 完整性和用户信任；错误原因更真实。 | 如果 active guard 泄漏或 running reviewer 没有 watchdog，可能造成等待过久；需要 UI 明确展示正在阻塞的 reviewer 和可取消操作。 | 本 PR 只保留监测和回归测试；不再把该项作为后续扩张入口。 |
| Backend batch / stagger scheduling | P0 | 已先落地 P0-a：批次重叠保护、`launch_batch_blocked` queue reason、extra reviewers 单独批次语义。下一步再设计“轻量 stagger”：默认小批次启动 reviewer，action bar 显示等待/已启动/未启动计数；高级参数暂不暴露。 | 当前已降低 mis-order 的直接风险；完整 stagger 可继续降低 provider burst 和 capacity failure，用户能看到渐进进展。 | 总耗时可能变长；用户可能误解为系统变慢；失败顺序和 retry merge 更复杂。 | 推荐继续设计，但不要把 P0-a 误认为完整 scheduler；下一轮必须先定默认批次、等待文案、取消/跳过语义。 |
| Backend-owned automatic retry redispatch scheduler | P1 | 先做显式 opt-in 的“自动补跑一次”：仅 transient capacity/timeout，带预算、最大等待和可取消状态。 | 减少用户盯着失败 slice 手动 retry 的成本，提升 Deep Review 完成率。 | 隐式成本、循环重试、过期上下文和 report 合并状态最容易引发信任问题。 | 谨慎推进；必须晚于 queue/stagger 可见状态，且默认关闭。 |
| User-facing effective-cap override controls | P1 | 不直接暴露 learned cap；只在 Review settings 提供“保守 / 平衡 / 更快”三档，并显示系统建议值。 | 高级用户可控制机器/provider 压力，同时避免暴露过多底层概念。 | 三档背后的真实 cap 仍需解释；错误选择会导致慢或不稳定。 | 推荐作为 settings 体验设计，不建议做裸数值输入。 |
| Hard prompt-byte clipping | P2 | 只做“诚实降级”：超阈值时让用户选择 full review / reduced review；coverage metadata 明示 omitted/reduced 文件。 | 给超大 diff 一个明确成本上限，减少超时。 | 如果表达不好，用户会误以为完整审查；可能漏掉关键文件。 | 可做，但必须和 report reliability/coverage UX 绑定，不可静默裁剪。 |
| Authoritative runtime strategy selection | P2 | 先保持 advisory；下一步做“建议升级/降级”确认，不自动覆盖用户选择。 | 降低用户选错深度导致的成本或质量问题。 | 自动覆盖会损害控制感；评分不可解释会造成争议。 | 推荐继续 advisory-first，积累误选/收益数据后再考虑自动化。 |
| Programmatic full tool-result cache | P3 | 继续只测量 duplicate cost；若收益显著，再做 session-scoped、content-size-limited cache。 | 降低重复 Read/GetFileDiff 成本。 | 直接缓存源码/diff 触碰隐私和失效语义，收益未测量前不值得。 | 暂不设计 UI；先把 diagnostics 数据跑实。 |
| Project-level review cache | P3 | 暂不做跨会话持久复用；若后续做，必须有用户可见管理、删除和失效说明。 | 大项目多轮审查可能明显提速。 | stale findings、隐私、删除权和策略/模型变化失效都很重。 | 延后。没有清晰 retention/deletion 前不应进入路线图。 |
| Generic global subagent queue | P4 | 不作为 Deep Review 需求推进；只有多个非 Deep Review 场景证明同类等待问题后再设计统一 queue。 | 长期可统一 hidden subagent 公平性和 telemetry。 | 极易让普通 subagent 意外变慢，破坏现有事件/settings contract。 | 延后到平台级 agent runtime 项目，不应由 Deep Review 顺手带入。 |
| Backend DAG scheduler | P4 | 保持 prompt-driven + deterministic guardrails；只把局部原语做稳，不替换整体编排。 | 理论上更确定、更易观测。 | 最大迁移半径；会改变模型协作方式、中断恢复和历史 session 兼容。 | 不建议近期做，除非 Deep Review 已成为独立工作流产品且需要强 SLA。 |

第三/第四项的实现收益与风险结论：

- Backend batch / stagger scheduling 是最值得优先设计的延期项；当前 P0-a 已补批次重叠保护，剩余收益集中在 provider burst 控制、完整启动节奏和等待计数可视化；风险仍集中在 wall-clock 变长、失败顺序和用户感知变慢。
- User-facing effective-cap override controls 有明确高级配置收益，但裸露数值会制造设置模型混乱；推荐改成三档意图式控制，并把 learned effective cap 保持为系统建议而非用户必须理解的概念。
- 本轮 queue liveness semantics 已从延期项转为本 PR 的发布收口内容；后续 provider queue / retry / stagger 只能基于这个等待语义继续设计，不能再引入“每个 reviewer 固定短超时”的口径。

## 行为变更检查点

如果实现过程中出现以下需求，必须停下重新确认设计，不能作为普通 refactor 合入：

1. 将 Deep Review local/provider queue 行为应用到所有 subagent。
2. 让 provider transient errors 对普通 subagent 自动 queue。
3. 用 generic queue event 替换 `DeepReviewQueueStateChanged`。
4. 将 retry 从结构化 model/user-issued retry 改为 backend-owned automatic redispatch。
5. 让 backend risk scoring 覆盖用户选择的 strategy。
6. 将 review cache 持久化到 session metadata 之外。
7. 硬裁剪 prompt bytes 或从 coverage metadata 中隐藏 changed files。
8. 改变 quick / normal / deep 语义但没有同步 cost-aware scope profile、prompt、report 和文档。
9. 新增页面或 modal 承载 queue / retry / reduced-depth 解释，而不是复用 action bar 或 Review settings。
10. 让 facade 文件继续积累新业务逻辑。
11. 将当前主 session / 其他 session / 普通 subagent 的活跃数量计入 Deep Review reviewer cap。
12. 在同一次 Deep Review 仍有 active reviewer 时，用固定短队列超时跳过后续 reviewer。

## 推荐执行方式

建议按里程碑顺序执行，每个重大里程碑内部拆成多个小 PR：

1. 先做无行为变化的结构拆分，降低后续功能实现风险。
2. 再补齐 provider queue 和 retry，因为它们共享 runtime queue、action bar、report reliability 和 settings 面。
3. 再做 cost-aware scope 与 shared evidence pack，因为它们依赖 manifest builder/parser、prompt、report 和 diagnostics 的边界稳定。
4. 最后做发布收口和文档对齐，确保 status wording 与真实代码一致。

每个 PR 的最小要求：

- 范围只覆盖一个清晰责任面。
- 有对应 focused tests。
- 若触碰 shared area，必须带 non-DeepReview regression。
- 若触碰用户可见字符串，必须带 locale completeness。
- 若触碰 docs 中已声明状态，必须同步更新状态或 pending 说明。

## 完成后的期望状态

阶段 1 发布收口完成后，Deep Review 应达到以下状态：

- Provider transient capacity queue 短时、可见、有上限、可控制。
- Retry 默认由用户显式触发，opt-in 自动 retry 也受预算、scope 和 elapsed guard 约束。
- Quick / normal / deep 的成本与覆盖语义清晰，报告不会夸大 reduced-depth 的审查深度。
- Reviewer 从 compact shared evidence 起步，减少重复发现，但不共享完整源码、完整 diff 或模型输出。
- 后端 Deep Review 逻辑位于专用模块树中。
- `TaskTool`、`CodeReviewTool`、`tool_pipeline` 和 events 只保留薄 Deep Review hook。
- 前端 review-team、Flow Chat launch、action bar、report 代码按责任拆分，并保留稳定 facade。
- 普通 Task、标准 Code Review、普通 action bar、普通 report export 有明确回归测试保护。
- Project-level cache、full tool-result reuse、hard prompt clipping、DAG scheduler、authoritative strategy selection 继续保持延期，除非后续产品决策批准。

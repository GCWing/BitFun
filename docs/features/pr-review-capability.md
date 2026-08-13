# PR 审核（Pull Request Review）能力需求文档

> 状态：需求 / 提案
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/architecture/review-lifecycle.md`](../architecture/review-lifecycle.md)（Review 记录、版本、新鲜度、再审查、问题延续）
> - [`docs/architecture/deep-review.md`](../architecture/deep-review.md)（当前 Review 执行：目标证据、只读角色、受管分片、补充检查）
> - [`docs/sdlc-harness/features/pr-quality-gate.md`](../sdlc-harness/features/pr-quality-gate.md)（变更就绪度与可选 PR 门禁投影）
> - [`docs/architecture/product-architecture.md`](../architecture/product-architecture.md)（分层与平台适配边界）

### 功能类型
<!-- 请描述功能建议的类型 -->
- [x] 新功能
- [x] 现有功能增强
- [ ] 用户体验优化
- [ ] 性能优化
- [ ] 接口/集成扩展
- [ ] 其他

### 优先级
<!-- 请选择功能建议的优先级 -->
- [ ] 紧急（P0 - 核心需求，强烈期望实现）
- [x] 高（P1 - 重要功能，期望尽快实现）
- [ ] 中（P2 - 有价值的功能，计划实现）
- [ ] 低（P3 - 锦上添花，有时间再做）

### 背景与动机
<!-- 请描述你为什么需要这个功能，解决了什么痛点 -->
- BitFun 已具备 `Review` / `DeepReview` 的只读审查执行能力，但目前一次 Review 运行仍主要由一个子会话表示，缺少面向 PR 的稳定产品身份。用户对某个 PR 的审查结果无法跨会话恢复、无法在 PR 面板看到“该 PR 最新审查”。
- PR 更新后旧审查被当作过时结果丢弃，缺少**新鲜度**信号与“审查当前版本”的一键再审查入口；同一 PR 上多次审查的问题无法做 new / repeated / changed 追踪，只能人工对照。
- 审查结论与门禁语义混淆风险：AI“未发现问题”可能被误读为“通过 / 可合并”，缺少“AI 结论仅为建议、非仓库门禁结果”的硬边界。
- 与仓库“产品逻辑平台无关、再通过平台适配器暴露”的规则不符的现状：PR 审查的产品身份、新鲜度、问题延续缺少稳定契约，难以在桌面 / 远程工作区 / 受管策略下一致投影。
- `review-lifecycle.md` 是已采纳的设计方向但尚未全部落地；本需求将其面向 PR 的产品能力显式化，作为可交付特性推进。

### 功能描述
<!-- 请详细描述你期望的功能行为 -->
- **审查入口**：支持从对话、已变更文件、Pull Request 三类入口发起审查；首次审查创建稳定的 **Review 记录**（审查谱系身份），后续再审查在**同一记录**内创建新版本（revision），不因文件列表或 PR 修订相似就启发式合并两次独立审查。
- **只读隔离执行**：每个版本由一个隔离的只读 reviewer 子会话执行（复用现有 `CodeReview` / `DeepReview` 执行策略），审查期间不得修改被审查目标；修复仍走独立的、用户批准的修复路径。
- **有界结果投影**：每个版本持久化“列表 / 卡片视图所需的最小结果投影”——结果可用性、完成时间、覆盖度（`complete` / `limited` / `failed` / `unknown`）、目标新鲜度（`current` / `stale` / `unknown`）、问题数量与风险等级、模型建议与简短评估；完整报告与 transcript 仅在用户打开详情时按需水合。
- **PR 面板投影**：PR 面板按“已验证的 provider 身份 + base/head”展示该 PR 最新匹配记录；旧版本与过时结果历史作为次要详情。base/head 不匹配标记为 stale，提供“审查当前版本”入口。
- **问题延续**：以归一化的 **group key**（路径 / 类别 / 标题）与 **occurrence fingerprint**（位置 / 严重度 / 确定性 / 描述 / 校验证据）双键追踪问题；提供系统观测（new / repeated / changed / not observed）与用户处置（open / resolved / dismissed）两个独立维度；模型沉默不得自动 resolve 问题。
- **执行与加载解耦**：执行状态与 transcript 加载状态相互独立；打开或重载详情不发起模型调用、不创建子回合；应用重启不得静默复活或重复未完成回合；未加载内容的子会话呈现 preparing / loading / load-failed 而非空白面板。
- **非门禁边界**：AI 结论永远以建议形式呈现，不得渲染为“通过 / approved / safe to merge / 仓库门禁结果”；门禁/强制审批仍由外部权威（CI、分支保护、CODEOWNERS、人类审查人）与可选的 [PR 门禁投影](../sdlc-harness/features/pr-quality-gate.md) 负责。
- **后台与持久化**：发起审查默认留在父任务，立即给出可用卡片，应用重启后仍可用；Review 记录与版本元数据通过既有会话历史持久化服务存储与查询，不引入并行的 Review 数据库或 UI 自有索引。

### 期望效果 / 使用场景
<!-- 描述该功能在什么场景下使用，以及使用后的预期效果 -->
1. 用户准备 PR 时一键发起审查，后台运行；结果以卡片形式留在父任务，不打断当前工作，应用重启后卡片仍可恢复。
2. PR 面板展示该 PR 的最新审查记录（含覆盖度、新鲜度、问题摘要、下一步建议），跨会话可查询恢复；无需手动翻历史会话定位“这个 PR 审过没”。
3. PR 在 review 后被推送新提交，旧审查自动标记为 stale，提供“审查当前版本”一键再审查；新结果归入同一记录的新版本，便于版本间对比。
4. 多次审查的问题通过 new / repeated / changed 状态追踪，用户可显式 resolve / dismiss；问题在后续审查中未被观测到时变为 `not observed` 而非自动 `resolved`，避免模型静默关闭风险。
5. 审查保持只读，修复走独立用户批准路径；AI“未发现问题”同时附带覆盖度与新鲜度，绝不被渲染为“通过 / 可合并”。

### 设计草案 / 参考示例
<!-- 如有设计稿、草图或参考的产品示例，请附在此处 -->
- **领域模型参考**：`review-lifecycle.md` 的 `ReviewRecord` / `ReviewRevision` / `TargetEvidence` / `ResultProjection` / `FindingObservation` / `FindingDisposition` 模型与 mermaid 状态机。
- **状态语义参考**：`pr-quality-gate.md` 的 `ready / attention / blocked / degraded` 与投影方式 `off / summary / advisory / required / blocking`，PR 审查默认落在 `summary` / `advisory`。
- **行业参照**：
  - [GitHub Copilot 代码审查](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review)：AI 审查以评论形式协助，强制审批由仓库规则配置（与“AI 仅建议、门禁由外部权威”一致）。
  - [CodeRabbit 审查强度](https://docs.coderabbit.ai/reference/configuration)：审查强度可配置、默认减少噪音（对应 Review strength 由显式意图控制）。
- **落地分层建议**（遵循仓库分层与边界）：
  1. Contracts（`src/crates/contracts`）：稳定 DTO / port——记录锚点、版本元数据、有界结果投影、问题 group key / occurrence fingerprint 契约，行为轻量、不向上依赖。
  2. Execution（`src/crates/execution`）：Review 执行 / 子会话 / 只读角色 / 受管分片 / 补充检查策略，复用既有 owner，不新增第二个 reviewer 执行器。
  3. Services（`src/crates/services`）：会话历史持久化服务作为 Review 记录与版本的**唯一**存储 / 查询 owner，不引入并行 Review 数据库。
  4. Assembly（`src/crates/assembly`）：声明 `ProductCapabilityId::PrReview`（或等价）与 capability pack 事实，按 delivery profile 选装；非桌面形态可裁剪。
  5. Interfaces / App / UI（`src/apps` + `src/web-ui`）：暴露 PR 审查 wire 契约与 PR 面板投影；UI 组件不得直接调用 host API，须走 adapter / infrastructure 层。
- **前置依赖**：需要“无损 source contract”——当前单一 `evidence_status` 不足以同时表达覆盖度与新鲜度，必须先让结构化 Review 输出保留独立的 `coverage` 与 `freshness` 字段（或等价机读 reason code），否则有界投影不得上线。

### 是否愿意贡献
<!-- 是否愿意参与该功能的开发或讨论 -->
- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

### 补充说明
<!-- 其他你认为有助于理解功能建议的信息，如相关 Issue 链接、文档等 -->
- 本需求是 `review-lifecycle.md`（已采纳设计方向）面向 PR 的产品化推进；落地前必须确认无损 source contract（独立 `coverage` / `freshness`）就绪，否则有界投影与 stale 判定不可靠。
- **非目标**（沿用 `review-lifecycle.md`）：不在每次保存 / 推送 / PR 更新时自动审查；不做自动评论发布 / 批准 / 合并 / 门禁强制；不做模糊语义匹配关闭问题；不把本地审查结果复用到 PR 而无仓库 / 目标 / 内容 / 策略 / 上下文等价性证明；不假装所有 provider 支持任意 revision-delta 审查。
- **远程工作场景**：PR 审查在远程工作区需保证 provider 身份与 base/head 可验证；若某能力无法在远程合理支持，应给出清晰的不支持态提示而非静默失败（参见仓库“远程兼容”全局规则）。
- **HarmonyOS PC CLI/TUI** 是未来平台目标，不属当前范围；如后续覆盖，按 [`docs/architecture/platform-portability-design.md`](../architecture/platform-portability-design.md) 单独立项。
- **治理边界**：不引入 Review 专属遥测管线；审查质量保护先由确定性验收证据保障，未来若 Quality Data Plane 具备生产生产者，仅发射已注册的生命周期与显式反馈事实。

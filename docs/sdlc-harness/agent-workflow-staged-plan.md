# BitFun 智能体工作流场景收敛计划

> 范围：承接 [product-requirements-agent-workflow-adjustment.md](product-requirements-agent-workflow-adjustment.md)，把智能体工作流、审查、并发和成本控制压回真实用户场景。
> 本文不是新的 SDLC Harness 阶段路线图，不重新定义 P-1/P0/P1/P2/P3/P4。落地阶段仍以 [implementation-plan.md](implementation-plan.md) 为准。

## 1. 收敛结论

BitFun 不需要把 dynamic workflow 做成一个新的主产品模式。用户真正要的是：

- 小任务快，并且知道哪些内容没有验证。
- 重要改动能被合适强度地看一眼，而不是默认跑重审查。
- 长任务不黑盒，token、耗时、阻塞和下一步清楚。
- 批量任务可以并发，但 GUI 只呈现一个任务、一组状态和少量决策点。
- 成本增加前必须说明为什么值得，且用户可以调整范围、暂停或只收敛已完成部分。

因此，workflow 只作为少数场景的后台执行策略。默认产品体验仍是“任务完成 -> 摘要 -> 已验证/未验证 -> 可选下一步”。

## 2. 不再推进的方向

| 不做 | 原因 |
|---|---|
| 不建立第二套 P0-P4 workflow 路线 | SDLC Harness 已有实施路线，重复阶段会制造治理歧义 |
| 不在 P0/P1 定义新的核心 workflow 对象模型 | 容易复制 Agent Kernel、DeepReview、Harness 和 QDP 的状态模型 |
| 不让用户选择或学习 workflow DSL | 用户关心任务结果，不关心编排语言 |
| 不默认启动多个 reviewer | 审查成本高，且小任务更需要速度和清晰未验证项 |
| 不默认显示任务控制台 | 控制台只服务批量、长任务或用户需要决策的场景 |
| 不默认生成完整证据包、图谱或 PR 门禁 | 这些只在 PR、团队、发布、事故、合规或大规模任务显露 |

## 3. 用户场景优先级

| 场景 | 默认阶段映射 | 用户目标 | 默认体验 |
|---|---|---|---|
| S1 低风险 bugfix / 文档 / 小 UI 调整 | SDLC P0 | 快速完成，并知道未验证项 | 单 agent，短摘要，不显示 workflow UI |
| S2a 本地显式审查 | SDLC P0/P1 | 用户主动要求提交前快速看一眼 | 可选 L1 快审；不自动进入 PR/团队流程 |
| S2b PR / 受保护分支 / 团队规则审查 | SDLC P2 | 发 PR 或进入受管路径前减少明显问题 | 默认 Review 面板；高风险或团队规则才建议 L2/L3 |
| S3 CI / 测试失败收敛 | SDLC P1/P2 | 找到失败原因并修复 | 先单 agent 诊断；多失败再聚类为队列 |
| S4 PR review comments 批量修复 | SDLC P2 | 快速处理可机械修复的意见 | 按评论/文件分组，修复后快速复核 |
| S5 大规模迁移 / 审计 | SDLC P3 | 高完成率且风险可控 | 先样本 gate，再预算确认，再批量执行 |

## 4. 场景设计

### 4.1 S1：低风险任务

| 项 | 要求 |
|---|---|
| 入口 | 用户要求修一个小问题、改文档、小样式或低风险局部逻辑 |
| 默认执行 | 单 agent，必要时运行最近的轻量验证 |
| 审查 | 不默认创建 reviewer；任务结束仅总结实际运行过的验证，L0 仍属后续 Verify 探索 |
| GUI | 保持聊天/任务结果视图，不显示成本面板或控制台 |
| 成本 | 不展示 token 明细，除非预计异常超时或用户选择成本模式 |
| 完成标准 | 给出改动、已验证、未验证、残余风险和下一步 |
| 禁止 | 因为存在 workflow 能力而自动进入多 agent、证据包或图谱 |

### 4.2 S2：本地审查与 PR 审查分层

| 项 | 要求 |
|---|---|
| 本地显式入口 | 用户在普通任务或提交前明确要求 review，可使用 L1 只读快审 |
| PR/团队入口 | 准备 PR、受保护分支、CODEOWNERS、团队策略或发布路径命中；归入既有 P2 PR/团队治理场景 |
| 默认执行 | 先固定当前修改或明确 Git range 的 base/head、文件状态和完整度；本地显式审查只做一个只读 reviewer；PR/团队路径按规则给 Review 面板和就绪度摘要 |
| 升级 | 命中安全、性能、架构、跨模块、关键 UI 流程或验证缺口时建议 L2 |
| L3 条件 | 用户明确要求严格审查、团队强策略、大型 PR 或核心接口变更 |
| GUI | 一个 Review 面板，按问题优先级合并输出 |
| 成本 | L2/L3 前显示计划独立检查/调用数、耗时倾向和收益，允许调整范围；不展示无法证明的 Token 估算 |
| 完成标准 | 必须修复、建议确认、已覆盖、未覆盖、下一步清楚 |
| 禁止 | 把 PR 审查压进 P0 默认体验，或把 DeepReview 作为普通 review 默认入口 |

当前产品落点：

- 文件变更菜单和命令面板只提供 `Review`，不让用户先选“普通/严格”。
- `/review` 默认根据目标事实选择最小充分覆盖；`/review strict` 只表达用户明确要求最广覆盖，`/DeepReview` 仅保留历史兼容。
- 目标证据先于 Review 决策：当前工作区使用一次有界 `HEAD -> worktree` 取证，但没有 immutable snapshot，因此最终 evidence status 始终为 `limited`；显式 Git range 由目标准备层固定 base/head，完整且无遗漏、workspace binding 为 matching_clean 时 evidence status 才可为 `complete`。Reviewer 不自行猜 ref，缺失、截断或预算耗尽必须进入覆盖说明，但不改写模型 recommendation。
- PR1 保留只读 Reviewer 现有的 `Git` 暴露以避免旧 PR/历史诊断场景先发生能力回退，但 prepared work packet 不把它作为 changed-code 证据，也不新增多操作 Git 工具。prepared target 只通过有界 `GetFileDiff` 消费目标 diff；只有本地仓库与目标 head 匹配且整个工作区干净时，现有 Read/Grep/Glob/LS 才补充 live context。不做逐工具全仓重验、任意 shell、fetch、checkout 或仓库状态写入。
- L2/L3 扩大独立覆盖前展示单一 Review 方案确认，包括范围、计划 reviewer/调用数、耗时倾向和只读边界；不展示无法证明的 token 估算。
- 修复不新开第二套产品界面：同一 Review 侧栏先把选中项交给 `ReviewFixer`；“审核修复”能精确归因时按原审核文件与 Fixer 直接改动文件的并集重新统计和决策，命令型修改无法可靠归因时明确提示并回退当前工作区 diff，在同一侧栏位置切换到新的隔离 Review 子会话。Fixer 基线和选中项在修复前持久化；follow-up 预留、relationship metadata 和后端 session id 复用同一 request id。启动确认不确定时保留稳定 turn 和已创建子会话、显示明确提示，不自动或在重启后重发启动消息；侧栏显示进行中和查看结果，只有明确失败的操作才提供重试。旧会话缺少范围信息时采用同样的显式工作区回退。
- PR 面板是唯一内置 PR Review 入口：adapter 固定 provider identity、base/head 和按需远程 diff，启动同一套自适应 Review，并按精确 PR revision 投影进行中、结果和过期状态。已移除 PR Review MiniApp 及其独立强度和 AI 草稿路径；平台 CI/审批/mergeability 事实仍与 AI Review 建议分层。本轮不自动触发 Review，不生成或发布 inline comment，也不增加 approve、merge、checkout、缓存或 Finding 生命周期。

2026-07-10 合入后产品复盘：

- 统一入口、自适应强度、只读 Reviewer、独立 ReviewFixer 和同侧栏 follow-up 已形成可用基线，不再新增 Review 执行分支。
- 当前闭合 workspace / Git range / provider PR 三类目标证据，但仍只有一套 Review 执行链路。该 PR 不新增长期目标数据库、合成 diff 引用、增量缓存计划、Finding 生命周期、自动评论或结果动作。
- 后续首先观察目标解析失败率、provider diff 缺失率、limited evidence 比例、diff 预算耗尽率、Review 成功率和 token 变化；没有指标证明收益前，不新增 PR3 默认范围。
- PR 自动审查、跨 Review 增量对照、反馈学习、完整远程 checkout 和大规模任务控制台均不进入本轮；完成后先观察目标正确率、覆盖缺口和用户决策时间。

### 4.3 S3：CI / 测试失败收敛

| 项 | 要求 |
|---|---|
| 入口 | 用户要求修 CI、测试失败、lint/typecheck 大量报错 |
| 默认执行 | 先做失败诊断和最近失败聚类，不立即启动批量 worker |
| 队列条件 | 多个失败可独立处理，且有可运行 oracle |
| GUI | 长任务条优先；只有多个独立失败时显示任务控制台 |
| 成本 | 多失败、多轮验证或多 reviewer 前提示预算 |
| 完成标准 | 修复了哪些失败、仍失败哪些、不可运行原因和下一步 |
| 禁止 | 无 oracle 时扩大并发；反复重试同类失败消耗 token |

### 4.4 S4：PR review comments 批量修复

| 项 | 要求 |
|---|---|
| 入口 | 用户要求处理 PR 评论或审查意见 |
| 默认执行 | 按评论线程、文件和主题分组，先处理低冲突意见 |
| 队列条件 | 评论数量多、机械修复明显、冲突范围可识别 |
| GUI | 单一任务控制台，默认显示完成、阻塞、失败和需决策评论 |
| 审查 | 修复后做快速复核；高风险评论才升级定向审查 |
| 成本 | 用户可选择只处理高价值评论或跳过低风险二次复核 |
| 完成标准 | 已处理、需人工确认、未处理和建议回复内容明确 |
| 禁止 | 为每条评论启动独立 chat 或独立 GUI 窗口 |

### 4.5 S5：大规模迁移 / 审计

| 项 | 要求 |
|---|---|
| 入口 | 用户明确要求批量迁移、全仓审计或大规模一致性修改 |
| 前置 | 有相似 work items、相对独立范围、可运行 oracle 或清晰人工验收标准 |
| 默认执行 | 先 1-3 个样本，验证规则、冲突模型、oracle 和输出质量 |
| 扩大条件 | 样本成功、预算确认、冲突策略明确 |
| GUI | 一个任务控制台，突出进度、预算、阻塞、冲突和用户决策 |
| 审查 | 分片审查只覆盖高风险路径、失败聚类和共享 contract |
| 成本 | 任务头常驻 token、耗时、并发、完成/阻塞/失败/跳过数量 |
| 完成标准 | 可收敛已完成部分，阻塞项单独列出，不因少量失败丢失全部收益 |
| 禁止 | 为追求完成率无上限追加 token；没有样本 gate 就全量并发 |

## 5. Review 强度规则

| 强度 | 用户表达 | 默认触发 | 成本倾向 |
|---|---|---|---|
| L0（后续探索） | 快速检查 | 等待 Verify evidence 设计，不在当前生产策略中触发 | 默认最低成本 |
| L1 | 独立快速审查 | 提交前、PR 前、小到中等风险 | 允许自动建议 |
| L2 | 定向审查 | 安全、性能、架构、跨模块、关键 UI 或验证缺口 | 需说明原因和预计成本 |
| L3 | 严格审查 | 大型 PR、核心接口、安全敏感、团队强策略、用户明确要求 | 必须预算确认，可调整范围 |

原则：

- 默认任务不启动 reviewer；显式 Review 以 L1 为主，L2/L3 只服务风险或明确意图。
- reviewer 默认只读；修复必须进入用户批准的执行阶段。
- 两轮审查没有新增有效问题时，应建议停止或保留核心检查。
- 缺少上下文或 oracle 时，先提问或诊断，不启动 L3。

## 6. 成本和解决率平衡

| 决策点 | 默认倾向 |
|---|---|
| 小任务 | 优先首个有用结果时间，牺牲部分覆盖但明确未验证项 |
| 中风险任务 | 优先 L1/L2 审查和最近验证，不默认批量并发 |
| 多失败任务 | 优先失败聚类和可运行 oracle，再决定是否队列化 |
| 大规模任务 | 优先样本成功率和可收敛性，再考虑并发 |
| 预算不足 | 优先高风险/高价值 item，跳过低风险二次审查 |
| 解决率不足 | 先问是否追加预算或调整目标范围，不静默扩大 token |

用户侧必须能看到：

- 为什么建议花更多 token。
- 不追加预算会放弃哪些覆盖。
- 追加预算预计提升的是解决率、覆盖率还是墙钟时间。
- 目前可安全收敛的结果是什么。

## 7. GUI 交互约束

| 场景 | 展示 |
|---|---|
| 普通任务 | 任务结果摘要：改动、验证、未验证、下一步 |
| 长任务 | 后台任务条：状态、阶段、预算、需决策 |
| 批量任务 | 单一任务控制台：执行域、沙箱状态、阶段、数量、异常、预算、可下钻详情 |
| 审查 | Review 面板：问题优先、覆盖范围、未覆盖风险、操作 |
| 成本确认 | 说明收益、成本、范围控制选项和停止选项 |

GUI 不展示多个 agent 窗口，不默认展示完整日志，不把内部术语作为主文案。

## 8. 指标保护 lens

本文不新增正式指标，只给采纳 workflow 能力时必须检查的保护视角。任何新指标都必须先进入 metrics spec；任何新采集都必须先进入 QDP 事件注册表。

| 观察项 | 用途 | 采纳边界 |
|---|---|---|
| 首个有用结果时间 | 防止默认路径变重 | 优先映射既有速度指标 |
| 用户打断率 / 弹窗触发率 | 防止成本和审查提示打扰默认路径 | 复用既有弱提示治理 |
| 成本预期偏差 | 防止 token/耗时无感知上涨 | 预算模式和采样口径明确后再采纳 |
| 任务解决率 | 判断用户目标是否完成 | 需要任务类型、完成判定和反馈来源 |
| 审查有效问题率 | 判断 reviewer 是否值得默认启用 | 只在 review feedback 事件稳定后采纳，不能作为 P0 默认门禁 |

P3/P4 再按需补充队列阻塞率、工作流回退率、后续返工率和维护 churn。不要让指标系统先于用户场景扩张。

## 9. 与既有文档的关系

| 文档 | 本文如何依赖 |
|---|---|
| [product-requirements.md](product-requirements.md) | 继承快速开发、上下文保障、团队治理、执行安全和合规/发布路径 |
| [implementation-plan.md](implementation-plan.md) | 复用既有阶段路线，不新增 workflow 专属 P0-P4 |
| [architecture/agent-workflow-design.md](architecture/agent-workflow-design.md) | 仅作为交互和边界补充，不定义独立核心架构 |
| [../architecture/deep-review.md](../architecture/deep-review.md) | 仅在 L3 严格审查中复用 DeepReview 的 reviewer 队列、只读 reviewer 和 action surface；通用任务生命周期、scheduler 和队列状态归 Agent Kernel，Harness 只通过 provider/plan/step 参与编排 |
| [architecture/quality-data-plane.md](architecture/quality-data-plane.md) | 复用既有最小事件和指标口径，不新增 P0 默认事件 |

## 10. 参考资料

- [Bun: Rewriting Bun in Rust](https://bun.com/blog/bun-in-rust)
- [Claude Code workflows](https://code.claude.com/docs/en/workflows)
- [GitHub Copilot cloud agent](https://docs.github.com/en/copilot/concepts/agents/cloud-agent/about-cloud-agent)
- [Cursor 1.0: Bugbot, Background Agent, MCP](https://cursor.com/changelog/1-0)
- [Google Jules public beta announcement](https://blog.google/innovation-and-ai/models-and-research/google-labs/jules/)

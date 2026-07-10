# BitFun Review 产品体验竞品基准与优化需求

> 调研日期：2026-07-10。
>
> 范围：复盘统一 `/review`、自适应审查、只读 Reviewer、修复后复审、成本确认和 GUI 并发表达，并对照同类产品确认下一步产品优化项。
>
> 本文是产品研究和候选需求，不是技术设计、实施计划或新增 SDLC Harness 阶段。是否进入实现，仍需回填 [product-requirements.md](../product-requirements.md)、[agent-workflow-staged-plan.md](../agent-workflow-staged-plan.md) 和对应实施计划。本文不授权新增 Review、Verify 或 Workflow 平行系统。

## 1. 结论先行

BitFun 已经完成了最重要的方向性收敛：用户只需要理解 `Review`，系统根据目标、范围和风险选择最小充分强度；Reviewer 默认只读，修复交给独立执行身份；普通任务中的审核留在原任务上下文，严格审核扩大覆盖前需要确认成本。

与本次选取的公开产品相比，下一步最值得做的不是增加更多 Reviewer、模式或设置，而是把 Review 从“生成一份报告”提升为“帮助用户做出继续修复、人工确认或提交决定”的决策支持：

1. **以问题为中心，而不是以报告和 Reviewer 为中心**：每个问题应有证据和明确动作；近期先提供本次 Review 内的不处理、追问和修复计划选择，问题级直接修复等待稳定映射。
2. **让复审逐步增量化**：近期先明确本次复审范围和变化；稳定身份、持久化和失效规则成立后，再回答“原问题是否关闭、是否引入新问题”。
3. **让成本确认表达收益**：用户需要知道额外成本换来了哪些覆盖，而不是只看到调用数和输入 Token 估算。
4. **给自动触发加刹车**：本地显式 Review 保持轻量；PR 自动审查只在“已准备好审查”或团队策略命中时启用，不默认每次 push 都重新消费成本。
5. **把并发复杂度留在后台**：普通 Review 仍是一张结果面板；只有真实的大规模迁移、审计或失败队列才进入单一任务控制台。

短期优先级应集中在现有 Review 面板和 follow-up 摘要，不建设跨 Review 生命周期、通用 Workflow DSL、独立 Verify 页面、Reviewer 编排器或新的团队治理后台。

## 2. 调研方法和证据边界

本次调研使用三类证据：

- BitFun 合入后的产品文档、中文文案和关键 Review UI 组件。
- 竞品截至调研日可访问的官方文档和产品公告。
- 竞品自己披露的成本、触发、限制和使用建议。

边界：

- 官方资料能证明产品公开行为，不能证明实际召回率、误报率或不同模型之间的真实效果。
- 各产品的 Token、信用点、Actions 分钟和订阅成本口径不可直接横向换算。
- Claude Code Review、Agent Teams、GitHub Copilot 中等 Review effort 等能力仍包含 preview 或 experimental 状态，不能直接当作稳定行业标准。
- 本文中的“对 BitFun 的启发”是产品推断，和竞品公开事实分开表达。

## 3. BitFun 当前产品基线

| 能力 | 当前状态 | 产品判断 |
|---|---|---|
| 单一入口 | `/review` 和 GUI Review 共用自适应决策；`/DeepReview` 只做历史兼容 | 方向正确，应继续隐藏 L1/L2/L3 和 DeepReview 心智 |
| 审查强度 | 根据目标事实、风险和用户严格意图选择 L1-L3 | 方向正确，不应增加更多用户可选档位 |
| 独立性 | CodeReview / DeepReview 只读，ReviewFixer 单独执行修复 | 是可信 Review 的必要底线，应保持 |
| 任务内审核 | 用户要求“完成并仔细审核”时，使用一个隔离 Reviewer，不另开产品页面 | 符合低摩擦任务心智 |
| 扩大覆盖确认 | L2/L3 前展示范围、输入提示词 Token 估算、调用数、并发和只读说明 | 已有成本提示，但收益表达和选择仍不完整 |
| 结果可信度 | 已有严重程度、确定性、validation note、覆盖和部分结果提示 | 数据基础较好，但问题状态和用户反馈闭环不足 |
| 修复闭环 | 同一侧栏中选择修复、恢复中断并启动独立 follow-up Review | 架构和恢复能力较完整，产品上仍缺少增量对照 |
| 容量设置 | 设置页提供最大并行审核工作和最长排队等待 | 适合高级控制，不应扩张成 Reviewer 配置中心 |
| PR Review | 目标边界是消费统一 Review 结果；当前 PR Review MiniApp 仍有独立 fast / focused / deep 选择和 AI 草稿生成路径，另一个 PR 面板只按工作区和文件重叠启发式展示“相关 Review 会话”数量，没有稳定 PR 身份关联 | 统一投影尚未完成，是当前最明确的重复产品逻辑；后续必须收敛到同一 Review 结果和问题状态，而不是继续扩展两条路径或把启发式关联固化为契约 |
| Verify | 保持探索，没有独立入口或默认门禁 | 方向正确，应先定义可信证据再讨论产品化 |

## 4. 竞品详细对比

### 4.1 Review 入口、结果和修复

| 产品 | 官方公开行为 | 做得好的产品选择 | 对 BitFun 的启发 | 不应照搬 |
|---|---|---|---|---|
| GitHub Copilot Code Review | 在 GitHub、IDE 和 CLI 的原有代码审查位置触发；CLI 使用 `/review`；PR 结果按普通评论呈现，不替代人工批准；支持 Low / Medium effort、自动审查、重新审查和路径级自定义指令（[使用方式](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review)，[自动审查与 effort](https://docs.github.com/en/copilot/how-tos/copilot-on-github/set-up-copilot/configure-automatic-review)） | Review 出现在用户本来就看 diff 的地方；AI 结论默认 advisory；团队规则复用仓库文件 | BitFun 应继续使用单一 Review 入口，并把结果投影到 diff、PR 或当前任务，而不是建立新产品中心 | 不复制多个平台各自独立的 Review 状态；不默认每次 push 触发 |
| Claude Code Review | 多个专门 Agent 并行查找问题，后续验证候选问题、去重、按严重程度排序；问题可评价有用或噪声；后续 push 可自动关闭已修复线程；支持一次性或持续审查（[官方文档](https://code.claude.com/docs/en/code-review)） | 把“候选发现”和“问题验证”分开；以问题状态而不是 Agent 输出组织结果；清楚披露平均耗时、仓库平均成本和月度上限 | BitFun 的 strict Review 应保持内部 Judge/验证职责，并把验证状态、问题关闭和反馈放到结果层 | 不把多 Agent 数量当作质量证明；不把每次 Review 变成约 20 分钟的默认重流程 |
| OpenAI Codex | Review 会匹配 PR 意图与 diff，读取代码库并运行测试；PR 线程内可继续要求修复；模型按任务复杂度动态调整工作时长（[Codex Review](https://openai.com/index/introducing-upgrades-to-codex/)）；桌面端用项目和独立 worktree 管理多个并行任务（[Codex App](https://openai.com/index/introducing-the-codex-app/)） | 用户表达目标，不选择内部编排；Review、修复和再次检查保持上下文连续；并行任务互相隔离 | BitFun 应继续动态选择强度，并让“修复后再看”成为同一 Review 闭环中的自然动作 | 不把多个独立任务线程等同于一个大任务内部的所有 worker；Review 内部并发不需要多个线程 |
| Cursor Bugbot | 自动审查 PR，并把问题修复交给 Cursor 或 Background Agent（[Bugbot 产品页](https://cursor.com/bugbot)）；2026 年新增本地 `/review`、相同 diff 跨本地和 PR 去重、仅审查上次 Review 后的新变化（[2026-06-10 更新，访问于 2026-07-10](https://cursor.com/changelog/bugbot-updates-june-2026)） | 从问题到修复的上下文交接短；同一 diff 不重复付费或重复打扰 | BitFun 可缩短问题到 ReviewFixer 的上下文交接，并在稳定身份契约具备后让本地 Review 与 PR 投影避免重复审查 | 不让用户在 `/review` 中选择内部 Agent；不默认对每次 PR 更新重跑；不新建 Bugbot 类平行入口 |
| CodeRabbit | 首次完整 Review，后续 commit 默认做增量 Review（[Review 概览](https://docs.coderabbit.ai/guides/code-review-overview)）；自动复审达到 commit 数量后可暂停，支持 incremental / full、pause / resume（[自动审查控制](https://docs.coderabbit.ai/configuration/auto-review)）；IDE 可应用单点修复或把问题交给编码 Agent（[IDE](https://docs.coderabbit.ai/ide)） | 明确区分增量与全量；主动避免活跃分支重复消费；修复动作靠近问题 | BitFun follow-up Review 应显示原问题关闭、新问题和未复核项；自动触发需要去抖和自动停止 | 不暴露大量命令作为 GUI 心智；不把“Fix all”变成未经确认的自动写入 |
| Devin Review | 对大型 PR 做逻辑分组、移动/复制识别；把高置信 Bug 与待调查 Flag 分开；支持问题解决状态、代码库问答、Code Owner 和受控 Auto-Fix（[官方文档](https://docs.devin.ai/work-with-devin/devin-review)） | 优先帮助用户理解复杂 diff；区分确定问题和需要调查的信息；问题可独立关闭 | BitFun 可把 certainty 和 validation note 转化为更直观的“已验证问题 / 建议确认”，并为单问题提供当前上下文追问 | 不把 Review 面板扩张成完整 GitHub 替代品；Auto-Fix 仍需明确授权 |
| Qodo | 多 Agent 使用仓库上下文、PR 历史和 Review Standards，强调减少低价值评论（[官方文档](https://docs.qodo.ai/code-review)） | 团队标准有单一来源，Review 目标明确聚焦高价值问题 | BitFun 项目规则继续复用已有文档和策略，不新建 Review 专属规则系统 | 不把厂商关于高召回和高精度的自述当作独立效果证据 |
| Graphite Agent | 自动 Review 强调真实 Bug、代码库上下文、可执行建议和反馈学习，并提供接受率与规则效果分析（[Review](https://graphite.com/docs/ai-reviews)，[配置与分析](https://graphite.com/docs/ai-reviews-setup)） | 把“少噪声、可执行”作为 Review 质量，而不是评论数量 | BitFun 后续衡量有效问题率、误报反馈和重复问题率 | 不在反馈数据和真实团队场景不足时先建组织级分析后台 |

### 4.2 大规模并发和动态 Workflow

| 来源 | 官方公开行为 | 产品启发 |
|---|---|---|
| Bun Rust 迁移 | 约 16,000 个编译错误按 crate 形成工作队列；每个执行单元采用修复、两个对抗 Reviewer、一个 Fixer；峰值约 64 个 Claude，运行在 4 个 worktree；整个迁移成本约 165,000 美元 API 定价（[Bun 迁移复盘](https://bun.com/blog/bun-in-rust)） | 这是百万行迁移、强 oracle 和高预算下的极端案例，不是普通 Review 默认值。真正可复用的是工作项、隔离、对抗审查、失败回队列和强测试 oracle |
| Claude Code Dynamic Workflows | JavaScript 脚本可编排几十到数百 Agent；后台运行；进度视图显示阶段、Agent 数、Token 和耗时；支持暂停、恢复、停止和保留已完成结果；官方建议先跑小样本，超过 25 Agent 或预计 150 万 Token 时给大任务警告（[官方文档](https://code.claude.com/docs/en/workflows)） | BitFun 的 GUI 应把一个大目标呈现为一个任务控制台，默认显示阶段、总进度、成本和异常；Worker 详情只下钻查看。启动前展示方案和成本，运行中可停止且不丢已完成结果 |
| Claude Agent Teams / Agent View | Subagent 适合少量聚焦任务；Agent Teams 适合需要互相沟通的少数长期同伴，但 Token 成本高且有协调开销；Agent View 用一屏查看多个独立会话（[并发方式对比](https://code.claude.com/docs/en/agents)） | “多个独立用户任务”和“一个任务内部并发”是两种 UI：前者可用任务列表，后者必须聚合为一个任务，不应混成多个聊天窗口 |
| Codex App | 多个任务按项目组织，每个任务使用独立 worktree，用户可以在任务之间切换并审查各自 diff（[官方公告](https://openai.com/index/introducing-the-codex-app/)） | BitFun 可借鉴独立任务的隔离和总览，但单个 Review 内部的 Reviewer 不应升级为独立用户任务 |

## 5. 选取样本中反复出现的模式与产品判断

### 5.1 官方材料中反复出现的模式（观察）

1. **入口就近**：本地改动在当前任务或 IDE，PR 审查在 PR；用户不应先去独立 Review 产品中心。
2. **结果问题化**：结果围绕问题、严重程度、证据、位置和动作组织，不围绕 Agent 名称组织。
3. **候选再验证**：多 Agent 的主要价值是提高候选覆盖，必须有去重、验证或独立复核来降低噪声。
4. **复审增量化**：只审新增变化、追踪旧问题关闭，并允许用户显式要求全量重审。
5. **修复需交接**：Review 和修改身份分开，但问题到 Fixer 的上下文交接应尽量一键完成。
6. **自动触发可停止**：支持手动、一次性、持续或团队策略，但活跃分支必须避免无休止复审。
7. **并发不等于多窗口**：大任务显示一个聚合进度；多个互相独立的用户任务才显示为任务列表。

### 5.2 这些公开材料尚未充分回答的问题（BitFun 产品判断）

- “没有发现问题”仍容易被误读成代码安全或完成质量已被证明。
- 多模型、多 Agent 和更长思考能提高覆盖，但同时放大成本、延迟和不一致结果。
- 自动复审容易重复评论或在每次 push 后制造噪声。
- 反馈学习需要稳定的问题身份、用户行为和隐私边界，不能靠简单点赞直接改变门禁。
- 大规模 Workflow 的强展示容易让普通用户承担内部调度心智。

BitFun 的机会不是复制所有竞品能力，而是用更少的产品概念，把“为什么可信、为什么值得花成本、下一步做什么”表达得更清楚。

## 6. 当前体验差距

| 差距 | 当前证据 | 用户影响 | 优先级 |
|---|---|---|---|
| 问题与修复计划缺少稳定映射和就地非写入动作 | `CodeReviewToolCard` 展示 severity、certainty、validation note 和 suggestion，但问题本身没有“不处理 / 追问”动作；修复计划在另一区块统一选择，问题与 remediation 没有稳定 ID 映射 | 用户需要在问题列表和 remediation 计划之间来回定位，也无法就当前问题直接追问或标记本次不处理 | A |
| “无问题”文案过度乐观 | 当前中文文案为“未发现问题，做得不错！” | 容易把有限覆盖误解为质量背书，与可信度和未覆盖提示冲突 | A |
| Follow-up 的本次复审范围不够清楚 | 已有精确范围恢复和独立 follow-up session，但结果入口主要表达“审核修复 / 查看审核” | 用户不知道本次复审覆盖了原范围、Fixer 改动还是整个工作区，也难判断摘要中的变化来自哪里 | A |
| 成本确认只给开始或取消 | L2/L3 方案框展示输入提示词 Token、调用数和并发，但主要动作只有“开始审核 / 取消” | 用户看到成本却不能直接选择“只保留核心检查”，也不清楚新增覆盖的具体收益 | A |
| PR Review 仍是平行实现 | PR Review MiniApp 自行推荐 fast / focused / deep 并直接生成 AI Review 草稿；另一处 PR 面板按工作区和文件重叠启发式筛选 Review 会话，主界面主要呈现相关数量 | 同一个“审查 PR”目标存在两套强度、生成和结果心智，且相关会话不等于同一 PR 的可信结果，用户无法确认是否重复消费或结论一致 | A |
| 成本估算和实际结果未形成闭环 | 确认框明确只估算 Reviewer 提示词输入；Review 结果路径没有直接对比预计和实际审查消耗 | 用户难以形成对下一次 Review 的成本预期 | B |
| 跨 Review 问题身份和失效规则尚未定义 | 当前 issue 无稳定 ID，remediation ID 依赖位置和顺序；PR 关联也仅依赖工作区和文件重叠 | 无法可靠承诺旧问题关闭、结果新鲜度或同 diff 复用；必须先明确身份、持久化所有者、兼容和失效契约 | B |
| 自动触发策略尚未统一 | 当前重点是本地显式 Review 和任务内 Review，PR Review MiniApp 仍有独立生成路径 | 在统一结果身份和 PR 收敛前增加自动触发，会放大重复 Review、Token 和噪声 | B |
| 用户反馈无法帮助收敛噪声 | 问题有 certainty，但没有“有用 / 错误 / 重复 / 非本次引入”等轻量反馈 | 系统难以识别长期无效规则，用户也无法阻止同类问题重复出现 | B |
| 设置仍偏运行时参数 | 普通设置页直接展示最大并行审核工作和最长排队等待 | 参数对排障和高级用户有价值，但普通用户更关心速度、成本和是否自动触发 | B |
| Review 跨界面文案尚未完全收敛 | PR Review 面板仍有较多英文主文案，部分标准 Review 等待提示仍使用“严格覆盖”表达 | 中文用户和普通 Review 用户会误判当前产品状态或审查强度 | B |
| 大任务控制台仍是设计而非真实场景闭环 | 文档已有单控制台原则，当前 Review UI 主要覆盖 strict Review 自身状态 | 不应提前扩张；应等待迁移、CI 失败队列等真实场景提供状态和交互需求 | C |

优先级说明：A 为下一次 Review 产品优化应优先解决；B 需要团队或 PR 场景和数据前置；C 只在真实大规模任务成立后设计。这里的 A/B/C 不对应 SDLC Harness 的 P0-P4。

## 7. 候选产品需求

### 7.1 A：完善本次 Review 内的问题处理

目标：近期让用户在一个面板内完成“理解问题 -> 决定动作 -> 发起修复 -> 查看本次复审摘要”；只有稳定身份和失效契约成立后，才承诺跨 Review 的问题关闭。

近期需求只复用本次 Review 已有报告、修复计划和侧栏，不建立跨 Review 生命周期：

- 每个问题稳定展示：严重程度、确定性、位置、简短证据、验证方式或未验证原因。
- Reviewer 可引用父任务中已经产生且能可靠归因的测试、CI 或命令结果；只读 Reviewer 不为此获得写入或任意命令权限，缺少证据时继续标记为未验证。
- 第一阶段继续使用现有修复计划项选择并交给 ReviewFixer；问题卡只增加 `不处理` 和 `带上下文追问`，追问复用当前 Review 侧栏和现有会话，不新建讨论线程或领域对象。
- 问题卡提供直接 `修复` 前，必须在本次 Review 内具备稳定的 `finding_id`、`remediation_id`，以及明确的一对一或一对多映射。该契约缺失时，不允许用标题、位置、顺序或文本相似度推断“修复此问题”。
- ReviewFixer 只消费用户选中的修复计划项；Reviewer 的只读边界保持不变。
- `不处理` 只改变当前 Review 内的呈现；是否形成项目规则或反馈数据必须另行授权，不能静默学习为永久忽略。
- 修复后的近期复审继续复用原范围和已选修复计划，结果摘要只说明本次覆盖及变化，不把启发式匹配写成原问题已关闭。
- “无问题”统一表达为“在本次已覆盖范围内未发现需要处理的问题”，并紧邻展示未覆盖和未运行验证。

跨 Review 关闭和增量对照属于后续候选，进入实现前必须另行明确稳定 Review/result/finding 身份、PR head 或 diff 变化后的失效规则、Agent Kernel 或既有 session metadata 的持久化责任、历史结果兼容和删除策略。只有这些契约成立，复审结果才能按原问题展示 `已确认修复`、`仍存在`、`无法确认`、`新发现`；默认仍只复核被修复问题及其直接影响范围，跨边界、验证失败或用户显式要求时才建议全量 Review。若既有所有者无法承载，不在本需求中临时新增第二套 Finding 生命周期。

验收心智：用户能回到原 Review 范围查看最新复审摘要，不会看到由启发式关联生成的“已确认修复”，也不会把有限 Review 当成完整质量证明。

### 7.2 A：把成本确认改成收益授权

目标：用户在 Review 扩大成本前，能判断是否值得，而不是理解 Reviewer 编排。

需求：

- 方案标题解释为什么扩大覆盖，例如“此次变更涉及权限和跨模块接口，建议增加安全与架构检查”。
- 默认展示选中的覆盖领域、未纳入领域和原因，不展示内部 Agent 类型。
- Token 使用区间必须标明估算口径；调用数和并发移入次级详情。
- 至少提供三个动作：`按建议审核`、`只保留核心检查`、`暂不审核`。
- “只保留核心检查”应保留最高风险覆盖，不允许用户误以为效果与完整方案相同。
- Review 结束后，在已有会话用量可归因时展示实际消耗与启动估算的区间关系；不可归因时明确写“无法单独统计”，不制造精确数字。

### 7.3 A：收敛 PR Review 并完成最小投影

目标：用户在 PR 面板判断“是否需要先处理问题”，并消除 PR Review MiniApp 与统一 Review 之间重复的强度选择和 AI 生成心智。

最低要求：

- 将当前 PR Review MiniApp 的 fast / focused / deep 和独立 AI 草稿路径明确标记为待收敛现状，不再为其新增策略、设置或 Reviewer 能力。
- PR adapter 继续拥有 provider、PR identity、远程 diff 读取、URL 直开、无本地 checkout 和评论发布生命周期；它把 provider-specific diff 映射为统一 Review 可消费的目标事实，并消费统一的质量决策和结果投影。PR identity、head SHA 和发布能力不进入平台无关的 Review 决策层。
- 统一 Review 只提供自适应强度、只读边界、AI Review 建议、覆盖和成本授权；PR MiniApp 不能保留第二套质量决策。
- PR 投影必须保持三层来源：代码托管平台提供 CI、审批、branch protection、mergeability 等原始权威事实和最终执行约束；既有“变更就绪度 / 可选 PR 门禁”模块消费证据、确定性失败、团队策略、残余风险接受和人工决策，生成 BitFun 的摘要、advisory、required 或 blocking 投影；统一 Review 的 finding 只作为输入和建议。
- AI Review 只能表达“建议优先修复”或“建议人工确认”，不能单独产生 BitFun 门禁或被命名为“阻止合并”。BitFun 的 required / blocking 仍只能来自既有门禁契约规定的确定性失败、组织策略、安全拒绝、未接受的明确残余风险或用户显式强策略。
- PR 摘要同时展示平台原始状态、BitFun 变更就绪度 / 门禁投影和 AI Review 建议优先处理的问题数量、最高风险、覆盖缺口、结果时间，并标明各自来源。
- 工作区或文件重叠只能标记“可能相关 Review”，不能证明结果仍匹配当前 PR diff。稳定 PR identity、head/diff 身份和失效规则具备后，才展示结果新鲜度、过期提示或同 diff 复用；旧结论不得作为当前合并状态。
- 提供“打开原 Review”作为主要下钻动作，不在 PR 面板复制完整报告或问题处理器。
- 目标态 PR 面板不独立启动 Reviewer、不实现自动触发策略、不直接修改代码；这些动作继续由统一 Review 和 ReviewFixer 承担。
- 主文案、空状态、认证失败和不可用状态必须完成现有语言资源覆盖，不形成独立英文体验。

前置关系：最小投影先复用平台已有事实和可追溯的 Review 摘要；跨 Review 问题关闭、结果新鲜度和同 diff 复用，必须等待 7.1 的稳定身份与失效契约。不能为了兼容当前 PR MiniApp 再新建一套 Finding、Review 状态、provider 生命周期或迁移平台。

### 7.4 B：定义自动 Review 的产品刹车

目标：在 PR 和团队场景中自动发现问题，但不在用户无感知时持续放大成本。

候选规则：

| 场景 | 默认行为 |
|---|---|
| 本地普通任务 | 仅用户明确要求时启动独立 Reviewer；高风险默认只形成 Review 建议，不自动启动 |
| 本地“准备提交 / 创建 PR” | 建议一次 Review；低风险保持 L1，高风险再请求扩大覆盖 |
| PR 草稿持续 push | 默认不每次自动 Review；保持等待或去抖状态 |
| PR 标记 ready | 团队启用时运行一次 Review |
| Review 后少量修复 push | 仅复核关联问题和新增变化 |
| 大范围重写或基线变化 | 建议全量 Review，并重新进行成本授权 |
| 高频 push | 自动暂停后续 Review，等用户或策略确认“准备好再看” |

除用户明确要求的本地 Review 外，上述 PR / 团队自动化均是尚未采纳的候选规则；实现前必须单独评审触发授权、收益和成本。高风险事实本身不能启动 Reviewer；团队自动 Review 需要已启用的明确策略。关联问题增量复核还必须等待 7.1 的稳定身份、持久化和失效契约。

设置原则：

- 普通设置不增加 L1/L2/L3、Reviewer 数量、Judge 或 Workflow 选项。
- 当前并发数和排队等待属于高级容量设置，可保留但不作为普通用户的主要 Review 偏好。
- 团队标准优先来自现有仓库指令、路径规则和组织策略，不在 UI 重复创建规则系统。

### 7.5 B：建立低噪声反馈闭环

目标：判断 Review 是否值得，而不是追求发现数量。

需求边界：

- 第一阶段不新增独立反馈入口；复用“不处理”的可选原因收集最小信号，不立即建设组织分析后台。
- 稳定问题身份、归因和隐私边界未完成前，原因只服务当前 Review 呈现，不形成跨 Review 学习。
- 用户反馈不能直接改写安全规则、组织门禁或全局 Reviewer prompt。
- 只有问题身份、隐私和 QDP 事件口径稳定后，才评估跨 Review 学习和规则效果分析。

### 7.6 C：大规模任务的单一控制台

只在 S3-S5 真实场景进入实现时采纳：

- 顶层只显示一个任务：目标、完成/阻塞/失败数量、阶段、耗时、Token 和需用户决策项。
- 默认不显示每个 Worker 的完整推理或聊天；用户下钻后才看输入、范围、结果和错误。
- 支持暂停、停止、保留已完成结果、跳过低优先项和调整范围。
- 新空闲 Worker 可以领取无依赖且无冲突的队列项，但这是调度行为，不是普通用户需要理解的概念。
- 多个独立用户任务才进入任务列表；一个任务内部的 64 个 Worker 永远不表现为 64 个 GUI 会话。

## 8. GUI 体验要求

### 8.1 普通 Review

- 从当前任务、文件变更或 `/review` 就地进入。
- 启动后用一行状态表示正在审查，不打开新窗口。
- 完成后先展示结论和必须处理的问题，再展示建议、覆盖和高级详情。
- 用户看不到“降级”“仅 L1”或“少用了 Reviewer”；产品表达为“已选择与当前变更匹配的检查”。

### 8.2 扩大覆盖

- 只有成本或耗时显著增加时才出现方案确认。
- 方案卡回答四个问题：为什么需要、增加哪些覆盖、大致成本、如何收敛。
- 用户拒绝扩大覆盖后，保留可安全完成的核心 Review，不把拒绝等同于取消全部审查。

### 8.3 结果与修复

- 问题列表是主视图，Reviewer 来源和运行细节是辅助信息。
- `不处理`、追问等非写入动作直接作用于问题；修复在稳定 `finding_id` / `remediation_id` 及一对一或一对多映射建立前继续使用修复计划项。映射建立后，问题卡修复必须解析到明确的 remediation 集合。
- 批量修复只在问题独立、无需产品决策且用户明确选择时出现。
- 修复和复审保持在同一侧栏位置，用状态变化表达进展，不新增界面。

### 8.4 并发和长任务

- GUI 默认显示阶段和异常，不滚动展示所有 Agent 日志。
- 顶层优先级为：需要用户决定、失败/冲突、预算风险、总体进度、Worker 详情。
- Token 和耗时展示实际值与预算，不用“正在运行 8 个 Agent”替代进度。

## 9. Token、耗时和解决率平衡

Review 的目标不是最大化覆盖，而是在当前约束下提高用户做出正确下一步决定的概率。

| 决策 | 产品倾向 |
|---|---|
| 是否增加 Reviewer | 只有新增视角能覆盖具体风险时增加，不因可并发而增加 |
| 是否自动复审 | 优先增量复核；高频变更自动暂停 |
| 是否全量重审 | 只在基线变化、跨边界影响、验证失败或用户明确要求时建议 |
| 是否继续循环 | 两轮没有新增有效问题，或问题只能重复推测时停止 |
| 预算不足 | 保留最高风险问题和已有成果，明确未覆盖，不用模糊的“完成”收尾 |
| 无可靠 oracle | 把结论标记为推断或待确认，不追加大量 Reviewer 制造虚假信心 |

任务结果应优先于问题数量。比“总 Token”更有意义的候选观察项包括“每个已解决任务增加了多少 Review 成本”和“Review 是否帮助用户更快达到可提交、可合并或明确继续修复的状态”；每个有效问题的 Review 成本只作为诊断指标。所有指标都只能在任务类型、问题状态和反馈数据稳定后进入正式 metrics spec。

## 10. 候选成功标准

这些是研究观察项，不是已经采纳的正式 KPI：

| 观察项 | 用途 | 防止的劣化 |
|---|---|---|
| Review 后任务目标达成率 | 按任务类型判断 Review 后是否达到可提交、可合并或用户定义的完成状态 | 发现更多问题却没有提高真实解决率 |
| Review 决策时间 | 从启动 Review 到用户选择提交、继续修复或停止的耗时 | 报告更完整但用户更难作决定 |
| 修复后重新打开或回归率 | 观察 Review 修复在后续验证或真实使用中是否反复失败 | 关闭问题只停留在界面状态 |
| 每个已解决任务的 Review 成本 | 按任务归因 Review 增量 Token 和耗时 | 用高成本换取有限任务收益 |
| 首个有效问题时间 | 判断 Review 是否及时产生价值 | 为完整报告等待过久 |
| 有效问题确认率 | 判断问题是否值得用户处理 | 评论数量上涨但噪声更大 |
| 重复问题率 | 判断增量复审是否真正收敛 | 修完又重复报同一问题 |
| 修复后关闭率 | 判断 Review -> Fixer -> Review 是否闭环 | 只生成修复任务，没有确认结果 |
| Review 取消/自动暂停率 | 判断成本提示和自动触发是否合理 | 用户被频繁打断或成本失控 |
| 估算区间偏离率 | 判断成本提示是否可信 | 输入 Token 估算被误解为总成本 |
| 人工升级为全量 Review 的比例 | 判断自适应选择是否足够 | 默认范围过轻或过重 |

正式采集前必须定义任务类型、完成判定、问题身份、分母、归因窗口、隐私边界和 QDP 事件，并回填 [governance/metrics-spec.md](../governance/metrics-spec.md)；不因本调研新增埋点。任务目标达成率和每个已解决任务成本是结果指标，问题确认率和关闭率只能作为诊断指标。

## 11. 明确非目标

- 不新增 DeepReview、ReviewTeam、PR Review、Verify Gate 或 Workflow Queue 平级入口。
- 不让用户选择 L1/L2/L3、Reviewer 数量、Judge 或内部 Agent 拓扑。
- 不默认在每个任务、每次 push 或每个文件上启动独立 Reviewer。
- 不把多 Agent 数量、运行时长或 Token 消耗包装为质量本身。
- 不让 Reviewer 在同一身份中修改代码。
- 不在“无问题”时承诺安全、正确或可合并。
- 不为普通 Review 建立任务控制台、组织分析后台或新的规则 DSL。
- 不在缺少可靠 oracle 时用重复 Review 替代验证。
- 不为兼容 `/DeepReview` 继续扩展专属产品能力。

## 12. 推荐推进顺序

1. **先做本次 Review 内的问题处理和诚实结论**：复用现有报告、修复计划、ReviewFixer 和 follow-up session，不承诺跨 Review 关闭，耦合最小。
2. **再完善收益授权，并单独评审增量身份契约**：补齐“核心检查”选择和实际成本反馈；旧问题状态对照必须先明确稳定身份、持久化和失效规则。
3. **再完成 PR 最小投影**：先分层展示平台原始权威事实、BitFun 变更就绪度 / 门禁投影与可追溯的 AI Review 建议；稳定身份契约完成后才增加结果新鲜度和同 diff 复用。
4. **PR 场景稳定后再定义自动触发**：先一次性和 ready 后 Review，再评估持续审查。
5. **有真实批量任务后再做控制台**：不先建设通用 Workflow 产品。
6. **有稳定反馈后再做分析和学习**：不让指标平台先于真实问题处理价值。

## 13. 参考资料

- [Bun: Rewriting Bun in Rust](https://bun.com/blog/bun-in-rust)
- [GitHub Copilot: Using code review](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review)
- [GitHub Copilot: Configuring automatic code review](https://docs.github.com/en/copilot/how-tos/copilot-on-github/set-up-copilot/configure-automatic-review)
- [Claude Code Review](https://code.claude.com/docs/en/code-review)
- [Claude Code: Dynamic workflows](https://code.claude.com/docs/en/workflows)
- [Claude Code: Run agents in parallel](https://code.claude.com/docs/en/agents)
- [OpenAI: Introducing upgrades to Codex](https://openai.com/index/introducing-upgrades-to-codex/)
- [OpenAI: Introducing the Codex app](https://openai.com/index/introducing-the-codex-app/)
- [Cursor Bugbot](https://cursor.com/bugbot)
- [Cursor: Bugbot performance, local review and incremental review (2026-06-10, accessed 2026-07-10)](https://cursor.com/changelog/bugbot-updates-june-2026)
- [CodeRabbit code review overview](https://docs.coderabbit.ai/guides/code-review-overview)
- [CodeRabbit automatic review controls](https://docs.coderabbit.ai/configuration/auto-review)
- [CodeRabbit IDE extension](https://docs.coderabbit.ai/ide)
- [Devin Review](https://docs.devin.ai/work-with-devin/devin-review)
- [Qodo Code Review experience](https://docs.qodo.ai/code-review)
- [Graphite AI Reviews](https://graphite.com/docs/ai-reviews)

# BitFun Review 产品体验竞品基准与优化需求

> 调研日期：2026-07-10。
>
> 范围：复盘统一 `/review`、自适应审查、只读 Reviewer、PR / Git range 目标证据、修复后复审、成本确认和 GUI 并发表达，并对照同类产品确认下一步产品优化项。
>
> 本文是产品研究和候选需求，不是技术设计、实施计划或新增 SDLC Harness 阶段。是否进入实现，仍需回填 [product-requirements.md](../product-requirements.md)、[agent-workflow-staged-plan.md](../agent-workflow-staged-plan.md) 和对应实施计划。本文不授权新增 Review、Verify 或 Workflow 平行系统。

## 1. 结论先行

BitFun 已经完成了最重要的方向性收敛：用户只需要理解 `Review`，系统根据目标、范围和风险选择最小充分强度；Reviewer 默认只读，修复交给独立执行身份；普通任务中的审核留在原任务上下文，严格审核扩大覆盖前需要确认成本。

当前 PR 只闭合 **Review 目标证据正确性**：显式 Git range 会先解析为完整 SHA，再形成 session-scoped 的文件状态、完整度与 workspace binding；`GetFileDiff` 对 prepared target 使用有界精确 diff、opaque cursor 和 reviewer 聚合预算，并对越界或不可消费证据 fail closed。显式 Git range 完整且无遗漏时可为 `complete`；当前工作区没有 immutable snapshot，因此最终始终为 `limited`，不得给出 clean approval。PR provider 取证和 PR Review MiniApp 接线不是已承诺后续。

与本次选取的公开产品相比，下一步最值得做的不是增加更多 Reviewer、模式或设置，而是把 Review 从“生成一份报告”提升为“帮助用户做出继续修复、人工确认或提交决定”的决策支持：

1. **先固定目标，再讨论结论**：本地改动、明确 Git range 和 PR 都必须在启动前形成带 base/head、文件状态、diff 引用和完整度的只读目标证据；Reviewer 不自行猜 ref 或把工作区 `HEAD` 当作任意目标。
2. **先证明目标闭环，再增加结果动作**：本轮不新增不处理、追问、问题生命周期或自动修复；这些只有在 PR 单入口上线后仍有明确决策障碍时才重新评估。
3. **PR 只保留一个 Review 心智**：PR adapter 负责 provider、身份、远程 diff 和发布；统一 Review 负责审查；PR 面板只投影平台事实、就绪度和 AI 建议。
4. **把增量、自动化和分析后置**：跨 Review 问题关闭、自动触发、反馈学习和组织分析都依赖更重的身份、持久化和归因，不进入当前 PR。
5. **把并发复杂度留在后台**：普通 Review 仍是一张结果面板；大任务控制台继续等待真实批量场景。

短期实施硬限制为当前一个 PR：只做目标证据正确性。PR 单入口与最小投影仅是条件式候选；只有上线数据证明存在高频且可由一个低耦合动作解决的决策障碍时才重新立项。除此之外不建设跨 Review 生命周期、自动 Review、通用 Workflow DSL、独立 Verify 页面、Reviewer 编排器、完整远程仓库缓存或新的团队治理后台。

## 2. 调研方法和证据边界

本次调研使用三类证据：

- BitFun 合入后的产品文档、中文文案和关键 Review UI 组件。
- 竞品截至调研日可访问的官方文档和产品公告。
- 竞品自己披露的成本、触发、限制和使用建议。

边界：

- 官方资料能证明产品公开行为，不能证明实际召回率、误报率或不同模型之间的真实效果。
- 各产品的 Token、信用点、Actions 分钟和订阅成本口径不可直接横向换算。
- Claude Code Review、Agent Teams、GitHub Copilot 中等 Review effort 等能力仍包含 preview 或 experimental 状态，不能直接当作稳定行业标准。
- OpenAI 已公开 Codex Review 的产品行为，但没有公开完整云端 PR Review 内部实现；本文只引用其可验证的目标、代码库上下文和验证行为。OpenCode 的 CLI、Agent 和权限文档能够证明本地 PR checkout 与命令级 Git 权限方式，但不能证明其等价于托管式、跨 provider PR Review。
- 本文中的“对 BitFun 的启发”是产品推断，和竞品公开事实分开表达。

## 3. BitFun 当前产品基线

| 能力 | 当前状态 | 产品判断 |
|---|---|---|
| 单一入口 | `/review` 和 GUI Review 共用自适应决策；`/DeepReview` 只做历史兼容 | 方向正确，应继续隐藏 L1/L2/L3 和 DeepReview 心智 |
| 审查强度 | 根据目标事实、风险和用户严格意图选择 L1-L3 | 方向正确，不应增加更多用户可选档位 |
| 独立性 | CodeReview / DeepReview 只读，ReviewFixer 单独执行修复 | 是可信 Review 的必要底线，应保持 |
| 任务内审核 | 用户要求“完成并仔细审核”时，使用一个隔离 Reviewer，不另开产品页面 | 符合低摩擦任务心智 |
| 目标证据 | 显式 Git range 形成 immutable-SHA target；当前工作区以有界 diff、聚合预算内的未跟踪文件指纹和冲突事实改善覆盖，但由于可变性始终为 `limited`；rename 保留 old/new path；`GetFileDiff` 对 prepared target 不回退错误 baseline 或无界全文，使用 opaque cursor、单文件边界和 reviewer 聚合预算 | 当前 PR 只闭合本地 workspace / Git range 的证据契约、传播和 fail-close；PR provider 与 MiniApp 统一接入需由指标重新立项；remote range 无 exact diff 时直接给可恢复错误，不自动 checkout 或缓存完整远程仓库 |
| Reviewer Git 边界 | CodeReview、DeepReview 和 specialist reviewer 保留既有 `Git` 暴露以兼容旧 PR/历史诊断场景，但 prepared work packet 不把它作为 changed-code 证据；目标绑定 diff 只通过 `GetFileDiff` 消费，`matching_clean` 时普通 Read/Grep/Glob/LS 仅补充仓库上下文 | 不增加新的 Git 工具或每次调用的全仓状态扫描；不让既有 Git 猜测/扩大 prepared refs，也不增加 shell、fetch、checkout 或测试执行 |
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
| OpenCode | `opencode pr <number>` 先获取并 checkout GitHub PR，再在该本地工作区启动 OpenCode；官方只读 Review Agent 示例禁止 edit，并只允许 `git diff`、`git log*` 等特定 bash 命令（[CLI](https://opencode.ai/docs/cli/)，[Agents](https://opencode.ai/docs/agents/)） | 本地 PR 先建立确定 checkout；权限可收敛到命令级，不必在 Reviewer 中开放全部 Git 或 shell | BitFun 借鉴“先确定目标、再限制权限”，但 PR1 只保留目标绑定 `GetFileDiff` 和 clean checkout 的普通只读上下文，不复制额外 Git 命令面 | 不把自动 fetch/checkout 作为所有 PR Review 前置；不把通用 shell、完整 Git 或新的多操作 Git 工具包装成只读 |
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

1. **目标先固定**：PR/range 必须绑定 base/head 与 diff；结论和行内评论不能依赖当前工作区或文件重叠猜测。
2. **入口就近**：本地改动在当前任务或 IDE，PR 审查在 PR；用户不应先去独立 Review 产品中心。
3. **结果问题化**：结果围绕问题、严重程度、证据、位置和动作组织，不围绕 Agent 名称组织。
4. **候选再验证**：多 Agent 的主要价值是提高候选覆盖，必须有去重、验证或独立复核来降低噪声。
5. **只读不等于无上下文**：prepared `GetFileDiff` 提供变更事实，clean checkout 可用现有 Read/Grep/Glob/LS 补充上下文；不因此新增 Git 工具、网络 fetch 或任意 shell。
6. **修复需交接**：Review 和修改身份分开，但问题到 Fixer 的上下文交接应尽量一键完成。
7. **增量和自动化可停止**：它们有价值，但依赖稳定身份和成本刹车，不能先于首次 Review 正确性。
8. **并发不等于多窗口**：大任务显示一个聚合进度；多个互相独立的用户任务才显示为任务列表。

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
| PR MiniApp 尚未消费统一目标证据 | 当前 PR 只提供本地 workspace/Git-range contract；provider 取证和现有 MiniApp 的独立 AI 草稿路径不在范围 | 同一 PR 仍可能出现两个 Review 心智和重复结论 | A1，仅在上线指标证明其为高频障碍后重新立项 |
| 远程或不匹配工作区只能使用降级证据 | 当前 PR 不自动 fetch、checkout、建 worktree 或缓存完整远程仓库；binary、超限和远程缺口会标为 partial/unknown 或在启动前停止 | 个别场景的上下文覆盖低于本地 clean checkout，但不会伪装成完整覆盖 | 接受的边界；当前 PR 不扩展远程仓库系统 |
| 问题与修复计划缺少稳定映射和就地非写入动作 | `CodeReviewToolCard` 展示 severity、certainty、validation note 和 suggestion，但问题本身没有“不处理 / 追问”动作；修复计划在另一区块统一选择 | 用户需要在问题列表和 remediation 计划之间来回定位 | B；先观察 PR 单入口后的真实决策障碍，不预先承诺新动作 |
| “无问题”文案可能被过度解读 | 当前结论仍需结合覆盖提示理解 | 有限覆盖可能被误解为质量背书 | B；PR1 已保证有限证据产生 warning，不单独扩张结果系统 |
| Follow-up 的本次复审范围不够清楚 | 已有精确范围恢复和独立 follow-up session，但结果入口主要表达“审核修复 / 查看审核” | 用户可能需要额外理解复审范围 | B；先用现有范围元数据，是否增加文案由数据决定 |
| 成本确认只给开始或取消 | L2/L3 方案框展示输入提示词 Token、调用数和并发，但主要动作只有“开始审核 / 取消” | 用户看到成本却不能直接选择“只保留核心检查”，也不清楚新增覆盖的具体收益 | B；不阻塞目标正确性和 PR 收敛 |
| PR Review 仍是平行实现 | PR Review MiniApp 自行推荐 fast / focused / deep 并直接生成 AI Review 草稿；另一处 PR 面板按工作区和文件重叠启发式筛选 Review 会话，主界面主要呈现相关数量 | 同一个“审查 PR”目标存在两套强度、生成和结果心智，且相关会话不等于同一 PR 的可信结果，用户无法确认是否重复消费或结论一致 | A |
| 成本估算和实际结果未形成闭环 | 确认框明确只估算 Reviewer 提示词输入；Review 结果路径没有直接对比预计和实际审查消耗 | 用户难以形成对下一次 Review 的成本预期 | B |
| 跨 Review 问题身份和失效规则尚未定义 | 当前 issue 无稳定 ID，remediation ID 依赖位置和顺序；PR 关联也仅依赖工作区和文件重叠 | 无法可靠承诺旧问题关闭、结果新鲜度或同 diff 复用；必须先明确身份、持久化所有者、兼容和失效契约 | B |
| 自动触发策略尚未统一 | 当前重点是本地显式 Review 和任务内 Review，PR Review MiniApp 仍有独立生成路径 | 在统一结果身份和 PR 收敛前增加自动触发，会放大重复 Review、Token 和噪声 | B |
| 用户反馈无法帮助收敛噪声 | 问题有 certainty，但没有“有用 / 错误 / 重复 / 非本次引入”等轻量反馈 | 系统难以识别长期无效规则，用户也无法阻止同类问题重复出现 | B |
| 设置仍偏运行时参数 | 普通设置页直接展示最大并行审核工作和最长排队等待 | 参数对排障和高级用户有价值，但普通用户更关心速度、成本和是否自动触发 | B |
| Review 跨界面文案尚未完全收敛 | PR Review 面板仍有较多英文主文案，部分标准 Review 等待提示仍使用“严格覆盖”表达 | 中文用户和普通 Review 用户会误判当前产品状态或审查强度 | B |
| 大任务控制台仍是设计而非真实场景闭环 | 文档已有单控制台原则，当前 Review UI 主要覆盖 strict Review 自身状态 | 不应提前扩张；应等待迁移、CI 失败队列等真实场景提供状态和交互需求 | C |

优先级说明：A0 是当前 PR 的 Review 正确性前置；A 的 PR 单入口仅是条件式候选；B 需要身份、成本或数据证明且不进入本轮；C 只在真实大规模任务成立后设计。这里的 A0/A/B/C 不对应 SDLC Harness 的 P0-P4。

## 7. 候选产品需求

### 7.1 A0：闭合本地 workspace / Git range 目标证据

目标：当前工作区和明确 Git range 的 Reviewer 分析用户指定的同一变更集，并能诚实说明缺失证据；PR/provider 取证需按 7.3 的指标门槛另行立项。

最小契约：

- 每次 Review 启动前生成 session-scoped 目标清单，至少包含 `source`、可证明的 `base_revision` / `head_revision`、目标指纹、文件新旧路径、增删改/重命名/删除状态、规范化 diff 引用、完整度和限制原因。完整 SHA 可声明内容不可变；live workspace 在有界 diff、冲突状态和未跟踪内容指纹都可用时可声明覆盖完整，但必须保留可变新鲜度限制。
- manifest 和 evidence pack 继续只携带元数据与受控引用，不内嵌完整 provider body、全仓源码或大段 diff；不为该契约新建长期数据库、第二套 Artifact Graph 或 Finding 生命周期。
- 当前工作区 prepared Review 固定使用一次 `HEAD -> worktree` 有界取证，但没有 snapshot，因此保持 `limited`；显式 Git range 必须由目标准备层固定 base/head 并生成准确 diff，Reviewer 不自行解释 ref。
- Reviewer 不新增通用或多操作 Git 工具，也不删除旧入口已有能力。prepared target 只通过禁用 external diff/textconv 的有界 `GetFileDiff` 消费变更；既有 Git 不得覆盖或扩大 prepared target；`matching_clean` 时现有 Read/Grep/Glob/LS 仅补充 live context，不做每次工具调用的全仓状态扫描。
- 删除文件、重命名、二进制、过大、冲突或未跟踪内容不可读必须有显式状态；任何缺口都进入独立的 target-evidence 覆盖说明。
- 文件名和 diff 内容视为不可信输入，不能改变工具权限或门禁策略。

验收心智：clean checkout 不会得到空 Review；明确 range 不会退化成工作区 diff；无变更、无法解析、remote range 和超限证据在 Reviewer 启动前停止或诚实降级；证据缺失时绝不显示“已完整检查”。

### 7.2 B：结果动作仅保留为待验证机会

这不是当前承诺。现有报告、修复计划和 ReviewFixer 已能完成主要闭环；只有当前目标证据改动上线后仍存在可量化的高频决策障碍时，才从下列候选中选择一个低耦合动作：

- 仅调整诚实“无问题”文案；或
- 仅增加一次带上下文追问；或
- 仅补一个 follow-up 范围说明。

不得把这些候选合并成一个预设 PR3。跨 Review 的 `finding_id`、不处理状态、关闭状态、增量对照、历史兼容和删除策略全部后置；没有稳定 owner 时不新增第二套状态模型。

### 7.3 A：收敛 PR Review 并完成最小投影

目标：PR 面板只保留一个“开始 Review”，让用户判断是否需要先处理问题，同时保留代码托管平台的权威事实和发布能力。

- 移除 PR Review MiniApp 的 fast / focused / deep 选择和独立 `app.ai.complete` 草稿路径；不再为其新增策略、设置或 Reviewer 能力。
- PR adapter 继续拥有 provider、PR identity、base/head、远程 diff、CI/审批/mergeability、URL、认证和评论发布生命周期，并把 7.1 的目标证据交给统一 Review。
- 若当前工作区与 PR head 确定匹配且整个工作区干净，可启用现有普通只读代码上下文；不匹配、dirty 或未 checkout 时只消费可用的 provider evidence，并清楚展示覆盖差异。
- PR 投影保持三层来源：平台原始权威事实；既有变更就绪度 / 可选门禁；AI Review 的问题、覆盖和建议。AI finding 不能单独产生 required / blocking。
- PR 摘要只显示建议优先处理的问题数、最高风险、覆盖缺口、目标 head 和结果时间；“打开原 Review”是主要下钻，不复制完整报告或问题处理器。
- 本轮不自动生成或发布 inline comment；保留现有手工 composer 和确认发布。
- head 变化后旧结果标记过期，不能作为当前合并状态；主文案、空状态、认证失败和 reduced coverage 使用现有语言资源。

### 7.4 B：收益授权与成本反馈

目标仍成立，但不阻塞当前 PR 的目标正确性。后续只在已有会话用量能够可靠归因时，评估“按建议审核 / 只保留核心检查 / 暂不审核”和预计/实际区间；不新增 Reviewer 档位或成本分析后台。

### 7.5 B：增量、自动触发和反馈学习

这些能力统一后置，不拆成并行实施线：

- 先有稳定 target/result/finding 身份、持久化 owner、head 失效和隐私边界，再讨论旧问题关闭、同 diff 复用或跨 Review 学习。
- PR 自动 Review 需要单独授权、去抖、暂停和预算规则；不默认每次 push 运行。
- 当前 PR 不新增“不处理”状态；未来若单独立项，也不得直接进入组织分析或 prompt 学习。

### 7.6 C：大规模任务控制台

继续只保留原则，不进入当前 PR：一个用户目标只显示一个任务、聚合进度和异常；只有真实 S3-S5 批量场景及可运行 oracle 成立后，才评估暂停、队列和 Worker 下钻。

## 8. GUI 体验要求

### 8.1 普通 Review

- 从当前任务、文件变更或 `/review` 就地进入。
- 启动前固定目标来源和 revision；普通用户只看“当前修改 / 指定变更 / PR #N”，base/head、完整度和检查路径放在可折叠详情。
- 启动后用一行状态表示正在审查，不打开新窗口。
- 完成后先展示结论和必须处理的问题，再展示建议、覆盖和高级详情。
- 用户看不到“仅 L1”或“少用了 Reviewer”；产品表达为“已选择与当前变更匹配的检查”。目标证据不完整属于必须展示的覆盖事实，不能被体验文案隐藏。

### 8.2 扩大覆盖

- 只有成本或耗时显著增加时才出现方案确认。
- 方案卡回答四个问题：为什么需要、增加哪些覆盖、大致成本、如何收敛。
- 用户拒绝扩大覆盖后，保留可安全完成的核心 Review，不把拒绝等同于取消全部审查。

### 8.3 结果与修复

- 问题列表是主视图，Reviewer 来源和运行细节是辅助信息。
- 当前继续使用已有报告、修复计划和 ReviewFixer；不新增“不处理”、追问、跨 Review 映射或问题级直接修复。
- 修复和复审保持在同一侧栏位置，用状态变化表达进展，不新增界面。

### 8.4 并发和长任务

- GUI 默认显示阶段和异常，不滚动展示所有 Agent 日志。
- 顶层优先级为：需要用户决定、失败/冲突、预算风险、总体进度、Worker 详情。
- Token 和耗时展示实际值与预算，不用“正在运行 8 个 Agent”替代进度。

## 9. Token、耗时和解决率平衡

Review 的目标不是最大化覆盖，而是在当前约束下提高用户做出正确下一步决定的概率。

| 决策 | 产品倾向 |
|---|---|
| 目标证据不完整 | 先缩小结论并显示缺口，不增加 Reviewer 掩盖错误或缺失 diff |
| 本地工作区不匹配 PR head | 使用 provider evidence 的 reduced coverage，不自动 checkout 或混入本地修改 |
| 是否增加 Reviewer | 只有新增视角能覆盖具体风险时增加，不因可并发而增加 |
| 是否自动复审 | 当前 PR 不实现；稳定身份和成本刹车成立后再评估 |
| 是否全量重审 | 只在基线变化、跨边界影响、验证失败或用户明确要求时建议 |
| 是否继续循环 | 两轮没有新增有效问题，或问题只能重复推测时停止 |
| 预算不足 | 保留最高风险问题和已有成果，明确未覆盖，不用模糊的“完成”收尾 |
| 无可靠 oracle | 把结论标记为推断或待确认，不追加大量 Reviewer 制造虚假信心 |

任务结果应优先于问题数量。比“总 Token”更有意义的候选观察项包括“每个已解决任务增加了多少 Review 成本”和“Review 是否帮助用户更快达到可提交、可合并或明确继续修复的状态”；每个有效问题的 Review 成本只作为诊断指标。所有指标都只能在任务类型、问题状态和反馈数据稳定后进入正式 metrics spec。

## 10. 候选成功标准

这些是研究观察项，不是已经采纳的正式 KPI：

| 观察项 | 用途 | 防止的劣化 |
|---|---|---|
| 目标解析正确率 | 当前工作区、显式 range 和 PR 的实际 base/head/diff 与用户目标一致 | 报告完整但审错变更集 |
| 目标证据完整度分布 | 统计完整、部分、截断、二进制和 provider 缺失，不把缺失算作通过 | 未覆盖文件被静默忽略 |
| 过期结果拦截率 | head 变化后旧结果和评论草稿不再发布或支撑当前就绪度 | 对错误 revision 发表评论或给合并建议 |
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
- 不扩大当前通用 `Git` 权限，也不新增多操作 Git 工具或任意 shell；prepared changed-code 只复用有界 `GetFileDiff`，旧入口继续保留原能力直至替代链路完整上线。
- 不默认 clone、fetch、checkout、创建 worktree 或缓存完整远程仓库。
- 不为目标证据新建长期数据库、第二套 Artifact Graph 或 provider 生命周期。
- 不在“无问题”时承诺安全、正确或可合并。
- 不为普通 Review 建立任务控制台、组织分析后台或新的规则 DSL。
- 不在缺少可靠 oracle 时用重复 Review 替代验证。
- 不为兼容 `/DeepReview` 继续扩展专属产品能力。
- 不在当前 PR 实现自动 Review、跨 Review 问题关闭、同 diff 复用、反馈学习、新 provider 或组织分析。

## 12. 当前 PR 与条件式后续

当前只承诺目标证据正确这一项，不允许把后置能力以“顺手预留”带入。后续首先观察目标解析失败率、limited evidence 比例、diff 预算耗尽率、Review 成功率和 token 变化。

| PR | 唯一目标 | 用户可见收益 | 必要范围 | 明确排除 | 退出条件 |
|---|---|---|---|---|---|
| 当前 PR：目标证据正确 | 显式 Git range 形成 immutable-SHA 目标；workspace 形成诚实的 limited 目标 | 不再审错 revision；精确文件/目录、clean checkout、删除/重命名、二进制、超限和远程缺口有诚实结果 | session-scoped target evidence；base/head、文件状态、完整度与 evidence status；上下文传播；prepared target/report fail-close；opaque cursor、单文件与 reviewer 聚合预算；uncertain launch 保留；降级和契约测试 | PR/provider 接线、额外 Git 工具、逐调用全仓重验、合成 diff refs、workspace 快照系统、增量缓存计划、自动 checkout、跨 Review identity、Reviewer shell/测试执行 | workspace/range/精确 scope 契约测试通过；越界路径不可达；不完整证据禁止 clean approval；普通 Agent 的既有 Git/diff 行为不变；token 不出现不可解释的大幅增长 |
| 条件式候选：PR 单入口与最小投影 | 仅当上线指标证明重复入口是高频决策障碍时，PR MiniApp 启动统一 Review 并分层展示平台事实、就绪度和 AI 建议 | 用户在 PR 原位置完成一次可信 Review | PR adapter 固定 identity/base/head/provider diff；移除独立 AI 草稿路径；head 失效；认证、空态、错误和 reduced coverage i18n | 未重新立项前不实现；即使立项也排除自动发布、自动/inline 评论映射、自动 Review、同 diff 复用、问题处理器、自动修复和新 provider | 必须先定义并验证入口重复率、目标错误率、成功率、耗时和 token guardrail |

合并判断：若当前 PR 为完成目标必须引入上述排除项，则先缩小用户承诺。合入后先观察真实 Review 的目标正确率、覆盖缺口、执行成功率、token 和用户决策时间；只有数据证明一个低耦合动作能显著降低决策时间时，才单独评估后续。

## 13. 参考资料

- [Bun: Rewriting Bun in Rust](https://bun.com/blog/bun-in-rust)
- [GitHub Copilot: Using code review](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review)
- [GitHub Copilot: Configuring automatic code review](https://docs.github.com/en/copilot/how-tos/copilot-on-github/set-up-copilot/configure-automatic-review)
- [Claude Code Review](https://code.claude.com/docs/en/code-review)
- [Claude Code: Dynamic workflows](https://code.claude.com/docs/en/workflows)
- [Claude Code: Run agents in parallel](https://code.claude.com/docs/en/agents)
- [OpenAI: Introducing upgrades to Codex](https://openai.com/index/introducing-upgrades-to-codex/)
- [OpenAI: Introducing the Codex app](https://openai.com/index/introducing-the-codex-app/)
- [OpenCode CLI: PR checkout](https://opencode.ai/docs/cli/)
- [OpenCode Agents: command-level review permissions](https://opencode.ai/docs/agents/)
- [GitHub REST: list pull request files](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests-files)
- [GitHub REST: pull request review comments](https://docs.github.com/en/rest/pulls/comments#create-a-review-comment-for-a-pull-request)
- [GitLab API: merge request diffs](https://docs.gitlab.com/api/merge_requests/#list-merge-request-diffs)
- [GitLab API: diff discussions](https://docs.gitlab.com/api/discussions/#create-a-new-thread-in-the-merge-request-diff)
- [Cursor Bugbot](https://cursor.com/bugbot)
- [Cursor: Bugbot performance, local review and incremental review (2026-06-10, accessed 2026-07-10)](https://cursor.com/changelog/bugbot-updates-june-2026)
- [CodeRabbit code review overview](https://docs.coderabbit.ai/guides/code-review-overview)
- [CodeRabbit automatic review controls](https://docs.coderabbit.ai/configuration/auto-review)
- [CodeRabbit IDE extension](https://docs.coderabbit.ai/ide)
- [Devin Review](https://docs.devin.ai/work-with-devin/devin-review)
- [Qodo Code Review experience](https://docs.qodo.ai/code-review)
- [Graphite AI Reviews](https://graphite.com/docs/ai-reviews)

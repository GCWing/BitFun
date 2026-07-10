# BitFun 智能体工作流与审查体验产品需求调整提案

> 范围：基于 Bun Rust 迁移文章、Claude Code dynamic workflows、GitHub Copilot cloud agent、Cursor Background Agent / Bugbot、Google Jules 和近期智能体 PR 研究，调整 BitFun 在长任务、并发任务、审查、token 成本和 GUI 交互上的产品需求。
>
> 本文是产品需求层调整，不是技术设计。它回答用户为什么需要这类能力、什么时候应该自动启用、什么时候应该收敛范围或停止、GUI 应如何表达并发，以及 BitFun 如何避免把工作流、证据和审查做成过重流程。

承接文档：

- [agent-workflow-staged-plan.md](agent-workflow-staged-plan.md)：将本文需求压回低风险任务、提交前审查、CI/测试失败、PR 评论批量修复和大规模迁移/审计等真实用户场景；不新增独立阶段路线。
- [architecture/agent-workflow-design.md](architecture/agent-workflow-design.md)：仅作为交互和边界补充，复用既有 Agent Kernel、DeepReview、Harness 和 QDP 契约，不定义新的核心对象模型。

权威边界：本文是候选产品调整提案，不替代 [product-requirements.md](product-requirements.md)、[implementation-plan.md](implementation-plan.md)、[governance/metrics-spec.md](governance/metrics-spec.md) 或 QDP 事件注册表。任何条目被采纳前，必须回填到对应权威文档；未回填前不得作为正式 PRD、阶段承诺、门禁规则或指标口径执行。

2026-07-10 状态说明：

- 已采纳并合入：统一 Review 主入口、DeepReview / ReviewTeam 内部化、低风险路径保护、L1-L3 自适应选择、只读 Reviewer 与 ReviewFixer 分离、同侧栏修复和 follow-up Review。对应 PR、commit、实现入口和回归测试见 [合入证据](review-verify-implementation-guardrails.md#合入证据)。
- 部分采纳：L2/L3 已有范围和输入提示词 Token 估算确认，但“保留核心检查”、实际成本反馈和增量问题状态仍需产品优化。
- 尚未采纳：既有 PR Review MiniApp 独立强度和 AI 草稿路径向统一 Review 收敛、通用动态 Workflow、CI / 测试失败队列、PR 自动复审策略、大规模任务控制台、独立 Verify 产品化和组织级 Review 分析。
- 合入后的详细竞品复盘和候选优化见 [research/review-product-experience-benchmark.md](research/review-product-experience-benchmark.md)；该文档不会自动把候选项升级为实施承诺。

## 1. 核心结论

BitFun 后续不应把 dynamic workflow 理解成一个需要用户学习的新模式，也不应把 DeepReview 做成独立且默认沉重的高级入口。更好的产品方向是：

1. **把用户任务自动映射到合适强度的执行策略**：低风险任务保持单 agent 快速完成；中风险任务追加轻量独立审查；高风险或大规模任务再进入多 agent、队列化、证据化的工作流。
2. **把并发能力做成 GUI 中的单一任务控制台**：用户看到的是一个任务、一个进度、一组阶段和异常，而不是 64 个窗口、64 个聊天或 64 条不可理解的日志。
3. **把审查和工作流从概念上后台化**：用户不需要理解 subagent、workflow、evidence pack、artifact graph。默认只看到任务状态、风险原因、成本预算、已验证/未验证项和下一步。
4. **把完成率、token、耗时做成产品级预算选择**：每次升级审查或并发执行，都要说明预估收益和成本；自动化可以启用，但不能让 token 在用户无感知时暴涨。
5. **把 DeepReview 收敛为统一 Review 的最高档策略**：普通 review、快速独立 review、定向多角色 review、严格审查应合并成一个自适应审查体验。
6. **把复杂治理能力收回到用户价值之后**：证据包、图谱、门禁、风险接受是后台支撑，只有在 PR、团队规则、发布、事故、合规或大规模迁移时显性化。

## 2. 业界参照与启发

| 来源 | 先进点 | 对 BitFun 的启发 | 需要避免的误读 |
|---|---|---|---|
| Bun Rust 迁移 | 用动态工作流把超大迁移拆成规则生成、任务队列、并发执行、独立审查、修复回路和强测试 oracle | BitFun 应学习“失败队列化、审查隔离、过程修复、测试真实性确认”，不是只学习并发数量 | 不应鼓励普通任务默认启动大量 agent，也不应把大规模迁移经验泛化到所有改动 |
| Claude Code dynamic workflows | workflow 可由脚本或 SDK 编排，适合批量、动态、可恢复任务 | BitFun 可以把工作流变成后台执行策略，而不是新的用户心智负担 | 不应让用户手写 workflow 才能获得价值 |
| GitHub Copilot cloud agent | 从 issue、dashboard、PR、CI 失败等入口异步启动任务，完成后创建或更新 PR，并请求人工 review | BitFun 应支持从任务、PR、CI、审查意见自然进入后台执行，并保留用户审核点 | 不应把 PR 作为所有任务的默认终点，个人本地任务仍要轻量 |
| GitHub Copilot code review | 自动 PR review、review effort level、可配置自动请求审查 | BitFun 的审查强度应该可配置、可自动触发、可按风险升级 | 不应把“自动审查”误做成无差别门禁 |
| Cursor Background Agent / Bugbot | 云端后台 agent、PR 自动审查、发现问题后回到编辑器修复、用 dashboard 展示用量 | BitFun 的 GUI 应围绕“后台任务控制台、异常优先、回到编辑器修复、成本可见”设计 | 不应在 GUI 上复制多终端或多聊天窗口 |
| Google Jules | 异步任务、GitHub 集成、安全云环境、多请求同时处理、用户先审计划和结果 | BitFun 可学习“异步托管任务 + 人类审核点 + 多请求队列”的产品形态 | 不应要求所有长任务都进云端，本地桌面仍是 BitFun 的强项 |
| 智能体 PR 实证研究 | 不同任务类型成功率差异明显，文档和测试更适合 agent，新功能和长期维护风险更高 | BitFun 应按任务类型选择审查强度和预算，不追求单一万能策略 | 不应只用短期合入率衡量成功，还要看返工、维护和 churn |

## 3. 用户核心诉求

用户真正关心的不是 workflow 本身，而是这些结果：

- 任务能不能完成，完成到什么可信度。
- 什么时候需要多花 token 和时间，为什么值得。
- 并发任务现在在做什么，有没有卡住、冲突或越权。
- AI 写出的改动是否经过合适强度的检查，而不是形式上跑了一个重流程。
- 小任务不要被大流程拖慢，大任务不要因为过度简化而失败。
- 高风险动作不要悄悄发生，低风险提示不要反复打断。
- 结果能不能自然变成 PR、验证摘要、审查修复或后续任务。

因此，产品需求应围绕“合适强度、清晰进度、成本可控、结果可信”设计，而不是围绕“工作流数量、subagent 数量、证据模型完整度”设计。

## 4. 产品定位调整

### 4.1 从固定模式改为自适应执行

现有体验容易把 Agentic、Plan、Debug、Review、DeepReview 等理解为不同模式。后续应调整为：

```text
用户表达目标
  -> BitFun 判断任务规模、风险、验证条件和预算
  -> 默认选一个最低足够强度的执行策略
  -> 风险或失败出现时渐进升级
  -> 完成后给出结果、证据摘要和可选后续动作
```

用户可以用自然语言表达“更快”“更稳”“只看安全”或“严格审查”，但 GUI 不为这些意图建立固定模式选择器，也不要求用户预先知道 DeepReview、workflow 或 subagent。

### 4.2 DeepReview 收敛为 Review 的最高档

建议统一成一个 `Review` 体验，内部策略可映射到四档；档位不作为普通用户入口或设置项：

| 内部档位 | 触发条件 | 用户看到什么 |
|---|---|---|
| L0 | 小改动、低风险、已有验证或用户追求速度 | 主 agent 自检、已验证/未验证摘要 |
| L1 | 小到中等 diff、局部 bugfix、准备提交前 | 一个独立 reviewer 的问题清单和可信度 |
| L2 | 触碰权限、性能、架构、前端体验、跨模块或缺验证 | 增加与风险匹配的独立覆盖，并说明成本 |
| L3 | 大迁移、核心接口、安全敏感、用户显式要求、团队强策略 | 最广适用覆盖、局部结果、修复建议和证据引用 |

现有 DeepReview 的队列、work packets、judge、partial coverage 和 action surface 仍有价值，但不应作为普通审查入口的默认形态。普通用户只应看到“审查强度已自动选择”。

迁移/兼容规则：

- 用户侧唯一主入口是 `Review`。
- `/DeepReview` 只能作为迁移窗口内的历史兼容输入，等价路由到 `Review: Strict`；默认导航、按钮和普通命令不应并列展示 `Review` 与 `DeepReview`。
- child session、auxiliary pane、work packets 和内部 capacity queue 应后台化为 L3 严格档实现细节；如果仍需要用户可见的辅助 pane，必须同步更新 DeepReview 架构文档并说明它不是第二个产品入口。
- 普通 review 输出必须合并到同一个 Review 面板，不能再把 DeepReview report 作为另一个窗口或另一个审查结果呈现。

## 5. TUI 与 GUI 的并发心智差异

TUI 用户能接受多个 terminal、多个进程、多个日志，因为核心心智是“我在控制执行”。GUI 用户更期待“系统替我管理复杂度，我只看状态和决策点”。因此 GUI 不应展示多个 agent 实例，而应展示一个任务控制台。

| 并发表达 | TUI 中合理 | GUI 中应如何表达 |
|---|---|---|
| 多个 agent 实例 | 多个 pane、多个命令、多个日志流 | 单一任务卡，内部显示 N 个 worker 正在处理 |
| 大量任务输出 | 用户自己 grep 或翻日志 | 默认聚合，只展示异常、阻塞、关键成果和成本 |
| 任务队列 | 文本列表或脚本输出 | 进度条 + 阶段 lane + 异常卡 + 可下钻任务表 |
| 冲突处理 | 命令行提示或手动 resolve | 明确显示冲突文件、占用 agent、推荐动作 |
| 成本 | 用户看 provider usage 或终端统计 | 任务头常驻 token/time/并发预算 |
| 审查结果 | 多个 reviewer 输出拼接 | 合并后的结论、分歧、未覆盖范围和可应用修复 |
| 手动控制 | kill process、改脚本、重跑 | 暂停、调整范围、停止、保留核心检查、追加预算 |

### 5.1 GUI 并发控制台

大规模任务在 GUI 中应表现为一个“任务控制台”，不是多个聊天窗口：

```text
任务标题：迁移 100 个 crates 到新接口
状态：运行中，42/100 完成，3 个阻塞，预计剩余 18 分钟
执行域：本地工作区 · 写入沙箱 ask · 网络 deny · 远程未启用
预算：已用 210k / 500k token，已用 34 / 90 分钟，并发 8 / 12

阶段：
  发现范围        100/100 完成
  生成迁移规则    1/1 完成，已审查
  执行迁移        42/100 完成，55 等待，3 阻塞
  独立审查        18/42 完成，2 个发现
  验证            15/42 完成，1 个失败

需要你决策：
  - crate X 与 crate Y 同时修改 shared contract，建议暂停 Y 等待 X 合并
  - token 预算预计不足，建议低风险 crate 只保留核心检查
```

这个控制台只在大规模任务中显性出现。普通任务不应进入这样的界面。

### 5.2 信息层级

GUI 的默认层级应是：

1. **一句话状态**：正在做什么、是否需要用户决策。
2. **执行域和沙箱状态**：本地/远程/云端、写入范围、网络、凭据和授权状态；该状态只做持续可见提示，不替代安全确认。
3. **阶段摘要**：发现、执行、审查、验证、收敛。
4. **异常优先**：冲突、失败、越权、预算不足、覆盖不足。
5. **成本与预算**：token、耗时、并发、剩余额度。
6. **下钻详情**：每个 worker 的输入、范围、输出、证据和日志。

不要默认展示所有 agent 的推理和完整日志。默认展示完整日志会把 GUI 退化成终端。

## 6. 工作流应服务哪些场景

workflow 不应成为通用默认。它适合满足以下条件的任务：

- 有大量相似 work items，例如 20 个以上文件、crate、测试失败、CI 错误或审查意见。
- 每个 item 可以相对独立处理。
- 有明确 oracle，例如编译、测试、lint、snapshot、diff contract 或人工验收标准。
- 失败可以自动转成队列项。
- 并发收益大于额外协调成本。
- 用户愿意用更多 token 换更高完成率或更短墙钟时间。

| 场景 | workflow 收益 | 默认策略 |
|---|---|---|
| 全仓迁移、跨 crate API 调整 | 高 | 先小样本，再批量队列，再分片审查 |
| CI 大量失败收敛 | 高 | 解析日志成 work items，按失败类型领取 |
| 大规模性能、安全、主题、i18n 审计 | 高 | owner/路径分片，独立审查结论 |
| PR review comments 批量修复 | 中高 | 按评论和文件分组，修复后复核 |
| 单个 bugfix | 低 | 单 agent 或 L1 review |
| 小 UI 调整 | 低 | 主 agent + 必要截图/类型检查 |
| 文档润色 | 低到中 | 默认轻量，只有关键 docs 才加 reviewer |
| 新功能探索 | 不稳定 | 先 plan 和样本，不直接大规模并发 |

## 7. 自动化与上手难度

用户不应先学习 workflow。BitFun 应把 workflow 包装成少数可理解的动作：

| 用户表达 | BitFun 自动映射 |
|---|---|
| “快点改完这个” | 低成本策略，少审查，任务结束给未验证项 |
| “稳一点” | 增加 L1 或 L2 审查，运行更完整验证 |
| “帮我把这批都迁掉” | 先样本迁移和规则确认，再提示是否进入批量 workflow |
| “这个 PR 发出去前严格看一下” | L2 或 L3 review，输出 PR 就绪摘要 |
| “修 CI” | 解析 CI 失败，形成失败队列，逐项修复和验证 |
| “预算有限” | 限制并发和二次审查，优先关键路径 |

自动化可以默认启用，但必须满足三条约束：

1. **成本阈值前确认**：超过预设 token、时间、并发或文件数量时，必须提示用户；小任务不显示成本面板，只有预计从轻路径升级为多 reviewer、并发 worker、长任务控制台或超过用户明确预算约束时才弹出确认。
2. **渐进放大**：先抽样或小批量验证，再扩大到全量。
3. **可随时调整范围**：用户能从严格审查切到更快策略，或停止低优先项，只保留关键验证。

## 8. Token、耗时和完成率平衡

BitFun 不能把“任务完成率最高”作为唯一目标。用户通常需要的是“在当前成本约束下足够可靠地完成”。

### 8.1 成本倾向信号（非固定模式）

以下是系统从任务和自然语言中识别的倾向，不应实现为要求普通用户预先选择的五个模式，也不应全部进入设置页。

| 用户表达或任务事实 | 用户心智 | 产品行为 |
|---|---|---|
| 快速 | 先给我一个可用结果 | 单 agent 为主，少量验证，明确未验证项 |
| 平衡 | 不要太慢，也别太冒险 | 默认策略，风险触发 L1/L2 审查 |
| 稳妥 | 这个改动重要，宁愿慢一点 | 更完整验证、更强 review、更保守合并 |
| 批量 | 我有大量相似工作 | 队列、并发、抽样、预算面板 |
| 受限 | token 或时间有限 | 限制并发，优先高风险或高价值 item |

### 8.2 升级和停止条件

自动升级应发生在：

- 验证失败且失败类型可分解。
- 发现跨边界影响或权限/安全风险。
- 用户准备 PR 或团队规则要求。
- 小样本成功，用户允许批量处理。
- reviewer 发现阻塞级或重要级问题。

自动停止或收敛范围应发生在：

- 两轮审查没有新增实质问题。
- token 或时间接近预算。
- 失败重复且需要用户信息。
- oracle 不可靠，继续执行只会堆推测。
- workflow 协调成本超过 item 处理成本。

### 8.3 成本可见性要求

大规模任务必须在任务头显示：

- 已用和预计 token。
- 已用和预计耗时。
- 当前并发和最大并发。
- 已完成、等待、阻塞、失败和跳过数量。
- 执行位置、沙箱等级、写入范围、网络/凭据状态和授权来源。
- 为什么建议追加预算、调整范围或停止。

小任务不显示复杂成本面板，只在可能超预算时提示。

最低产品契约：

- 默认轻路径不主动显示 token 面板；结束摘要只写已验证、未验证和残余风险。
- 进入 L2/L3 review、并发 worker、批量队列或长任务控制台前，必须展示“预计多花多少 token/时间、能换来哪些覆盖或墙钟收益、有哪些范围控制和停止选项”。
- 预算确认至少提供三个动作：继续、保留核心检查、只收敛已完成结果。
- 预算说明只使用估算区间，不承诺精确 token；超过估算上限前必须暂停扩大范围。
- 无可靠 oracle、连续两轮无新增有效问题、冲突需要人工信息、或协调成本高于 item 处理成本时，默认建议停止或保留核心检查。

## 9. 隔离审查的产品要求

隔离 reviewer 不只是“开一个 subagent”。产品层应要求四种隔离，但不把这些概念暴露给普通用户：

| 隔离类型 | 用户价值 | 产品呈现 |
|---|---|---|
| 上下文隔离 | reviewer 不被实现者思路带偏 | “已做独立审查” |
| 权限隔离 | reviewer 不能边审边改，降低误操作 | “审查只读，修复需确认” |
| 执行隔离 | 多个 worker 不互相踩文件或状态 | “无冲突 / 有冲突待处理” |
| 模型或角色隔离 | 高风险时获得不同视角 | “安全/性能/架构分歧” |

用户可以手动触发“独立审查”或“严格审查”，但系统也应在准备 PR、风险升高、验证失败或团队规则命中时自动建议。

## 10. DeepReview 与普通 Review 合并后的体验

### 10.1 用户入口

保留一个主入口：`Review`。

GUI 只提供 `Review` 主动作，由系统选择最小充分强度。用户需要改变倾向时，通过自然语言补充“更快”“严格审查”或“只看安全/性能/架构/前端”等目标；`/review strict` 只作为明确要求最广覆盖的命令表达，不新增可见档位菜单。

不要把 DeepReview 作为用户必须理解的并列产品入口，也不要在普通设置中暴露实际档位、Reviewer 数量或 Judge。

兼容要求：

- 既有 `/DeepReview` 可以继续存在，但只能文案化为迁移窗口内的历史兼容输入，等价路由到 “Review: Strict”。
- 如果一个场景从 `/DeepReview` 启动，用户仍应回到统一 Review 面板读取合并结果。
- DeepReview 的 child session 和 auxiliary pane 不应成为普通用户的第二套窗口模型；若因排障必须显示，默认折叠并标记为高级详情。

### 10.2 输出形态

审查输出应按问题优先，而不是按 reviewer 输出拼接：

```text
结论：建议先修 2 个问题再提交

必须修复：
  1. ...
  2. ...

建议确认：
  1. ...

已覆盖：
  - 业务逻辑
  - 权限边界
  - 性能热点

未覆盖：
  - 没有运行端到端测试，因为 ...

下一步：
  - 应用修复
  - 修复后快速复审
  - 复制 PR 就绪摘要
```

### 10.3 范围收敛要求

当严格审查过重时，系统应自动选择更合适的范围；用户只应看到 BitFun 为当前目标选择了最合适的 Review 方式：

- diff 很小且没有风险标签，使用快速独立审查。
- 缺少足够上下文或 oracle，先做诊断和提问，不启动 L3。
- provider 容量不足，保留核心检查并说明等待或继续的成本。
- 用户选择成本受限，二次审查只覆盖高风险文件。

## 11. 过度设计和过度审核的防线

BitFun 的优势不应表现为“每一步都有重流程”，而应表现为“该轻则轻，该重则重”。

### 11.1 不应默认做的事

- 不应默认为每个任务生成完整 evidence pack。
- 不应默认启动多个 reviewer。
- 不应默认把所有任务都推到 PR 或云端。
- 不应默认暴露 artifact graph、policy profile、workflow DSL。
- 不应把模型建议升级成阻断。
- 不应因为能并发就并发。
- 不应把“严格审查”包装成用户无法跳过的产品姿态，除非安全或组织策略要求。

### 11.2 应默认做的事

- 给出短任务摘要。
- 标明已验证和未验证。
- 对高风险动作做清晰安全确认。
- 在准备 PR 或风险升高时建议合适强度审查。
- 超成本前确认。
- 失败后解释能继续做什么，而不是只显示失败。
- 把复杂证据后台保留，在需要时投影。

## 12. 候选产品调整议题

下表是待采纳议题，不是正式 PRD 编号。采纳任一议题前，必须合并到 [product-requirements.md](product-requirements.md)，并按需要同步 [implementation-plan.md](implementation-plan.md)、[governance/metrics-spec.md](governance/metrics-spec.md) 和 QDP 事件注册表。

| 编号 | 调整项 | 采纳条件 |
|---|---|---|
| WF-CAND-01 | 统一 Review 入口 | 能证明 `/DeepReview`、child session 和 auxiliary pane 不再形成第二套普通用户入口，并有兼容迁移规则 |
| WF-CAND-02 | 轻路径保护 | 低风险任务仍默认单 agent、无 workflow UI、无默认 reviewer，且首次有用结果不被拉长 |
| WF-CAND-03 | 成本确认 | 已定义预算触发、估算展示、范围控制动作和停止条件，并不会把成本面板带入小任务 |
| WF-CAND-04 | 场景化 workflow | 仅 CI/测试失败、PR 评论批量修复、大规模迁移/审计等明确场景进入队列或并发 |
| WF-CAND-05 | 单一 GUI 任务控制台 | 批量任务只显示一个控制台，并固定展示执行域、沙箱状态、异常、预算和可下钻详情 |
| WF-CAND-06 | 审查只读且修复分离 | reviewer 默认只读；修复动作必须经过用户确认后的执行阶段 |
| WF-CAND-07 | 样本和 oracle 优先 | 大规模自动化必须先样本验证；没有可靠 oracle 或人工验收标准时不得扩大执行 |
| WF-CAND-08 | 可调整范围和可收敛 | 长任务必须支持暂停、停止、保留核心检查、跳过低优先项和保留已完成结果 |

## 13. 指标治理调整

本文只提出 workflow 视角下的指标保护 lens，不新增正式指标口径。正式采纳前必须补齐 metrics spec 的负责人、分母、窗口、数据来源、阶段和解释边界；需要新事件时还必须进入 QDP 事件注册表。

| 候选观察项 | 说明 | 治理要求 |
|---|---|---|
| 首个有用结果时间 | 小任务是否仍然快，防止默认路径被 review/workflow 拖慢 | 映射到既有速度指标或补充 metrics spec |
| 用户打断率 / 弹窗触发率 | 成本确认和审查提示是否打扰默认路径 | 复用既有弱提示和弹窗降噪指标 |
| 成本预期偏差 | 用户事后是否认为 token 或耗时超出预期 | 需要定义预算模式、分母和采样方式后才可采纳 |
| 任务解决率 | 当前成本约束下用户目标是否实际完成 | 需要定义任务类型、完成判定和人工反馈来源 |
| 审查有效问题率 | reviewer 发现的问题中，最终被用户、测试或后续修复确认的占比 | 需要 review feedback 事件稳定后进入 P2+，不能作为 P0 默认门禁 |
| 队列阻塞率 | 批量任务中阻塞、冲突、跳过的分布 | 仅真实批量场景稳定后采纳 |
| 后续返工率 / churn | 合入或完成后因 AI 改动导致返工或大幅修改的比例 | 仅长期回放和维护分析采纳 |

## 14. 采纳方式

这不是新的 P0-P4 路线。若采纳本文方向，应该按以下方式回填到既有路线：

- 低风险任务轻路径、已验证/未验证摘要和用户显式 L1 快审，回填到既有 P0/P1 的低摩擦体验，不默认进入 PR 审查。
- PR、受保护分支和团队规则场景，回填到既有 P2 的 PR/团队治理路径；P1 最多做 shadow/advisory，不形成默认阻断。
- CI/测试失败和 PR comments 批量修复，只有在已有 long-running task queue、取消/恢复、执行域提示和成本确认可用后，才回填到 P1/P2 对应场景。
- 大规模迁移/审计必须等样本 gate、oracle、预算确认、冲突处理和回放指标稳定后，再回填到 P3 复杂生命周期场景。
- 指标只在 metrics spec 和 QDP 注册表补齐后采纳；未补齐前只能作为调研观察项。

## 15. 参考资料

- [Bun: Rewriting Bun in Rust](https://bun.com/blog/bun-in-rust)
- [Claude Code workflows](https://code.claude.com/docs/en/workflows)
- [GitHub Copilot cloud agent: starting sessions](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/cloud-agent/start-copilot-sessions)
- [GitHub Copilot cloud agent on GitHub](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/cloud-agent/use-cloud-agent-on-github)
- [GitHub Copilot automatic code review](https://docs.github.com/en/copilot/how-tos/copilot-on-github/set-up-copilot/configure-automatic-review)
- [Cursor 1.0: Bugbot, Background Agent, MCP](https://cursor.com/changelog/1-0)
- [Cursor Cloud Agents](https://cursor.com/docs/cloud-agent)
- [Google Jules](https://jules.google/)
- [Google Jules public beta announcement](https://blog.google/innovation-and-ai/models-and-research/google-labs/jules/)
- [Comparing AI Coding Agents: A Task-Stratified Analysis of Pull Request Acceptance](https://arxiv.org/abs/2602.08915)
- [On the Use of Agentic Coding: An Empirical Study of Pull Requests on GitHub](https://arxiv.org/abs/2509.14745)
- [Investigating Autonomous Agent Contributions in the Wild](https://arxiv.org/abs/2604.00917)

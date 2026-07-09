# Review / Verify 收敛实施护栏

> 范围：约束统一 Review、DeepReview 收敛、ReviewTeam 内部化、PR Review 投影和 Verify 探索的后续两次大型 PR。
> 本文是实施护栏，不新增 SDLC Harness 阶段路线，不定义新的 workflow DSL，也不替代 [product-requirements.md](product-requirements.md)、[agent-workflow-staged-plan.md](agent-workflow-staged-plan.md) 或 [features/pr-quality-gate.md](features/pr-quality-gate.md)。

## 1. 产品目标

后续实现必须解决用户问题，而不是展示内部架构能力：

- 小任务要快，默认不给用户增加审查、门禁或 workflow 心智负担。
- 用户表达“仔细审核”“提交前检查”“准备 PR”时，BitFun 能按风险选择足够的 review 强度。
- 严格审查必须站在待审核代码和完成声明的对立面，尝试证伪，而不是替实现者背书。
- 并发能力在 GUI 中表现为一个任务、一组进度和少量决策点，而不是多个窗口、多个会话入口或多个不可理解的日志流。
- token 和耗时增长前必须让用户知道收益、成本、范围控制和停止选择；自动化不能在用户无感知时显著放大成本。
- PR、团队治理和门禁是结果投影，不是所有任务的默认路径。

## 2. 名词收敛

| 概念 | 用户可见定位 | 内部兼容 |
|---|---|---|
| `Review` / `/review` | 唯一主审查入口；用户不需要先理解审查强度 | 后续统一路由到 L0/L1/L2/L3 |
| `Review: Strict` / 严格审查 | Review 的最高强度，适合高风险、大范围或用户显式要求 | 复用现有 `DeepReview` child session、manifest、queue 和 report enrichment |
| `/DeepReview` | 迁移窗口内的历史兼容命令，不作为长期产品入口 | PR1 保留手输兼容；PR2 起迁移到 `/review`，最终从普通体验中移除 |
| `ReviewTeam` | 严格审查的内部 reviewer 配置 | 类型名、配置路径和 manifest 名称可保留，避免破坏历史设置 |
| PR Review | PR/代码托管平台的评论、线程和就绪度投影 | 不拥有新的 reviewer 调度器 |
| Verify | 任务闭环中的证据生产动作 | 先探索，不在 PR1 中变成强制门禁或独立界面 |

禁止新增与上表平级的用户概念，例如 `DeepReview`、`ReviewTeam`、`Verify Gate`、`Workflow Queue` 作为普通用户必须理解的入口。`/DeepReview` 只能服务迁移兼容，不能继续被包装成高级用户入口或新增能力承载点。

## 3. 两次大型 PR 边界

### PR1：统一 Review 入口和可见命名收敛

目标：

- 用户入口从 `Review` / `Deep Review` 并列，收敛为 `Review` / `Review: Strict`。
- `ReviewTeam` 在界面上表述为 strict review 的覆盖配置，而不是一个独立产品入口。
- `/DeepReview` 只保留手输历史兼容，不在普通命令发现、设置入口或引导文案中强化。
- DeepReview 架构文档明确：`deep_review` 会话、`DeepReview` agent、`ReviewTeamRunManifest` 和队列是 L3 strict review 的兼容实现细节。

允许：

- 调整用户可见文案、设置页说明、会话 badge、report 标题、consent dialog 和相关测试。
- 保留内部类型、API、配置 key、历史 session kind 和旧命令常量。
- 保留辅助 pane，只要文案不把它包装成第二套审查产品。

禁止：

- 不新增动态决策引擎。
- 不改变 reviewer admission、queue、retry、manifest shape 或 backend policy。
- 不把 Verify 设计成新门禁。
- 不为 PR Review 新建一套 reviewer 执行器。

### PR2：动态 Review/Verify 决策和结果投影

目标：

- 引入单一质量决策入口，根据用户意图、diff 规模、风险、验证证据、预算和团队策略选择 L0/L1/L2/L3。
- 引入 `/review` 统一命令和对应 GUI 入口，由系统根据问题、待审核范围、变更难度、风险、质量诉求和预算动态选择强度。
- 在任务执行过程中，当用户要求“仔细审核”或风险升高时，自动触发合适强度的只读 adversarial review。
- Verify 作为证据生产动作进入任务闭环，但默认表现为已验证/未验证摘要，不独立启动新界面。
- Review、Verify 和 PR 就绪度共用结果投影，PR 面板只消费摘要和证据，不拥有执行策略。
- `/DeepReview` 开始迁移为 `/review --strict` 或等价内部路由；普通用户不需要学习或选择 DeepReview。

允许：

- 新增小而明确的 policy/decision 模型。
- 把现有 CodeReview、Strict Review、PR readiness 和 verify evidence 通过同一结果摘要投影。
- 在高风险或用户明确要求时建议升级，并展示 token/耗时预估、范围控制和停止选项。
- 对历史 `/DeepReview` 调用给出轻量迁移提示或静默路由到统一 Review，不再扩展 `/DeepReview` 专属交互。

禁止：

- 不新增通用 workflow DSL。
- 不复制 Agent Kernel 的任务生命周期、scheduler 或队列。
- 不默认对每个任务启动 reviewer。
- 不把缺失证据写成通过状态。
- 不让自动化在没有用户提示或策略原因时显著增加 token。

## 4. Review 强度规则

| 强度 | 默认适用 | 执行倾向 | 用户呈现 |
|---|---|---|---|
| L0 自检 | 文档、小样式、低风险局部改动、已有验证 | 主 agent 总结已验证/未验证 | 任务完成摘要 |
| L1 快速独立审查 | 提交前、少量代码变更、用户要求“review” | 1 个只读 subagent 或等价隔离 reviewer | 简短问题清单和可信度 |
| L2 定向审查 | 安全、性能、架构、关键 UI、跨模块、验证缺口 | 2-3 个定向只读 reviewer | 合并结论、分歧和未覆盖范围 |
| L3 严格审查 | 大型 PR、核心接口、安全敏感、大迁移、用户明确要求严格 | 复用 DeepReview 内部 capacity queue、work packets 和 judge | Review: Strict，带预算和范围控制选择 |

所有 L1-L3 review 都必须是对抗性的：reviewer 只能读取、查找、分析和提交审查结果；不能同时执行修复，不能继承实现者的自我证明，不能把“我刚实现的内容看起来没问题”当作独立结论。

只读 Review 和修正阶段必须分开判断：

- 用户只说“review / 仔细审核 / 提交前检查 / 看看有没有问题”时，默认只读 Review，输出问题、证据、可信度和未覆盖项。
- 用户说“修复并审核 / 修完再检查 / 发现问题就修”，或在 Review 结果页明确选择修复动作时，才进入修正阶段。
- 修正阶段只能消费已确认的 Review 结论、用户选择的修复项和必要上下文；不能让原 reviewer 在同一轮里直接改代码。
- 没有明确可验证 oracle、问题需要产品决策、或修复会扩大范围时，先回填输入框或请求用户确认，不自动写入。
- 修复后是否复审由用户诉求、风险等级、变更范围和预算共同决定；低风险单点修复可快速复核，高风险或跨边界修复才进入严格复审。

## 5. GUI 交互原则

并发和 review 在 GUI 中必须降低复杂度：

- 对普通任务，默认仍是聊天/任务结果视图，不弹出控制台。
- 对 strict review，只显示一个 Review 页面或辅助面板，内部 reviewer 作为进度和结果来源展示。
- 对批量迁移、CI 聚类或 PR 评论批量修复，显示单一任务控制台：进度、阻塞、预算、冲突和需决策项。
- 不因为后台存在多个 subagent 就打开多个 GUI 窗口。
- 不用“workflow”“subagent queue”“evidence graph”等内部术语作为普通用户主标签。

## 6. 成本和完成率平衡

默认策略：

- 小任务优先首个有用结果时间。
- 中风险任务优先 L1/L2 和最近验证，不默认 L3。
- 大规模任务先做样本 gate，再扩大并发。
- 预算不足时保留高风险/高价值项，跳过低风险二次审查。
- 两轮 review 没有新增有效问题时，建议停止或保留核心检查。
- 缺少 oracle 或上下文时，先说明未验证项，不静默追加 token。

产品必须展示：

- 为什么建议升级 review 或 verify。
- 预计增加的是解决率、覆盖率还是墙钟速度。
- 不升级会放弃哪些覆盖。
- 当前可安全收敛的结果是什么。

## 7. 历史兼容和取舍

迁移窗口内必须兼容：

- 历史 `deep_review` session kind。
- `DeepReview` agent type。
- `/DeepReview` 命令的手输解析和旧会话恢复。
- `deepReviewRunManifest`、`ReviewTeamRunManifest` 和 `ai.review_teams.default` 配置路径。
- 既有 report/action-bar persistence。

不得为了兼容牺牲的关键诉求：

- 默认产品入口必须是统一 Review。
- 长期命令入口必须收敛到 `/review`，由系统动态选择审查强度。
- `/DeepReview` 不能成为新的高级入口、能力分支或用户必须学习的概念。
- ReviewTeam 不应继续被包装成普通用户必须理解的独立产品。
- PR Review 不应成为第二套执行器。
- Verify 不应默认变成新门禁。
- 对抗性 review 不能被实现者自检替代。

## 8. PR 前复查清单

每个 PR 合入前必须检查：

- 是否只改了当前 PR 承诺的阶段范围。
- 是否没有新增重复入口、重复策略对象或重复 UI 文案体系。
- 是否保留必要的历史 session/config/command 迁移兼容，同时没有把 `/DeepReview` 固化为长期入口。
- 是否没有把低风险任务默认升级为重流程。
- 是否说明 token/耗时/完成率的取舍。
- 是否对 review/verify 的 adversarial 要求保持明确。
- 是否通过了最小匹配验证，并由独立第三方视角完成对抗性复查。

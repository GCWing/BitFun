# Review / Verify 收敛实施护栏

> 范围：记录统一 Review、DeepReview 收敛和 ReviewTeam 内部化两次大型 PR 的已合入基线，明确既有 PR Review 独立 AI 草稿路径仍待收敛，并约束 Verify 探索和后续产品优化不重新扩张概念。
> 本文是实施护栏，不新增 SDLC Harness 阶段路线，不定义新的 workflow DSL，也不替代 [product-requirements.md](product-requirements.md)、[agent-workflow-staged-plan.md](agent-workflow-staged-plan.md) 或 [features/pr-quality-gate.md](features/pr-quality-gate.md)。
> 合入后的竞品复盘、体验差距和候选需求见 [research/review-product-experience-benchmark.md](research/review-product-experience-benchmark.md)。

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
| `Review` / `/review` | 唯一主审查入口；用户不需要先理解审查强度 | 当前统一路由到 L1/L2/L3；L0 留给后续 Verify 探索 |
| `Review: Strict` / 严格审查 | Review 的最高强度，适合高风险、大范围或用户显式要求 | 复用现有 `DeepReview` child session、manifest、queue 和 report enrichment |
| `/DeepReview` | 迁移窗口内的历史兼容命令，不作为长期产品入口 | 静默路由到 `/review strict` 等价路径，最终从普通体验中移除 |
| `ReviewTeam` | 严格审查的内部 reviewer 配置 | 类型名、配置路径和 manifest 名称可保留，避免破坏历史设置 |
| PR Review | PR/代码托管平台的评论、线程和就绪度投影 | 既有 MiniApp 独立强度和 AI 草稿路径属于待收敛历史实现；不再扩展，目标态不拥有 reviewer 调度器 |
| Verify | 任务闭环中的证据生产动作 | 先探索，不在 PR1 中变成强制门禁或独立界面 |

禁止新增与上表平级的用户概念，例如 `DeepReview`、`ReviewTeam`、`Verify Gate`、`Workflow Queue` 作为普通用户必须理解的入口。`/DeepReview` 只能服务迁移兼容，不能继续被包装成高级用户入口或新增能力承载点。

## 3. 已完成的两次大型 PR 基线

两次大型 PR 已完成并合入。以下内容保留为行为和兼容基线，不代表继续扩张实现范围：

| 阶段 | 已达成结果 | 后续约束 |
|---|---|---|
| PR1 | 统一 Review 可见命名，DeepReview 和 ReviewTeam 收回内部兼容层 | 不恢复第二套入口或设置心智 |
| PR2 | `/review` 自适应选择 L1-L3，只读 Reviewer 与 ReviewFixer 分离，同侧栏修复和 follow-up Review 闭环；未包含既有 PR Review MiniApp 的独立 AI 草稿路径收敛 | 后续优先改善问题状态、增量复审、成本授权和 PR 结果投影，不新建调度体系 |

### 合入证据

| 基线 | PR / commit | 关键实现入口 | 回归证据 |
|---|---|---|---|
| PR1：可见命名与历史入口收敛 | [PR #1488](https://github.com/GCWing/BitFun/pull/1488) / [`f072467ea`](https://github.com/GCWing/BitFun/commit/f072467ea4332aafeb6ff25cb1fc4cd133c2ff64) | [commandParser.ts](../../src/web-ui/src/flow_chat/deep-review/launch/commandParser.ts)、[ReviewConfig.tsx](../../src/web-ui/src/infrastructure/config/components/ReviewConfig.tsx) | [commandParser.test.ts](../../src/web-ui/src/flow_chat/deep-review/launch/commandParser.test.ts)、[ReviewConfig.test.tsx](../../src/web-ui/src/infrastructure/config/components/ReviewConfig.test.tsx) |
| PR2：自适应质量决策与同侧栏闭环 | [PR #1497](https://github.com/GCWing/BitFun/pull/1497) / [`99ac62ffd`](https://github.com/GCWing/BitFun/commit/99ac62ffde8f942c52fbae9dc0a19f622173fe74) | [review.rs](../../src/crates/contracts/product-domains/src/review.rs)、[ReviewService.ts](../../src/web-ui/src/flow_chat/services/ReviewService.ts) | [review_policy.rs](../../src/crates/contracts/product-domains/tests/review_policy.rs)、[ReviewService.test.ts](../../src/web-ui/src/flow_chat/services/ReviewService.test.ts)、[BtwSessionPanel.review-action.test.tsx](../../src/web-ui/src/flow_chat/components/btw/BtwSessionPanel.review-action.test.tsx) |

### PR1：统一 Review 入口和可见命名收敛（已完成）

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

### PR2：动态 Review 决策和只读审查闭环（已完成）

目标：

- 引入单一质量决策入口，根据用户意图、diff 规模、风险和项目策略选择 L1/L2/L3。
- 引入 `/review` 统一命令和对应 GUI 入口，由系统根据问题、待审核范围、变更难度、风险、质量诉求和预算动态选择强度。
- 在任务执行过程中，只有用户明确要求“完成并仔细审核”等诉求时，主 Agent 才在当前任务内触发一个只读 adversarial reviewer；风险升高只影响用户显式发起的 `/review` / GUI Review 强度，或形成 Review 建议，不自动启动额外 Reviewer。风险驱动的自动触发属于后续候选，必须单独评审收益和成本授权。
- Verify 继续作为后续探索，不在本 PR 增加生产决策字段、证据生产器、门禁或独立界面。
- 目标边界是统一 Review 结果作为既有变更就绪度 / PR 门禁的输入，不拥有执行策略或门禁决策；门禁继续基于证据、确定性失败、团队策略、残余风险接受和人工决策生成，代码托管平台事实与最终执行约束保持独立。当前 PR 面板仅按工作区和文件重叠启发式展示相关 Review 会话数量，没有稳定 PR 身份关联；既有 PR Review MiniApp 仍自行选择 fast / focused / deep 并生成 AI 草稿。两条路径都尚未形成统一投影，不能把目标态或启发式关系误写成现状契约，也不能继续扩展其策略和 Reviewer 能力。
- `/DeepReview` 开始迁移为 `/review strict` 等价内部路由；普通用户不需要学习或选择 DeepReview。

允许：

- 新增小而明确的 policy/decision 模型。
- 把现有 CodeReview 和 Strict Review 通过同一结果摘要投影。
- 在高风险或用户明确要求时建议升级，并展示 token/耗时预估、范围控制和停止选项。
- 对历史 `/DeepReview` 调用给出轻量迁移提示或静默路由到统一 Review，不再扩展 `/DeepReview` 专属交互。

禁止：

- 不新增通用 workflow DSL。
- 不复制 Agent Kernel 的任务生命周期、scheduler 或队列。
- 不默认对每个任务启动 reviewer。
- 不把缺失证据写成通过状态。
- 不让自动化在没有用户提示或策略原因时显著增加 token。

PR2 已合入实现边界：

- 平台无关的产品域策略只接收用户意图、目标事实和项目策略，输出 L1-L3、执行模式、策略级别、原因和是否需要成本确认。
- GUI 和 `/review` 共用同一个预构建计划；确认前后不重新解析目标或重建 manifest，避免用户确认的范围与实际执行漂移。
- L1 使用一个独立只读 `CodeReview`；L2 的 core 与 extra reviewer 总数最多为 3，且不追加 judge；L3 才使用完整适用 reviewer、capacity queue 和 judge。
- 用户在普通任务中明确要求“完成并仔细审核”时，主 Agent 完成实现后在当前任务内调用 1 个隔离 `CodeReview`，不另开产品界面，内部 Task 名称默认折叠和匿名化；需要多 reviewer 的覆盖统一进入 `/review` 和成本确认，不在提示词中复制 reviewer-count 策略，也不在用户无感知时增加并发成本。
- Review 与修复分为两个运行身份：`CodeReview` / `DeepReview` 只读，用户批准后由 `ReviewFixer` 修改；能精确归因时，修复后按“原审核文件 + `ReviewFixer` 直接改动文件”重新统计 diff 和决策。命令、Git 或 stdin 工具一旦出现，无论成功、失败或中断，都明确提示并回退当前工作区 diff，不伪装成窄范围复核。Fixer 基线和本次选中的 remediation id 在发送修复任务前持久化，重启只恢复原范围内未完成项。follow-up 预留携带唯一 request id，并把同一 id 写入现有子会话 relationship metadata、派生后端 session id；创建响应不确定时保留同一预留供重试，后端按 agent、relationship kind、parent session 和 parent request 复用既有会话，不把可变化的 parent turn 位置当作幂等身份。侧栏明确区分重试、进行中、完成、失败、取消和查看结果。最终范围、改动文件和子会话关系继续使用现有 session metadata，不新增事务系统。
- L2/L3 的 manifest 约束不能只依赖前端组装：portable runtime 校验 L2 的 normal、最多 3 个 active reviewer、无 judge，以及 L3 的 deep、全部非条件 core reviewer、`ReviewJudge` quality gate，并要求条件 reviewer 启用或明确标记 `not_applicable`；缺少 `qualityDecision` 的历史 manifest 保持兼容。创建幂等短路只用于带 parent request 的 Review/DeepReview 子会话，不改变普通显式 ID 会话的 coordinator 恢复路径。
- 文件范围统计必须通过已注册、支持远程路由的 workspace API 包含可读取的未跟踪文件内容。普通 `Review` intent 下，当前策略对 `Partial` / `Unknown` 目标保持 L1/Quick，项目策略 override 也不会让未解析目标静默扩展并发；显式 `/review strict` 仍优先进入 L3/Deep。只有 `Resolved` 且非空目标的变更行数未知时，普通 Review 的风险下限才进入 L2。若后续产品要求未解析的普通目标也进入 L2，必须另开实现 PR 修改策略和回归测试，不能把目标态写成已合入基线。
- L2/L3 确认框按每个实际 work packet（包含 judge）分别估算后求和；单 reviewer 最大值只服务 prompt guardrail，不能乘调用数冒充总成本。容量设置描述为可选工作的等待窗口，不承诺整个 Review 的硬耗时上限；容量 policy 串行保存，保存期间锁定全部容量输入，失败立即恢复最后确认值。
- Verify 本 PR 不进入生产决策契约。后续探索必须先定义可信 evidence producer，不能把缺失证据解释为通过。

合入后仍需解决的是产品表达和问题闭环，而不是继续增加 Review 执行机制：

- 近期问题动作只在本次 Review 内生效并复用现有修复计划；跨 Review 的处理状态和修复后关闭必须先定义稳定身份、持久化所有者和失效规则，不能用启发式关联冒充闭环。
- “未发现问题”必须限定为本次覆盖范围，不能形成质量背书。
- 扩大覆盖确认需要说明新增收益，并提供保留核心检查的收敛选择。
- 自动复审应优先增量化并具备暂停条件，不能默认在每次 push 后持续消费成本。
- 当前容量设置只作为高级运行控制，不扩张为 Reviewer、策略档位或 Workflow 配置中心。

这些条目在进入实现前必须先按 [Review 产品体验竞品基准与优化需求](research/review-product-experience-benchmark.md) 的优先级回填权威 PRD 和实施计划。

## 4. Review 强度规则

| 强度 | 默认适用 | 执行倾向 | 用户呈现 |
|---|---|---|---|
| L0 自检（后续探索） | 文档、小样式、低风险局部改动、已有验证 | 由 Verify 设计定义可信摘要；当前未接入生产策略 | 任务完成摘要 |
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

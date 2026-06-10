# BitFun 全软件生命周期 Harness 设计文档

> 范围：定义 BitFun 作为 Harness 产品加载和治理外部软件工程时的目标领域模型、逻辑架构、模块职责边界和跨阶段控制闭环。
> 边界：本文不以 BitFun 仓库自身治理为主线。BitFun 自身的文档、模块边界或工程治理问题应作为独立内部验证与平台建设输入承载，不混入产品目标架构。

## 1. 设计目的

BitFun 的目标形态不是只服务于自身仓库的工程治理工具，而是一个面向任意目标项目的 Agentic Engineering Harness。用户把一个外部工程加载到 BitFun 后，BitFun 应能够理解该工程的规则、代码、任务、变更、验证、评审、发布与运行反馈，并在关键节点提供上下文组织、证据沉淀、风险分级、质量门禁、人工确认和持续评估。

因此，本文讨论的是三类对象之间的关系：

| 对象 | 定义 | 设计含义 |
|---|---|---|
| BitFun 产品 | 提供 Agent Runtime、工具集成、Hook/Event、质量数据、图谱、门禁和协作界面的宿主系统 | 负责承载 Harness 能力，但不把自身仓库规则硬编码为通用规则 |
| 目标项目 | 被用户加载、分析、修改、评审和发布的软件工程，可以是任意语言、仓库结构和团队流程 | Harness 的主要治理对象 |
| 外部系统 | 目标项目依赖的 issue、文档、代码托管、CI、制品、发布、观测和知识库系统 | BitFun 通过 adapter 集成，不直接替代 |

主设计要回答四个问题：

1. 当 BitFun 面向外部目标项目时，软件生命周期中的真实断裂点是什么。
2. 为了承载多项目、多团队、多流程，BitFun 需要怎样的目标领域模型。
3. BitFun 的产品逻辑架构应如何分层，避免把 Agent 执行、质量事实、策略判断和团队协作混在一起。
4. 为什么第一阶段从 PR 证据链切入，但架构必须能继续扩展到需求、发布、运行和复盘。

## 2. 产品问题域识别

### 2.1 被治理对象不是 BitFun 仓库，而是目标项目

BitFun 可以用自身仓库作为内部验证场景，但产品设计不能默认目标项目具有同样的语言、目录、CI、分支策略、agent rules 文档、模块边界或发布流程。真实用户加载的项目可能存在以下差异：

| 差异维度 | 目标项目可能状态 | 对 Harness 的要求 |
|---|---|---|
| 项目结构 | monorepo、multi-repo、单体仓库、生成代码仓库、服务加前端混合仓库 | 先发现结构和边界，再建立项目画像 |
| 工程规则 | 有 agent rules，也可能只有 README、贡献指南、隐性团队约定或完全缺失 | 规则来源需要分级、溯源、冲突处理和缺失提示 |
| 验证体系 | CI 完整、CI 不稳定、只有本地脚本、没有测试、依赖私有环境 | required checks 必须可解释，并能区分缺失、失败、跳过和不可运行 |
| 交付流程 | issue 到 PR、ticket 到 release、直接 commit、人工发布、灰度发布 | Artifact Graph 不能绑定单一流程 |
| 质量文化 | 强 branch protection、弱 review、依赖 owner 经验、合规审计强约束 | Gate 策略需要按项目和团队可配置 |
| 权限边界 | 本地工程、远程 workspace、企业网络、私有依赖、敏感文件 | Hook/Event 和工具调用必须默认最小权限 |

因此，BitFun 要建设的是可适配目标项目的 Harness 能力，而不是把某个仓库的实践固化为产品功能。

### 2.2 外部项目中的关键断裂点

从目标项目视角看，当前软件生命周期通常存在六类断裂：

| 断裂点 | 表现 | 后果 |
|---|---|---|
| 项目理解断裂 | 代码结构、规则、owner、CI、发布方式和历史风险分散在不同系统或人的经验中 | Agent 容易读错边界、跑错验证、误判风险 |
| 交付物断裂 | requirement、ticket、spec、plan、diff、test、review、release、incident 之间缺少稳定关系 | 变更是否满足需求、问题来自哪里、哪些验证覆盖了风险都难以证明 |
| 证据断裂 | 上下文读取、命令输出、测试结果、review finding、跳过理由和人工确认散落在会话、终端、PR 和 CI 中 | 审计、复盘、交接和再次评审需要重复整理 |
| 控制断裂 | tool approval、hook、CI、review、release gate 和人工审批不在同一控制平面 | 高风险动作难以一致阻断、降级、审计和复跑 |
| 反馈断裂 | CI failure、review blocker、post-merge defect、incident 没有稳定回流到规则、测试和评估集 | 工具无法从失败中校准，团队也难以沉淀工程知识 |
| 责任断裂 | Agent 自动结论、工具事实、人类判断和风险接受边界不清 | 自动化结果容易被误认为已验证事实，发布责任不可追踪 |

这些断裂说明：BitFun 的核心产品机会不是继续强化单点代码生成能力，而是给目标项目补上一个可配置、可追踪、可审计的生命周期 Harness。

### 2.3 目标系统要解决的本质问题

全局设计要解决的是：

```text
当一个外部软件工程被加载到 BitFun 后，
如何把项目规则、交付物、执行轨迹、质量证据、风险策略、人工决策和失败反馈
组织成可追踪、可审计、可演进的工程控制系统。
```

PR 是第一个落地切口，因为它通常同时连接 diff、review、CI、issue、owner、风险接受和团队协作。但 PR 不是产品边界。完整架构必须继续向上连接需求和设计，向下连接发布、运行、incident 与回归资产。

## 3. 目标领域模型

### 3.1 顶层领域

BitFun SDLC Harness 面向目标项目的领域模型分为六组核心对象：

| 领域 | 核心对象 | 作用 |
|---|---|---|
| Project Profile | workspace、repository、language、framework、module、owner、rule source、verification capability、release model | 描述目标项目是什么、如何构建、如何验证、由谁负责 |
| Lifecycle Artifact | requirement、ticket、acceptance criteria、spec、design decision、plan、task、changeset、test、review、PR、release、incident、learning asset | 表达目标项目从需求到运行反馈的交付物 |
| Execution Trace | session、turn、model run、tool run、command run、file operation、hook invocation、adapter call | 记录 BitFun 如何读取、修改和验证目标项目 |
| Quality Evidence | evidence pack、evidence item、command result、CI result、review finding、context provenance、skip reason、stale marker | 证明当前结论来自哪些事实和证据 |
| Risk and Policy | risk signal、required check、gate decision、approval、override、budget profile、permission policy | 表达哪些风险需要处理、哪些检查必须执行、哪些责任需要人工确认 |
| Evaluation and Learning | eval task、oracle、benchmark、trace replay、failure pattern、rule update、skill、regression test | 把目标项目中的失败和经验回流成可重复验证的资产 |

这六组对象之间的关系构成产品架构骨架：

```text
Project Profile
  constrains -> Execution Trace
  informs -> Risk and Policy
  provides context for -> Lifecycle Artifact

Lifecycle Artifact
  is changed by -> Execution Trace
  is justified by -> Quality Evidence
  is protected by -> Gate Decision
  is improved by -> Evaluation and Learning

Quality Evidence
  supports -> Review / PR / Release / Incident response
  expires with -> artifact or context change

Risk and Policy
  selects -> Required Checks / Review Profile / Approval Path
  feeds back from -> CI Failure / Review Blocker / Incident / Override
```

### 3.2 生命周期主链路

目标项目的完整生命周期链路是：

```text
Feedback / Issue / Requirement
  -> Acceptance Criteria / Spec / Design Decision
  -> Plan / Task
  -> ChangeSet / Code / Config / Migration
  -> Test / Verification / Benchmark
  -> Review / Gate / Approval
  -> PR / Merge / Release
  -> Telemetry / Incident / User Report
  -> Regression Test / Rule / Skill / Evaluation Task
```

这条链路不是要求所有节点在 P0 同时实现，而是定义 BitFun 要逐步承载的产品逻辑。任意子模块都必须能说明自己服务链路中的哪一段、消费哪些对象、输出哪些对象，以及何时需要人工确认。

### 3.3 项目画像模型

面向外部目标项目时，Project Profile 是所有 Harness 判断的前置条件。它至少包含：

| 画像维度 | 内容 | 典型来源 |
|---|---|---|
| 结构画像 | repo、package、module、service、frontend、backend、generated code、test tree | 文件系统、依赖文件、构建配置、代码搜索 |
| 规则画像 | 项目说明、贡献指南、agent rules、CODEOWNERS、分支策略、验证矩阵 | README、CONTRIBUTING、agent rules、CI、仓库设置 |
| 验证画像 | 可运行命令、CI jobs、测试分层、flaky 信号、环境依赖 | package scripts、cargo/npm/maven/gradle、CI 配置、历史结果 |
| 风险画像 | hot files、incident 关联、历史 review blocker、安全敏感区、发布关键路径 | Git history、PR comments、incident links、owner 标注 |
| 权限画像 | 本地/远程 workspace、敏感文件、secret、网络、shell、MCP、桌面能力 | BitFun policy、项目配置、用户授权 |

项目画像不是一次性扫描结果，而是随项目变化、用户确认和失败反馈持续更新的上下文资产。缺少画像时，系统应输出 `unknown` 或 `degraded`，而不是把未知项目误判为低风险。

### 3.4 控制点模型

Harness 的本质是在交付物流转中设置控制点。每个控制点都应面向目标项目输出可解释状态：

| 控制点 | 关注问题 | 输出 |
|---|---|---|
| Project understanding control | 当前项目结构、规则、验证能力是否被正确识别 | project profile、unknown area、stale warning |
| Context control | 当前任务是否拿到了正确、有效、不过期的上下文 | context provenance、conflict warning |
| Permission control | 工具、插件、命令、文件和外部系统访问是否越权 | approval、deny、audit |
| Risk control | 变更属于什么风险等级，需要哪些验证和 reviewer | risk tags、required checks、review profile |
| Evidence control | 是否有足够证据支持当前结论 | EvidencePack、missing evidence、degraded state |
| Review control | 问题是否被发现、处理、验证或接受 | finding lifecycle、stale review |
| Release control | 是否具备发布、观测和回滚信心 | readiness、risk acceptance、rollback plan |
| Learning control | 失败是否沉淀为可复用资产 | eval task、regression test、rule、skill |

这使 BitFun 从 Agent 工作台升级为面向目标项目的生命周期控制平面。

## 4. 目标逻辑架构

### 4.1 分层架构

目标架构分为六层：

```text
User and Team Surfaces
  Agent Workbench / Project Map / PR Review / Release Readiness / Incident Review

Lifecycle Harness Control Plane
  Risk Policy / Gate / Review / Impact Analysis / Evaluation / Approval

Artifact and Evidence Plane
  Artifact Graph / EvidencePack / Finding Lifecycle / Risk Acceptance

Quality Data Plane
  Lifecycle Events / Execution Trace / Verification Results / Cost and Audit

Project Integration Plane
  Git / CI / Issue Tracker / Docs / Release / Observability / Knowledge Base

Agent and Tool Runtime
  Sessions / Agents / Tools / Terminal / MCP-LSP / Hooks / Adapters
```

各层职责：

| 层 | 解决的问题 | 不承担的职责 |
|---|---|---|
| User and Team Surfaces | 让用户在目标项目的任务、PR、发布和复盘中消费 Harness 结论 | 不直接承载策略判断 |
| Lifecycle Harness Control Plane | 负责风险、门禁、影响分析、评估和审批编排 | 不直接保存原始事实 |
| Artifact and Evidence Plane | 负责交付物关系、证据投影、finding 生命周期和风险接受记录 | 不执行工具，不替代外部系统 |
| Quality Data Plane | 负责事实、事件、轨迹、成本、审计和证据引用 | 不做业务策略，不判断是否可发布 |
| Project Integration Plane | 负责连接目标项目及其外部系统 | 不把外部系统语义泄漏为内部 canonical model |
| Agent and Tool Runtime | 负责实际执行、工具调用、Hook/Event 触发和 adapter 执行 | 不自行宣布质量结论 |

### 4.2 逻辑架构关系

```text
Target Project and External Systems
  source code / rules / tickets / CI / release / observability
        |
        v
Project Integration Plane
        |
        v
Agent and Tool Runtime
        |
        v
Quality Data Plane
        |
        v
Artifact and Evidence Plane
        |
        v
Lifecycle Harness Control Plane
        |
        v
User and Team Surfaces
```

这不是严格调用栈，而是责任流：

- 目标项目和外部系统提供真实工程对象。
- Project Integration Plane 负责读取和同步这些对象，并做 adapter 隔离。
- Agent Runtime 在授权范围内执行分析、修改、验证和工具调用。
- Quality Data Plane 固化事实、事件和成本。
- Artifact and Evidence Plane 组织交付物关系、证据和 finding 状态。
- Harness Control Plane 做风险、门禁、评估和审批判断。
- 用户界面和团队流程消费判断，并给出确认、覆盖或风险接受。

### 4.3 为什么第一阶段仍从 PR 切入

PR 是第一个验证闭环，不是架构边界。原因：

- PR 是大多数目标项目已经接受的协作对象，不要求 BitFun 先替代 issue、CI 或 release 平台。
- PR 同时连接 diff、review、CI、owner、风险接受和发布信心，适合验证 EvidencePack、Risk Classifier、Gate 和 Artifact Graph 的最小关系。
- PR 的失败模式高频可见，能快速积累后验校准信号，例如 review blocker、CI failure、stale evidence 和 override。
- PR 之后可以自然扩展到需求影响分析、release readiness 和 incident 回溯。

因此，第一阶段聚焦 PR 是产品落地策略；目标架构仍是面向外部目标项目的全生命周期领域模型。

## 5. 产品采用路径

从产品设计角度看，BitFun 不宜在早期一次性提供完整 SDLC Harness 能力。可采用的最小路径是：

```text
Open Project
  -> Project Profile
  -> Local Diff / PR
  -> EvidencePack
  -> Risk and Required Checks
  -> Gate Result
  -> Review / Confirm / Override
  -> Graph and Eval Feedback
```

这条路径的产品含义是：

| 阶段 | 用户问题 | BitFun 输出 |
|---|---|---|
| 加载项目 | BitFun 是否理解我的项目结构、规则和验证方式 | Project Profile、未知区域、冲突规则、可运行验证 |
| 准备变更 | 当前 diff 有哪些风险和必须验证项 | risk tags、required checks、context provenance |
| 提交 PR | reviewer 需要哪些证据才能信任变更 | EvidencePack、Gate result、open risks |
| 评审修复 | review finding 是否已处理，证据是否过期 | finding lifecycle、stale evidence |
| 发布复盘 | 本次变更与发布、incident 和回归资产如何关联 | Artifact Graph、release/incident backtrace |

因此，主设计的第一产品闭环不是“自动写代码”，而是“加载外部项目后，给每一次变更提供可解释的质量保护”。这是 BitFun 区别于单纯 IDE Agent 或单点 PR Review 工具的核心路径。

## 6. 模块职责与边界

下表定义主设计拆出的子模块。每个模块的详细数据模型、流程、策略和成功标准在对应子模块文档中展开。

| 模块 | 详细设计 | 核心解决问题 | 职责边界 | 目标状态 |
|---|---|---|---|---|
| Project Profile and Integration | [project-profile-integration.md](architecture/project-profile-integration.md) | 目标项目结构、规则、验证能力和外部系统不可见 | 负责项目画像和 adapter 边界，不直接做风险或 gate 判断 | 新项目加载后能明确已知、未知、冲突和过期信息 |
| Quality Data Plane | [quality-data-plane.md](architecture/quality-data-plane.md) | 目标项目质量事实分散，证据不可复用 | 只负责事件、证据引用、指标和审计事实，不负责复杂策略判断 | gate、review、eval、impact 都能引用同一事实基础 |
| Artifact Graph | [artifact-graph.md](architecture/artifact-graph.md) | 目标项目生命周期交付物关系断裂 | 负责交付物节点和可信链接，不替代需求、代码托管、CI 或 observability 系统 | 从最小 PR 图谱扩展到需求、发布和 incident 回溯 |
| Risk Classifier | [risk-classifier.md](features/risk-classifier.md) | 外部项目风险规则差异大，验证选择依赖人工经验 | 负责输出风险标签、原因、置信度和 required checks；不直接宣称变更安全 | PR 和任务都有可解释的项目级风险画像和验证建议 |
| PR Quality Gate | [pr-quality-gate.md](features/pr-quality-gate.md) | PR 放行缺少统一证据、风险和 review 状态 | 负责聚合证据、风险、验证、Deep Review 和人工确认；不替代 CI 或 human approval | PR 可输出 `pass/warn/fail/degraded`，并说明依据和未覆盖风险 |
| Agent Evaluation | [agent-evaluation.md](features/agent-evaluation.md) | 不同目标项目中的 prompt、tool schema、model 和策略改动缺少长期评估 | 负责任务集、oracle、trace replay 和 A/B；不做通用模型排名 | Harness 改动能被质量、成本、安全和稳定性共同评估 |
| Requirement Impact Analysis | [requirement-impact-analysis.md](features/requirement-impact-analysis.md) | 需求、验收标准、API 或设计变更后的影响面不清 | 负责输出候选影响、置信度、证据和 required checks；不宣称完整自动影响分析 | 高风险变更能形成可确认的影响面和验证清单 |
| OpenCode Compatibility | [opencode-compatibility.md](features/opencode-compatibility.md) | 常用 hook/plugin 生态难以在 BitFun 加载的项目中复用 | 只作为 adapter，不改变 BitFun canonical event、permission、artifact 和 policy model | 常见 Harness 插件能力可迁移，同时安全边界保持清晰 |

## 7. 跨模块主流程

### 7.1 项目加载与画像建立

```text
Open target project
  -> project structure discovery
  -> rule and verification source discovery
  -> Project Profile
  -> unknown / conflict / stale markers
  -> user confirmation when needed
  -> Quality Data Plane events
```

这个流程是所有后续 Harness 判断的前提。没有项目画像时，Risk Classifier、PR Gate 和影响分析都必须显式降级。

### 7.2 PR 质量闭环

```text
ChangeSet in target project
  -> execution trace
  -> Quality Data Plane
  -> EvidencePack
  -> Risk Classifier
  -> PR Quality Gate
  -> Deep Review when needed
  -> Artifact Graph update
  -> Evaluation feedback
```

这个流程验证最小可行 Harness：证据、风险、门禁、评审、图谱和评估都能在一个高频场景中形成闭环。

### 7.3 需求变更影响闭环

```text
Requirement / Spec / API change
  -> Artifact Graph lookup
  -> Requirement Impact Analysis
  -> Risk Classifier
  -> required checks
  -> human confirmation
  -> PR Gate / Release Readiness
```

这个流程把架构从 PR 向上连接到需求和设计，避免质量控制只发生在代码提交后。

### 7.4 运行期反馈闭环

```text
Incident / User Report / Telemetry
  -> Artifact Graph backtrace
  -> related release / PR / diff / test
  -> EvidencePack and risk review
  -> regression test / rule / skill / eval task
```

这个流程把架构从 PR 向下连接到发布和运行期，避免 incident 只停留在人工复盘中。

## 8. 模块协作原则

1. **目标项目与宿主产品分离**：目标项目的规则、CI、发布流程和团队约定进入 Project Profile；BitFun 内部实现细节不得被当成通用项目规则。
2. **事实与策略分离**：Quality Data Plane 记录事实，Risk Classifier 和 PR Gate 使用事实做策略判断。
3. **图谱与门禁分离**：Artifact Graph 表达交付物关系，PR Quality Gate 决定当前 PR 是否满足质量条件。
4. **候选与事实分离**：Requirement Impact Analysis 和模型推断只能生成候选，确认状态由人类或确定性证据补齐。
5. **兼容与内核分离**：OpenCode Compatibility 只映射外部 API，BitFun 内部始终以 canonical event 和 permission model 为准。
6. **评估与执行分离**：Agent Evaluation 评估策略和能力变化，不直接修改生产策略；策略变更必须经过明确发布路径。
7. **高风险与低风险分流**：低风险变更保持轻量，风险升高时逐步增加 required checks、Deep Review 和人工确认。

## 9. 设计硬约束

从独立架构和产品审查视角看，以下约束必须作为后续实现的准入条件：

| 硬约束 | 原因 | 设计要求 |
|---|---|---|
| 画像先于判断 | 未理解目标项目时，任何风险和 gate 结论都可能误导用户 | Project Profile 不完整时输出 `unknown/degraded` |
| 证据先于自动化 | Agent 能执行不等于变更可信 | Gate、review、release readiness 必须引用 evidence ref |
| 候选不等于事实 | LLM 适合召回和总结，但不适合单独宣布影响面或安全性 | LLM 输出默认是 candidate，需要 confidence、evidence 和 confirmation |
| 成本为核心约束 | Deep Review 和长程 Agent 容易导致 token、耗时和 CI 成本预算约束不足 | 所有高成本能力必须有 budget、profile、降级和跳过原因 |
| 外部生态不能侵入核心模型 | 兼容 OpenCode、Codex 或 Claude Hook 不能牺牲 BitFun 自身模型一致性 | compatibility 只能作为 adapter，内部只使用 canonical model |
| 图谱不能无界增长 | 低质量链接会比没有链接更危险 | Artifact Graph 先追求高价值、可验证、可失效的边 |
| 人类责任不可省略 | 风险接受、发布责任和例外处理必须可追踪 | override 必须记录 actor、reason、residual risk 和 evidence |
| 评估必须防泄漏 | 公开 benchmark 存在评测集泄漏和过拟合风险，单一分数无法代表产品质量 | Evaluation 使用 holdout、真实任务回放、成本和安全联合指标 |

## 10. 非目标

- 不把 BitFun 仓库自身治理当作本文主线。
- 不要求目标项目采用 BitFun 仓库的目录、语言、CI 或文档结构。
- 不直接替代 Jira、GitHub、Linear、CI、observability 或 release 平台。
- 不把所有质量判断交给 LLM。
- 不承诺需求影响分析的自动完整性。
- 不把任意第三方 plugin 视为可信执行主体。
- 不把 Deep Review 作为所有 PR 的默认阻塞门禁。
- 不在主设计文档中展开各子模块的详细 schema、事件清单、策略规则和执行计划。

## 11. 全局风险与治理策略

| 风险 | 可能后果 | 治理策略 |
|---|---|---|
| 产品边界未收敛 | 基础设施建设周期过长，短期难证明价值 | 首个可验证场景收敛为目标项目的 PR EvidencePack、Risk Classifier 和 lightweight gate |
| 把自身验证经验泛化为产品规则 | 外部项目适配失败，用户需要改造项目才能使用 | BitFun 自身经验只作为验证样本，不进入默认策略；默认策略必须项目可配置 |
| 项目画像误判 | Agent 读错结构、跑错验证、误判 owner 或风险 | profile 必须有来源、置信度、新鲜度和用户确认状态 |
| Artifact Graph 边界过宽 | 低质量链接累积、关系过期、置信度不足，最终降低可用性 | 先覆盖最小 PR 图谱，所有边具备来源、置信度、新鲜度和确认状态 |
| Quality Data Plane 过重 | telemetry bloat、隐私风险、查询成本上升 | 采用事件预算、retention、redaction、sampling 和标准导出 |
| OpenCode 兼容侵入核心模型 | 外部 API 语义影响 BitFun 权限、事件和策略模型 | 兼容层必须是 adapter，内部只承认 BitFun canonical model |
| Deep Review 成本分级约束不足 | token 与耗时过高，低风险 PR 被阻塞 | 按风险触发，先运行低成本结构化检查，再选择 targeted/full review |
| Risk Classifier 权威错觉 | 自动风险标签被误认为事实 | 输出原因、证据、置信度和人工 override；阻塞决策必须引用确定性证据 |
| 需求影响分析误报/漏报 | 低价值候选过多会降低采用率，高漏报会影响发布安全 | 输出候选集合和置信度，高风险低置信链接要求人工确认 |
| Evaluation Harness 无 oracle | 无法判断 Agent 或 Harness 是否真的变好 | 每类任务必须定义 oracle：测试、diff expectation、review rubric 或 human acceptance |
| 插件与 hook 安全边界不足 | 第三方插件绕过权限、泄露数据或篡改证据 | 默认最小权限、超时、redaction、审计和禁用策略 |
| 团队采用路径不清 | 工具价值停留在概念层，难进入日常流程 | 优先嵌入项目加载、本地 diff、PR 描述、required checks 和 reviewer 工作流 |

## 12. 成功状态

全局设计达到目标时，BitFun 应具备以下状态：

- 任意目标项目被加载后，BitFun 能建立可解释的 Project Profile，并明确未知、冲突和过期区域。
- 需求、spec、diff、test、review、PR、release、incident 之间具备可追踪关系。
- 本地 diff 或 PR 可以生成结构化 EvidencePack。
- PR 风险、required checks、Deep Review 状态和未覆盖风险可解释。
- 高风险变更能够触发更强验证和人工确认，低风险变更保持低成本流转。
- Deep Review、影响分析、评估和插件扩展都有预算、降级和审计边界。
- 线上问题和评审失败可以回流到测试、规则、skill 或 eval 任务，形成持续改进闭环。

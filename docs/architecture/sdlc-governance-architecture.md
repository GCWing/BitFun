# BitFun 可配置开发体验与工程治理架构设计

> 产品输入：[sdlc-governance-product-requirements.md](../specs/sdlc-governance-product-requirements.md)
> 研究输入：[sdlc-governance-external-research.md](../specs/sdlc-governance-external-research.md)（非规范参考，引用前复核）
> 范围：把产品需求转为架构边界、领域模型、配置优先级和模块职责。用户画像、体验路径和功能需求以产品需求文档为准。
> 子模块：[security-boundary.md](security-boundary.md)、[quality-data-plane.md](quality-data-plane.md)、[evidence-pack.md](evidence-pack.md)、[artifact-graph.md](artifact-graph.md)、[project-profile-integration.md](project-profile-integration.md)、[agent-workflow-design.md](agent-workflow-design.md)

## 文档结构（原 sdlc-harness README 入口索引）

> 原 docs/sdlc-harness/README.md 的导航表折叠于此；路径已迁到四目录布局。

| 文档 | 角色 | 主要内容 |
|---|---|---|
| [sdlc-governance-external-research.md](../specs/sdlc-governance-external-research.md) | 非规范调研参考 | 外部产品、论文、标准与趋势信号；使用前复核时效性 |
| [sdlc-governance-product-requirements.md](../specs/sdlc-governance-product-requirements.md) | 产品需求 | 产品定位、用户画像、体验路径、产品规格、关键边界、平台差异和成功指标 |
| [sdlc-governance-agent-workflow-adjustment.md](../specs/sdlc-governance-agent-workflow-adjustment.md) | 非权威候选调整提案 | 智能体工作流、并发 GUI、Review 范围控制、token 成本和任务完成度平衡；采纳前需回填权威文档 |
| [sdlc-governance-agent-workflow-staged-plan.md](../plans/sdlc-governance-agent-workflow-staged-plan.md) | 场景收敛计划 | 将工作流、审查、并发和成本控制压回真实用户场景，不新增独立阶段路线 |
| [sdlc-governance-architecture.md](sdlc-governance-architecture.md) | 架构设计（本文） | 设计目标、领域模型、配置层级、模块边界和架构风险 |
| [sdlc-governance-implementation-plan.md](../plans/sdlc-governance-implementation-plan.md) | 实施计划 | 按用户收益切片组织快速路径、上下文保障、团队治理、复杂生命周期能力的阶段落地 |
| [sdlc-governance-traceability-matrix.md](../specs/sdlc-governance-traceability-matrix.md) | 追踪矩阵 | 需求、设计、功能规格、执行阶段和测试方法的映射 |
| [security-boundary.md](security-boundary.md) | 安全边界 | prompt 注入、hook/MCP/网络/凭据/shell、执行位置、沙箱等级和应急放行规则 |
| [configurable-policy-profile.md](../specs/configurable-policy-profile.md) | 配置化策略 | 任务、操作、环境、项目和团队配置如何共同决定内部策略画像、提示、验证和审查 |
| [evidence-pack.md](evidence-pack.md) | 证据包设计 | 证据包负责人、状态、生命周期、风险接受和 PR/审查/回放投影契约 |
| [sdlc-governance-metrics-spec.md](../specs/sdlc-governance-metrics-spec.md) | 指标规格 | 开发效率、安全提示、质量治理和阶段退出指标的公式、分母、窗口和负责人 |
| [self-governance-notes.md](../guideline/self-governance-notes.md) | 自身治理说明 | 记录 BitFun 仓库自身作为内部验证项目暴露出的文档、边界和治理问题 |

## 阅读建议

1. 先读[调研文档](../specs/sdlc-governance-external-research.md)，确认市场正在从单点 AI IDE 走向仓库指令、路径规则、沙箱、异步智能体和可选审查/治理；外部事实使用前须复核。
2. 再读产品需求，确认 BitFun 的默认体验、用户画像、产品规格、关键边界、平台差异和成功指标。
3. 需要架构边界时读本文及同目录子模块设计。
4. 需要落地顺序时读实施计划。
5. 需要检查覆盖关系时读追踪矩阵。
6. 需要实现契约时再读配置化策略、安全边界、证据包、质量数据面（QDP）、风险分类和门禁等子模块。

## 0. 核心定位与全局基础准则

BitFun 面向任意目标项目提供可配置的智能体开发体验。产品定位和需求以 [sdlc-governance-product-requirements.md](../specs/sdlc-governance-product-requirements.md) 为准。

**目标项目治理护栏：** 不要把 BitFun **本仓库**的假设硬编码成目标项目规则。质量保护行为必须可配置、目标感知（target-aware）、有证据支撑、按风险分级、考虑成本，并且可审计。

- **快速开发**：质量保障要求较低、探索性、演示、文档和低风险改动优先完成任务，只给必要提示和轻量结果摘要。
- **上下文保障**：核心路径、权限、网络、数据迁移、发布或团队 PR 等场景触发验证建议、风险说明和审查人建议。
- **团队治理**：项目或组织通过配置启用统一规则、审查强度、强制检查、门禁、风险接受和审计。
- **执行安全**：prompt 注入、恶意 hook、MCP、网络、凭据、跨目录写入、删除和发布凭据等风险始终走独立安全边界，并展示执行位置、沙箱等级和授权范围。
- **阶段收益**：每个阶段都先交付可解释的用户收益，同时说明必要技术前置、延期边界和质量一致性要求。

配置化策略把这些能力组合成按需显露的开发路径：

```text
默认快速路径
  -> 风险出现时进入上下文保障
  -> 项目或组织需要时进入团队治理
  -> 安全边界始终启用
```

证据包、交付物图谱、质量数据面和评测系统作为后台支撑能力使用；普通任务只展示完成任务所需的摘要、提示和下一步建议。Harness 作为内部工程术语，仅用于描述受控执行、证据校验、策略约束和评估回放能力。

### 全局基础准则

这些准则适用于产品需求、架构设计、实施计划和子模块文档。非通用准则只写在对应子模块，并说明触发条件、退出条件和与全局准则的关系。

- **默认轻量，关键风险强保护**：普通低风险开发不进入重流程；凭据、网络外发、危险 shell、跨目录写、删除、发布、Harness 主动配置和 prompt 注入等安全风险始终走安全边界；外部能力沿用其 owner 的安全决定。
- **按动作效果判定，不按工具名称判定**：tool、MCP、skills、插件、hook、shell 和内置能力都映射为能力、目标、数据、来源和副作用，再由策略判断。
- **未知能力默认受限**：新增扩展必须声明能力和可能副作用；未声明、声明不完整或运行时行为超出声明时，不能按低风险处理。
- **用户确认不是万能授权**：确认只在指定范围、期限、执行域和能力内生效；组织策略、安全拒绝和关键凭据保护不能被本地确认绕过。
- **模型只参与解释和候选判断**：风险解释、建议检查和候选影响可以由模型辅助；授权、阻断、审计、状态写入和策略变更必须由确定性策略和内核事实执行。
- **体验和性能预算常驻**：风险判断、提示、事件和扫描不能明显拖慢默认路径；高成本分析、深度证据和完整图谱默认按需、异步或离线执行。
- **扩展能力复用现有归属接口**：Harness 只消费已注册的类型化工具、Hook 变换、事件、权限结果和诊断，
  不定义跨能力统一消息、效果协议或界面扩展对象。新增插件主机/适配器契约必须来自真实垂直切片，
  其交付阶段以产品架构与 OpenCode 兼容文档为准，不与 SDLC Harness 阶段互相替代。

## 1. 设计目标

| 目标 | 架构要求 |
|---|---|
| 快速路径轻量 | 项目打开、任务执行和结果摘要不依赖完整证据包或图谱 |
| 用户语言稳定 | 内部策略画像不直接外露，统一转换为任务状态、弱提示、确认和设置 |
| 策略可解释 | 每次提示、升级、阻断和覆盖都有来源、原因和适用范围 |
| 阶段收益可交付 | 架构能力按用户收益切片落地，技术跑道、API 预设和测试桩不伪装成用户价值 |
| 安全独立 | 执行安全与质量治理分层，安全边界优先于用户覆盖和质量模式 |
| 配置可组合 | 用户、任务、工作区、路径、团队和组织策略按稳定优先级合成 |
| 证据可追溯 | PR、发布、事故和合规场景可以引用证据包、质量事件和交付物图谱 |
| 评估可回放 | 策略、工具、模型、上下文和安全版本能进入评测与指标分析 |
| 生态可扩展 | Harness 复用工具、Hook、事件、权限和诊断的类型化归属接口；插件主机、适配器和产品入口按真实消费路径演进，不在 Harness 内建立通用扩展协议 |

## 2. 复杂来源

| 来源 | 例子 | 架构要求 |
|---|---|---|
| 结构复杂 | 单体多项目、多语言、多服务、生成代码、跨仓库依赖 | 渐进项目画像和路径级规则 |
| 流程复杂 | issue、spec、PR、发布、事故分散在多个系统 | 交付物图谱后台关联，按场景呈现 |
| 验证复杂 | 私有依赖、不稳定 CI、本地环境缺口 | 替代验证建议和不可验证解释 |
| 风险复杂 | 权限、网络、凭据、迁移、发布、远程工作区 | 安全边界和守护策略分层处理 |
| 团队复杂 | CODEOWNERS、组织策略、合规审计 | 团队治理和风险接受 |
| 信息复杂 | 旧文档、冲突规则、prompt 注入、未知配置 | 来源、信任、新鲜度和冲突状态 |

## 3. 顶层领域模型

| 领域 | 核心对象 | 作用 |
|---|---|---|
| 项目理解 | 工作区、仓库、执行环境、远程上下文、语言、框架、模块、规则来源、验证能力 | 减少用户解释和错误路径选择 |
| 用户画像 | 用户类型、任务熟悉度、偏好、授权范围、受管状态 | 选择默认入口、解释语言和提示密度 |
| 任务与意图 | 任务、阶段、目标路径、会话模式、用户覆盖 | 决定快速、PR、发布或应急路径 |
| 阶段收益 | 用户可见收益、必要技术前置、延期边界、降级解释、质量一致性检查 | 约束阶段交付必须产生可解释体验变化 |
| 体验视图 | 内部策略画像、用户可见状态、提示层级、确认策略、设置项 | 降低用户学习成本和提示噪音 |
| 执行安全 | 权限、执行位置、沙箱等级、网络域名、凭据访问、Harness 主动配置审核、应急放行 | 防止 prompt、配置或工具越权 |
| 扩展消费 | 类型化工具、Hook 变换、公开事件、权限结果、来源与运行诊断 | 在不读取生态原始对象或主机内部状态的前提下复用扩展能力 |
| 变更信心 | 变更摘要、验证摘要、风险提示、未验证项、跳过检查 | 给用户可理解的信心和下一步行动 |
| Review 生命周期 | 稳定记录、不可变修订、执行阶段、结果可用性、问题结论、证据覆盖、目标新鲜度和用户处置 | 让后台审查可恢复、可复审且不把内部 child 变成第二个产品入口 |
| 团队治理 | 路径规则、审查强度、强制检查、风险接受、审计策略 | 统一团队和高可靠场景体验 |
| 生命周期上下文 | issue、spec、计划、PR、发布、事故、学习资产 | 支撑复杂项目追溯和复盘 |
| 评测与学习 | 轨迹回放、评测任务、判定标准、反馈、指标 | 证明策略改善体验和质量 |

## 4. 逻辑架构

```text
用户界面
  快速工作台 / 变更摘要 / PR 就绪度 / 发布审查 / 设置

体验视图层
  任务状态 / Review 记录与修订 / 弱提示 / 可折叠摘要 / 确认 / 受限原因 / 降噪规则

阶段收益编排
  用户收益 / 技术前置 / 延期边界 / 降级解释 / 质量一致性检查

配置化策略面
  意图识别 / 策略画像 / 风险提示 / 审查画像 / 覆盖策略

安全边界
  执行位置 / 沙箱等级 / 权限 / 网络 / 凭据 / Harness 主动配置审核 / 应急放行

扩展消费边界
  类型化工具 / Hook 变换 / 公开事件 / 权限结果 / 来源与运行诊断

插件主机与生态适配（Harness 外部）
  来源发现 / 策略计算 / 进程执行 / 生态转换 / 健康与降级

交付物与证据面
  Review 目标证据 / 证据包 / 交付物图谱 / 问题生命周期 / 风险接受

质量数据面
  生命周期事件 / 执行轨迹 / 验证摘要 / 指标 / 本地审计

项目集成面
  Git / PR provider / CI / Issue / 文档 / 发布 / 观测 / 知识库

智能体与工具运行时
  会话 / 智能体 / 终端 / 文件系统 / MCP-LSP / Tool ABI / 适配器
```

关键边界：

- 用户界面展示用户可理解的任务状态、摘要、提示和确认。
- 体验视图层把内部策略画像映射为用户语言，并负责提示合并、延后和降噪。
- Review 记录是现有只读 child 执行之上的稳定用户投影；复审创建新修订，目标证据、执行器和修复动作继续由原 owner 负责。
- Review 的完整产品能力由一个主审和按具体问题选择的有界复核能力共同提供；内置规则、Skills 和用户配置的只读审核能力是内部来源，不形成用户可见的固定团队。普通与严格 Review 的额度、证据范围和停止条件由既有 Review 执行 owner 约束，不新增 Harness 调度层。
- 阶段收益编排约束每次落地必须说明用户可见收益、后台前置、延期边界和质量一致性。
- 配置化策略面决定内部体验强度、检查建议、证据展示层级和用户覆盖选项。
- 安全边界负责权限、执行位置、沙箱等级、网络、凭据和高风险动作隔离；权限确认、快照/回滚隔离和运行时沙箱必须分开表达。
- Harness 不定义跨能力统一消息、效果协议或界面扩展对象。插件主机和适配器在 Harness 外部
  完成来源发现、执行与生态转换，再通过已有 owner 暴露类型化工具、Hook、事件、权限结果和诊断。Hook 可以按稳定
  语义变换允许字段，但结构校验、策略上限与最终提交仍由对应 owner 完成；模型推断仍只能作为候选。
- 交付物、证据和质量数据面支撑解释、审查、发布和复盘。
- 项目集成面适配外部系统，并把外部语义映射为内部稳定事件和接口。当前工作区和明确 Git range 先固定 revision、文件状态和完整度，再由交付物与证据面形成 session-scoped 目标清单；工作区可声明 prepared diff 覆盖完整，但最终 evidence status 保持 limited。Reviewer 不自行 fetch、checkout 或猜 ref。
- 智能体运行时执行任务，并通过策略面和安全边界获取质量结论与授权状态。

权威状态 owner：

| 状态类别 | Owner | 非 owner 行为 |
|---|---|---|
| 任务事实、事件序列和审计事实落盘 | Agent Kernel | 产品入口、插件和适配器只能经类型化 owner 接口提交请求或结果；Security Boundary 只提交安全决策和安全审计 payload，不直接维护事件序列 |
| 工具执行结果 | Execution | 插件工具与内置/MCP 工具走同一注册、权限、取消和结果路径；插件进程或 Harness 不能绕过 Execution 直接写结果 |
| Review 目标证据 | 项目集成面解析当前工作区 / Git range / PR 的原始事实，交付物与证据面生成本次会话的固定目标清单和完整度；只有 immutable revision 或真实快照可声明内容不可变 | Reviewer、PR 面板和质量决策层只能消费目标证据或提交展示/执行请求；不能改写 base/head、静默补 fetch、把文件重叠当作目标身份 |
| Review 记录、修订摘要与用户处置 | 既有 session-history persistence owner 保存记录锚点、修订有界投影和稀疏用户处置，提供按父任务或精确 PR identity 的只读摘要查询，并按记录执行归档、保留和删除 | UI 已加载 session 不是全量索引；Review child、PR adapter、质量数据面和模型不能各自维护记录身份、覆盖用户处置、复制完整报告或单独删除仍有后续修订的锚点 |
| 权限和安全决策 | Security Boundary 产生 permission decision、`security.decided` payload 和安全审计 payload | ACP、MCP、hook、plugin 或模型建议不能直接 approve、deny；Agent Kernel 只记录决策事实，不重新判定权限 |
| 就绪度和门禁视图 | 变更就绪度 / PR 门禁，基于证据、策略和人工决策生成 | Execution 和插件不能写通过、失败或阻断结论 |
| 质量数据视图 | Quality Data Plane | 只归一化、查询和生成只读视图，不成为新的权威状态源 |

## 5. 模块边界

| 模块 | 文档 | 产品角色 | 默认显露 |
|---|---|---|---|
| 配置化策略画像 | [configurable-policy-profile.md](../specs/configurable-policy-profile.md) | 决定内部策略画像、检查强度和提示视图 | 是 |
| 阶段收益编排 | [implementation-plan.md](../plans/sdlc-governance-implementation-plan.md) | 把能力拆成用户收益、技术前置、延期边界和验收切片 | 发布说明/阶段评审中 |
| Review 生命周期 | [../architecture/review-lifecycle.md](../architecture/review-lifecycle.md) | 在现有 Review 执行之上投影一个可恢复记录及其修订、覆盖、新鲜度和问题连续性 | 是，但默认不展开内部执行详情 |
| Review 执行 | [../architecture/deep-review.md](../architecture/deep-review.md) | 约束目标证据、只读主审、按问题选择的有界复核能力、文件分包、调用预算和聚合结果 | 仅呈现一个 Review 及自然语言进度 |
| 安全边界 | [security-boundary.md](security-boundary.md) | 管执行安全、执行位置、沙箱等级、权限、应急放行 | 是，但低噪音 |
| 项目画像与集成 | [project-profile-integration.md](project-profile-integration.md) | 渐进理解项目结构、规则和验证能力 | 部分 |
| 质量数据面 | [quality-data-plane.md](quality-data-plane.md) | 记录事件、验证、提示和安全决策 | 否 |
| 证据包 | [evidence-pack.md](evidence-pack.md) | 统一证据只读视图和 schema | 快速路径否，PR/治理时是 |
| 交付物图谱 | [artifact-graph.md](artifact-graph.md) | 关联 diff、验证、PR、issue、发布、事故 | 快速路径否，复杂项目按需 |
| 风险分类器 | [risk-classifier.md](../specs/risk-classifier.md) | 输出风险原因、检查建议和升级信号 | 以提示形式 |
| 变更就绪度 / PR 门禁 | [pr-quality-gate.md](../specs/pr-quality-gate.md) | 生成 PR 信心摘要；强策略下成为门禁 | 仅准备 PR 或配置启用 |
| 需求影响分析 | [requirement-impact-analysis.md](../specs/requirement-impact-analysis.md) | 高风险需求/API/设计变更的影响候选 | 按需 |
| 智能体评测 | [agent-evaluation.md](../specs/agent-evaluation.md) | 评估智能体、上下文、策略和治理策略 | 否 |
| OpenCode 扩展消费 | [opencode-compatibility-sdlc.md](../specs/opencode-compatibility-sdlc.md) | 消费已由归属模块注册的 OpenCode 工具、Hook、事件、权限结果和诊断 | 仅在能力真实可用时显露 |

## 6. 配置层级

| 层级 | 例子 | 作用 |
|---|---|---|
| 用户偏好 | 用户选择快/严、语言、默认授权范围 | 个体体验默认值 |
| Session/任务覆盖 | “这次快速放行网络”“本任务只读分析” | 临时治理强度 |
| 工作区配置 | `.bitfun/config.toml` 或 `.bitfun/quality.yaml` | BitFun 专属模式、检查、安全策略 |
| 现有仓库规则 | AGENTS.md、CONTRIBUTING、CODEOWNERS、CI、`.github/instructions` | 项目知识和规则来源 |
| 工具特定规则 | `.coderabbit.yaml`、`.gitlab/duo/*`、`.kiro/steering/*` | 外部工具和路径级经验 |
| 组织策略 | 受管配置、受保护分支、企业策略 | 强制策略和不可绕过限制 |

```text
组织拒绝 / 受管强制要求
  > 安全边界
  > 最近确认的路径或团队规则
  > 工作区配置
  > 允许范围内的会话/任务覆盖
  > 全局/用户默认值
```

## 7. 硬约束

| 约束 | 设计要求 |
|---|---|
| 内部画像不直接外露 | `fast/assist/review/guarded/regulated` 作为内部状态词；用户侧展示任务状态、原因和下一步 |
| 提示先弱后强 | 默认行内提示、可折叠摘要和任务结束汇总；弹窗只用于安全、不可逆、组织强制或关键流程 |
| 门禁按配置显露 | 门禁在 PR、团队、强策略或合规场景显露 |
| 安全边界独立执行 | prompt 注入、凭据、网络、hook 等风险走独立判定 |
| 沙箱能力显式分层 | 本地、远程、ACP、MCP、插件、浏览器/桌面和云端任务都输出执行位置、沙箱等级和降级原因 |
| 阶段按收益切片 | 同一模块可以跨阶段交付；每个阶段必须有用户可见收益、必要技术前置、延期边界和质量一致性检查 |
| 技术接口后台化 | 证据包、质量数据面和图谱服务摘要、审查、发布和复盘 |
| 未知先进入建议态 | 新项目初期先提示和建议，安全或团队策略再阻断 |
| 用户可临时放行 | 应急放行有范围、期限、记录和撤销 |
| 组织策略可强制 | 受管策略高于本地覆盖 |
| 模型输出作为候选 | 模型只能输出解释、摘要、风险或影响候选；策略改变和权威状态来自确定性证据、用户决策或受管策略 |
| Review 目标先于执行 | 当前修改、明确 Git range 和 provider PR 必须先形成带 revision、文件状态和完整度的只读目标证据；证据缺失只能降级，不能由 Reviewer 猜测或写成完整覆盖 |
| Review 状态保持正交 | 执行阶段、结果可用性、问题结论、证据覆盖和目标新鲜度分别投影；有限覆盖不能隐藏已有问题，过期不能改写历史执行状态，模型未重复问题不能自动改变用户处置 |
| Reviewer Git 最小权限 | 保留既有 Reviewer Git 暴露以兼容旧入口，但 prepared target evidence 不把它作为 changed-code 证据，也不新增 Git 工具或任意 shell；prepared target 只通过有界 `GetFileDiff` 消费变更。只有本地仓库与目标 head 匹配且整个工作区干净时，现有 Read/Grep/Glob/LS 才补充 live context；不做逐调用全仓扫描、网络、checkout 或仓库状态修改 |
| 能力与操作边界统一 | tool、MCP、skills、插件、hook 和内置能力都必须携带足够的能力、目标对象、数据类别、来源和副作用信息，供现有安全 owner 判定 |
| 未声明能力受限 | 新增扩展未声明能力、声明不完整或运行时行为超出声明时，只能进入受限模式或安全确认，不能按低风险静默执行 |
| 策略不写死工具名 | 策略引擎以能力、效果、数据、来源、执行域和配置上下文判定；工具名只用于展示、审计、兼容和调试 |
| 外部适配不写权威状态 | 外部插件和适配器可以返回类型化工具结果或合法 Hook 变换；任务事实和审计事实落盘、工具结果提交、安全决策 payload、就绪度/门禁仍分别由 Agent Kernel、Execution、Security Boundary 和变更就绪度模块负责 |
| 工具复写显式授权 | 用户确认复写哪个工具和能力范围；复写表按项目执行域生效，不能静默覆盖全局工具 |

## 8. 架构风险

| 风险 | 治理策略 |
|---|---|
| 产品默认过重 | P0 以快速路径和低摩擦指标验收 |
| 技术计划脱离用户收益 | 阶段评审必须检查用户可见收益、延期边界和质量一致性，后台技术只能作为明确前置项进入 |
| 内部术语外露 | 通过体验视图层统一用户语言，设置和日志才展示调试级策略信息 |
| 安全提示太频繁 | 沙箱、白名单、范围和域名策略降噪 |
| 应急放行被滥用 | 范围、期限、组织禁用、隔离建议和审计 |
| 项目规则被注入污染 | 规则来源、信任、新鲜度和恶意指令检测 |
| 路径规则冲突 | 最近规则、冲突展示和人工处理 |
| 扩展点时序漂移 | 事件 id、sequence、deadline、epoch 和幂等键约束；过期响应不应用 |
| 能力声明被绕过 | 适配器注册、工具复写、MCP 描述和插件调用都必须经过对应 owner 的结构、能力和策略校验；未知能力默认受限 |
| 工具复写静默越权 | 内置工具复写必须显式展示、按项目生效，并重新经过安全边界 |
| 图谱和证据过早显露 | 只在解释、PR、发布、事故时显性化 |
| Review 审错目标或复用过期结果 | base/head、目标指纹、完整度和 workspace binding 进入目标证据；head、diff 或绑定变化后旧结果只能作为历史引用，不能发布或支撑当前就绪度 |
| “只读 Git”仍产生副作用 | Reviewer 的既有 Git 暴露不扩权，prepared target evidence 不将其作为 changed-code 证据；本地目标 diff 走禁用 external diff/textconv 的有界 `GetFileDiff`，provider PR diff 按文件读取并复核 base/head；普通 Agent 和旧 Review 保留既有行为；exact diff 不可用时明确降级或在 Reviewer 启动前停止 |
| 平均体验掩盖局部问题 | 指标按用户画像、任务风险、内部策略画像、用户可见视图、入口平台和受管状态切片 |

# BitFun 子模块设计：项目画像与集成

> 上游文档：[design.md](../design.md)
> 模块角色：在 BitFun 加载外部目标项目后，渐进发现、归一化、版本化项目画像，并通过适配器连接项目依赖的代码托管、议题（issue）、CI、文档、发布和观测系统。

## 1. 模块定位

项目画像是渐进项目理解能力。它让 BitFun 更快进入有用状态，并在风险出现时解释检查、提示、审查和配置建议的来源。

它分三层：

| 层级 | 目的 | 用户体验 |
|---|---|---|
| 轻量项目理解（Lightweight Project Understanding） | 快速识别语言、包管理器、脚本、git 状态、README/AGENTS/CONTRIBUTING/CI 入口 | 快速路径可直接工作 |
| 已确认画像（Confirmed Profile） | 确认规则、负责人、路径边界、验证能力、主动配置信任状态 | PR/团队场景提供更准建议 |
| 集成项目上下文（Integrated Project Context） | 连接 issue、CI、文档、发布、观测系统和多仓库 | 复杂项目、发布和复盘时显露 |

项目画像采用分层成熟度：快速路径先读取低成本信号；PR、团队治理、发布和复盘场景再补齐负责人、规则、验证能力、外部系统和主动配置状态。这样既能降低误判和脏链接，也能保证第一次有用动作足够快。

主动配置是本模块必须发现但不能信任的对象：hook、plugin、自定义工具、MCP server、智能体规则中可改变执行、权限、上下文或网络访问的配置。主动配置默认只作为项目事实，执行权由 [安全边界](security-boundary.md) 和用户/团队策略决定。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [GitHub Copilot 仓库指令](https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/add-custom-instructions/add-repository-instructions) | 仓库/路径指令和 AGENTS.md 已成为项目规则入口 |
| [Kiro Steering](https://kiro.dev/docs/steering/) | 工作区、全局、团队和加载范围模式可以减少无关上下文 |
| [CodeRabbit 路径指令](https://docs.coderabbit.ai/configuration/path-instructions) | 路径级规则更适合单体多项目仓库和团队差异 |
| [CDEvents](https://cdevents.dev/docs/primer/) | CI/CD 事件应保持松耦合、声明式和互操作 |
| [SLSA Provenance](https://slsa.dev/spec/v0.1/provenance) / [in-toto attestation](https://slsa.dev/blog/2023/05/in-toto-and-slsa) | 构建和交付证据需要来源证明、时间、环境和生成方式 |

设计约束：

- 项目画像必须来源可追踪、可刷新、可失效。
- 缺失或冲突信息必须显式暴露，不能用默认假设掩盖。
- P0 只做快速路径所需轻量画像，不建设企业级集成平台。
- 适配器只负责读取、同步和投影外部系统语义，不改变 BitFun 规范模型。
- 项目画像必须支持多语言、多仓库、多 CI、多发布模式。
- 未信任主动配置不得影响执行、模式确认或就绪度通过判断。
- 用户可以跳过非关键画像补全，但跳过结果必须影响信心摘要。

## 3. 范围与边界

范围：

- 发现目标项目结构、语言、框架、模块、负责人、规则来源和验证能力。
- 识别未知区域、规则冲突、过期规则和不可访问外部系统。
- 发现主动配置并记录来源、hash、权限声明、启用范围和信任状态。
- 为配置化策略、安全边界、风险分类器、证据包、交付物图谱和评测提供项目画像。
- 提供代码托管、issue、文档、CI、发布和观测系统的适配器边界。

边界：

- 目标项目的配置管理、需求管理、CI 和发布系统保留系统主权，BitFun 通过适配器读取和投影。
- 项目画像以实际仓库信号为准，避免把单一技术栈或单一组织流程作为模板。
- 目标项目无需先改造成 BitFun 推荐结构；BitFun 通过路径、规则和适配器逐步理解。
- P0 只覆盖快速路径所需画像；组织知识图谱和企业权限系统属于后续集成能力。
- 主动配置发现只记录候选执行面，信任审批由安全边界和团队策略完成。

## 4. 输入、输出与数据模型

输入：

| 输入 | 示例 |
|---|---|
| 仓库事实 | 文件树、依赖文件、构建配置、测试目录、生成文件 |
| 规则来源 | README、CONTRIBUTING、AGENTS.md、`.github/instructions`、CODEOWNERS、模块文档 |
| 验证来源 | package scripts、任务运行器、CI 工作流、测试报告、lint/typecheck/build 命令 |
| 主动配置来源 | hooks、plugins、自定义工具、MCP servers、智能体规则、自动化配置 |
| 负责人来源 | CODEOWNERS、git 历史、issue assignee、团队映射 |
| 外部集成 | GitHub/GitLab、Jira/Linear、Confluence/Notion、CI、制品仓库、观测系统 |
| 用户确认 | 手动确认模块边界、负责人、验证命令、敏感区域和不支持状态 |

输出：

```ts
interface ProjectProfile {
  project_id: string;
  maturity: "lightweight" | "confirmed" | "integrated";
  roots: ProjectRoot[];
  languages: LanguageProfile[];
  modules: ModuleProfile[];
  rule_sources: RuleSource[];
  verification_capabilities: VerificationCapability[];
  ownership: OwnershipProfile;
  integrations: IntegrationProfile[];
  risk_areas: RiskArea[];
  active_configs: ActiveConfigProfile[];
  unknowns: ProfileUnknown[];
  conflicts: ProfileConflict[];
  freshness: FreshnessSnapshot;
  confidence: number;
}
```

关键状态：

| 状态 | 含义 | 下游影响 |
|---|---|---|
| `confirmed` | 来源明确且已被用户或确定性证据确认 | 可作为强制策略、风险和图谱的强依据 |
| `inferred` | 由文件、配置、历史或静态分析推断 | 可作为候选依据，需要展示置信度 |
| `unknown` | 缺少足够信息 | 下游保持建议态或降级状态，必要时要求人工确认 |
| `conflicting` | 多个规则来源冲突 | 下游不得自动选择高风险路径 |
| `stale` | 来源已变更或超过刷新窗口 | 需要刷新或重新确认 |

画像生成优先级：

| 来源 | 优先级 | 说明 |
|---|---:|---|
| 组织拒绝/安全策略 | 0 | 不允许被用户级配置覆盖 |
| 用户确认 | 1 | 高风险规则、负责人、发布边界以用户确认为准 |
| 确定性配置 | 2 | CI、build、package、CODEOWNERS、类型化配置 |
| 项目文档 | 3 | README、贡献指南、智能体规则、模块文档 |
| 历史信号 | 4 | 共同变更、事故、审查阻塞项、高频热点文件 |
| 模型推断 | 5 | 只能生成候选，不作为事实 |

## 5. 核心流程

```text
打开目标项目
  -> 执行轻量本地发现
  -> 允许快速路径启动
  -> 渐进发现规则和验证来源
  -> 发现主动配置并发送到安全边界
  -> 归一化画像并标记未知、冲突和过期状态
  -> 只对关键缺口请求确认
  -> 发出 project.profiled 事件
  -> 项目变化时刷新或失效
```

主动配置状态：

| 状态 | 含义 | 下游影响 |
|---|---|---|
| `discovered` | 已发现配置，但尚未审核 | 只能展示，不得执行 |
| `trusted` | 用户或策略确认来源、hash、权限和范围 | 可按权限执行并写审计 |
| `changed` | 内容、hash、权限或来源变化 | 原信任失效，需要重新确认 |
| `disabled` | 用户、策略或安全规则禁用 | 不参与执行，可保留审计记录 |

## 6. 适配器边界

| 适配器 | 读取对象 | 输出到 BitFun |
|---|---|---|
| Git 适配器 | branch、diff、commit、PR ref、历史 | 变更集、负责人提示、风险信号 |
| Issue 适配器 | issue、ticket、验收标准、负责人、状态 | 交付物节点、需求上下文 |
| 文档适配器 | 设计文档、运行手册、决策记录、团队规则 | 规则来源、上下文来源 |
| CI 适配器 | workflow、job、检查、artifact、日志摘要 | 验证能力、证据项 |
| 发布适配器 | 发布、artifact、环境、回滚信息 | 发布就绪度上下文 |
| 观测适配器 | 事故、指标、轨迹/日志链接、告警 | 运行时反馈和图谱回溯 |

适配器只输出规范事实和证据引用，不直接写就绪度、门禁或安全结论。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | 轻量项目理解、规则入口发现、验证命令候选、主动配置发现、未知/冲突标记 |
| P1 | 用户确认、画像刷新、路径级规则、主动配置信任审查持久化 |
| P2 | GitHub/GitLab PR、issue、CI 适配器；团队策略包 |
| P3 | 文档、发布、观测适配器；多仓库和多工作区支持 |
| P4 | 画像漂移看板、跨项目画像对比和治理指标 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 画像误判导致错误门禁 | 未确认画像只能作为候选；强策略必须引用已确认或确定性证据 |
| 入门过重 | P0 必须在几分钟内生成可用轻量画像，并允许边工作边补全 |
| 对 BitFun 自身验证样本过拟合 | 默认模式不能内置 BitFun 路径、语言或验证命令 |
| 外部系统耦合 | 适配器输出规范事实，不让外部载荷泄漏到核心策略 |
| 敏感信息泄露 | 画像写入前执行脱敏，凭据和私有日志只存引用或摘要 |
| 主动配置被误认为可信规则 | hook、plugin、自定义工具只作为已发现事实，必须信任审查后才可执行 |
| 画像过期 | 文件、CI、规则或集成状态变化必须触发新鲜度更新 |
| 用户不信任推断 | 用户界面必须展示来源、置信度、冲突和确认状态 |

## 9. 成功标准

- 新目标项目加载后可快速生成轻量画像并开始快速路径。
- 用户无需先配置完整 `.bitfun` 就能完成普通任务。
- 风险分类器、变更就绪度和安全边界能解释所用项目事实来自哪里。
- 未知或冲突规则会降低信心，并进入建议态或降级状态。
- 外部适配器接入不会改变 BitFun 规范事件、交付物、权限和策略模型的一致性。
- 主动配置能被发现、展示、确认、禁用和重新确认，且默认不自动执行。

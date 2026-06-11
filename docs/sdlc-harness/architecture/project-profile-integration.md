# BitFun 子模块设计：Project Profile and Integration

> 上游文档：[design.md](../design.md)
> 模块角色：在 BitFun 加载外部目标项目后，渐进发现、归一化、版本化项目画像，并通过 adapter 连接项目依赖的代码托管、issue、CI、文档、发布和观测系统。

## 1. 模块定位

Project Profile 是渐进项目理解能力，不是用户开始开发前必须完成的建模步骤。它的产品目标是让 BitFun 更快进入有用状态，同时在风险出现时能解释“我为什么建议这个检查/提示/审查/配置”。

它分三层：

| 层级 | 目的 | 用户体验 |
|---|---|---|
| Lightweight Project Understanding | 快速识别语言、包管理器、脚本、git 状态、README/AGENTS/CONTRIBUTING/CI 入口 | Fast Path 可直接工作 |
| Confirmed Profile | 确认规则、owner、路径边界、验证能力、主动配置信任状态 | PR/team 场景提供更准建议 |
| Integrated Project Context | 连接 issue、CI、docs、release、observability、多仓库 | 复杂项目、发布和复盘时显露 |

没有 Project Profile，Risk Classifier 容易把未知项目误判为低风险，Change Readiness 容易推荐错误验证，Artifact Graph 容易建立脏链接。反过来，如果把完整 Profile 作为入口前置条件，普通用户会在第一次有用动作前被重流程劝退。

主动配置是本模块必须发现但不能信任的对象：hook、plugin、custom tool、MCP server、agent rules 中可改变执行、权限、上下文或网络访问的配置。主动配置默认只作为 profile fact，执行权由 [Security Boundary](security-boundary.md) 和用户/团队策略决定。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [GitHub Copilot repository instructions](https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/add-custom-instructions/add-repository-instructions) | repo/path instructions 和 AGENTS.md 已成为项目规则入口 |
| [Kiro Steering](https://kiro.dev/docs/steering/) | workspace、global、team 和 inclusion mode 可以减少无关上下文 |
| [CodeRabbit path instructions](https://docs.coderabbit.ai/configuration/path-instructions) | 路径级规则更适合 monorepo 和团队差异 |
| [CDEvents](https://cdevents.dev/docs/primer/) | CI/CD 事件应保持松耦合、声明式和互操作 |
| [SLSA Provenance](https://slsa.dev/spec/v0.1/provenance) / [in-toto attestation](https://slsa.dev/blog/2023/05/in-toto-and-slsa) | 构建和交付证据需要 provenance，而不是仅保存日志摘要 |

设计约束：

- 项目画像必须来源可追踪、可刷新、可失效。
- 缺失或冲突信息必须显式暴露，不能用默认假设掩盖。
- P0 只做 Fast Path 所需轻量画像，不建设企业级集成平台。
- adapter 只负责读取、同步和投影外部系统语义，不改变 BitFun canonical model。
- 项目画像必须支持多语言、多仓库、多 CI、多发布模式。
- 未信任主动配置不得影响执行、profile confirmation 或 readiness pass。
- 用户可以跳过非关键画像补全，但跳过结果必须影响 confidence summary。

## 3. 范围与非目标

范围：

- 发现目标项目结构、语言、框架、模块、owner、规则来源和验证能力。
- 识别未知区域、规则冲突、过期规则和不可访问外部系统。
- 发现主动配置并记录来源、hash、权限声明、启用范围和信任状态。
- 为 Adaptive Control、Security Boundary、Risk Classifier、EvidencePack、Artifact Graph 和 Evaluation 提供项目画像。
- 提供代码托管、issue、文档、CI、发布和观测系统的 adapter 边界。

非目标：

- 不替代目标项目的配置管理、需求管理、CI 或发布系统。
- 不把任何单一项目结构当作默认模板。
- 不要求目标项目先改造成 BitFun 推荐结构。
- 不在 P0 做完整组织知识图谱或企业权限系统。
- 不把主动配置 discovery 当成 trust approval。

## 4. 输入、输出与数据模型

输入：

| 输入 | 示例 |
|---|---|
| Repository facts | 文件树、依赖文件、构建配置、测试目录、生成文件 |
| Rule sources | README、CONTRIBUTING、AGENTS.md、`.github/instructions`、CODEOWNERS、module docs |
| Verification sources | package scripts、task runner、CI workflow、test reports、lint/typecheck/build commands |
| Active config sources | hooks、plugins、custom tools、MCP servers、agent rules、automation config |
| Ownership sources | CODEOWNERS、git history、issue assignee、team mapping |
| External integrations | GitHub/GitLab、Jira/Linear、Confluence/Notion、CI、artifact registry、observability |
| User confirmation | 手动确认模块边界、owner、验证命令、敏感区域和不支持状态 |

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
| `confirmed` | 来源明确且已被用户或确定性证据确认 | 可作为 required policy、risk、graph 的强依据 |
| `inferred` | 由文件、配置、历史或静态分析推断 | 可作为候选依据，需要展示置信度 |
| `unknown` | 缺少足够信息 | 下游保持 advisory/degraded 或要求人工确认 |
| `conflicting` | 多个规则来源冲突 | 下游不得自动选择高风险路径 |
| `stale` | 来源已变更或超过刷新窗口 | 需要刷新或重新确认 |

画像生成优先级：

| 来源 | 优先级 | 说明 |
|---|---:|---|
| 组织 deny/security policy | 0 | 不允许被用户级配置覆盖 |
| 用户确认 | 1 | 高风险规则、owner、发布边界以用户确认为准 |
| 确定性配置 | 2 | CI、build、package、CODEOWNERS、typed config |
| 项目文档 | 3 | README、贡献指南、agent rules、模块文档 |
| 历史信号 | 4 | co-change、incident、review blocker、hot files |
| 模型推断 | 5 | 只能生成候选，不作为事实 |

## 5. 核心流程

```text
open target project
  -> lightweight local discovery
  -> allow Fast Path to start
  -> discover rules and verification sources incrementally
  -> discover active config and send to Security Boundary
  -> normalize profile and mark unknown/conflict/stale
  -> ask confirmation only for critical gaps
  -> emit project.profiled event
  -> refresh / invalidate on project changes
```

主动配置状态：

| 状态 | 含义 | 下游影响 |
|---|---|---|
| `discovered` | 已发现配置，但尚未审核 | 只能展示，不得执行 |
| `trusted` | 用户或策略确认来源、hash、权限和范围 | 可按权限执行并写审计 |
| `changed` | 内容、hash、权限或来源变化 | 原信任失效，需要重新确认 |
| `disabled` | 用户、策略或安全规则禁用 | 不参与执行，可保留审计记录 |

## 6. Adapter 边界

| Adapter | 读取对象 | 输出到 BitFun |
|---|---|---|
| Git adapter | branch、diff、commit、PR ref、history | changeset、owner hints、risk signals |
| Issue adapter | issue、ticket、acceptance criteria、assignee、status | artifact nodes、requirement context |
| Docs adapter | design doc、runbook、decision record、team rules | rule source、context provenance |
| CI adapter | workflow、job、check、artifact、log summary | verification capability、evidence item |
| Release adapter | release、artifact、environment、rollback info | release readiness context |
| Observability adapter | incident、metric、trace/log link、alert | runtime feedback and graph backtrace |

Adapter 只输出 normalized facts 和 evidence references，不直接写 readiness、gate 或安全结论。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | 轻量项目理解、规则入口发现、验证命令候选、主动配置发现、unknown/conflict 标记 |
| P1 | 用户确认、profile refresh、path-scoped rules、active config trust review 持久化 |
| P2 | GitHub/GitLab PR、issue、CI adapter；team policy package |
| P3 | 文档、发布、observability adapter；多仓库和多 workspace 支持 |
| P4 | profile drift dashboard、跨项目画像对比和治理指标 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 画像误判导致错误门禁 | 未确认画像只能作为候选；强策略必须引用 confirmed 或 deterministic evidence |
| onboarding 过重 | P0 必须在几分钟内生成可用轻量画像，并允许边工作边补全 |
| 对 BitFun 自身验证样本过拟合 | 默认 profile 不能内置 BitFun 路径、语言或验证命令 |
| 外部系统耦合 | adapter 输出 canonical facts，不让外部 payload 泄漏到核心策略 |
| 敏感信息泄露 | profile 写入前执行 redaction，secret 和私有日志只存引用或摘要 |
| 主动配置被误认为可信规则 | hook/plugin/custom tool 只作为 discovered fact，必须 trust review 后才可执行 |
| 画像过期 | 文件、CI、规则或集成状态变化必须触发 freshness 更新 |
| 用户不信任推断 | UI 必须展示来源、置信度、冲突和确认状态 |

## 9. 成功标准

- 新目标项目加载后可快速生成 lightweight profile 并开始 Fast Path。
- 用户无需先配置完整 `.bitfun` 就能完成普通任务。
- Risk Classifier、Change Readiness 和 Security Boundary 能解释所用项目事实来自哪里。
- 未知或冲突规则会降低 confidence 或进入 advisory/degraded，而不是默认通过。
- 外部 adapter 接入不会改变 BitFun canonical event、artifact、permission 和 policy model 的一致性。
- 主动配置能被发现、展示、确认、禁用和重新确认，且默认不自动执行。

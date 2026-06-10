# BitFun 生命周期工程子模块设计：Project Profile and Integration

> 上游文档：[design.md](../design.md)
> 模块角色：在 BitFun 加载外部目标项目后，发现、归一化、版本化项目画像，并通过 adapter 连接项目依赖的代码托管、issue、CI、文档、发布和观测系统。

## 1. 模块定位

Project Profile and Integration 是风险、门禁、影响分析和质量保护判断的入口层。它回答三个问题：

1. 当前被加载的目标项目是什么。
2. 该项目有哪些规则、边界、验证能力、owner、风险区域和外部系统。
3. 哪些信息已经确认，哪些信息未知、冲突、过期或不可访问。

没有 Project Profile，Risk Classifier 容易把未知项目误判为低风险，PR Gate 容易要求错误验证，Artifact Graph 容易建立脏链接，Deep Review 容易读取错误上下文。因此本模块不只是“项目扫描”，而是 BitFun 面向外部项目可用性的基础。

Project Profile 还必须识别“主动配置”：hook、plugin、custom tool、MCP server、agent rules 中可改变执行、权限、上下文或网络访问的配置。主动配置默认只作为 profile fact，不自动获得执行权。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [Kiro Steering](https://kiro.dev/docs/steering/) / [Specs](https://kiro.dev/docs/specs/) | 项目规则、产品意图、技术栈和结构说明应成为核心上下文，而不是临时 prompt |
| [Atlassian Rovo Dev](https://www.atlassian.com/software/rovo-dev) / [Software Collection](https://www.atlassian.com/collections/software) | Agent 需要连接 Jira、文档、代码、CI 和团队上下文，而不只读取仓库文件 |
| [Harness Software Delivery Knowledge Graph](https://www.harness.io/blog/knowledge-graphs-for-ai-software-delivery) | 软件交付知识图谱必须以语义层和持续同步为基础，避免只做 RAG 堆料 |
| [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/concepts/semantic-conventions/) | 跨系统事件和指标需要稳定命名与资源语义 |
| [CDEvents](https://cdevents.dev/docs/primer/) | CI/CD 事件应保持松耦合、声明式和互操作，不应形成点对点强耦合 |
| [SLSA Provenance](https://slsa.dev/spec/v0.1/provenance) / [in-toto attestation](https://slsa.dev/blog/2023/05/in-toto-and-slsa) | 构建和交付证据需要 provenance，而不是仅保存日志摘要 |

设计约束：

- 项目画像必须来源可追踪、可刷新、可失效。
- 缺失或冲突信息必须显式暴露，不能用默认假设掩盖。
- adapter 只负责读取、同步和投影外部系统语义，不改变 BitFun canonical model。
- 项目画像必须支持多语言、多仓库、多 CI、多发布模式。
- 主动配置必须记录来源、hash、权限声明、启用范围和信任状态；未信任前不得影响执行或 gate。
- P0 只做本地项目和 PR 证据链所需画像，不建设企业级集成平台。

## 3. 范围与非目标

范围：

- 发现目标项目结构、语言、框架、模块、owner、规则来源和验证能力。
- 识别未知区域、规则冲突、过期规则和不可访问外部系统。
- 为 Quality Data Plane、Risk Classifier、Artifact Graph、PR Gate 和 Evaluation 提供项目画像。
- 提供代码托管、issue、文档、CI、发布和观测系统的 adapter 边界。

非目标：

- 不替代目标项目的配置管理、需求管理、CI 或发布系统。
- 不把任何单一项目结构当作默认模板。
- 不要求目标项目先改造成 BitFun 推荐结构。
- 不在 P0 做完整组织知识图谱或企业权限系统。

## 4. 输入、输出与数据模型

输入：

| 输入 | 示例 |
|---|---|
| Repository facts | 文件树、依赖文件、构建配置、测试目录、生成文件 |
| Rule sources | README、CONTRIBUTING、agent rules、CODEOWNERS、module docs、team policy |
| Verification sources | package scripts、task runner、CI workflow、test reports、lint/typecheck/build commands |
| Active config sources | hooks、plugins、custom tools、MCP servers、agent rules、automation config |
| Ownership sources | CODEOWNERS、git history、issue assignee、team mapping |
| External integrations | GitHub/GitLab、Jira/Linear、Confluence/Notion、CI、artifact registry、observability |
| User confirmation | 手动确认模块边界、owner、验证命令、敏感区域和不支持状态 |

输出：

```ts
interface ProjectProfile {
  project_id: string;
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
| `confirmed` | 来源明确且已被用户或确定性证据确认 | 可作为 gate、risk、graph 的强依据 |
| `inferred` | 由文件、配置、历史或静态分析推断 | 可作为候选依据，需要展示置信度 |
| `unknown` | 缺少足够信息 | 下游必须 degraded 或要求人工确认 |
| `conflicting` | 多个规则来源冲突 | 下游不得自动选择高风险路径 |
| `stale` | 来源已变更或超过刷新窗口 | 需要刷新或重新确认 |

## 5. 核心流程

```text
open target project
  -> local structure discovery
  -> rule and verification source discovery
  -> external adapter probing
  -> profile normalization
  -> conflict and unknown detection
  -> active config discovery
  -> user confirmation for critical gaps
  -> emit project.profiled event
  -> refresh / invalidate on project changes
```

画像生成优先级：

| 来源 | 优先级 | 说明 |
|---|---:|---|
| 用户确认 | 1 | 高风险规则、owner、发布边界以用户确认为准 |
| 确定性配置 | 2 | CI、build、package、CODEOWNERS、typed config |
| 项目文档 | 3 | README、贡献指南、agent rules、模块文档 |
| 历史信号 | 4 | co-change、incident、review blocker、hot files |
| 模型推断 | 5 | 只能生成候选，不作为事实 |

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

Adapter 只输出 normalized facts 和 evidence references，不直接写 gate 结论。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | 本地项目画像、规则来源、验证能力、主动配置发现、未知/冲突标记、`project.profiled` 事件 |
| P1 | GitHub/GitLab PR、issue、CI adapter；用户确认、active config trust review 持久化和 profile refresh |
| P2 | 文档、发布、observability adapter；多仓库和多 workspace 支持 |
| P3 | 团队级策略包、profile drift dashboard、跨项目画像对比和治理指标 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 画像误判导致错误门禁 | 未确认画像只能作为候选；高风险 gate 必须引用 confirmed 或 deterministic evidence |
| 对 BitFun 自身验证样本过拟合 | 默认 profile 不能内置 BitFun 路径、语言或验证命令 |
| onboarding 过重 | P0 必须在几分钟内生成可用的最小画像，并允许后续增量确认 |
| 外部系统耦合 | adapter 输出 canonical facts，不让外部 payload 泄漏到核心策略 |
| 敏感信息泄露 | profile 写入前执行 redaction，secret 和私有日志只存引用或摘要 |
| 主动配置被误认为可信规则 | hook/plugin/custom tool 只作为 discovered fact，必须 trust review 后才可执行 |
| 画像过期 | 文件、CI、规则或集成状态变化必须触发 freshness 更新 |
| 用户不信任推断 | UI 必须展示来源、置信度、冲突和确认状态 |

## 9. 成功标准

- 新目标项目加载后可生成最小 Project Profile。
- Risk Classifier 和 PR Gate 能解释所用项目规则和验证能力来自哪里。
- 未知或冲突规则会导致 `degraded` 或人工确认，而不是默认通过。
- 外部 adapter 接入不会改变 BitFun canonical event、artifact、permission 和 policy model 的一致性。
- 用户可以修正项目画像，修正结果会影响后续 gate、risk 和 graph 判断。
- 主动配置能被发现、展示、确认、禁用和重新确认，且默认不自动执行。

# BitFun 生命周期工程子模块设计：Quality Data Plane

> 上游文档：[design.md](../design.md)
> 模块角色：为 BitFun 加载的目标项目提供统一事件、证据、指标和审计数据模型。

## 1. 模块定位

Quality Data Plane 是 BitFun 面向外部目标项目建设生命周期工程能力的基础数据层。它不负责业务决策，而负责把项目画像、会话、工具调用、文件变更、验证命令、Deep Review、PR gate、CI、发布和运行期反馈统一成可追踪、可审计、可回放的数据事实。

首个落地目标必须收敛在目标项目的 PR 证据链：Project Profile、本地 diff、必跑验证、风险分类、轻量 gate、PR 描述和后续审计。P0 不采集完整 SDLC 事件。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/concepts/semantic-conventions/) | traces、metrics、logs、resources 需要统一语义，避免观测数据孤岛 |
| [CDEvents](https://cdevents.dev/) | CI/CD 事件需要可互操作的事件模型 |
| [SLSA provenance](https://slsa.dev/spec/v1.0/) / [in-toto attestations](https://in-toto.io/) | 构建、验证和 artifact metadata 应具备 provenance 与 attestation |
| [GitHub Advanced Security with AI coding agents](https://docs.github.com/en/code-security/how-tos/use-ghas-with-ai-coding-agents) | AI coding agent 需要与 secret、dependency、vulnerability 信号联动 |
| [Harness Software Delivery Knowledge Graph](https://www.harness.io/blog/knowledge-graphs-for-ai-software-delivery) | 交付上下文的价值来自新鲜度、权限、策略和结果改善，而不是事件数量 |
| [NIST SP 800-218A](https://csrc.nist.gov/pubs/sp/800/218/a/final) | AI 进入 SDLC 后，模型、数据、工具、权限和供应链都属于安全开发边界 |

设计约束：

- P0 只采集 Project Profile 和 PR EvidencePack 所需事件。
- 每类事件必须定义 retention、privacy、redaction、payload size 和导出策略。
- EvidencePack 只保存摘要和引用，不长期保存无界原始日志。
- 内部事件模型保持 canonical；OpenTelemetry、CDEvents、SLSA 等是导出适配，不是内部事实来源。
- 事件字段需要稳定语义命名，避免子模块各自定义不可对齐的事实。
- 证据必须区分 trust tier：确定性事实、外部系统事实、人工确认、模型推断和插件建议不能混为同一等级。
- 事件 schema 需要 registry 和 owner；新增事件域必须声明 producer、consumer、retention、privacy、migration 和导出策略。

## 3. 范围与非目标

范围：

- 定义 `LifecycleEvent` envelope。
- 统一 session、tool、file、verification、review、gate、cost、security 的事件域。
- 为 EvidencePack、Artifact Graph、Risk Classifier、PR Gate 和 Agent Evaluation 提供事实输入；EvidencePack 的投影 contract 由 [evidence-pack.md](evidence-pack.md) 维护。
- 支持本地 append-only audit、投影查询和外部导出。

非目标：

- 不建设通用日志平台。
- 不采集所有终端输出和模型上下文。
- 不替代 CI、APM、SIEM 或数据仓库。
- 不把模型摘要作为原始事实。

## 4. 输入、输出与数据模型

核心输入：

| 输入 | 来源 |
|---|---|
| 项目画像事件 | project structure、rules、owner、verification profile、release model |
| 会话事件 | Agent Runtime、mode、tool call、approval |
| 文件事件 | diff、rename、delete、generated file、watcher |
| 验证事件 | command、exit code、duration、log summary、artifact ref |
| Review 事件 | Deep Review manifest、finding、judge、remediation |
| Gate 事件 | required checks、risk tags、pass/warn/fail/degraded |
| 成本事件 | token、model、wall-clock、tool time |
| 安全事件 | permission、secret exposure、plugin execution、policy deny |
| 主动配置事件 | hook/plugin/custom tool 信任状态、hash、权限声明、启用范围 |

核心输出：

- EvidencePack source events 和 evidence refs。
- Artifact Graph edge evidence。
- Risk Classifier calibration features。
- PR Gate status 和 degraded reasons。
- Agent Evaluation replay traces。
- 审计导出和最小指标集。

事件 envelope：

```ts
interface LifecycleEvent {
  id: string;
  type: string;
  version: number;
  timestamp: string;
  source: EventSource;
  actor: EventActor;
  scope: EventScope;
  correlation: EventCorrelation;
  payload: unknown;
  evidence?: EvidenceReference[];
  risk?: RiskSnapshot;
  privacy: PrivacyClass;
  retention: RetentionPolicy;
}
```

证据信任等级：

| 等级 | 来源 | 是否可作为阻塞性 gate 依据 |
|---|---|---|
| `deterministic` | 本地命令、测试、CI check、签名制品、静态配置 | 可以 |
| `external_system` | GitHub、Jira、CI、observability adapter 返回的已认证事实 | 可以，但需记录 adapter 和刷新时间 |
| `human_confirmed` | 用户确认、reviewer 决策、risk acceptance | 可以，但必须记录 actor 和 reason |
| `model_inferred` | LLM 摘要、候选影响面、候选风险标签 | 不可以，只能作为候选或说明 |
| `plugin_suggested` | 第三方 hook/plugin 产生的建议 | 不可以，必须经过 BitFun policy 或人工确认 |

## 5. 核心流程

```text
Project/Agent/Tool/CI/Review event
  -> normalize LifecycleEvent
  -> redact and classify privacy
  -> append local audit log
  -> update projection stores
  -> expose EvidencePack / Gate / Eval queries
  -> optional export adapter
```

P0 最小事件集：

| 事件 | 用途 |
|---|---|
| `project.profiled` | 关联项目结构、规则来源和验证能力 |
| `session.started` / `session.completed` | 关联一次工作流 |
| `file.changed` | 更新 diff summary 和风险候选 |
| `tool.completed` | 采集验证命令和工具输出摘要 |
| `verification.completed` | 形成 required check evidence |
| `evidence_pack.generated` | 固化 EvidencePack id、版本、状态和 source event 集合 |
| `risk.classified` | 固化风险标签、原因和置信度 |
| `gate.completed` | 固化 PR Gate 结果 |
| `review.completed` | 关联可选 Deep Review 证据 |
| `active_config.discovered` | 固化 hook/plugin/custom tool/MCP/agent rules 的来源、hash、权限声明和未信任状态 |

## 6. 策略与治理

- **本地优先**：默认写入本地 append-only log，外部导出需显式配置。
- **事件预算**：每类事件设置 payload size、采样和保留周期。
- **隐私分级**：区分 public、project、sensitive、secret；secret 不进入长期事件。
- **证据引用**：大日志、报告、截图、trace 使用 `EvidenceReference` 引用，不内嵌。
- **语义稳定**：核心字段采用稳定命名和版本，不把 UI 文案或外部 payload 直接写入事件模型。
- **Schema registry**：每个事件 type 需要 owner、producer、consumer、version、migration 和删除策略。
- **信任分层**：Gate 和 release readiness 必须能区分事实、候选、建议和人工接受。
- **可重放性**：关键事件版本化，schema 变更提供 migration 或兼容读取。
- **导出隔离**：导出到 GitHub、OpenTelemetry、CDEvents 或云端时保留 redaction 和权限策略。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | Project Profile 事件、PR EvidencePack 事件、验证命令摘要、Gate 结果、本地审计 |
| P1 | Artifact Graph 投影、Deep Review finding lifecycle、`active_config.reviewed`、成本指标 |
| P2 | CI/发布/incident 事件接入、外部标准导出 |
| P3 | Evaluation replay、跨团队质量指标、策略回放分析 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| telemetry bloat | P0 事件集必须能支撑 PR gate，同时事件量可解释、可裁剪 |
| 原始日志泄露 | EvidencePack 不应包含长期原始日志；敏感片段必须 redaction |
| 事件不可治理 | 每个事件域必须定义 owner、retention、schema version 和导出策略 |
| schema 漂移 | 新事件或字段必须先进入 registry，并提供兼容读取或迁移策略 |
| 模型摘要覆盖事实层 | 模型输出只能作为 derived evidence，不能覆盖原始 command/CI/review 事实 |
| 跨模块耦合过重 | 上游模块只能依赖查询接口和 event schema，不能直接读取内部存储 |
| 审计不可复现 | Gate、review、risk 等关键结论必须能追溯到 event id 和 evidence ref |

## 9. 成功标准

- 低风险 PR 可生成完整 EvidencePack，不依赖完整图谱。
- Gate 结果可追溯到验证事件、风险事件和人工 override。
- Deep Review token、耗时、scope、skipped context 可被统一记录。
- 事件模型能够导出到至少一种外部标准或平台。
- 敏感信息扫描和 redaction 在事件写入前执行。
- hook/plugin/custom tool 的信任状态可通过事件追溯到来源、hash、权限和审核人。

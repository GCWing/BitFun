# BitFun 子模块设计：Quality Data Plane

> 上游文档：[design.md](../design.md)
> 模块角色：为 BitFun 加载的目标项目提供统一事件、证据、指标和审计数据模型，用于解释、恢复、回放和校准产品控制策略。

## 1. 模块定位

Quality Data Plane 是事实数据层，不是质量门禁层。它负责把项目画像、任务、工具调用、文件变更、验证命令、控制决策、安全授权、review、CI、发布和运行期反馈整理成可追踪、可裁剪、可回放的数据事实。

新的 P0 不再以 PR Gate 为中心。P0 事件集必须优先支撑三件事：

1. 普通任务结束时能解释做了什么、验证了什么、没验证什么。
2. 安全敏感动作能追溯 allow/ask/deny/break-glass 的原因和范围。
3. Adaptive Control 能用真实数据校准是否过度打断、误升级或漏提示。

EvidencePack、Artifact Graph、Risk Classifier、Change Readiness、PR Gate 和 Agent Evaluation 都消费同一事实层，但不能直接读取内部存储或各自重新定义事实字段。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/concepts/semantic-conventions/) | traces、metrics、logs、resources 需要统一语义，避免观测数据孤岛 |
| [CDEvents](https://cdevents.dev/) | CI/CD 事件需要可互操作的事件模型 |
| [SLSA provenance](https://slsa.dev/spec/v1.0/) / [in-toto attestations](https://in-toto.io/) | 构建、验证和 artifact metadata 应具备 provenance 与 attestation |
| [Codex approvals/security](https://developers.openai.com/codex/agent-approvals-security) | approval、安全授权和工具执行要能审计，但不能把安全 prompt 做成质量 gate |
| [NIST SP 800-218A](https://csrc.nist.gov/pubs/sp/800/218/a/final) | AI 进入 SDLC 后，模型、数据、工具、权限和供应链都属于安全开发边界 |

设计约束：

- P0 只采集 Fast Path、Security Boundary 和 confidence summary 所需事件。
- 每类事件必须定义 retention、privacy、redaction、payload size 和导出策略。
- 事件字段需要稳定语义命名，避免子模块各自定义不可对齐的事实。
- EvidencePack 只保存摘要和引用，不长期保存无界原始日志。
- 内部事件模型保持 canonical；OpenTelemetry、CDEvents、SLSA 等是导出适配，不是内部事实来源。
- 证据必须区分 trust tier：确定性事实、外部系统事实、人工确认、模型推断和插件建议不能混为同一等级。
- 新事件域必须声明 producer、consumer、retention、privacy、migration 和导出策略。

## 3. 范围与非目标

范围：

- 定义 `LifecycleEvent` envelope。
- 统一 task、session、control、security、tool、file、verification、review、gate、cost、active config 的事件域。
- 为 EvidencePack、Artifact Graph、Risk Classifier、Change Readiness、PR Gate 和 Agent Evaluation 提供事实输入。
- 支持本地 append-only audit、投影查询和外部导出。

非目标：

- 不建设通用日志平台。
- 不采集所有终端输出和模型上下文。
- 不替代 CI、APM、SIEM 或数据仓库。
- 不把模型摘要作为原始事实。
- 不用事件数量证明产品质量。

## 4. 输入、输出与数据模型

核心输入：

| 输入 | 来源 |
|---|---|
| 项目画像事件 | project structure、rules、owner、verification profile、release model |
| 任务事件 | user intent、profile、task completion、confidence summary |
| 控制事件 | Adaptive Control decision、reason、profile transition |
| 安全事件 | permission、sandbox、network、secret、prompt injection、break-glass |
| 工具事件 | command、tool call、approval、exit code、duration |
| 文件事件 | diff、rename、delete、generated file、watcher |
| 验证事件 | command、exit code、duration、log summary、artifact ref |
| Review/Gate 事件 | Deep Review、finding、readiness、gate projection |
| 成本事件 | token、model、wall-clock、tool time |
| 主动配置事件 | hook/plugin/custom tool 信任状态、hash、权限声明、启用范围 |

核心输出：

- Fast task confidence summary。
- Security Boundary audit refs。
- EvidencePack source events 和 evidence refs。
- Risk Classifier calibration features。
- Change Readiness 和 PR Gate status。
- Artifact Graph edge evidence。
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

| 等级 | 来源 | 是否可作为 required/blocking 依据 |
|---|---|---|
| `deterministic` | 本地命令、测试、CI check、签名制品、静态配置 | 可以 |
| `external_system` | GitHub、Jira、CI、observability adapter 返回的已认证事实 | 可以，但需记录 adapter 和刷新时间 |
| `human_confirmed` | 用户确认、reviewer 决策、risk acceptance | 可以，但必须记录 actor 和 reason |
| `model_inferred` | LLM 摘要、候选影响面、候选风险标签 | 不可以，只能作为候选或说明 |
| `plugin_suggested` | 第三方 hook/plugin 产生的建议 | 不可以，必须经过 BitFun policy 或人工确认 |

## 5. P0 最小事件集

| 事件 | 用途 |
|---|---|
| `project.profiled.light` | 关联轻量项目结构、规则入口和验证候选 |
| `task.started` / `task.completed` | 关联一次用户任务与结果摘要 |
| `control.decided` | 固化 profile、触发原因、recommended/required actions |
| `security.decided` | 固化 allow/ask/deny/break-glass、范围、原因和残余风险 |
| `user.override.recorded` | 记录跳过、风险接受或临时放行 |
| `active_config.discovered` | 固化 hook/plugin/custom tool/MCP/agent rules 的来源、hash、权限声明和未信任状态 |
| `file.changed` | 更新 diff summary 和风险候选 |
| `tool.completed` | 采集工具和命令输出摘要 |
| `verification.completed` | 形成验证 evidence |
| `confidence.summary.generated` | 固化任务结束的用户可见信心摘要 |
| `evidence_pack.generated` | 在需要时固化 summary/evidence refs/full pack |

P1/P2 再引入：

| 事件 | 触发 |
|---|---|
| `risk.control_hinted` | Contextual Assurance |
| `readiness.generated` | PR 或 review 场景 |
| `gate.projected` | team/guarded/regulated 策略 |
| `review.completed` | targeted/full review |
| `artifact.edge.updated` | 复杂项目图谱 |

## 6. 核心流程

```text
Project/Task/Agent/Tool/Security event
  -> normalize LifecycleEvent
  -> redact and classify privacy
  -> append local audit log
  -> update projection stores
  -> expose summary / evidence / readiness / eval queries
  -> optional export adapter
```

治理规则：

- **本地优先**：默认写入本地 append-only log，外部导出需显式配置。
- **事件预算**：每类事件设置 payload size、采样和保留周期。
- **隐私分级**：区分 public、project、sensitive、secret；secret 不进入长期事件。
- **证据引用**：大日志、报告、截图、trace 使用 `EvidenceReference` 引用，不内嵌。
- **语义稳定**：核心字段采用稳定命名和版本，不把 UI 文案或外部 payload 直接写入事件模型。
- **信任分层**：readiness、Gate 和 release readiness 必须能区分事实、候选、建议和人工接受。
- **可重放性**：关键事件版本化，schema 变更提供 migration 或兼容读取。
- **导出隔离**：导出到 GitHub、OpenTelemetry、CDEvents 或云端时保留 redaction 和权限策略。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | 轻量项目、任务、控制、安全、验证和 confidence summary 事件 |
| P1 | risk hint、readiness、targeted review、成本和提示体验指标 |
| P2 | team policy、Gate projection、active config trust review、外部 CI/PR 事件 |
| P3 | Artifact Graph、release、incident、observability 事件接入 |
| P4 | Evaluation replay、跨团队指标、策略回放分析 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| telemetry bloat | P0 事件集必须能解释 Fast Path 和安全决策，不能追求全量 SDLC |
| 原始日志泄露 | EvidencePack 不应包含长期原始日志；敏感片段必须 redaction |
| 事件不可治理 | 每个事件域必须定义 owner、retention、schema version 和导出策略 |
| schema 漂移 | 新事件或字段必须先进入 registry，并提供兼容读取或迁移策略 |
| 模型摘要覆盖事实层 | 模型输出只能作为 derived evidence，不能覆盖原始 command/CI/review 事实 |
| 跨模块耦合过重 | 上游模块只能依赖查询接口和 event schema，不能直接读取内部存储 |
| 审计不可复现 | control、security、readiness、gate、review、risk 等关键结论必须能追溯到 event id 和 evidence ref |

## 9. 成功标准

- 普通任务可以生成可解释 confidence summary。
- 安全提示、拒绝、break-glass 都能追溯到事件和范围。
- EvidencePack、Change Readiness 和 Gate 复用同一事实层。
- Deep Review token、耗时、scope、skipped context 可被统一记录。
- 事件模型能够导出到至少一种外部标准或平台。
- hook/plugin/custom tool 的信任状态可通过事件追溯到来源、hash、权限和审核人。

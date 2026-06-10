# BitFun 生命周期工程子模块设计：EvidencePack

> 上游文档：[design.md](../design.md)
> 模块角色：把目标项目一次变更的上下文、验证、风险、跳过项、人工确认和审计引用组织成可投影到 PR、Gate、Review 和后续回放的证据包。

## 1. 模块定位

EvidencePack 是质量事实能力和质量保护能力之间的稳定投影。Quality Data Plane 记录事件和证据引用，EvidencePack 负责把这些事实整理成一次变更可消费、可审计、可失效的证据快照。

它不是原始日志仓库，也不是 Gate 决策本身。Gate 只能消费 EvidencePack 和风险策略做判断；EvidencePack 只能陈述有哪些证据、证据来自哪里、是否过期、哪些检查被跳过，以及哪些风险需要人工接受。

外部系统的成熟实践说明了这个边界：[GitHub Checks](https://docs.github.com/rest/checks) 把检查结论、摘要和详情投影到 commit/PR；[SLSA provenance](https://slsa.dev/provenance) 关注 artifact 的 where/when/how；[OpenTelemetry semantic conventions](https://opentelemetry.io/docs/specs/semconv/) 关注跨系统数据语义稳定。EvidencePack 应吸收这些思想，但保持 BitFun 内部 canonical evidence model。

## 2. 设计约束

- EvidencePack 由 Artifact and Evidence Plane 负责投影和版本化。
- 原始事实来自 Quality Data Plane 的 `LifecycleEvent` 和 `EvidenceReference`。
- EvidencePack 不长期保存完整终端日志、prompt、模型上下文或第三方 payload。
- 每个 EvidencePack 必须绑定 target project、changeset、profile version、policy version 和生成时间。
- EvidencePack 必须能表达 `fresh`、`partial`、`stale`、`blocked` 和 `superseded`。
- 缺少证据、证据过期或主动配置未确认时，不得把 EvidencePack 标为 complete。
- PR 文本、GitHub Check、Review UI 和 Evaluation replay 都应消费同一 EvidencePack contract。

## 3. 范围与非目标

范围：

- 定义 EvidencePack 的 owner、生命周期、状态和最小字段。
- 汇总 context、change、verification、risk、review、active config、skip reason 和 open risk。
- 支撑 PR Quality Gate、Risk Classifier、Artifact Graph 和 Agent Evaluation。

非目标：

- 不执行验证命令。
- 不决定 PR 是否 pass。
- 不替代 CI、GitHub Checks、release attestation 或外部审计系统。
- 不声明需求影响分析完整。
- 不把模型摘要当作确定性证据。

## 4. 输入、输出与数据模型

输入：

| 输入 | 来源 |
|---|---|
| Project Profile snapshot | 项目结构、规则、验证能力、owner、主动配置状态 |
| Changeset summary | Git diff、file change、rename/delete、generated file |
| Verification evidence | `verification.completed`、CI check、命令摘要、artifact ref |
| Risk classification | 风险标签、required checks、review profile、policy version |
| Review evidence | Deep Review finding、human review、stale marker |
| Active config evidence | hook/plugin/custom tool/MCP/agent rules 的发现、hash、权限和 trust state |
| Human decision | override、risk acceptance、confirmation、rejection |

输出：

```ts
type EvidencePackStatus =
  | "fresh"
  | "partial"
  | "stale"
  | "blocked"
  | "superseded";

interface EvidencePack {
  id: string;
  version: number;
  project_id: string;
  changeset_id: string;
  profile_version: string;
  policy_version: string;
  generated_at: string;
  status: EvidencePackStatus;
  context: ContextEvidence[];
  change: ChangeEvidence;
  verification: VerificationEvidence[];
  risk: RiskEvidence;
  review: ReviewEvidence[];
  active_config: ActiveConfigEvidence[];
  skipped_checks: SkippedCheck[];
  open_risks: OpenRisk[];
  risk_acceptances: RiskAcceptance[];
  source_events: string[];
  evidence_refs: EvidenceReference[];
}
```

关键字段语义：

| 字段 | 语义 |
|---|---|
| `source_events` | 生成该包使用的 event id 集合 |
| `evidence_refs` | 指向日志摘要、报告、CI、截图、trace 或外部系统事实的引用 |
| `skipped_checks` | 未运行检查的原因、触发规则、可接受条件和残余风险 |
| `open_risks` | 尚未被证据覆盖或人工接受的风险 |
| `risk_acceptances` | 人工接受记录，包含 actor、reason、scope、expires_at、residual_risk |

## 5. 生命周期

```text
source events
  -> build EvidencePack draft
  -> attach profile and policy versions
  -> classify freshness and completeness
  -> expose to Gate / PR text / Review UI
  -> mark stale when changeset, profile, policy, verification, review, or active config changes
  -> supersede with a new EvidencePack version
```

状态规则：

| 状态 | 触发条件 | 下游行为 |
|---|---|---|
| `fresh` | required evidence 完整且 source versions 未变化 | Gate 可用于 pass/warn/fail 判断 |
| `partial` | 有缺失推荐证据、非阻塞 skipped check 或低风险 unknown | Gate 最多 `warn`，PR 文本必须展示缺口 |
| `stale` | diff、Project Profile、policy、required checks、review scope 或 active config 变化 | Gate 不得继续保持 `pass` |
| `blocked` | 必要验证失败、主动配置未确认且高权限、或证据不可访问 | Gate 应 `fail` 或 `degraded` |
| `superseded` | 新版本 EvidencePack 取代旧版本 | 旧包保留审计，不作为当前判断依据 |

## 6. 与其他模块的边界

| 模块 | 关系 |
|---|---|
| Quality Data Plane | 提供事实事件、信任等级、隐私分类和 evidence refs |
| Project Profile | 提供 project/profile/rule/active config snapshot |
| Risk Classifier | 消费 EvidencePack 的 context/change/verification，输出 risk evidence |
| PR Quality Gate | 消费 EvidencePack，产出 gate decision，不修改原始证据 |
| Artifact Graph | 可把 EvidencePack 作为 artifact node，并把 evidence refs 挂到 graph edge |
| Agent Evaluation | 使用 EvidencePack 和 source events 做 replay 与失败归因 |

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P-1 | 定义 EvidenceReference、EvidencePack schema、status、staleness 和 risk acceptance contract |
| P0 | 为本地 diff 生成 EvidencePack v0，覆盖 profile、change、verification、risk、skipped checks、open risks |
| P1 | 接入 PR 投影、stale evidence、finding lifecycle 和 active config trust review |
| P2 | 接入 requirement impact、release readiness、incident backtrace 和外部 attestation 引用 |
| P3 | 支撑 trace replay、质量趋势和跨项目 evidence coverage 分析 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| EvidencePack 变成日志包 | 只保存摘要和引用，完整日志通过受控 EvidenceReference 访问 |
| Gate 与 EvidencePack 状态不一致 | Gate result 必须引用 `evidence_pack_id` 和 `policy_version` |
| 人工接受掩盖证据缺失 | risk acceptance 不能把 missing evidence 改写成 pass，只能记录接受范围和残余风险 |
| 证据过期不可见 | changeset/profile/policy/check/review/active config 变化必须标记 stale |
| 模块重复定义字段 | EvidencePack schema 是唯一 PR 证据投影 contract，其他模块只能扩展引用或消费 |

## 9. 成功标准

- 低风险本地 diff 可生成 `fresh` 或 `partial` EvidencePack。
- PR Gate 可通过 `evidence_pack_id` 追溯所有关键证据。
- PR 文本和 Review UI 使用同一个 EvidencePack 投影。
- skipped checks、open risks、risk acceptances 不会被隐藏。
- 证据过期后，旧 EvidencePack 不再支撑 `pass`。

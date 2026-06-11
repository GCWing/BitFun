# BitFun 子模块设计：EvidencePack

> 上游文档：[design.md](../design.md)
> 模块角色：把一次任务或变更的上下文、验证、风险、跳过项、人工决策和安全授权整理成可投影、可失效、可回放的证据快照。

## 1. 模块定位

EvidencePack 是后台证据投影 contract，不是默认用户界面，也不是 Gate 决策。Fast Path 下用户通常只看到 confidence summary；只有准备 PR、团队策略启用、风险升级、release/incident 追溯或 evaluation replay 时，才需要展示 evidence refs 或 full pack。

Quality Data Plane 记录事件和引用，EvidencePack 负责把这些事实整理成一次任务或 changeset 可消费、可审计、可失效的快照。它只能陈述证据是什么、来自哪里、是否过期、哪些检查被跳过、哪些风险被接受；不能自行判断代码是否可合入。

外部系统的成熟实践说明了这个边界：[GitHub Checks](https://docs.github.com/rest/checks) 把检查结论和摘要投影到 commit/PR；[SLSA provenance](https://slsa.dev/provenance) 关注 artifact 的 where/when/how；[OpenTelemetry semantic conventions](https://opentelemetry.io/docs/specs/semconv/) 关注跨系统语义稳定。EvidencePack 应吸收这些思想，但保持 BitFun 内部 canonical evidence model。

## 2. 设计约束

- EvidencePack 由 Artifact and Evidence Plane 负责投影和版本化。
- 原始事实来自 Quality Data Plane 的 `LifecycleEvent` 和 `EvidenceReference`。
- EvidencePack 不长期保存完整终端日志、prompt、模型上下文或第三方 payload。
- EvidencePack 必须能表达 `fresh`、`partial`、`stale`、`blocked` 和 `superseded`。
- EvidencePack 必须支持 display tier，避免 full pack 默认污染 Fast Path。
- 缺少证据、证据过期或主动配置未确认时，不得把 EvidencePack 标为 complete。
- PR 文本、Review UI、Gate、release readiness 和 Evaluation replay 都应消费同一 EvidencePack contract。

## 3. Evidence Display Tiers

| Tier | 用户可见内容 | 适用场景 |
|---|---|---|
| `none` | 不展示证据结构，只保留后台事件 | Fast Path 中间过程 |
| `summary` | 已做什么、未做什么、信心和下一步 | Fast/assist 任务结束 |
| `evidence_refs` | 摘要加命令、CI、文件、review、security decision 引用 | PR readiness、review、team advisory |
| `full_pack` | 完整证据包、策略版本、风险接受、staleness、audit refs | guarded/regulated、release、incident、eval |

展示层由 [Adaptive Control Profile](../features/adaptive-control-profile.md) 决定。证据是否存在和是否展示是两个独立问题：后台可以生成最小证据摘要，但 Fast Path 不应把它包装成治理流程。

## 4. 输入、输出与数据模型

输入：

| 输入 | 来源 |
|---|---|
| Project Profile snapshot | 项目结构、规则、验证能力、owner、主动配置状态 |
| Task and changeset summary | 用户意图、Git diff、file change、rename/delete、generated file |
| Verification evidence | `verification.completed`、CI check、命令摘要、artifact ref |
| Risk Control Hint | 风险标签、recommended/required checks、review profile |
| Security decision | allow/ask/deny/break-glass、授权范围、残余风险 |
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

type EvidenceDisplayTier =
  | "none"
  | "summary"
  | "evidence_refs"
  | "full_pack";

interface EvidencePack {
  id: string;
  version: number;
  project_id: string;
  task_id: string;
  changeset_id?: string;
  profile_version: string;
  policy_version: string;
  generated_at: string;
  status: EvidencePackStatus;
  display_tier: EvidenceDisplayTier;
  context: ContextEvidence[];
  change?: ChangeEvidence;
  verification: VerificationEvidence[];
  risk: RiskEvidence[];
  security: SecurityEvidence[];
  review: ReviewEvidence[];
  active_config: ActiveConfigEvidence[];
  skipped_checks: SkippedCheck[];
  open_risks: OpenRisk[];
  risk_acceptances: RiskAcceptance[];
  break_glass_decisions: BreakGlassDecision[];
  source_events: string[];
  evidence_refs: EvidenceReference[];
}
```

关键字段语义：

| 字段 | 语义 |
|---|---|
| `source_events` | 生成该包使用的 event id 集合 |
| `evidence_refs` | 指向日志摘要、报告、CI、截图、trace 或外部系统事实的引用 |
| `security` | 执行安全决策摘要，不作为质量 pass 依据 |
| `skipped_checks` | 未运行检查的原因、触发规则、可接受条件和残余风险 |
| `open_risks` | 尚未被证据覆盖或人工接受的风险 |
| `risk_acceptances` | 质量风险接受记录 |
| `break_glass_decisions` | 安全边界临时放行记录，必须与质量风险接受分开 |

## 5. 生命周期

```text
source events
  -> build evidence summary
  -> attach profile and policy versions
  -> classify freshness and completeness
  -> choose display tier
  -> expose summary / refs / full pack
  -> mark stale when changeset, profile, policy, verification, review, or active config changes
  -> supersede with a new EvidencePack version
```

状态规则：

| 状态 | 触发条件 | 下游行为 |
|---|---|---|
| `fresh` | 当前 tier 所需证据完整且 source versions 未变化 | 可支撑 readiness 或 Gate 判断 |
| `partial` | 推荐证据缺失、非阻塞 skipped check 或低风险 unknown | summary/advisory 展示缺口 |
| `stale` | diff、Project Profile、policy、required checks、review scope 或 active config 变化 | 不得继续支撑 pass/ready |
| `blocked` | 必要验证失败、安全拒绝、高权限主动配置未确认或证据不可访问 | 下游应 blocked/fail/degraded |
| `superseded` | 新版本 EvidencePack 取代旧版本 | 旧包保留审计，不作为当前判断依据 |

## 6. 与其他模块的边界

| 模块 | 关系 |
|---|---|
| Quality Data Plane | 提供事实事件、信任等级、隐私分类和 evidence refs |
| Adaptive Control Profile | 决定 evidence display tier 和是否进入 PR/Team/Regulated 投影 |
| Security Boundary | 产生安全决策，EvidencePack 只记录摘要和授权引用 |
| Project Profile | 提供 project/profile/rule/active config snapshot |
| Risk Classifier | 消费 context/change/verification，输出 risk evidence |
| Change Readiness / PR Gate | 消费 EvidencePack，产出 readiness 或 gate decision |
| Artifact Graph | 可把 EvidencePack 作为 artifact node，并把 evidence refs 挂到 graph edge |
| Agent Evaluation | 使用 EvidencePack 和 source events 做 replay 与失败归因 |

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P-1 | 定义 EvidenceReference、EvidencePack schema、status、display tier、staleness 和 risk acceptance contract |
| P0 | 为 Fast Path 生成 summary tier，记录验证、安全决策和 skipped checks |
| P1 | 支撑 PR readiness 的 evidence refs、stale evidence 和 targeted review evidence |
| P2 | 支撑 team/guarded 的 PR Gate projection、risk acceptance 和 active config trust review |
| P3 | 接入 requirement impact、release readiness、incident backtrace 和外部 attestation 引用 |
| P4 | 支撑 trace replay、控制策略评估和跨项目 evidence coverage 分析 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| EvidencePack 变成用户必须理解的流程 | display tier 默认 summary 或 none，full_pack 只在强场景显露 |
| EvidencePack 变成日志包 | 只保存摘要和引用，完整日志通过受控 EvidenceReference 访问 |
| 安全放行和质量接受混淆 | break-glass 与 risk acceptance 分开字段、分开 UI |
| Gate 与 EvidencePack 状态不一致 | Gate result 必须引用 `evidence_pack_id` 和 `policy_version` |
| 人工接受掩盖证据缺失 | risk acceptance 不能把 missing evidence 改写成 pass |
| 证据过期不可见 | changeset/profile/policy/check/review/active config 变化必须标记 stale |
| 模块重复定义字段 | EvidencePack schema 是唯一证据投影 contract，其他模块只能扩展引用或消费 |

## 9. 成功标准

- Fast Path 能给出简洁 confidence summary，不暴露完整证据包。
- PR readiness 可通过 evidence refs 追溯关键证据。
- skipped checks、open risks、risk acceptances 和 break-glass 不会被隐藏。
- 证据过期后，旧 EvidencePack 不再支撑 pass/ready。
- Full EvidencePack 只在团队、发布、审计、复盘或评测场景显性使用。

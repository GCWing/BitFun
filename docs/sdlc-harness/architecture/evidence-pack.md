# BitFun 子模块设计：证据包

> 上游文档：[design.md](../design.md)
> 模块角色：把一次任务或变更的上下文、验证、风险、跳过项、人工决策和安全授权整理成可呈现、可失效、可回放的证据快照。

## 1. 模块定位

证据包是后台证据视图和 schema。快速路径下用户看到信心摘要；准备 PR、团队策略启用、风险升级、发布/事故追溯或评测回放时，系统按配置展示证据引用或完整证据包。

质量数据面记录事件和引用，证据包负责把这些事实整理成一次任务或变更集可消费、可审计、可失效的快照。证据包陈述证据内容、来源、新鲜度、跳过检查、风险接受和安全授权；合入判断由变更就绪度、团队策略、CI、分支保护和人工审查共同决定。

外部系统的成熟实践说明了这个边界：[GitHub Checks](https://docs.github.com/rest/checks) 把检查结论和摘要呈现到提交（commit）或 PR；[SLSA provenance](https://slsa.dev/provenance) 关注制品的来源、时间和生成方式；[OpenTelemetry semantic conventions](https://opentelemetry.io/docs/specs/semconv/) 关注跨系统语义稳定。证据包应吸收这些思想，但保持 BitFun 内部规范证据模型。

## 2. 设计约束

- 证据包由交付物与证据层（Artifact and Evidence Plane）负责生成视图和版本化。
- 原始事实来自质量数据面的 `LifecycleEvent` 和 `EvidenceReference`。
- 证据包保存摘要和引用，完整终端日志、prompt、模型上下文或第三方载荷按隐私策略另行保留或丢弃。
- 证据包必须能表达 `fresh`、`partial`、`stale`、`blocked` 和 `superseded`。
- 证据包必须支持展示层级，避免完整证据包默认污染快速路径。
- 缺少证据、证据过期或主动配置未确认时，证据包使用 `partial`、`stale` 或 `blocked` 状态。
- PR 文本、审查界面、门禁、发布就绪度和评测回放都应消费同一证据包 schema。

## 3. 证据展示层级

| 层级 | 用户可见内容 | 适用场景 |
|---|---|---|
| `none` | 不展示证据结构，只保留后台事件 | 快速路径中间过程 |
| `summary` | 已做什么、未做什么、信心和下一步 | 快速任务或辅助建议任务结束 |
| `evidence_refs` | 摘要加命令、CI、文件、审查和安全决策引用 | PR 就绪度、审查、团队建议投影 |
| `full_pack` | 完整证据包、策略版本、风险接受、新鲜度和审计引用 | 守护/合规策略、发布、事故、评测 |

展示层由 [配置化策略画像](../features/configurable-policy-profile.md) 决定。证据存在性和证据展示层级相互独立：后台可以生成最小证据摘要，快速路径只展示任务闭环需要的信息。

## 4. 输入、输出与数据模型

输入：

| 输入 | 来源 |
|---|---|
| 项目画像快照 | 项目结构、规则、验证能力、负责人、主动配置状态 |
| Review 目标证据 | 当前工作区、明确 Git range 或 provider PR 的 base/head、目标指纹、provider identity、文件状态、有界 diff 可用性、完整度、workspace binding 和失效状态 |
| 任务与变更摘要 | 用户意图、Git diff、文件变更、重命名/删除、生成文件 |
| 验证证据 | `verification.completed`、CI 检查、命令摘要、制品引用 |
| 风险策略提示 | 风险标签、推荐/强制检查、审查强度 |
| 安全决策 | allow/ask/deny/应急放行、授权范围、残余风险 |
| 审查证据 | 严格审查问题、人工审查、过期标记 |
| 主动配置证据 | hook、plugin、自定义工具、MCP、智能体规则的发现、hash、权限和信任状态 |
| 人工决策 | 覆盖、风险接受、确认、拒绝 |

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

interface ReviewTargetFileEvidence {
  path: string;
  previousPath?: string;
  status: "added" | "modified" | "deleted" | "renamed" | "copied" | "unknown";
  completeness: "complete" | "partial" | "unavailable";
}

interface ReviewTargetEvidence {
  version: 1;
  source: "workspace" | "git_range";
  fingerprint: string;
  baseRevision?: string;
  headRevision?: string;
  completeness: "complete" | "partial" | "unknown" | "stale";
  workspaceBinding: "matching_clean" | "matching_dirty" | "mismatched" | "unavailable";
  files: ReviewTargetFileEvidence[];
  limitations: string[];
  omittedFileCount?: number;
}

type ReviewEvidenceStatus = "complete" | "limited" | "stale" | "failed";

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
  review_target?: ReviewTargetEvidence;
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

`ReviewTargetEvidence.completeness` describes prepared target facts. The final
report carries `ReviewEvidenceStatus` separately from its recommendation. Only
an immutable, complete Git range with no omitted files and a matching clean
workspace may report evidence status `complete`. Mutable workspace evidence is
always `limited`; this status does not rewrite the model recommendation.
Synthetic diff references and embedded diff bodies are intentionally excluded;
reviewers page through the target-bound diff tool using opaque cursors.

关键字段语义：

| 字段 | 语义 |
|---|---|
| `source_events` | 生成该包使用的事件 id 集合 |
| `evidence_refs` | 指向日志摘要、报告、CI、截图、轨迹或外部系统事实的引用 |
| `review_target` | 本次 Review 的目标摘要；changed-code 内容通过 prepared `GetFileDiff` 的有界分页读取，不嵌入 evidence pack。Git range 和 provider PR 的 immutable revision 可声明内容不可变；PR diff 按文件从 provider 读取并在读取前复核 base/head。live workspace 可声明 prepared diff 覆盖完整，但最终 evidence status 仍为 limited。revision 变化时工具返回 stale/limited 而不是读取错误 diff |
| `security` | 执行安全决策摘要，包括执行位置、沙箱等级组合、降级原因和授权范围；不作为质量通过依据 |
| `skipped_checks` | 未运行检查的原因、触发规则、可接受条件和残余风险 |
| `open_risks` | 尚未被证据覆盖或人工接受的风险 |
| `risk_acceptances` | 质量风险接受记录 |
| `break_glass_decisions` | 安全边界临时放行记录，必须与质量风险接受分开 |

## 5. 生命周期

```text
源事件
  -> 固定 Review 目标并构建证据摘要
  -> 附加画像和策略版本
  -> 判断新鲜度与完整度
  -> 选择展示层级
  -> 显露摘要、引用或完整证据包
  -> 变更集、画像、策略、验证、审查或主动配置变化时标记过期
  -> 用新的 EvidencePack 版本取代旧版本
```

状态规则：

| 状态 | 触发条件 | 下游行为 |
|---|---|---|
| `fresh` | 当前层级所需证据完整且来源版本未变化 | 可支撑就绪度或门禁判断 |
| `partial` | 推荐证据缺失、非阻塞跳过项或低风险未知 | 摘要或建议投影展示缺口 |
| `stale` | Review base/head、目标指纹、workspace binding、diff、项目画像、策略、强制检查、审查范围或主动配置变化 | 不得继续支撑通过/就绪判断或当前 head 的评论发布 |
| `blocked` | 必要验证失败、安全拒绝、高权限主动配置未确认或证据不可访问 | 下游应进入阻断、失败或降级状态 |
| `superseded` | 新版本证据包取代旧版本 | 旧包保留审计，不作为当前判断依据 |

## 6. 与其他模块的边界

| 模块 | 关系 |
|---|---|
| 质量数据面 | 提供事实事件、信任等级、隐私分类和证据引用 |
| 配置化策略画像 | 决定证据展示层级和是否进入 PR、团队或合规投影 |
| 安全边界 | 产生安全决策，证据包只记录摘要和授权引用 |
| 项目画像 | 提供项目、策略画像、规则和主动配置快照 |
| 风险分类器 | 消费上下文、变更和验证信息，输出风险证据 |
| 变更就绪度 / PR 门禁 | 消费证据包，产出就绪度或门禁决策 |
| 交付物图谱 | 可把证据包作为交付物节点，并把证据引用挂到图谱边 |
| 智能体评测 | 使用证据包和源事件做回放与失败归因 |

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P-1 | 定义 EvidenceReference、证据包结构、状态、展示层级、新鲜度和风险接受字段 |
| P0 | 为快速路径生成摘要层级，记录验证、安全决策、沙箱等级和跳过项 |
| P1 | 支撑 PR 就绪度的证据引用、过期证据和定向审查证据 |
| P2 | 支撑团队/守护策略的 PR 门禁投影、风险接受和主动配置信任审查 |
| P3 | 接入需求影响、发布就绪度、事故回溯和外部证明引用 |
| P4 | 支撑轨迹回放、治理策略评估和跨项目证据覆盖率分析 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 证据包变成用户必须理解的流程 | 展示层级默认摘要或不展示，`full_pack` 只在强场景显露 |
| 证据包变成日志包 | 只保存摘要和引用，完整日志通过受控 EvidenceReference 访问 |
| 安全放行和质量接受混淆 | 应急放行与风险接受分开字段、分开用户界面 |
| 门禁与证据包状态不一致 | 门禁结果必须引用 `evidence_pack_id` 和 `policy_version` |
| 人工接受掩盖证据缺失 | 风险接受不能把缺失证据改写成通过 |
| 证据过期不可见 | 变更集、策略画像、策略、检查、审查或主动配置变化必须标记过期 |
| 模块重复定义字段 | 证据包结构是唯一证据视图和 schema，其他模块只能扩展引用或消费 |

## 9. 成功标准

- 快速路径能给出简洁信心摘要，不暴露完整证据包。
- PR 就绪度可通过证据引用追溯关键证据。
- 跳过项、未关闭风险、风险接受和应急放行不会被隐藏。
- 证据过期后，旧证据包不再支撑通过/就绪判断。
- 完整证据包只在团队、发布、审计、复盘或评测场景显性使用。

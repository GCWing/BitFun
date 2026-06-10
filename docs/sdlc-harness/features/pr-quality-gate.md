# BitFun 生命周期工程子模块设计：PR Quality Gate

> 上游文档：[design.md](../design.md)
> 模块角色：将目标项目的 EvidencePack、Risk Classifier、Deep Review、验证命令、安全信号和人工风险接受整合为 PR 级质量门禁。

## 1. 模块定位

PR Quality Gate 是生命周期质量保护的首个产品级闭环。它不是替代 GitHub Checks、CI、安全扫描、CODEOWNERS 或人工 reviewer，而是在目标项目 PR 提交或更新前，把本地 diff、项目规则、验证证据和风险信号转化为一个简洁、可解释、可审计的 gate 结果。

P0 采用 lightweight gate：先覆盖目标项目 Project Profile、本地 diff/PR 的 EvidencePack、required checks、risk classification、PR 描述生成和可选 Deep Review 触发。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [GitHub Checks API](https://docs.github.com/en/rest/checks) / [rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets) | PR 门禁需要状态、结论、日志和可审计结果 |
| [GitHub Copilot code review](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review) | AI review 正在进入 PR 协作流程 |
| [Rovo acceptance criteria checks](https://support.atlassian.com/rovo/docs/check-acceptance-criteria-in-a-code-review/) / [RovoDev Code Reviewer](https://arxiv.org/html/2601.01129v1) | 验收标准、PR cycle time、可执行反馈和上下文完整性决定 AI review 价值 |
| [Cursor Bugbot](https://cursor.com/blog/building-bugbot) / [Qodo Code Review](https://docs.qodo.ai/code-review) | PR 级 logic bug、security、compliance review 是 AI IDE 外延能力 |
| [Codex Hooks](https://developers.openai.com/codex/hooks) / [Claude Code Hooks](https://code.claude.com/docs/en/hooks) / [OpenCode Plugins](https://opencode.ai/docs/plugins/) | Gate 应消费标准化 hook/event，不依赖某个 agent runtime 内部实现 |

设计约束：

- 默认不对所有 PR 执行 full Deep Review。
- blocking decision 必须引用确定性证据。
- `degraded` 是核心状态，优于错误的 `pass`。
- 人工 override 必须记录 reason、actor、time 和 residual risk。
- Gate 上线必须分阶段：shadow、advisory、required、blocking，不能直接从设计进入默认阻塞。
- Project Profile 未确认、证据过期或成本超预算时，必须展示降级原因和未覆盖风险。

## 3. 范围与非目标

范围：

- 聚合 EvidencePack、risk classification、required checks、Deep Review 和人工确认。
- 输出 `pass|warn|fail|degraded`。
- 生成 PR 描述中的质量证据块。
- 标记 stale review 和 stale evidence。

非目标：

- 不替代 CI 或 branch protection。
- 不代替人类最终发布责任。
- 不保证完整影响面分析。
- 不让第三方 hook 绕过 BitFun policy。

## 4. 输入、输出与数据模型

输入：

| 输入 | 来源 |
|---|---|
| Project Profile | 项目结构、规则来源、验证能力、owner、发布模型 |
| Diff summary | Git working tree 或 PR diff |
| Project rules | agent rules、contribution guide、module docs、CODEOWNERS、verification profile |
| Verification evidence | command、exit code、duration、log summary |
| Risk classification | Risk Classifier |
| Hook events | Quality Data Plane |
| Active config trust | Project Profile + Quality Data Plane |
| Deep Review result | optional targeted/full review |
| Artifact links | issue、spec、acceptance criteria、design doc |

输出：

```ts
interface PrGateResult {
  status: "pass" | "warn" | "fail" | "degraded";
  mode: "shadow" | "advisory" | "required" | "blocking";
  risk_level: "low" | "medium" | "high" | "unknown";
  confidence: number;
  policy_version: string;
  evidence_pack_id: string;
  summary: string;
  required_checks: RequiredCheckResult[];
  evidence: EvidenceReference[];
  open_risks: OpenRisk[];
  risk_acceptance?: RiskAcceptance;
  budget: {
    deep_review_required: boolean;
    deep_review_profile: "none" | "targeted" | "full";
    estimated_tokens: number;
    estimated_minutes: number;
    actual_tokens?: number;
    actual_minutes?: number;
  };
  degraded_reasons: string[];
}
```

人工风险接受模型：

```ts
interface RiskAcceptance {
  actor: string;
  reason: string;
  scope: "current_changeset" | "current_pr" | "release" | "timeboxed";
  residual_risk: string;
  evidence: EvidenceReference[];
  accepted_at: string;
  expires_at?: string;
}
```

状态语义：

| 状态 | 含义 | 行为 |
|---|---|---|
| `pass` | 必要证据完整且无阻塞风险 | 允许提交或更新 PR |
| `warn` | 存在非阻塞风险或推荐检查缺失 | 允许继续，但在 PR 文本中显式展示 |
| `fail` | 必要验证失败或违反阻塞策略 | 阻止自动提交或要求人工 override |
| `degraded` | 上下文、证据或工具状态不足，无法可靠判断 | 允许记录人工风险接受，但不得改写为 `pass` |

上线模式：

| 模式 | 行为 | 适用阶段 |
|---|---|---|
| `shadow` | 只生成结果，不影响提交和 PR | P0 初期校准 |
| `advisory` | 在 PR 文本或本地报告中提示风险 | P0/P1 默认 |
| `required` | 要求展示结果和 skipped/open risks | P1 团队协作 |
| `blocking` | 对确定性失败或缺失人工风险接受进行阻断 | 仅在误报率、成本和 override 流程稳定后启用 |

状态转换约束：

- `degraded` 不能因为用户确认而变成 `pass`；只能保留 `degraded` 并附加 risk acceptance，或在补齐证据后重新生成 EvidencePack 和 Gate。
- `fail` 只能由确定性失败、阻塞策略违反或已禁用主动配置仍被引用触发。
- `warn` 适合非阻塞 skipped check、推荐检查缺失或低风险 open risk。
- 所有状态都必须引用 `evidence_pack_id`、`policy_version` 和生成该结果的 source events。

主动配置 gate 策略：

| 情况 | Gate 行为 |
|---|---|
| 新增项目级 hook/plugin/custom tool 但未 trust review | `degraded`，展示来源、权限和未确认原因 |
| 主动配置 hash、权限或执行范围变化 | `degraded` 或 `fail`，取决于是否涉及 shell/network/secret/filesystem |
| 已禁用主动配置仍被事件引用 | `fail`，必须修正执行链路或移除引用 |
| 插件产出 recommendation | 只能进入 open risk 或 evidence candidate，不改变 pass/fail |

## 5. 核心流程

```text
local diff / PR
  -> Project Profile
  -> EvidencePack
  -> Risk Classifier
  -> required checks
  -> lightweight gate
  -> optional Deep Review by risk
  -> PR description / GitHub Check summary
```

PR 质量证据块：

```markdown
Quality Evidence

- Gate: warn
- Risk: medium
- Required checks:
  - passed: project type check
  - missing: project regression test
- Open risks:
  - Runtime boundary behavior changed without dedicated regression evidence.
- Deep Review: skipped, estimated value lower than required checks for this change.
```

## 6. 策略与治理

Deep Review 预算策略：

| 风险画像 | 默认策略 |
|---|---|
| 低风险 docs 或小范围文案 | 不触发 Deep Review |
| 中风险 UI 或 adapter | 本地验证证据不足时触发 targeted review |
| 高风险项目核心逻辑、integration adapter、安全、发布或远程运行边界 | targeted 或 full review |
| 大规模跨层 PR | 先完成低成本结构化检查，再决定 full review |

每次 Deep Review 都必须暴露：

- 预估 token 和耗时。
- 实际 token 和耗时。
- scope、skipped files、truncated context。
- advisory 或 blocking。
- 是否因预算降级。
- 与 lightweight gate 相比的增量价值假设。

Stale review 规则：

- diff 变化影响 reviewed hunk，finding 标记 `stale`。
- risk tags 或 required checks 变化，Gate 标记 `needs_rerun`。
- EvidencePack 过期，status 不得继续保持 `pass`。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | lightweight gate、本地报告、PR 质量证据块、degraded 状态 |
| P1 | GitHub Check summary、finding lifecycle、stale review |
| P2 | 需求验收标准、release readiness、人工风险接受审计 |
| P3 | 团队级 gate 策略、预算优化和质量趋势 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| full Deep Review 成本分级约束不足 | 低风险 PR 默认不触发；高风险先跑低成本检查 |
| 过早 blocking 降低采用 | 先 shadow/advisory 收集误报、漏报、耗时和 override 数据 |
| Gate 假阳性阻塞交付 | 每个 fail 必须有具体 evidence 和 override path |
| Gate 假阴性放过风险 | critical path 小 diff 不得按行数降级为 low |
| 缺证据仍 pass | 缺失 evidence store 或检查结果时必须 `degraded` 或 `fail` |
| 人工确认掩盖缺证据 | risk acceptance 只能记录接受范围和残余风险，不能把 `degraded` 改写成 `pass` |
| 未信任主动配置影响 gate | 未 trust review 的 hook/plugin/custom tool 只能导致 degraded/open risk，不能提供 pass 证据 |
| AI review 低精度 finding | finding 必须有生命周期、stale 标记和人工解决状态 |
| 插件绕过策略 | Gate 只消费 canonical event，不信任插件直接写入 pass 结论 |

## 9. 成功标准

- 多数 PR 准备流程能生成结构化 Gate 结果。
- 低风险 PR 不被 full Deep Review 阻塞。
- 高风险 PR 能暴露 required checks 和未覆盖风险。
- PR 描述减少 reviewer 追问验证信息的成本。
- Deep Review token 与耗时可见、可预算、可降级。
- 主动配置变化能被 Gate 显式降级、阻断或要求重新确认。
- Gate result 能通过 `evidence_pack_id`、`policy_version` 和 risk acceptance 追溯判断依据。

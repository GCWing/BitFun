# BitFun 子模块设计：Change Readiness and Optional PR Gate

> 上游文档：[design.md](../design.md)
> 模块角色：在用户准备提交、发起 PR、进入团队协作或项目开启强策略时，把变更、验证、风险和人工决策投影为可读的 readiness summary；只有在配置或风险要求下才升级为 PR Gate。

## 1. 模块定位

Change Readiness 是产品体验层，PR Gate 是其中的可选强治理投影。默认 Fast Path 不应让用户先理解 Gate、EvidencePack 或审计模型；它只需要在任务结束时给出简洁的改动、已验证项、未验证项和下一步建议。

当用户进入 PR、release、团队受管控目录或 `guarded/regulated` profile 时，本模块才把后台 EvidencePack、Risk Control Hint、验证证据和风险接受记录组合成更严格的结果。这个结果可以投影到本地报告、PR 描述、GitHub Check 或团队 required policy，但不替代 CI、branch protection、安全扫描、CODEOWNERS 或人类 reviewer。

关键边界：

- 安全阻断来自 [Security Boundary](../architecture/security-boundary.md)，不是本模块的质量判断。
- 风险等级来自 [Risk Classifier](risk-classifier.md)，但风险等级不是事实结论。
- EvidencePack 只提供证据快照；本模块不修改原始证据。
- blocking 只能来自确定性失败、组织策略或未被接受的明确残余风险。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [GitHub Copilot code review](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review) | AI review 默认以 comment 形式协助，不天然等同 required approval |
| [GitHub Checks API](https://docs.github.com/en/rest/checks) / [rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets) | 强策略需要稳定状态、结论、日志和审计语义 |
| [GitLab warn mode](https://docs.gitlab.com/user/application_security/policies/merge_request_approval_policies/) | 新策略应先 advisory/warn 校准，再进入 required/blocking |
| [CodeRabbit review profile](https://docs.coderabbit.ai/reference/configuration) | review 强度应可配置，默认减少噪音 |
| [Codex approvals/security](https://developers.openai.com/codex/agent-approvals-security) / [Claude permissions](https://code.claude.com/docs/en/permissions) | 执行授权和质量 readiness 必须拆开 |

设计约束：

- Fast Path 不生成可见 Gate。
- `summary` 和 `advisory` 是默认产品形态。
- `required` 和 `blocking` 只在团队/组织配置、确定性失败、安全拒绝或明确用户选择强模式时启用；风险等级本身只能建议升级，不能单独阻塞。
- `degraded` 是合法状态，优于错误的 ready/pass。
- 人工 override 必须记录 reason、actor、scope、expires_at 和 residual risk。
- 低风险任务不默认触发 full Deep Review。
- 未信任 hook/plugin/custom tool/MCP 只能产生 open risk 或 degraded，不提供 pass 证据。

## 3. 输入、输出与数据模型

输入：

| 输入 | 来源 |
|---|---|
| Adaptive Control Decision | profile、recommended/required checks、evidence mode、review mode |
| Security Boundary Decision | allow/ask/deny/break-glass、安全残余风险、授权范围 |
| Change summary | Git diff、文件变更、生成文件、删除/重命名 |
| Verification evidence | 本地命令、CI check、artifact ref、不可运行原因 |
| Risk Control Hint | 风险标签、触发原因、置信度、检查建议 |
| Evidence projection | summary、evidence refs 或 full EvidencePack |
| Human decision | 跳过检查、风险接受、break-glass、reviewer 决策 |

输出分两层：

```ts
interface ChangeReadinessSummary {
  level: "ready" | "attention" | "blocked" | "degraded";
  profile: "fast" | "assist" | "review" | "guarded" | "regulated";
  user_visible_level: "none" | "summary" | "advisory" | "required" | "blocking";
  summary: string;
  verified: VerificationSummary[];
  missing_or_skipped: SkippedCheck[];
  risk_hints: RiskHint[];
  security_actions: SecurityActionSummary[];
  next_actions: string[];
  evidence_display: "none" | "summary" | "evidence_refs" | "full_pack";
}

interface PrGateProjection {
  status: "pass" | "warn" | "fail" | "degraded";
  mode: "shadow" | "advisory" | "required" | "blocking";
  evidence_pack_id?: string;
  policy_version: string;
  required_checks: RequiredCheckResult[];
  open_risks: OpenRisk[];
  risk_acceptance?: RiskAcceptance;
  degraded_reasons: string[];
}
```

状态语义：

| 状态 | 用户含义 | 可继续吗 |
|---|---|---|
| `ready` / `pass` | 该 profile 下需要的证据完整，未发现未接受阻塞风险 | 可以 |
| `attention` / `warn` | 有建议检查、open risk 或非阻塞缺口 | 可以，但应展示后果 |
| `blocked` / `fail` | 确定性失败、安全拒绝、组织 required policy 未满足 | 不应自动继续 |
| `degraded` | 上下文、证据、工具、主动配置或外部系统不足以可靠判断 | 可以人工接受残余风险，但不能改写为 pass |

## 4. 核心流程

```text
task or PR intent
  -> Adaptive Control Decision
  -> Security Boundary summary
  -> collect change and verification evidence
  -> apply Risk Control Hint
  -> choose evidence display tier
  -> generate Change Readiness Summary
  -> optionally project PR Gate
```

默认体验：

| 场景 | 行为 |
|---|---|
| 无 git 临时目录、小工具、demo、文档 | 只给任务摘要和安全提示，不生成 Gate |
| 常规个人项目 | 给 summary/advisory；检查是推荐，不阻断 |
| 用户准备 PR | 生成 Change Readiness Summary，可插入 PR 文本 |
| 团队配置启用 required checks | 生成 PR Gate Projection |
| 受管控目录、发布、迁移、权限、网络或安全变更 | 升级到 review/guarded；blocking 仍需安全拒绝、确定性失败或 managed policy |

PR 文本投影示例：

```markdown
Change Readiness

- Profile: review
- Status: attention
- Verified:
  - type check passed
- Not verified:
  - integration test skipped: private service unavailable
- Open risks:
  - Runtime boundary changed; no dedicated regression evidence.
- Security:
  - Network access denied; no break-glass used.
- Next:
  - Run integration test in CI or accept residual risk for this PR.
```

## 5. 策略与治理

上线模式：

| 模式 | 行为 | 适用 |
|---|---|---|
| `off` | 不展示 readiness | Fast Path 中间过程 |
| `summary` | 只展示改动、验证、未验证和下一步 | Fast/assist 默认 |
| `advisory` | 展示风险和建议检查，不阻断 | review 默认 |
| `required` | 要求显式展示 skipped/open risks/risk acceptance | guarded/team policy |
| `blocking` | 对确定性失败或组织策略阻断 | regulated 或明确策略 |

Deep Review 预算策略：

| 风险画像 | 默认策略 |
|---|---|
| 低风险 docs、文案、小范围脚本 | 不触发 |
| 中风险 UI、adapter、测试不足 | 证据弱时 targeted review |
| 高风险核心逻辑、AI adapter、安全、远程、发布 | targeted 或 full review |
| 大规模跨层 PR | 先做结构化检查，再决定 full review |

人工风险接受规则：

- `degraded` 不能因为确认而变成 `pass`；只能保留 degraded 并附加 risk acceptance，或补齐证据后重算。
- risk acceptance 必须有范围和过期时间；默认不跨 PR 或 session 持久化。
- break-glass 是安全授权，不等同质量风险接受；两者必须分开记录。

## 6. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | Fast Task Summary、security action summary、推荐检查、低噪音 advisory |
| P1 | Change Readiness Summary、evidence refs、targeted review trigger |
| P2 | repo/path/team policy、PR Gate projection、required checks、risk acceptance |
| P3 | release readiness、finding lifecycle、stale evidence、team trend |

## 7. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 产品默认过重 | Fast Path 不展示 Gate；以 interruption rate 和 time to first useful action 验收 |
| Gate 假阳性阻塞交付 | 每个 fail 必须有确定性 evidence、策略来源和 override path |
| Gate 假阴性放过风险 | critical path 小 diff 不得只按行数降级 |
| 安全和质量混淆 | security deny/break-glass 只由 Security Boundary 产生 |
| 缺证据仍 pass | 缺证据只能 attention/degraded/fail，不能 ready/pass |
| AI review 低精度 finding | finding 必须有 scope、staleness、resolution 和人工反馈 |
| 插件绕过策略 | 插件只能产生 evidence/recommendation，不能直接写 pass/fail |

## 8. 成功标准

- 普通任务完成时没有被 PR/Gate 概念打断。
- 准备 PR 时能给出清晰、短、可行动的 Change Readiness Summary。
- 团队项目能把 readiness 投影为 required/blocking policy。
- 高风险变更能解释 required checks、review trigger 和 residual risk。
- 安全授权、质量风险接受和 Deep Review 成本分别可追踪。

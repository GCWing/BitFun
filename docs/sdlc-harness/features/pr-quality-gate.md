# BitFun 子模块设计：变更就绪度与可选 PR 门禁

> 上游文档：[product-requirements.md](../product-requirements.md)、[design.md](../design.md)
> 模块角色：在用户准备提交、发起 PR、进入团队协作或项目开启强策略时，把变更、验证、风险和人工决策投影为可读的就绪度摘要；只有在配置或风险要求下才升级为 PR 门禁。

## 1. 模块定位

变更就绪度是产品体验层，PR 门禁是其中的可选强治理投影。快速路径输出简洁的改动、已验证项、未验证项和下一步建议；当用户准备 PR、进入受管目录、开启团队策略或触发内部强策略时，再投影为更严格的就绪度或门禁结果。

在严格场景中，本模块把后台证据包、风险策略提示（Risk Policy Hint）、验证证据和风险接受记录组合成可审查结果。这个结果可以投影到本地报告、PR 描述、GitHub 检查状态或团队强制策略；CI、分支保护、安全扫描、CODEOWNERS 和人类审查人继续作为外部权威信号。

关键边界：

- 安全阻断来自 [安全边界](../architecture/security-boundary.md)。
- 风险等级来自 [风险分类器](risk-classifier.md)，并作为建议、检查和审查强度输入。
- 证据包只提供证据快照；本模块不修改原始证据。
- 阻断只能来自确定性失败、组织策略或未被接受的明确残余风险。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [GitHub Copilot 代码审查](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review) | AI 审查优先以评论形式协助，强制审批由仓库规则配置 |
| [GitHub Checks API](https://docs.github.com/en/rest/checks) / [rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets) | 强策略需要稳定状态、结论、日志和审计语义 |
| [GitLab 警告模式](https://docs.gitlab.com/user/application_security/policies/merge_request_approval_policies/) | 新策略应先建议/警告校准，再进入强制/阻断 |
| [CodeRabbit 审查强度](https://docs.coderabbit.ai/reference/configuration) | 审查强度应可配置，默认减少噪音 |
| [Codex approvals/security](https://developers.openai.com/codex/agent-approvals-security) / [Claude 权限](https://code.claude.com/docs/en/permissions) | 执行授权和质量就绪度必须拆开 |

设计约束：

- 快速路径不生成可见门禁。
- 摘要（`summary`）和建议（`advisory`）是默认产品形态。
- 强制（`required`）和阻断（`blocking`）只在团队/组织配置、确定性失败、安全拒绝或用户明确选择强策略时启用；风险等级本身只能建议升级，不能单独阻塞。
- 降级（`degraded`）是合法状态，优于错误的就绪/通过结论。
- 人工覆盖必须记录原因、操作者、范围、过期时间和残余风险。
- 低风险任务不默认触发完整深度审查。
- 未信任 hook、plugin、自定义工具或 MCP 只能产生未关闭风险或降级状态，不提供通过证据。

## 3. 输入、输出与数据模型

输入：

| 输入 | 来源 |
|---|---|
| 配置化策略决策 | 内部策略画像、推荐/强制检查、证据展示层级、审查策略 |
| 安全边界决策 | allow/ask/deny/应急放行、安全残余风险、授权范围 |
| 变更摘要 | Git diff、文件变更、生成文件、删除/重命名 |
| 验证证据 | 本地命令、CI 检查、制品引用、不可运行原因 |
| 风险策略提示 | 风险标签、触发原因、置信度、检查建议 |
| 证据投影 | 摘要、证据引用或完整证据包 |
| 人工决策 | 跳过检查、风险接受、应急放行、审查人决策 |

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
| `ready` / `pass` | 该策略下需要的证据完整，未发现未接受阻塞风险 | 可以 |
| `attention` / `warn` | 有建议检查、未关闭风险或非阻塞缺口 | 可以，但应展示后果 |
| `blocked` / `fail` | 确定性失败、安全拒绝、组织强制要求策略未满足 | 停止当前自动流程并处理原因 |
| `degraded` | 上下文、证据、工具、主动配置或外部系统不足以可靠判断 | 可以人工接受残余风险，但不能改写为通过 |

## 4. 核心流程

```text
任务或 PR 意图
  -> 配置化策略决策
  -> 安全边界摘要
  -> 收集变更与验证证据
  -> 应用风险策略提示
  -> 选择证据展示层级
  -> 生成变更就绪度摘要
  -> 按需投影 PR 门禁
```

默认体验：

| 场景 | 行为 |
|---|---|
| 无 git 临时目录、小工具、演示、文档 | 只给任务摘要和安全提示，不生成门禁 |
| 常规个人项目 | 给摘要或建议投影；检查是推荐，不阻断 |
| 用户准备 PR | 生成变更就绪度摘要，可插入 PR 文本 |
| 团队配置启用强制检查 | 生成 PR 门禁投影 |
| 受管控目录、发布、迁移、权限、网络或安全变更 | 升级到审查或守护策略；阻断仍需安全拒绝、确定性失败或受管策略 |

PR 文本投影示例：

```markdown
变更就绪度

- 任务状态：需要关注
- 状态：attention
- 已验证：
  - 类型检查通过
- 未验证：
  - 集成测试已跳过：私有服务不可用
- 未关闭风险：
  - 运行时边界发生变化，缺少专门回归证据
- 安全：
  - 网络访问被拒绝，未使用应急放行
- 下一步：
  - 在 CI 中运行集成测试，或接受本 PR 的残余风险
```

## 5. 策略与治理

投影方式：

| 投影 | 行为 | 适用 |
|---|---|---|
| `off` | 不展示就绪度 | 快速路径中间过程 |
| `summary` | 只展示改动、验证、未验证和下一步 | 快速路径和辅助建议默认 |
| `advisory` | 展示风险和建议检查，不阻断 | 审查默认 |
| `required` | 要求显式展示跳过项、未关闭风险和风险接受 | 守护策略或团队策略 |
| `blocking` | 对确定性失败或组织策略阻断 | 合规策略或明确阻断策略 |

深度审查预算策略：

| 风险画像 | 默认策略 |
|---|---|
| 低风险 docs、文案、小范围脚本 | 不触发 |
| 中风险界面、适配器、测试不足 | 证据弱时定向审查 |
| 高风险核心逻辑、AI 适配器、安全、远程、发布 | 定向或完整审查 |
| 大规模跨层 PR | 先做结构化检查，再决定完整审查 |

人工风险接受规则：

- `degraded` 不能因为确认而变成 `pass`；只能保留降级状态并附加风险接受，或补齐证据后重算。
- 风险接受必须有范围和过期时间；默认不跨 PR 或会话持久化。
- 应急放行属于安全授权，质量风险接受属于交付决策；两者必须分开记录。

## 6. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | 快速任务摘要、安全动作摘要、推荐检查、低噪音建议投影 |
| P1 | 变更就绪度摘要、证据引用、定向审查触发 |
| P2 | 仓库、路径、团队策略、PR 门禁投影、强制检查、风险接受 |
| P3 | 发布就绪度、问题生命周期、过期证据、团队趋势 |

## 7. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 产品默认过重 | 快速路径不展示门禁；以打断率和首次有用动作耗时验收 |
| 门禁假阳性阻塞交付 | 每个失败结论必须有确定性证据、策略来源和覆盖路径 |
| 门禁假阴性放过风险 | 关键路径小 diff 不得只按行数降级 |
| 安全和质量混淆 | 安全拒绝和应急放行只由安全边界产生 |
| 缺证据仍通过 | 缺证据只能进入注意、降级或失败状态，不能进入就绪/通过状态 |
| AI 审查低精度问题 | 问题必须有范围、新鲜度、处理状态和人工反馈 |
| 插件绕过策略 | 插件只能产生证据或建议，不能直接写通过/失败结论 |

## 8. 成功标准

- 普通任务完成时没有被 PR/门禁概念打断。
- 准备 PR 时能给出清晰、短、可行动的变更就绪度摘要。
- 团队项目能把就绪度投影为强制要求/阻断策略。
- 高风险变更能解释强制检查、审查触发和残余风险。
- 安全授权、质量风险接受和深度审查成本分别可追踪。

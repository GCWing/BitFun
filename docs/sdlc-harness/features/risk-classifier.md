# BitFun 生命周期工程子模块设计：Risk Classifier

> 上游文档：[design.md](../design.md)
> 模块角色：根据目标项目画像、变更内容、路径、历史信号和交付阶段，生成风险标签、必跑验证、reviewer 要求和 Deep Review profile。

## 1. 模块定位

Risk Classifier 是目标项目 PR EvidencePack 和 PR Gate 的策略输入层。它的输出是决策建议，不是事实结论。任何阻塞门禁必须引用确定性证据，例如失败测试、缺失 required check、违反项目规则或人工风险接受缺失。

P0 应优先使用从 Project Profile 派生的可解释规则和路径矩阵；模型辅助只用于补充说明、风险摘要和检查建议，不直接决定 pass/fail。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [GitHub rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets) | 质量门禁需要可解释、可配置、可审计 |
| [Rovo acceptance criteria checks](https://support.atlassian.com/rovo/docs/check-acceptance-criteria-in-a-code-review/) | PR 风险应结合 linked work item 和验收标准 |
| [Agentless](https://arxiv.org/abs/2407.01489) | 简单可解释 pipeline 是强基线，不应过早依赖复杂自治 |
| [NIST SP 800-218A](https://csrc.nist.gov/pubs/sp/800/218/a/final) | AI 相关变更需要把模型、数据、工具和供应链纳入风险识别 |

设计约束：

- 输出必须包含 reason、evidence、confidence 和 override path。
- 风险标签不能替代人工责任边界。
- 规则、模型提示和路径矩阵都必须版本化。
- 校准依赖 post-merge defect、review blocker、CI failure、override 结果。
- Project Profile 中 `unknown/conflicting/stale` 的规则不得被当作低风险依据。
- 主动配置变化，例如 hook、plugin、custom tool、MCP server 或 agent rules，必须进入安全敏感风险路径，不得按普通文档变更处理。
- 风险分类必须支持项目级策略覆盖，避免把 BitFun 自身验证路径或某类技术栈硬编码为默认规则。

## 3. 范围与非目标

范围：

- 基于 Project Profile、diff、路径、项目规则、历史信号生成风险标签。
- 生成 required checks、reviewer 建议、Deep Review profile。
- 向 PR Gate 输出可解释决策输入。
- 收集后验信号用于校准。

非目标：

- 不预测所有线上缺陷。
- 不用黑盒模型直接阻塞 PR。
- 不替代 CODEOWNERS、分支保护或安全扫描。
- 不保证需求影响分析完整性。

## 4. 输入、输出与数据模型

输入：

| 输入 | 示例 |
|---|---|
| Project Profile | 语言、框架、模块、owner、规则来源、验证能力、发布模型 |
| Diff metadata | 文件路径、hunk、rename/delete、生成文件、行数 |
| Project policy | agent rules、contribution guide、module docs、CODEOWNERS、verification profile |
| Active config state | hook/plugin/custom tool/MCP server 的 discovered/trusted/changed/disabled 状态 |
| Artifact links | issue、spec、acceptance criteria、design decision |
| Historical signals | flaky tests、past incidents、review findings、hot files |
| Verification state | 已运行/缺失/失败/过期的 required checks |

输出：

```ts
interface RiskClassification {
  level: "low" | "medium" | "high" | "unknown";
  tags: RiskTag[];
  reasons: string[];
  evidence: EvidenceReference[];
  confidence: number;
  required_checks: RequiredCheck[];
  reviewer_requirements: ReviewerRequirement[];
  deep_review_profile: "none" | "targeted" | "full";
  override_policy: OverridePolicy;
}
```

## 5. 核心流程

```text
project profile
  -> diff summary
  -> profile freshness and conflict check
  -> path and module rule matching
  -> artifact graph context lookup
  -> historical risk enrichment
  -> required checks generation
  -> Deep Review profile selection
  -> emit risk.classified event
  -> collect calibration feedback
```

风险标签示例：

| 标签 | 触发条件 |
|---|---|
| `project_core` | 目标项目关键业务逻辑、核心服务、公共库或关键运行路径 |
| `integration_adapter` | 外部服务、provider、协议、schema、stream、cache 变更 |
| `security_sensitive` | auth、secret、filesystem、shell、network、permission |
| `active_config_sensitive` | hook、plugin、custom tool、MCP server、agent rules 或自动化配置变化 |
| `deployment_sensitive` | release、migration、infra、remote workspace、runtime boundary |
| `ui_behavior` | 用户可见状态、交互流程、前端 adapter、review surface |
| `docs_only` | 文档或注释变更，无行为影响 |
| `generated_large_diff` | 大量生成文件、snapshot、lockfile |

## 6. 策略与治理

| 风险等级 | 默认策略 |
|---|---|
| low | required checks 完整即可 pass，不触发 Deep Review |
| medium | 需要相关验证；证据不足时 warn 或 targeted Deep Review |
| high | 必须列出阻塞检查、owner/reviewer、Deep Review profile 和人工 override 条件 |
| unknown | 缺少上下文时进入 degraded，不得标记为 low |

主动配置策略：

| 状态 | 默认风险策略 |
|---|---|
| discovered | `unknown` 或 `medium`，不得执行，不得作为 pass 依据 |
| trusted | 按声明权限和影响范围参与风险分类 |
| changed | 至少 `medium`，涉及 shell/network/secret/filesystem 时升为 `high` |
| disabled | 记录 open risk，确认不影响 required checks 后可降级 |

画像状态策略：

| Project Profile 状态 | 分类策略 |
|---|---|
| confirmed | 可使用项目路径矩阵和验证能力生成 required checks |
| inferred | 可生成建议，但阻塞性 gate 需要确定性证据或人工确认 |
| unknown | 输出 `unknown` 或 `degraded`，不得降级为 low |
| conflicting | 暂停自动 pass，要求用户确认规则优先级 |
| stale | 重新刷新画像或标记风险结论过期 |

Deep Review profile：

- `none`：低风险 docs/copy/small isolated change。
- `targeted`：中风险 UI、adapter、测试覆盖不足。
- `full`：核心运行时、AI adapter、安全、远程、跨层重构或大规模 PR。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | Project Profile、路径矩阵、项目规则、required checks、risk.classified event |
| P1 | Artifact Graph context、历史风险、override 记录 |
| P2 | 后验校准、风险规则 dashboard、策略版本对比 |
| P3 | 模型辅助排序和异常检测，但仍保留确定性阻塞依据 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 风险等级被当成事实 | UI 和 PR 文本必须展示 evidence、confidence、override |
| 低风险误判 | critical path 小 diff 必须触发规则，不得只按行数判断 |
| 主动配置被当成 docs-only | hook/plugin/custom tool/MCP/agent rules 变化必须触发 active_config_sensitive |
| 高风险误报 | generated large diff 和低行为影响变更应能降级处理 |
| required checks 过多 | 每个 required check 必须有触发原因和取消条件 |
| Deep Review 成本被放大 | 只有 high 或 evidence weak 的 medium 风险才默认触发 targeted/full review |
| 规则长期失准 | post-merge defect、review blocker、override 结果必须回流校准 |

## 9. 成功标准

- PR Gate 能用分类结果生成可解释 required checks。
- 低风险 PR 不默认触发 full Deep Review。
- 高风险 PR 至少暴露风险原因、必跑验证和未覆盖风险。
- reviewer 能理解并覆盖风险结论。
- 误报/漏报可通过校准机制被量化和修正。

# BitFun 子模块设计：Risk and Control Classifier

> 上游文档：[design.md](../design.md)
> 模块角色：根据任务意图、操作类型、项目画像、变更内容、路径、历史信号和团队策略，生成风险提示、控制建议、验证建议和 review profile。

## 1. 模块定位

Risk and Control Classifier 是 Adaptive Control 的策略输入层，不是 PR Gate 专属模块，也不是阻塞决策引擎。它回答：

```text
这个任务或变更需要多少额外信心？
```

它输出的是可解释建议：建议哪些检查、是否建议 targeted review、是否需要 evidence refs、是否可能进入 Team Governance。任何阻塞仍必须由确定性证据、Security Boundary 或明确项目/组织策略触发；分类器不能凭模型判断把 advisory 变成 required/blocking。

安全敏感信号会被识别并传递给 [Security Boundary](../architecture/security-boundary.md)，但安全 allow/deny/break-glass 不由本模块决定。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [GitHub rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets) | 强约束需要可解释、可配置、可审计 |
| [CodeRabbit path instructions](https://docs.coderabbit.ai/configuration/path-instructions) | 路径级规则比项目级单一规则更适合复杂仓库 |
| [Kiro steering](https://kiro.dev/docs/steering/) | workspace、team、conditional inclusion 能减少无关上下文和误触发 |
| [Agentless](https://arxiv.org/abs/2407.01489) | 简单可解释 pipeline 是强基线，不应过早依赖复杂自治 |
| [NIST SP 800-218A](https://csrc.nist.gov/pubs/sp/800/218/a/final) | AI 相关变更需要把模型、数据、工具和供应链纳入风险识别 |

设计约束：

- 输出必须包含 reason、evidence、confidence 和 override path。
- 风险标签不能替代人工责任边界。
- 规则、模型提示、路径矩阵和团队配置都必须版本化。
- 校准依赖 post-merge defect、review blocker、CI failure、override 和 false escalation。
- Project Profile 中 `unknown/conflicting/stale` 的规则不得被当作低风险依据。
- 主动配置变化，例如 hook、plugin、custom tool、MCP server 或 agent rules，必须进入安全敏感风险路径。
- 不把 BitFun 自身验证路径或某类技术栈硬编码为默认规则。

## 3. 风险维度

| 维度 | 示例 | 下游用途 |
|---|---|---|
| Task risk | 临时脚本、PR、release、migration、incident fix | 决定默认 profile |
| Action risk | shell、network、secret、跨目录写、删除、发布凭据 | 传递给 Security Boundary |
| Change risk | 核心逻辑、API/schema、adapter、UI 行为、generated diff | 生成验证和 review 建议 |
| Environment trust | 本地项目、远程 workspace、未信任主动配置、私有 CI | 决定提示和证据需求 |
| Project policy | repo/path/team rules、CODEOWNERS、required checks | 决定 required 或 advisory |
| Historical risk | hot file、flaky test、incident、review blocker | 校准风险等级 |

## 4. 输入、输出与数据模型

输入：

| 输入 | 示例 |
|---|---|
| User intent | ask、edit、debug、PR、release、throwaway tool |
| Project Profile | 语言、框架、模块、owner、规则来源、验证能力、发布模型 |
| Diff metadata | 文件路径、hunk、rename/delete、生成文件、行数 |
| Project policy | agent rules、contribution guide、module docs、CODEOWNERS、verification profile |
| Active config state | hook/plugin/custom tool/MCP server 的 discovered/trusted/changed/disabled 状态 |
| Artifact links | issue、spec、acceptance criteria、design decision |
| Historical signals | flaky tests、past incidents、review findings、hot files |
| Verification state | 已运行/缺失/失败/过期的 recommended/required checks |

输出：

```ts
interface RiskControlHint {
  level: "low" | "medium" | "high" | "unknown";
  tags: RiskTag[];
  axes: {
    task: RiskAxis;
    action: RiskAxis;
    change: RiskAxis;
    environment: RiskAxis;
    project_policy: RiskAxis;
  };
  reasons: string[];
  evidence: EvidenceReference[];
  confidence: number;
  recommended_checks: RequiredCheck[];
  required_checks: RequiredCheck[];
  review_profile: "none" | "targeted" | "full";
  evidence_display_hint: "none" | "summary" | "evidence_refs" | "full_pack";
  control_profile_hint: "fast" | "assist" | "review" | "guarded" | "regulated";
  override_policy: OverridePolicy;
}
```

风险标签示例：

| 标签 | 触发条件 |
|---|---|
| `throwaway_or_demo` | 临时工具、demo、无 git 项目、用户明确快速试验 |
| `project_core` | 关键业务逻辑、核心服务、公共库或关键运行路径 |
| `integration_adapter` | 外部服务、provider、协议、schema、stream、cache |
| `security_sensitive` | auth、secret、filesystem、shell、network、permission |
| `prompt_injection_sensitive` | 外部文档、issue、网页、MCP 输出可能影响 agent 指令 |
| `active_config_sensitive` | hook、plugin、custom tool、MCP server、agent rules 或自动化配置变化 |
| `deployment_sensitive` | release、migration、infra、remote workspace、runtime boundary |
| `ui_behavior` | 用户可见状态、交互流程、前端 adapter、review surface |
| `docs_only` | 文档或注释变更，无行为影响 |
| `generated_large_diff` | 大量生成文件、snapshot、lockfile |

## 5. 核心流程

```text
user intent and project context
  -> task/action/change/environment risk scan
  -> profile freshness and conflict check
  -> path and module rule matching
  -> historical risk enrichment
  -> recommended/required checks generation
  -> review profile and evidence display hint
  -> emit risk.control_hinted event
  -> collect calibration feedback
```

策略：

| 风险等级 | 默认策略 |
|---|---|
| low | 保持 `fast` 或 `assist`，只给推荐检查，不触发 Deep Review |
| medium | 建议相关验证；证据弱时建议 targeted review 或 evidence refs |
| high | 列出 required-if-policy checks、owner/reviewer、review profile 和 override 条件 |
| unknown | 不降级为 low；保持 summary/advisory 或 degraded，等待确认 |

主动配置策略：

| 状态 | 默认策略 |
|---|---|
| discovered | 不执行；输出 `active_config_sensitive`，交给 Security Boundary |
| trusted | 按声明权限和影响范围参与分类 |
| changed | 至少 medium；涉及 shell/network/secret/filesystem 时 high |
| disabled | 记录 open risk，确认不影响验证后可降级 |

## 6. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | task/action/change 风险标签、推荐检查、`risk.control_hinted` 事件 |
| P1 | path matrix、项目规则、targeted review trigger、false escalation 反馈 |
| P2 | team policy、required checks、guarded/regulated 配置 |
| P3 | Artifact Graph context、历史风险和 release/incident 信号 |
| P4 | 后验校准、策略 A/B、模型辅助排序和异常检测 |

## 7. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 风险等级被当成事实 | UI 和 PR 文本必须展示 evidence、confidence、override |
| 低风险误判 | critical path 小 diff 必须触发规则，不得只按行数判断 |
| 普通任务被误升级 | false escalation rate 必须进入 P0/P1 指标 |
| 主动配置被当成 docs-only | hook/plugin/custom tool/MCP/agent rules 变化必须触发 active_config_sensitive |
| required checks 过多 | 每个 required check 必须有触发原因和取消条件 |
| Deep Review 成本被放大 | 只有 high 或 evidence weak 的 medium 风险才默认触发 targeted/full review |
| 规则长期失准 | post-merge defect、review blocker、override 和 skipped reason 必须回流校准 |

## 8. 成功标准

- Adaptive Control 能用分类结果选择合理 profile。
- 普通低风险任务不被强质量流程打断。
- 高风险变更能暴露风险原因、必跑验证和未覆盖风险。
- 安全敏感动作能被正确转交 Security Boundary。
- 误升级、漏推荐、无价值检查都可通过反馈量化和修正。

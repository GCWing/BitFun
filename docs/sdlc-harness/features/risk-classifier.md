# BitFun 子模块设计：风险与策略分类器

> 上游文档：[design.md](../design.md)
> 模块角色：根据任务意图、操作类型、项目画像、变更内容、路径、历史信号和团队策略，生成风险提示、策略建议、验证建议和审查强度。

## 1. 模块定位

风险与策略分类器是配置化策略的输入层。它回答：

```text
这个任务或变更需要多少额外信心？
```

它输出可解释建议：建议哪些检查、是否建议定向审查、是否需要证据引用、是否可能进入团队治理。阻塞由确定性证据、安全边界或明确项目/组织策略触发；分类器负责给出建议、原因和置信度。

安全敏感信号会被识别并传递给 [安全边界](../architecture/security-boundary.md)，但安全允许、拒绝或应急放行不由本模块决定。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [GitHub rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets) | 强约束需要可解释、可配置、可审计 |
| [CodeRabbit 路径指令](https://docs.coderabbit.ai/configuration/path-instructions) | 路径级规则比项目级单一规则更适合复杂仓库 |
| [Kiro steering](https://kiro.dev/docs/steering/) | 工作区、团队、条件加载能减少无关上下文和误触发 |
| [Agentless](https://arxiv.org/abs/2407.01489) | 简单可解释流程是强基线，复杂自治能力应建立在可解释基线之上 |
| [NIST SP 800-218A](https://csrc.nist.gov/pubs/sp/800/218/a/final) | AI 相关变更需要把模型、数据、工具和供应链纳入风险识别 |

设计约束：

- 输出必须包含原因、证据、信心和覆盖路径。
- 风险标签用于提示、检查和审查建议；人工责任边界由团队策略和风险接受记录确定。
- 规则、模型提示、路径矩阵和团队配置都必须版本化。
- 校准依赖合入后缺陷、审查阻塞项、CI 失败、覆盖和误升级。
- 项目画像中 `unknown/conflicting/stale` 的规则进入未知或降级路径。
- 主动配置变化，例如 hook、plugin、自定义工具、MCP server 或智能体规则，必须进入安全敏感风险路径。
- 默认规则保持技术栈无关；BitFun 自身验证路径只作为内部样本。

## 3. 风险维度

| 维度 | 示例 | 下游用途 |
|---|---|---|
| 任务风险 | 临时脚本、PR、发布、迁移、事故修复 | 决定默认模式 |
| 动作风险 | shell、网络、凭据、跨目录写、删除、发布凭据 | 传递给安全边界 |
| 变更风险 | 核心逻辑、API/schema、适配器、界面行为、生成 diff | 生成验证和审查建议 |
| 环境信任度 | 本地项目、远程工作区、未信任主动配置、私有 CI | 决定提示和证据需求 |
| 项目策略 | 仓库/路径/团队规则、CODEOWNERS、强制检查 | 决定强制要求或建议模式 |
| 历史风险 | 高频热点文件、不稳定测试、事故、审查阻塞项 | 校准风险等级 |

## 4. 输入、输出与数据模型

输入：

| 输入 | 示例 |
|---|---|
| 用户意图 | ask、edit、debug、PR、发布、临时工具 |
| 项目画像 | 语言、框架、模块、负责人、规则来源、验证能力、发布模型 |
| Diff 元数据 | 文件路径、hunk、重命名/删除、生成文件、行数 |
| 项目策略 | 智能体规则、贡献指南、模块文档、CODEOWNERS、验证模式 |
| 主动配置状态 | hook、plugin、自定义工具、MCP server 的 discovered/trusted/changed/disabled 状态 |
| 交付物链接 | issue、spec、验收标准、设计决策 |
| 历史信号 | 不稳定测试、历史事故、审查问题、高频热点文件 |
| 验证状态 | 已运行/缺失/失败/过期的推荐/强制检查 |

输出：

```ts
interface RiskPolicyHint {
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
  policy_profile_hint: "fast" | "assist" | "review" | "guarded" | "regulated";
  override_policy: OverridePolicy;
}
```

风险标签示例：

| 标签 | 触发条件 |
|---|---|
| `throwaway_or_demo` | 临时工具、演示、无 git 项目、用户明确快速试验 |
| `project_core` | 关键业务逻辑、核心服务、公共库或关键运行路径 |
| `integration_adapter` | 外部服务、provider、协议、schema、stream、cache |
| `security_sensitive` | auth、凭据、filesystem、shell、网络、权限 |
| `prompt_injection_sensitive` | 外部文档、issue、网页、MCP 输出可能影响智能体指令 |
| `active_config_sensitive` | hook、plugin、custom 工具、MCP server、智能体规则或自动化配置变化 |
| `deployment_sensitive` | 发布、migration、infra、remote 工作区、运行时 boundary |
| `ui_behavior` | 用户可见状态、交互流程、前端适配器、审查 surface |
| `docs_only` | 文档或注释变更，无行为影响 |
| `generated_large_diff` | 大量生成文件、snapshot、lockfile |

## 5. 核心流程

```text
用户意图和项目上下文
  -> 扫描任务、动作、变更和环境风险
  -> 检查画像新鲜度与冲突
  -> 匹配路径和模块规则
  -> 补充历史风险
  -> 生成推荐/强制检查
  -> 生成审查画像和证据展示提示
  -> 发出 risk.policy_hinted 事件
  -> 收集校准反馈
```

策略：

| 风险等级 | 默认策略 |
|---|---|
| low | 保持低干预策略画像，只给推荐检查，不触发深度审查 |
| medium | 建议相关验证；证据弱时建议定向审查或证据引用 |
| high | 列出按策略强制的检查、负责人/审查人、审查强度和覆盖条件 |
| unknown | 不降级为 low；保持摘要或建议投影，必要时进入降级状态并等待确认 |

主动配置策略：

| 状态 | 默认策略 |
|---|---|
| discovered | 不执行；输出 `active_config_sensitive`，交给安全边界 |
| trusted | 按声明权限和影响范围参与分类 |
| changed | 至少中风险；涉及 shell、网络、凭据或文件系统时为高风险 |
| disabled | 记录未关闭风险，确认不影响验证后可降级 |

## 6. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | 任务/动作/变更风险标签、推荐检查、`risk.policy_hinted` 事件 |
| P1 | 路径矩阵、项目规则、定向审查触发、误升级反馈 |
| P2 | 团队策略、强制检查、守护/合规配置 |
| P3 | 交付物图谱上下文、历史风险和发布/事故信号 |
| P4 | 后验校准、策略 A/B、模型辅助排序和异常检测 |

## 7. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 风险等级被当成事实 | 界面和 PR 文本必须展示证据、信心和覆盖范围 |
| 低风险误判 | 关键路径小 diff 必须触发规则，不得只按行数判断 |
| 普通任务被误升级 | 误升级率必须进入 P0/P1 指标 |
| 主动配置被当成纯文档变更 | hook、plugin、自定义工具、MCP、智能体规则变化必须触发 active_config_sensitive |
| 强制检查过多 | 每个强制检查必须有触发原因和取消条件 |
| 深度审查成本被放大 | 只有高风险或证据薄弱的中风险才默认触发定向/完整审查 |
| 规则长期失准 | 合入后缺陷、审查阻塞项、覆盖和跳过原因必须回流校准 |

## 8. 成功标准

- 配置化策略能用分类结果选择合理模式。
- 普通低风险任务保持快速路径和轻量建议。
- 高风险变更能暴露风险原因、必跑验证和未覆盖风险。
- 安全敏感动作能被正确转交安全边界。
- 误升级、漏推荐、无价值检查都可通过反馈量化和修正。

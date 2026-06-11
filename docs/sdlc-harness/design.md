# BitFun 自适应工程开发与控制体验设计

> 范围：定义 BitFun 加载外部软件工程后的默认产品体验、复杂项目来源、自适应控制模型、执行安全边界、模块职责和长期演进路径。
> 边界：本文不以 BitFun 仓库自身治理为主线。BitFun 自身的文档、模块边界或工程治理问题应作为独立内部验证与平台建设输入承载，不混入产品目标架构。

## 1. 设计目的

BitFun 的目标不是把所有项目都放进强质量流程，而是成为面向任意目标项目的 Agentic Development 产品：默认帮助用户快速理解、修改、运行、验证和交付；当任务、权限、发布或团队流程变复杂时，再逐步显露上下文、证据、审查、策略和治理能力。

主设计回答四个问题：

1. 如何让普通项目和临时小工具默认保持低摩擦。
2. 如何在复杂项目、团队协作、发布和合规场景中逐步升级控制强度。
3. 如何把执行安全与质量治理拆开，避免 Fast Path 绕过安全底线。
4. 如何保证 EvidencePack、Artifact Graph、QDP、Risk、Gate 等技术只作为产品体验的后台支撑，而不是默认暴露给用户的复杂流程。

一句话目标：

```text
Fast Path by default, contextual assurance when needed, security boundary always on.
```

## 2. 市场与产品判断

当前先进开发工具的共同趋势不是默认强门禁，而是“快速执行 + 项目规则 + 可选审查 + 安全边界”：

| 市场信号 | 对 BitFun 的启发 |
|---|---|
| [GitHub Copilot custom instructions](https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/add-custom-instructions/add-repository-instructions) 支持 repo、path 和 AGENTS.md 指令 | 项目知识应优先读取现有规则文件，并按路径/上下文渐进适用 |
| [GitHub Copilot code review](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review) 默认 comment，不计入 required approval | AI review 默认应是低摩擦提示，不应天然阻塞 |
| [Codex sandbox/approval](https://developers.openai.com/codex/agent-approvals-security) 和 [Claude permissions](https://code.claude.com/docs/en/permissions) 拆分 sandbox 与 approval | 执行安全必须独立于质量治理，且用技术边界减少弹窗 |
| [CodeRabbit review profile](https://docs.coderabbit.ai/reference/configuration) 默认 `chill`，`assertive` 可选 | 审查强度应可调，默认减少噪音 |
| [CodeRabbit path instructions](https://docs.coderabbit.ai/configuration/path-instructions) 和 [GitLab Duo instructions](https://docs.gitlab.com/user/gitlab_duo/customize_duo/review_instructions/) 支持路径规则 | 复杂项目来源主要来自路径、团队和场景，不只是项目整体 |
| [GitLab warn mode](https://docs.gitlab.com/user/application_security/policies/merge_request_approval_policies/) 用于先验证策略影响 | 强策略应先 advisory/warn，再进入 required/blocking |
| [Kiro steering](https://kiro.dev/docs/steering/) 区分 workspace/global/team 和 inclusion mode | 项目规则需要作用域、优先级和加载时机 |
| [Jules](https://jules.google/) 强调 async task、plan、diff、PR review | 长任务可异步代理，但用户体验仍是计划、diff 和批准，而不是强治理术语 |

这些信号说明：BitFun 的差异化不应是“比别人更强硬地 gate”，而应是“在不同复杂度和风险下给出最合适的控制强度”。

## 3. 产品体验原则

| 原则 | 设计含义 |
|---|---|
| 默认快速 | 普通任务不要求完整 Project Profile、EvidencePack 或 Gate |
| 渐进显露 | 只有风险、团队配置、PR/release 阶段或用户选择触发时，才显露更多控制 |
| 安全独立 | prompt injection、hook/MCP/network/secret/shell/delete/publish 不被 Fast Path 降级 |
| 解释优先 | 每一次提示、验证、确认或阻断都必须解释来源和后果 |
| 用户可决策 | 允许 one-shot/session/worktree 范围的临时放行，但有范围、期限和残余风险 |
| 项目规则复用 | 优先读取 AGENTS.md、CONTRIBUTING、CI、CODEOWNERS、`.github`、`.coderabbit.yaml`、`.gitlab/duo` 等已有资产 |
| 复杂项目友好 | monorepo、多仓库、私有 CI、flaky checks、合规审计、灰度发布和 incident 回流都有升级路径 |
| 技术后台化 | Graph、QDP、EvidencePack、Eval 是支撑能力，不是默认用户流程 |

## 4. 目标用户路径

### 4.1 Fast Path：默认路径

```text
open project / folder
  -> lightweight project understanding
  -> user asks a task
  -> edit / run / inspect
  -> concise result and confidence summary
  -> optional PR summary or next check
```

Fast Path 适合：

- 无 git 临时目录。
- demo、小工具、脚本、文档、探索性改动。
- 个人项目或低风险仓库。
- 用户明确只想快速试验。

Fast Path 不应默认展示 Gate、audit、risk acceptance、Artifact Graph 或完整 EvidencePack。

### 4.2 Contextual Assurance：风险出现时升级

```text
change or action risk appears
  -> explain trigger
  -> suggest checks / sandbox / reviewer / context
  -> keep advisory unless team policy or critical safety says otherwise
```

触发例子：

- 改到核心路径、权限、网络、AI adapter、数据迁移、发布配置。
- 项目规则冲突或关键验证不可运行。
- 用户准备生成 PR。
- 证据过期或上次 review 不再覆盖新 diff。

### 4.3 Team Governance：项目或组织要求时显露

```text
project/team config enabled
  -> apply path rules and review profile
  -> generate change readiness / EvidencePack
  -> required checks / targeted review / risk acceptance
  -> PR or release projection
```

Team Governance 不是默认路径。它来自显式配置、受保护分支、组织策略、合规要求或用户选择。

### 4.4 Security Boundary：始终启用

```text
tool or config wants capability
  -> classify security risk
  -> allow in sandbox / ask / deny / break-glass
  -> record scoped decision
```

Security Boundary 只关注执行安全，不判断质量好坏。详见 [security-boundary.md](architecture/security-boundary.md)。

## 5. 复杂项目来源

BitFun 必须面对的复杂性不是单一“项目等级”，而是多种来源叠加：

| 复杂来源 | 例子 | 产品要求 |
|---|---|---|
| 结构复杂 | monorepo、多语言、多服务、generated code、跨仓库依赖 | 渐进画像和路径级规则 |
| 流程复杂 | issue/spec/PR/release/incident 分散在多个系统 | Artifact Graph 后台关联，按需显露 |
| 验证复杂 | 私有依赖、flaky CI、本地环境不完整、测试分层不清 | 替代验证建议和不可验证解释 |
| 风险复杂 | 权限、网络、secret、迁移、发布、远程 workspace | Security Boundary + guarded profile |
| 团队复杂 | owner、CODEOWNERS、组织策略、合规审计 | Team Governance 和 risk acceptance |
| 时间复杂 | hotfix、临时修复、长程 async task、多 agent 并行 | task-level profile 和工作区隔离 |
| 信息复杂 | 旧文档、冲突规则、prompt injection、未知来源配置 | source/trust/conflict/staleness |

因此，Adaptive Control 不能只挂在 Project Profile 上。项目配置给默认值，当前任务和动作决定运行态。

## 6. 顶层领域模型

| 领域 | 核心对象 | 体验作用 |
|---|---|---|
| Project Understanding | workspace、repo、language、framework、module、rule source、verification capability | 让用户少解释，减少 agent 走错路 |
| Task and Intent | task、mode、stage、user override、session profile | 决定当前是快跑、准备 PR、发布还是应急 |
| Execution Safety | permission、sandbox、network domain、secret access、active config trust、break-glass | 防止 prompt/config/tool 越权 |
| Change Confidence | change summary、verification summary、risk hint、open question、skipped check | 给用户可理解的信心，而不是默认 gate |
| Team Governance | path rules、review profile、required checks、risk acceptance、audit policy | 在团队/强质量场景统一体验 |
| Lifecycle Context | issue、spec、plan、PR、release、incident、learning asset | 支撑复杂项目追溯和复盘 |
| Evaluation and Learning | trace replay、eval task、oracle、feedback、metric | 证明策略真的改善体验和质量 |

## 7. 逻辑架构

```text
User Surfaces
  Fast Workbench / Change Summary / PR Readiness / Release Review / Settings

Adaptive Control Plane
  Intent detection / Control profile / Risk hint / Review profile / Override policy

Security Boundary
  Sandbox / Permission / Network / Secret / Active config trust / Break-glass

Artifact and Evidence Plane
  EvidencePack / Artifact Graph / Finding lifecycle / Risk acceptance

Quality Data Plane
  Lifecycle events / Execution trace / Verification summary / Metrics / Local audit

Project Integration Plane
  Git / CI / Issue / Docs / Release / Observability / Knowledge base

Agent and Tool Runtime
  Sessions / Agents / Terminal / Filesystem / MCP-LSP / Hooks / Adapters
```

关键边界：

- User Surfaces 不直接决定策略，只展示当前 profile 下最少必要信息。
- Adaptive Control Plane 决定体验强度，不执行 shell/network/secret 授权。
- Security Boundary 不关心质量，只执行权限和隔离。
- Artifact/Evidence/QDP 支撑解释和复盘，不应把普通任务变成审计任务。
- Project Integration 只做 adapter，不把外部系统语义泄漏为内部 canonical model。
- Agent Runtime 不自行宣布质量结论或安全豁免。

## 8. 模块边界

| 模块 | 文档 | 产品角色 | 默认显露 |
|---|---|---|---|
| Adaptive Control Profile | [adaptive-control-profile.md](features/adaptive-control-profile.md) | 决定当前任务的控制强度和提示形态 | 是 |
| Security Boundary | [security-boundary.md](architecture/security-boundary.md) | 管执行安全、权限、sandbox、break-glass | 是，但低噪音 |
| Project Profile and Integration | [project-profile-integration.md](architecture/project-profile-integration.md) | 渐进理解项目结构、规则和验证能力 | 部分 |
| Quality Data Plane | [quality-data-plane.md](architecture/quality-data-plane.md) | 记录事件、验证、提示和安全决策 | 否 |
| EvidencePack | [evidence-pack.md](architecture/evidence-pack.md) | 统一证据投影 contract | Fast Path 否，PR/治理时是 |
| Artifact Graph | [artifact-graph.md](architecture/artifact-graph.md) | 关联 diff、验证、PR、issue、release、incident | Fast Path 否，复杂项目按需 |
| Risk Classifier | [risk-classifier.md](features/risk-classifier.md) | 输出风险原因、检查建议和升级信号 | 以提示形式 |
| Change Readiness / PR Gate | [pr-quality-gate.md](features/pr-quality-gate.md) | 生成 PR 信心摘要；强模式下成为 Gate | 仅准备 PR 或配置启用 |
| Requirement Impact Analysis | [requirement-impact-analysis.md](features/requirement-impact-analysis.md) | 高风险需求/API/设计变更的影响候选 | 按需 |
| Agent Evaluation | [agent-evaluation.md](features/agent-evaluation.md) | 评估 agent、context、policy 和控制策略 | 否 |
| OpenCode Compatibility | [opencode-compatibility.md](features/opencode-compatibility.md) | 兼容 hook/plugin/custom tool 生态 | 受 Security Boundary 管控 |

## 9. 配置层级

BitFun 应复用现有生态配置，并提供自己的最小补充：

| 层级 | 例子 | 作用 |
|---|---|---|
| User preference | 用户选择快/严、语言、默认授权范围 | 个体体验默认值 |
| Session/task override | “这次快速放行网络”“本任务只读分析” | 临时控制强度 |
| Workspace config | `.bitfun/config.toml` 或 `.bitfun/quality.yaml` | BitFun 专属 profile、checks、security policy |
| Existing repo rules | AGENTS.md、CONTRIBUTING、CODEOWNERS、CI、`.github/instructions` | 项目知识和规则来源 |
| Tool-specific rules | `.coderabbit.yaml`、`.gitlab/duo/*`、`.kiro/steering/*` | 外部工具和路径级经验 |
| Organization policy | managed config、protected branch、enterprise policy | 强制策略和不可绕过限制 |

优先级原则：

```text
organization deny / managed required
  > security boundary
  > nearest confirmed path or team rule
  > workspace config
  > session/task override within the allowed range
  > global/user default
```

这条优先级只描述冲突裁决，不代表所有项目规则都天然 blocking。每条规则必须带 enforcement mode：`default`、`advisory`、`required` 或 `blocking`。用户的 task override 可以降低 advisory 噪音、跳过推荐检查或请求 risk acceptance，但不能静默绕过组织 deny、受管控 path rule、required check、Security Boundary 或 protected branch 规则。

## 10. 分阶段产品路径

| 阶段 | 主题 | 用户价值 |
|---|---|---|
| P0 | Fast Path + Security Boundary | 快速完成普通开发任务，同时不放开关键安全风险 |
| P1 | Contextual Assurance | 高风险变更出现时给出清晰提示、验证建议和 PR readiness |
| P2 | Team Governance | 项目/团队可以用配置文件统一规则、review profile 和 required checks |
| P3 | Lifecycle Context | 复杂项目把需求、PR、发布、incident 和回归资产连起来 |
| P4 | Evaluation and Optimization | 用真实任务、反馈和指标持续优化控制策略和 agent 行为 |

## 11. 硬约束

| 约束 | 原因 | 设计要求 |
|---|---|---|
| 默认不强制 Gate | 普通项目和临时任务会被重流程劝退 | Gate 只在 PR/团队/强策略场景显露 |
| 安全不能被 Fast Path 绕过 | prompt injection、secret、network、hook 风险与质量无关 | Security Boundary 独立 enforcement |
| 技术 contract 不能成为默认 UI | 用户不应学习内部架构才能完成任务 | EvidencePack/QDP/Graph 后台化 |
| 未知不等于高风险阻塞 | 新项目初期必然信息不足 | 先提示和建议，只有安全/团队策略才阻断 |
| 用户可临时放行 | 小工具和应急场景需要效率 | break-glass 有范围、期限、记录和撤销 |
| 组织策略可强制 | 企业合规需要不可绕过 | managed policy 高于本地 override |
| 模型输出不是事实 | LLM 适合候选和解释，不适合单独授权或阻断 | 确定性证据、用户决策和 policy 才能改变状态 |

## 12. 全局风险与治理

| 风险 | 后果 | 治理策略 |
|---|---|---|
| 产品默认过重 | 普通用户不采用，临时任务流失 | P0 以 Fast Path 和低摩擦指标验收 |
| 质量能力被弱化成玩具 | 复杂项目无法信任 | Team/Guarded/Regulated 场景保留 EvidencePack、Gate、audit |
| 安全提示太频繁 | 用户养成盲点确认 | sandbox、allowlist、scope、domain policy 降噪 |
| break-glass 被滥用 | 安全边界失效 | 范围、期限、组织禁用、隔离建议和审计 |
| 项目规则被注入污染 | agent 按恶意文档执行 | rule source/trust/staleness 和 hostile instruction 检测 |
| 路径规则冲突 | 同一文件应用互相矛盾策略 | nearest rule + conflict display + manual resolution |
| 图谱和证据过早显露 | 用户体验复杂化 | 只在解释、PR、release、incident 时显性化 |
| 指标只看质量不看效率 | 产品优化方向偏向重管控 | 指标同时覆盖 time-to-first-useful-action、interruption、override、false escalation |

## 13. 成功状态

- 新用户打开普通项目，可以在几分钟内完成一次有用改动，不需要配置强质量流程。
- 当任务触及安全敏感动作，BitFun 能低噪音地说明风险并给出安全放行或隔离路径。
- 当用户准备 PR，BitFun 能给出简洁 change readiness，而不是把内部 EvidencePack 全量暴露。
- 当团队启用规则，BitFun 能稳定执行路径级检查、review profile、required checks 和 risk acceptance。
- 当项目复杂到跨需求、发布和 incident，Artifact Graph 和 EvidencePack 能提供追溯，但不污染普通任务体验。
- 产品指标能同时证明速度、信心、安全和质量，而不是只证明 Gate 更严格。

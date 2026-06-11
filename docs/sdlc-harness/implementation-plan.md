# BitFun 自适应工程开发与控制体验实施计划

> 范围：基于 [design.md](design.md)，定义 BitFun 面向外部目标项目落地 Fast Path、Adaptive Control、Security Boundary、Team Governance 和复杂生命周期能力的阶段路线。
> 目标：先证明默认开发体验简洁快速，再逐步增加上下文信心、团队规则、证据链、图谱、评估和强治理能力。

## 1. 执行定位

本计划不把“强质量保护”作为默认起点。执行主线改为：

```text
Architecture runway
  -> Fast Path
  -> Security Boundary
  -> Contextual Assurance
  -> Team Governance
  -> Lifecycle Context
  -> Evaluation and optimization
```

每个阶段都必须回答两个问题：

1. 这个能力是否让普通开发更快、更清楚、更少中断。
2. 这个能力是否在复杂项目或高风险场景中提供更强控制，而不是把所有项目默认变重。

## 2. 执行原则

| 原则 | 执行含义 |
|---|---|
| 快速路径先行 | P0 以用户完成有用任务为核心，不以 Gate 结果为核心 |
| 安全边界独立 | prompt injection、network、secret、hook/MCP、shell、delete、publish 先落地 |
| 渐进画像 | Project Profile 先识别可运行入口、规则来源和验证能力，不要求完整建模 |
| 提示优先 | 默认 advisory；required/blocking 只来自安全、组织策略或用户显式升级 |
| 配置复用 | 先读取 AGENTS.md、CONTRIBUTING、CI、CODEOWNERS、`.github`、`.coderabbit.yaml`、`.gitlab/duo` |
| 证据后台化 | EvidencePack 先做内部 contract 和 PR/readiness 投影，不作为默认 UI |
| 成本可见 | 每个 review/check/escalation 记录耗时、token、用户中断和跳过原因 |
| 反证驱动 | 一旦提示噪音、误升级、误阻断过高，就收缩策略 |

## 3. 阶段路线图

| 阶段 | 主题 | 阶段成果 | 进入下一阶段条件 |
|---|---|---|---|
| P-1 | 架构和产品边界 | Adaptive Control schema、Security Boundary schema、最小事件、配置优先级 | 能明确安全、质量、项目规则、用户 override 的边界 |
| P0 | Fast Path + Security Boundary | 快速项目打开、轻量项目理解、低噪音安全提示、简洁任务摘要 | 普通任务能低摩擦完成，安全越界不会静默发生 |
| P1 | Contextual Assurance | 风险触发提示、recommended checks、change readiness、可选 targeted review | 高风险变更能解释为什么升级，低风险变更不被拖慢 |
| P2 | Team Governance | repo/path/team 配置、review profile、required checks、PR EvidencePack | 团队规则能统一体验且不污染临时任务 |
| P3 | Lifecycle Context | Artifact Graph、requirement/release/incident linkage、risk acceptance | 复杂项目能追溯需求到发布和事故回流 |
| P4 | Evaluation and Optimization | trace replay、control metrics、strategy A/B、holdout | 控制策略能用速度、质量、安全和成本联合评估 |

## 4. P-1：架构和产品边界

| 交付件 | 内容 | 验收方式 |
|---|---|---|
| Adaptive Control contract | profile、reason、display level、checks、review mode、override option | 能解释每次提示/升级/阻断来源 |
| Security Boundary contract | permission、sandbox、network、secret、active config trust、break-glass | 安全决策不依赖 Gate 或质量策略 |
| Configuration precedence | organization deny/managed required、security、confirmed path/team rule、workspace config、task override、user default | 冲突规则有确定优先级，用户 override 不能绕过强策略 |
| Minimal event registry | `project.opened`、`task.started`、`control.decided`、`security.decided`、`verification.completed` | Fast Path 和安全提示可追踪 |
| Evidence display tiers | none、summary、evidence_refs、full_pack | EvidencePack 不默认污染 UI |
| Product metrics spec | time-to-first-useful-action、interruption rate、false escalation、break-glass rate | 阶段验收不只看质量 |

退出条件：可以描述 P0 所需对象和事件，且每个高权限动作都有 allow/ask/deny/break-glass 的明确路径。

## 5. P0：Fast Path + Security Boundary

### 5.1 阶段目标

让用户打开任意外部项目后，能快速完成一次普通开发任务，并在不理解内部治理术语的情况下获得简洁结果和必要安全保护。

### 5.2 交付件

| 交付件 | 内容 |
|---|---|
| Lightweight Project Understanding | 识别语言、包管理器、常用脚本、git 状态、README/AGENTS/CONTRIBUTING/CI 入口 |
| Fast Task Summary | 展示改动、运行命令、未验证项、下一步建议 |
| Security Boundary v0 | 工作区写、shell、network、secret、delete、cross-root write 的基础决策 |
| Active Config Discovery | 发现 hook/plugin/MCP/custom tool/agent rules，默认未信任 |
| One-shot Break-glass | 单次命令、域名、目录、session 范围的临时放行 |
| Minimal QDP | 记录任务、工具、验证、安全决策和用户 override |

### 5.3 验收成果

- 无 git 临时目录可以完成小工具任务，不要求 PR/Gate。
- 常规项目可以在不配置 `.bitfun` 的情况下识别主要开发命令。
- 安全敏感动作会提示原因、范围和可选隔离路径。
- 用户可以一次性放行低/中风险越界，但不能静默持久化。
- 任务结束时有简洁 confidence summary。

### 5.4 过程风险

| 风险 | 处置 |
|---|---|
| 安全提示太频繁 | 先用 sandbox/allowlist 降噪，再提示 |
| 项目理解不准 | 只把 inferred 结论用于提示，不用于强策略 |
| 用户跳过太多 | 显示残余风险，但不把普通跳过升级成审计 |
| 临时项目被误判成团队项目 | 无显式团队配置时保持 `fast` |

## 6. P1：Contextual Assurance

### 6.1 阶段目标

当任务出现风险或用户准备 PR 时，BitFun 提供上下文信心：为什么这个变更值得额外检查、建议跑什么、跳过会有什么后果。

### 6.2 交付件

| 交付件 | 内容 |
|---|---|
| Adaptive Control Decision | `fast/assist/review/guarded/regulated` 运行态和原因 |
| Risk Hint v0 | 路径、模块、操作、历史信号和安全敏感标签 |
| Recommended Checks | 推荐验证命令、替代验证、不可运行原因 |
| Change Readiness Summary | PR 前摘要：变更、验证、风险、未覆盖项 |
| Targeted Review Trigger | 仅在 high 或 evidence weak medium 时建议 targeted review |
| EvidencePack summary mode | 只生成摘要和 evidence refs，不默认 full pack |

### 6.3 验收成果

- 低风险改动不触发 Deep Review。
- 高风险改动能解释升级原因。
- 检查建议有触发原因和取消条件。
- CI/private env 不可运行时输出替代建议，而不是伪 fail。
- PR 摘要减少 reviewer 追问，但不默认阻塞。

## 7. P2：Team Governance

### 7.1 阶段目标

让团队通过配置文件和现有规则资产统一 BitFun 行为，而不是依赖每个用户手工设置。

### 7.2 交付件

| 交付件 | 内容 |
|---|---|
| `.bitfun/quality.yaml` 或 `bitfun.toml` | profile 默认值、路径规则、required checks、review profile、security policy |
| Existing rules import | AGENTS.md、CONTRIBUTING、CODEOWNERS、CI、`.github/instructions`、`.coderabbit.yaml`、`.gitlab/duo` |
| Path-scoped policy | 核心模块、安全目录、docs、tests、generated code 分别配置 |
| PR EvidencePack projection | Team/Guarded 场景生成可追踪 evidence refs |
| Required checks mode | 只在配置或确定性风险下 required |
| Risk Acceptance | actor、reason、scope、residual risk、expires_at |

### 7.3 验收成果

- 团队能用 repo 配置统一体验。
- 个人临时任务不被团队配置误污染，除非在受管控 workspace 内。
- 路径规则冲突能显示并要求确认。
- required/blocking 有明确来源，不由模型单独触发。

## 8. P3：Lifecycle Context

### 8.1 阶段目标

为复杂项目提供需求、PR、发布、incident、回归资产之间的追溯能力，但只在用户需要解释、发布或复盘时显性化。

### 8.2 交付件

| 交付件 | 内容 |
|---|---|
| Artifact Graph minimal loop | `diff -> verification -> evidence_pack -> PR` |
| Requirement Impact candidates | 高风险需求/API/设计变更影响候选和人工确认 |
| Release Readiness | CI、review、known risk、rollback、telemetry 汇总 |
| Incident-to-Test | incident 回溯 release/PR/diff/test gap |
| Stale Evidence | 新 commit、policy、risk、review scope 变化后失效 |

### 8.3 验收成果

- 普通任务不需要看到图谱。
- 复杂 PR 可以解释证据和关系来源。
- release readiness 能说明可发布、不可发布或已接受风险。
- incident 能转成回归候选或规则更新。

## 9. P4：Evaluation and Optimization

### 9.1 阶段目标

用真实任务和产品指标优化 agent 行为、控制策略和质量治理，避免只优化 benchmark 或只优化强管控。

### 9.2 交付件

| 交付件 | 内容 |
|---|---|
| Trace replay | 回放任务、命令、验证、提示和 override |
| Eval Card | 任务集、oracle、模型/context/tool/policy 版本、成本、安全事件 |
| Control A/B | 比较提示频率、误升级、成功率、用户放行和质量结果 |
| Holdout | 防泄漏任务集和真实项目回归 |
| Strategy calibration | post-merge defect、review blocker、CI failure、user feedback 回流 |

### 9.3 验收成果

- 能证明新策略没有让普通任务变慢。
- 能证明安全提示减少盲目确认。
- 能证明 high-risk 场景质量更稳定。
- 能发现哪些 Gate/Review/Evidence 能力没有产品价值，应下线或降级。

## 10. 看护指标

详细口径见 [metrics-spec.md](governance/metrics-spec.md)。P0/P1 必须优先看这些指标：

- Time to first useful action。
- User interruption rate。
- Security prompt acceptance / dismissal。
- Break-glass rate and scope。
- False escalation rate。
- Low-risk task completion rate。
- Recommended check follow-through。
- PR readiness adoption。
- Required check precision。
- Post-merge defect / review blocker feedback。

## 11. 发布和 PR 策略

- P0/P1 默认不创建强制 GitHub Check；优先本地 summary 和 PR text block。
- Team Governance 开启后，才将 readiness 或 Gate 投影到 PR status/comment。
- blocking 只允许来自 Security Boundary、组织策略、确定性失败或明确 required policy。
- 每次发布 PR 文档必须说明默认体验是否变重，以及如何避免普通项目被重流程影响。

## 12. 关键里程碑

| 里程碑 | 判定标准 |
|---|---|
| M0：边界清晰 | Adaptive Control、Security Boundary、Evidence display tiers 和配置优先级稳定 |
| M1：Fast Path 可用 | 普通项目无需配置即可完成有用开发任务 |
| M2：安全低噪音 | 高风险动作可控，普通命令不频繁打断 |
| M3：上下文信心可解释 | 高风险变更有理由、建议检查和跳过后果 |
| M4：团队规则可复用 | repo/path/team 配置能统一 PR readiness 和 required checks |
| M5：复杂项目可追溯 | EvidencePack/Graph 支撑 PR/release/incident，但不污染 Fast Path |
| M6：策略可评估 | 控制策略能用效率、质量、安全和成本联合衡量 |

## 13. 完成标准

这组能力完成时，BitFun 应表现为：

- 普通开发像轻快助手。
- 高风险动作像可靠安全执行环境。
- 团队项目像可配置协作系统。
- 复杂项目像可追溯工程平台。
- 所有复杂技术都服务于某个清晰体验，而不是为了架构完整性而存在。

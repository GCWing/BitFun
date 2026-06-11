# BitFun 自适应工程体验看护指标规格

> 上游文档：[implementation-plan.md](../implementation-plan.md)
> 用途：把实施计划中的看护指标转成可采样、可解释、可用于阶段评审的 metric spec，避免指标停留在口号层。

## 1. 指标治理原则

- 每个指标必须有 owner、分母、采样窗口、数据来源和适用阶段。
- P0/P1 先收集 baseline，不用指标直接阻塞交付。
- 指标只能用于趋势、校准和阶段退出判断，不能替代具体 evidence。
- 体验、安全、质量和成本必须一起看，不能用单个指标证明策略正确。
- 与质量或安全相关的指标必须能追溯到 EvidencePack、LifecycleEvent、policy version 或 Eval Card。
- 指标口径变化必须记录 version，并保留兼容读取或重新计算策略。

## 2. P0/P1 核心体验与安全指标

| 指标 | 公式 | Owner | 数据来源 | 窗口 | 用途 |
|---|---|---|---|---|---|
| Time to first useful action | 项目打开到首次可见有用结果的中位耗时 | Product Experience | `task.started`、`tool.completed`、`task.completed` | 每周 | 判断 Fast Path 是否真实低摩擦 |
| Low-risk task completion rate | 完成的 low-risk 任务数 / low-risk 任务总数 | Adaptive Control | `control.decided`、`task.completed` | 每周 | 防止低风险场景被强流程拖慢 |
| User interruption rate | 每个任务的确认、提示、阻断次数 | Product Experience | `security.decided`、`control.decided`、UI event | 每周 | 控制提示噪音 |
| False escalation rate | 人工或后验判定无必要的 profile 升级数 / profile 升级总数 | Adaptive Control | `control.decided`、override、feedback | 每两周 | 校准自适应控制 |
| Security prompt acceptance rate | 用户接受安全提示建议的次数 / 安全提示总数 | Security Boundary | `security.decided`、user response | 每周 | 判断提示是否有价值 |
| Break-glass rate and scope | break-glass 次数按 scope/risk 聚合 / 安全提示总数 | Security Boundary | `security.decided`、`user.override.recorded` | 每周 | 发现安全策略过紧或真实绕行需求 |
| Active config unresolved rate | 未确认主动配置关联任务数 / 含主动配置任务总数 | Project Profile | active config events、security events | 每周 | 判断 trust review 是否阻塞体验 |
| Confidence summary coverage | 生成 summary 的完成任务数 / 完成任务总数 | Artifact and Evidence Plane | `confidence.summary.generated` | 每周 | 确保任务结果可解释 |

口径说明：

- low-risk 任务由 Risk and Control Classifier 给出，并需排除已触发安全高风险动作的任务。
- interruption 必须区分 security prompt、quality suggestion、review suggestion 和 required policy。
- break-glass 不是失败指标；它用于发现策略和真实工作之间的张力。
- Active config unresolved rate 上升时，需要区分恶意/未知配置、实现缺口和用户主动禁用。

## 3. P1/P2 上下文信心与团队治理指标

| 指标 | 公式 | Owner | 数据来源 | 窗口 | 用途 |
|---|---|---|---|---|---|
| Recommended check follow-through | 被用户执行或 CI 覆盖的 recommended checks / recommended checks 总数 | Risk Classifier | readiness、verification events | 每两周 | 判断建议是否有行动价值 |
| Required check precision | 人工或后验确认有价值的 required checks / required checks 总数 | Risk Classifier | Gate result、override、review feedback | 每两周 | 校准路径矩阵，减少低价值检查 |
| Required check missing rate | 后验发现应运行但未推荐的 checks / 后验确认需要的 checks 总数 | Risk Classifier | CI failure、review blocker、post-merge defect | 每两周 | 控制 false ready 风险 |
| PR readiness adoption | 使用 readiness summary 的 PR 数 / BitFun 准备的 PR 总数 | Change Readiness | `readiness.generated`、PR projection | 每周 | 衡量 PR 体验价值 |
| Gate degraded rate | `degraded` Gate 数 / Gate projection 总数 | Change Readiness | `gate.projected` | 每周 | 发现 profile、evidence、tool 或 trust model 缺口 |
| Deep Review value rate | 产生有效 finding 或避免缺陷的 Deep Review 数 / Deep Review 总数 | Review System | review events、feedback | 每两周 | 防止高成本 review 泛化 |
| Risk acceptance audit coverage | 有 actor/reason/scope/residual risk 的风险接受数 / 风险接受总数 | Change Readiness | risk acceptance events | 每周 | 确保人工放行可追踪 |

## 4. P3/P4 复杂生命周期与评测指标

| 指标 | 公式 | Owner | 数据来源 | 窗口 | 用途 |
|---|---|---|---|---|---|
| Confirmed link ratio | confirmed graph edges / non-expired graph edges | Artifact Graph | graph edge state | 每两周 | 衡量图谱是否可信 |
| Stale link rate | stale graph edges / graph edges | Artifact Graph | graph edge state、file/check/review changes | 每两周 | 判断图谱刷新是否跟上变更 |
| Impact precision | 被确认有效的 impact candidates / impact candidates 总数 | Requirement Impact Analysis | confirmation queue、review feedback | 每两周 | 降低低价值候选 |
| Impact recall proxy | 后验发现遗漏影响项 / 后验确认影响项总数 | Requirement Impact Analysis | review blocker、incident、manual add | 每月 | 发现高风险漏报 |
| Eval card coverage | 有 Eval Card 的决策任务集 / 用于决策的任务集总数 | Agent Evaluation | eval registry | 每月 | 防止无血缘 eval 进入决策 |
| Holdout contamination rate | 标记污染的 holdout tasks / holdout tasks 总数 | Agent Evaluation | eval lineage、prompt/export logs | 每月 | 防止评测集失效 |
| Replay reproducibility rate | 可在固定环境复现的 replay runs / replay runs 总数 | Agent Evaluation | trace replay result | 每月 | 判断评估基础设施稳定性 |
| Incident-to-regression latency | incident 确认到 regression candidate/test/rule 入库的中位耗时 | Lifecycle Context | incident、graph、eval backlog | 每月 | 衡量右移反馈闭环 |

## 5. 阶段退出建议

这些阈值不是硬编码产品策略，只是阶段评审参考。每个目标项目可以在 Project Profile 或 team policy 中覆盖阈值。

| 阶段 | 建议观察条件 |
|---|---|
| P-1 -> P0 | Adaptive Control、Security Boundary、LifecycleEvent、Evidence display tier 和 metric spec 均有 owner 与版本 |
| P0 -> P1 | Time to first useful action、interruption rate、security prompt response、confidence summary coverage 有 baseline |
| P1 -> P2 | false escalation 可归类；recommended check follow-through 有样本；PR readiness 被实际使用 |
| P2 -> P3 | required check precision 有人工反馈；risk acceptance audit 接近完整；active config unresolved 不再由实现缺口主导 |
| P3 -> P4 | impact precision/recall proxy 可采样；release/incident 证据可回写 graph；Eval Card coverage 接近完整 |

## 6. 不应使用的指标方式

- 不用单次 benchmark 分数证明产品质量。
- 不用 PR cycle time 单独判断 gate 好坏，必须同时看 false ready/block、review feedback、defect 和用户打断。
- 不用 token 成本单独优化策略，必须和质量、风险等级、用户接受度一起看。
- 不把模型生成的“风险摘要数量”当作真实 finding density。
- 不把未确认的 graph edge 计入 confirmed link ratio。
- 不把 break-glass 直接当作安全失败；必须结合风险等级、范围和后验结果判断。

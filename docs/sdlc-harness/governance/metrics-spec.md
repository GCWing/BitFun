# BitFun 生命周期工程看护指标规格

> 上游文档：[implementation-plan.md](../implementation-plan.md)
> 用途：把实施计划中的看护指标转成可采样、可解释、可用于阶段门槛的 metric spec，避免指标停留在口号层。

## 1. 指标治理原则

- 每个指标必须有 owner、分母、采样窗口、数据来源和适用阶段。
- P0/P1 先收集 baseline，不用指标直接阻塞交付。
- 指标只能用于趋势、校准和阶段退出判断，不能替代具体 evidence。
- 与质量或安全相关的指标必须能追溯到 EvidencePack、LifecycleEvent、policy version 或 Eval Card。
- 指标口径变化必须记录 version，并保留兼容读取或重新计算策略。

## 2. P0/P1 核心指标

| 指标 | 公式 | Owner | 数据来源 | 窗口 | 用途 |
|---|---|---|---|---|---|
| EvidencePack coverage | 有 EvidencePack 的目标项目本地 diff 数 / 目标项目本地 diff 总数 | Artifact and Evidence Plane | `evidence_pack.generated`、changeset event | 每周 | 判断 P0 是否覆盖主要 PR 准备流程 |
| Gate degraded rate | `degraded` Gate 数 / Gate 总数 | Lifecycle Control Plane | `gate.completed` | 每周 | 发现 profile、evidence、tool 或 trust model 缺口 |
| Required check precision | 人工或后验确认有价值的 required checks / required checks 总数 | Risk Classifier | Gate result、override、review feedback | 每两周 | 校准路径矩阵，减少低价值检查 |
| Required check missing rate | 后验发现应运行但未推荐的 checks / 后验确认需要的 checks 总数 | Risk Classifier | CI failure、review blocker、post-merge defect | 每两周 | 控制 false pass 风险 |
| Deep Review budget skip rate | 因预算跳过或降级的 Deep Review 数 / 被策略建议的 Deep Review 数 | PR Quality Gate | Gate budget、Deep Review event | 每周 | 衡量成本约束是否过紧 |
| Active config unresolved rate | 未确认主动配置关联的 changed diffs / 含主动配置 diffs 总数 | Project Profile | active config events、EvidencePack | 每周 | 判断 trust review 是否阻塞 P0/P1 使用 |

口径说明：

- “目标项目本地 diff”只统计 BitFun 已打开并生成 Project Profile 的 workspace，不统计 BitFun 自身仓库的内部开发分支，除非该仓库被显式作为目标项目加载。
- `degraded` 不等同失败。连续升高说明证据、profile 或 trust model 仍不成熟。
- Required check precision 必须区分 `blocking`、`recommended` 和 `informational`，不得把所有提示混成一个分母。

## 3. P2/P3 扩展指标

| 指标 | 公式 | Owner | 数据来源 | 窗口 | 用途 |
|---|---|---|---|---|---|
| Confirmed link ratio | confirmed graph edges / non-expired graph edges | Artifact Graph | graph edge state | 每两周 | 衡量图谱是否可信 |
| Stale link rate | stale graph edges / graph edges | Artifact Graph | graph edge state、file/check/review changes | 每两周 | 判断图谱刷新是否跟上变更 |
| Impact precision | 被确认有效的 impact candidates / impact candidates 总数 | Requirement Impact Analysis | confirmation queue、review feedback | 每两周 | 降低低价值候选 |
| Impact recall proxy | 后验发现遗漏影响项 / 后验确认影响项总数 | Requirement Impact Analysis | review blocker、incident、manual add | 每月 | 发现高风险漏报 |
| Eval card coverage | 有 Eval Card 的决策任务集 / 用于决策的任务集总数 | Agent Evaluation | eval registry | 每月 | 防止无血缘 eval 进入决策 |
| Holdout contamination rate | 标记污染的 holdout tasks / holdout tasks 总数 | Agent Evaluation | eval lineage、prompt/export logs | 每月 | 防止评测集失效 |
| Replay reproducibility rate | 可在固定环境复现的 replay runs / replay runs 总数 | Agent Evaluation | trace replay result | 每月 | 判断评估基础设施稳定性 |
| Incident-to-regression latency | incident 确认到 regression candidate/test/rule 入库的中位耗时 | Lifecycle Control Plane | incident、graph、eval backlog | 每月 | 衡量右移反馈闭环 |

## 4. 阶段退出建议

这些阈值不是硬编码产品策略，只是阶段评审参考。每个目标项目可以在 Project Profile 中覆盖阈值。

| 阶段 | 建议观察条件 |
|---|---|
| P-1 -> P0 | EvidencePack schema、LifecycleEvent registry、trust model 和 metric spec 均有 owner 与版本 |
| P0 -> P1 | EvidencePack coverage 达到可观测 baseline；Gate degraded reason 可归类；required check precision 有人工反馈样本 |
| P1 -> P2 | stale evidence 能被稳定触发；active config unresolved rate 不再由实现缺口主导；PR 文本减少重复验证追问 |
| P2 -> P3 | impact precision/recall proxy 可采样；release/incident 证据可回写 graph；Eval Card coverage 接近完整 |

## 5. 不应使用的指标方式

- 不用单次 benchmark 分数证明产品质量。
- 不用 PR cycle time 单独判断 gate 好坏，必须同时看 false pass/block、review feedback 和 defect。
- 不用 token 成本单独优化策略，必须和质量、风险等级、用户接受度一起看。
- 不把模型生成的“风险摘要数量”当作真实 finding density。
- 不把未确认的 graph edge 计入 confirmed link ratio。

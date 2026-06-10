# BitFun 全软件生命周期工程能力与质量保护实施计划

> 范围：基于完整设计体系，定义 BitFun 面向外部目标项目建设生命周期工程能力与质量保护闭环的执行方案、阶段性交付件、成果验收、过程风险、关键里程碑和质量保护方案。
> 目标：在控制复杂度、token 成本、运行耗时和团队采用成本的前提下，把项目画像、证据链、交付物图谱与生命周期质量保护能力分阶段落地。

## 1. 执行定位

本计划不是设计文档索引，也不重新定义架构。设计边界由主设计和子模块设计给出；本计划只回答五个执行问题：

1. 先交付什么。
2. 每个阶段产出什么可验收成果。
3. 哪些过程风险会影响落地。
4. 关键里程碑如何判断是否达成。
5. 如何用质量保护方案控制平台化建设边界。

执行主线固定为：

```text
Architecture runway
  -> Project Profile
  -> PR EvidencePack
  -> Risk Classifier
  -> lightweight PR Gate
  -> Artifact Graph minimal loop
  -> Requirement / Release / Incident lifecycle
  -> Evaluation and optimization loop
```

## 2. 执行原则

| 原则 | 执行含义 |
|---|---|
| 画像先行 | 目标项目结构、规则、验证能力和未知区域必须先进入 Project Profile |
| 小闭环优先 | P0/P1 只围绕目标项目 PR 证据链证明价值，不一次性覆盖全 SDLC |
| 证据优先 | Gate、review、impact analysis、release readiness 都必须能追溯证据 |
| 成本可见 | Deep Review、模型调用、hook、评测任务必须记录 token、耗时和降级原因 |
| Adapter 边界 | OpenCode 兼容只作为 adapter，不改变 BitFun canonical event、artifact、permission model |
| 反证驱动 | 每个阶段都定义失败信号，一旦命中就先收缩范围或修正策略 |
| 本地优先 | 默认本地存储和本地审计，外部导出必须显式配置和可审计 |
| 架构先导 | contract、event、provider registry、trust model 先稳定，再建设高层产品自动化 |

## 3. 阶段路线图

| 阶段 | 主题 | 阶段成果 | 进入下一阶段条件 |
|---|---|---|---|
| P-1 | 架构准备与边界验证 | 能力分类、canonical event、EvidencePack contract、hook trust model、provider registry、metric spec 草案 | 能明确哪些能力属于平台支撑、质量保护或受控扩展 |
| P0 | 目标项目 PR 证据链 MVP | Project Profile、EvidencePack、Risk Classifier、lightweight PR Gate、事件最小集 | 大多数本地改动可生成可解释 gate 结果 |
| P1 | 团队 PR 质量闭环 | PR gate 集成、finding lifecycle、stale evidence、最小 Artifact Graph | PR review 过程能复用证据链并减少人工补充 |
| P2 | 需求到发布闭环 | 需求影响分析、release readiness、incident-to-test | 高风险变更能输出影响面、发布风险和回归补充 |
| P3 | 长期评估与优化 | BitFun Engineering Bench、trace replay、A/B、策略校准 | prompt、tool、context、gate 和 policy 改动可被量化评估并持续优化 |

### 3.1 P-1：架构准备与边界验证

P-1 不交付完整用户闭环，只为 P0 降低架构返工风险。

| 交付件 | 内容 | 验收方式 |
|---|---|---|
| 能力分类表 | 区分工程理解、交付物组织、质量事实、质量保护、受控扩展和长期评估 | 主设计和子模块命名不再把全部能力统称为 Harness |
| Canonical event 草案 | `project.profiled`、`file.changed`、`verification.completed`、`risk.classified`、`gate.completed` 等最小事件 | Quality Data Plane 可表达 P0 EvidencePack |
| EvidencePack contract 草案 | EvidenceReference、EvidencePack status、trust tier、privacy、retention、staleness、risk acceptance | Gate 结果能引用证据包和证据引用而非复制日志 |
| Provider registry 草案 | adapter、tool provider、lifecycle policy provider、quality-control capability pack 注册边界 | 不需要改 Agent Runtime kernel 即可接入 P0 能力 |
| Hook trust model | 项目级 hook/plugin/config 的来源、hash、权限、超时、禁用和审计策略 | 未信任主动配置不得影响执行或 gate |
| Eval data policy | golden set、holdout、线上回放、数据污染标记和 Eval Card 模板 | P3 不需要重做评估数据治理 |
| Metric spec 草案 | 看护指标的公式、分母、窗口、owner 和阶段用途 | 指标可用于阶段评审，而不是事后解释 |

P-1 的退出条件是：P0 所需对象、事件、EvidencePack 和指标能够被 schema 描述，且所有高权限或高成本能力都有降级状态。

## 4. P0：目标项目 PR 证据链 MVP

### 4.1 阶段目标

建立最小可用的 PR 质量控制闭环，让 BitFun 在加载外部目标项目后，能在不改造完整 CI/GitHub App 的前提下，为本地 diff 或 PR 准备阶段生成结构化质量证据。

### 4.2 阶段交付件

| 交付件 | 内容 | 依赖 |
|---|---|---|
| `Project Profile v0` | 项目结构、规则来源、验证能力、owner、未知区域和冲突规则 | Project Profile and Integration |
| `LifecycleEvent` 最小事件集 | project、session、file、tool、verification、risk、gate、review 事件 | Quality Data Plane |
| `EvidencePack v0` | context、change、verification、risk、skipped checks、open risks、risk acceptance contract | EvidencePack + Quality Data Plane |
| `Risk Classifier v0` | 基于 Project Profile 的路径/模块规则、risk tags、required checks、Deep Review profile | Risk Classifier |
| `Lightweight PR Gate` | `pass/warn/fail/degraded`、evidence refs、open risks | PR Quality Gate |
| PR 质量证据块 | 自动生成 PR description 的质量摘要 | EvidencePack + Gate |

### 4.3 验收成果

- 目标项目本地 diff 可生成 EvidencePack。
- Project Profile 能展示项目结构、规则来源、验证能力以及未知或冲突区域。
- Project Profile 能发现主动配置并默认按未信任处理。
- 风险分类结果包含 reason、confidence、evidence 和 override path。
- required checks 可解释为什么需要运行。
- 缺少 evidence store、检查结果过期或上下文不足时输出 `degraded`，不得标记为 `pass`。
- 低风险 PR 不默认触发 full Deep Review。

### 4.4 过程风险

| 风险 | 处置 |
|---|---|
| Project Profile 误判 | inferred 规则先进入 degraded 或人工确认，不参与阻塞性 pass |
| EvidencePack 需要大量人工修正 | 收缩事件源，优先修正 context provenance 和 verification summary |
| required checks 低价值提示过多 | 使用 override 反馈校准路径矩阵 |
| gate 对低风险改动产生不必要阻断 | 调整 fail/warn/degraded 语义，先非阻塞运行一段周期 |
| Deep Review token 成本过高 | P0 只允许 targeted/full review 作为显式风险升级 |
| P-1 contract 不稳定 | P0 只允许 additive schema 变更；破坏性变更必须同步迁移和文档 |

### 4.5 质量保护

- 每个 gate result 必须可追溯到 event id 和 evidence ref。
- PR 文本不得隐藏 skipped checks 和 open risks。
- Gate 逻辑先 rule-based，模型输出只作为说明或候选。
- P0 不接入任意第三方插件写 gate 结论。

## 5. P1：团队 PR 质量闭环

### 5.1 阶段目标

将 P0 的本地验证证据链接入团队 PR 流程，形成 reviewer 可消费、可复跑、可审计的质量闭环。

### 5.2 阶段交付件

| 交付件 | 内容 | 依赖 |
|---|---|---|
| PR Gate 集成 | PR status/comment 或本地 report 到 PR 文本的稳定投影 | P0 Gate |
| Finding Lifecycle | finding 状态、owner、resolution、stale 标记 | Deep Review + PR Gate |
| Stale Evidence 检测 | 新 commit、risk tag 或 required check 变化后标记过期 | Quality Data Plane |
| Artifact Graph minimal loop | `diff -> verification -> evidence_pack -> PR`，并可选连接 issue/spec/review | Artifact Graph |
| OpenCode Compatibility L0/L1 | 常用 hook/event 映射、只读 plugin 能力和 trust review 持久化 | Hook/Event Bus |

### 5.3 验收成果

- PR 能展示 evidence、risk、required checks、Deep Review 状态和 open risks。
- Finding 可以被解决、失效、复跑和审计。
- Artifact Graph 只覆盖最小 PR 证据链，但每条边都有 provenance、confidence、staleness。
- OpenCode 兼容层不影响 BitFun canonical event 和 permission model。

### 5.4 过程风险

| 风险 | 处置 |
|---|---|
| PR cycle time 被拉长 | 将 Deep Review 降级为风险触发，默认优先轻量 gate |
| Artifact Graph 链接低可信 | 暂停扩展节点类型，优先补 edge evidence 和 confirmation |
| 兼容层侵入核心 | 强制 adapter 边界，核心模块不得依赖 OpenCode payload |
| Reviewer 认为 AI finding 精度不足 | 记录 finding precision、dismiss reason 和 human override |

### 5.5 质量保护

- 高风险 PR 才允许 full Deep Review 作为推荐或阻塞要求。
- stale review 不得继续支撑 gate pass。
- plugin 只能产出 evidence 或 recommendation，不能直接写 pass/fail。
- 所有人工 override 必须记录 reason 和 residual risk。

## 6. P2：需求到发布闭环

### 6.1 阶段目标

把质量控制从 PR 扩展到需求变更、发布准备和运行期问题回溯，形成左移和右移并重的生命周期闭环。

### 6.2 阶段交付件

| 交付件 | 内容 | 依赖 |
|---|---|---|
| Requirement Impact Analysis | 需求/API/设计变更影响候选、置信度、required checks | Artifact Graph |
| Spec/Acceptance Artifact | 轻量 spec、验收标准、设计决策和关联 PR | Artifact Graph |
| Release Readiness | CI、review、known risk、rollback、telemetry 汇总 | PR Gate + Quality Data Plane |
| Incident-to-Test | incident 关联 release/PR/diff/test gap，生成回归补充候选 | Artifact Graph + Evaluation |
| Test Quality Gate | coverage delta、flaky risk、mutation subset、测试意图 | Quality Data Plane |

### 6.3 验收成果

- 需求或 API 变更能生成影响候选和人工确认清单。
- release readiness 能解释为什么可发布、哪些风险被接受、如何回滚。
- incident 能回溯到 release、PR、diff、需求和测试缺口。
- 确认/拒绝的影响链接会写回 Artifact Graph。

### 6.4 过程风险

| 风险 | 处置 |
|---|---|
| 影响分析低置信候选过多 | 优先 graph-first retrieval，LLM semantic expansion 只做候选补充 |
| 发布仪表盘只有展示没有决策力 | 每个 readiness item 必须绑定证据和 owner |
| incident 回溯依赖人工记忆 | 强制 release、PR、gate、telemetry 关联 event id |
| 测试质量指标过重 | 对关键模块先试点 mutation subset，不全仓铺开 |

### 6.5 质量保护

- 高风险低置信影响项必须人工确认。
- 需求影响分析不能作为完整事实，只能输出候选、置信度和验证建议。
- 发布放行必须保留 risk acceptance audit。
- incident 复盘至少沉淀一个 regression candidate、rule 或 benchmark task。

## 7. P3：评估和持续优化

### 7.1 阶段目标

建立长期 Agent Evaluation 体系，让 prompt、tool schema、context policy、hook policy、Deep Review profile 和模型组合的变更可以被量化评估。

### 7.2 阶段交付件

| 交付件 | 内容 | 依赖 |
|---|---|---|
| BitFun Engineering Bench | 真实 issue、终端任务、review defect、PR gate、impact analysis 黄金集和 holdout set | Agent Evaluation |
| Trace Replay | 固定输入、工具版本、policy 版本和输出 oracle | Quality Data Plane |
| Policy A/B | prompt、tool schema、context policy、model、budget 对比 | Agent Evaluation |
| Failure Mining | 失败 trace 聚类，生成 rule、test、skill、policy 建议 | Quality Data Plane |
| Adaptive Budget | 按风险和历史成功率动态分配模型与 Deep Review 成本 | Risk Classifier |

### 7.3 验收成果

- 关键 prompt、tool schema、context policy、gate policy 改动可通过固定任务集回放。
- 评测结果同时展示成功率、质量、token、耗时、tool calls 和安全事件。
- 失败可归因到模型、工具、上下文、策略或产品交互。
- 线上缺陷、review blocker 和 override 能进入 eval backlog。

### 7.4 过程风险

| 风险 | 处置 |
|---|---|
| benchmark 过拟合 | 保留 holdout set，记录任务来源和泄漏风险 |
| 无 oracle 评测泛滥 | 无 oracle 任务只能用于探索，不进入决策 |
| 只优化模型忽略上下文、工具和策略 | model、prompt、tool schema、context、policy 版本分开记录 |
| 成本被成功率掩盖 | 所有评测必须同时报告 token、wall-clock、tool call 和 retry |

### 7.5 质量保护

- 每类 eval 必须声明 oracle。
- 公开 benchmark、内部 golden set 和 holdout set 分开管理。
- 高风险自动化任务使用隔离环境和最小权限。
- Evaluation 结论不能直接修改生产策略，必须经过人工批准或灰度。
- 失败样本进入 rule/test/skill 前需要去重和稳定性检查。

## 8. 关键里程碑

| 里程碑 | 判定标准 |
|---|---|
| M0：架构边界可执行 | P0 contract、event、EvidencePack、trust model、provider registry 和 metric spec 有最小定义，且不要求改 Agent Runtime kernel |
| M1：Project Profile 与 EvidencePack 可用 | 目标项目本地 diff 可生成结构化 profile/context/change/verification/risk/open risks |
| M2：Risk Classifier 可解释 | required checks 有触发原因、置信度和 override 反馈 |
| M3：Lightweight Gate 可用 | Gate 输出 `pass/warn/fail/degraded`，且缺证据不误判 pass |
| M4：PR 质量块可复用 | PR 描述能自动生成并减少 reviewer 追问验证信息 |
| M5：Artifact Graph 最小闭环 | `diff -> verification -> evidence_pack -> PR` 可查询、可确认、可失效，并可逐步接入 issue/spec/review |
| M6：Deep Review 成本受控 | token、耗时、scope、skipped context 和 budget 降级可见 |
| M7：需求影响分析可用 | 高风险需求/API 变更可输出影响候选、required checks 和人工确认项 |
| M8：Evaluation 回放可用 | 关键策略变更可通过固定任务集比较质量和成本 |

## 9. 全程质量保护方案

| 保护面 | 方案 |
|---|---|
| 文档与设计一致性 | 每个实现 PR 引用对应设计模块和阶段目标 |
| 行为等价 | 涉及现有 Agent Runtime、Deep Review、工具权限的变更必须有回归测试或行为对比 |
| 数据安全 | event、EvidencePack、plugin output 写入前执行 redaction 和 privacy classification |
| 成本控制 | Deep Review、模型评测、LLM impact analysis 设预算、缓存和降级状态 |
| 插件安全 | OpenCode compatibility 默认最小权限、超时、审计、禁用未知来源 |
| PR 治理 | Gate 必须展示 skipped checks、open risks、override reason 和 residual risk |
| 可回滚性 | 每个阶段能力默认 feature flag 或配置开关，允许降级为只读/只提示 |
| 评估闭环 | post-merge defect、CI failure、review blocker、incident 回流到 calibration/eval backlog |
| 指标治理 | 看护指标必须引用 [metrics-spec.md](governance/metrics-spec.md) 的公式、分母、窗口和 owner |

## 10. 看护指标

| 类别 | 指标 |
|---|---|
| 交付效率 | PR cycle time、lead time、time-to-first-useful-plan |
| 验证质量 | required check precision、CI first-pass rate、flaky rate、rerun count |
| AI 质量 | AI-authored diff 占比、review finding density、post-merge defect rate |
| 质量保护成本 | token per accepted change、Deep Review wall-clock、budget skip rate |
| 审计完整性 | EvidencePack coverage、gate degraded rate、risk acceptance audit coverage |
| 图谱质量 | confirmed link ratio、stale link rate、impact analysis precision/recall |
| 主动配置治理 | untrusted active config rate、trusted hook re-review rate、plugin deny/degraded rate |
| 评估治理 | eval card coverage、holdout contamination rate、replay reproducibility rate |
| 运行闭环 | incident-to-regression-test latency、release rollback readiness |

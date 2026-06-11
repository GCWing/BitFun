# BitFun 子模块设计：交付物图谱

> 上游文档：[design.md](../design.md)
> 模块角色：把目标项目中的任务、需求、设计、代码、测试、评审、CI、发布、运行期和复盘资产建模为可追踪、可失效、可确认的关系层。

## 1. 模块定位

交付物图谱是复杂项目的后台关系层。它让 BitFun 在解释变更、准备 PR、发布、复盘或评估时，回答“这次变更和哪些工程资产有关，证据是否新鲜，关系是谁确认的”。

P0 建立隐藏的最小关系投影，用于支撑证据引用和就绪度；issue、spec、完整审查系统和企业知识图谱在复杂项目场景中逐步接入：

```text
任务 -> diff -> 验证 -> 证据引用 -> 就绪度摘要
```

当用户进入 PR、发布、事故、需求影响或团队治理场景时，再逐步显露图谱视图和人工确认队列。

约束：交付物图谱表达可治理的工程对象和可失效关系；RAG 作为候选召回手段使用，确认状态、证据引用和失效规则由图谱契约维护。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [Atlassian Software Collection](https://www.atlassian.com/collections/software) | 工作项、文档、代码和团队上下文正在图谱化 |
| [Harness Software Delivery Knowledge Graph](https://www.harness.io/blog/knowledge-graphs-for-ai-software-delivery) | 知识图谱必须围绕用例最小建模，并以新鲜度和结果改善衡量价值 |
| [Kiro Specs](https://kiro.dev/docs/specs/) / [Steering](https://kiro.dev/docs/steering/) | spec、steering 和项目规则正在成为 AI 原生工程交付物 |
| [Rovo acceptance criteria 检查](https://support.atlassian.com/rovo/docs/check-acceptance-criteria-in-a-code-review/) | PR 可以检查代码是否满足关联工作项的验收标准 |
| [TraceLLM](https://arxiv.org/html/2602.01253v1) | LLM 可辅助轨迹链接，但需要置信度和人工确认 |

## 3. 范围与边界

范围：

- 建模交付物节点、边、证据和确认状态。
- 支撑就绪度、PR 审计视图、需求变更影响视图、发布就绪度和事故回溯视图。
- 为风险分类器、证据包和评测提供可解释上下文。

边界：

- 目标项目已有的 Jira、Linear、GitHub、CI 或观测系统保留系统主权，BitFun 只做适配和投影。
- P0 使用最小关系投影；完整企业知识图谱属于后续复杂项目能力。
- LLM 推断链接、向量检索结果和相似度召回进入候选状态，人工确认或确定性证据才能生成已确认边。
- 外部系统同步优先按需读取和投影，双向同步作为明确集成能力单独设计。
- 普通任务用户只看到摘要和下一步建议，图谱概念在 PR、发布、事故和复盘场景显露。

## 4. 输入、输出与数据模型

核心节点：

```ts
type ArtifactKind =
  | "task"
  | "issue"
  | "requirement"
  | "acceptance_criteria"
  | "spec"
  | "design_decision"
  | "plan"
  | "diff"
  | "code_symbol"
  | "test"
  | "verification"
  | "review_finding"
  | "ci_check"
  | "release"
  | "incident"
  | "metric"
  | "policy"
  | "evidence_pack";
```

核心边：

```ts
interface ArtifactEdge {
  from: ArtifactId;
  to: ArtifactId;
  relation: string;
  source_event_id: string;
  created_by: "system" | "agent" | "human" | "integration";
  confidence: number;
  evidence: EvidenceReference[];
  last_verified_at: string;
  staleness: "fresh" | "stale" | "unknown";
  confirmation_status: "auto" | "confirmed" | "rejected" | "expired" | "hidden_support";
}
```

核心输出：

- 就绪度关联证据。
- PR 证据包关联交付物。
- 变更影响候选。
- 强制检查种子。
- 过期审查 / 过期链接告警。
- 发布就绪度上下文。
- 事故到测试回归候选链接。

## 5. 核心流程

```text
收集任务和项目交付物
  -> 创建隐藏支撑节点
  -> 推断候选边
  -> 附加来源和置信度
  -> 只在产品上下文需要时显露
  -> 对高风险低置信边要求确认
  -> 将确认结果写回图谱
```

关键视图：

| 视图 | 显露条件 | 用途 |
|---|---|---|
| 就绪度支撑视图 | PR/审查场景 | 展示 diff、验证、风险和未关闭缺口 |
| 需求变更视图 | 明确 spec/API/验收变更 | 展示受影响文件、测试、负责人、发布风险 |
| 任务视图 | 长任务或团队协作 | 展示从 spec 到计划、diff、验证的工作链路 |
| 事故回溯视图 | 运行期问题复盘 | 展示事故、发布、PR、测试缺口和回归补充 |

## 6. 策略与治理

- **边质量优先**：链接少但可信，优先于链接多但不可验证。
- **后台优先**：快速路径下图谱只做支撑，不成为用户流程。
- **语义层优先**：先定义交付物类型、关系、来源、信心和新鲜度，再考虑 RAG 或 embedding 扩展。
- **人工确认**：高风险低置信链接进入待确认状态，不直接影响通过/就绪判断。
- **新鲜度管理**：diff、test、审查、CI 状态变化会触发边新鲜度更新。
- **来源分级**：人工确认 > CI/测试证据 > 静态分析 > LLM 推断。
- **可删除但不可篡改**：错误链接用拒绝/过期状态表达，不静默删除审计事实。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | `task -> diff -> verification -> evidence refs` 隐藏支撑投影 |
| P1 | 就绪度关联证据、过期证据、PR 支撑视图 |
| P2 | issue/spec/审查链接、团队 PR 审计视图 |
| P3 | 需求影响、发布就绪度、事故到测试回归 |
| P4 | 跨团队质量趋势、图谱查询和预测性风险提示 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 图谱边界扩张过快 | P0 只能支撑快速路径和就绪度，不建设完整 SDLC |
| 图谱概念污染默认体验 | 普通任务只展示摘要，不展示图谱 |
| 低质量链接影响判断可信度 | 所有边必须携带信心、证据、新鲜度和确认状态 |
| LLM 链接幻觉 | LLM 只生成候选边，高风险链接必须人工确认 |
| 外部系统同步成本过高 | 默认本地投影和按需导出，双向实时同步作为独立集成能力 |
| 链接过期不可见 | 任何 diff、test、审查、CI 变更都应能标记过期 |
| 团队不使用 | 先嵌入就绪度和审查人工作流，避免单独打开图谱工具 |

## 9. 成功标准

- 快速路径可获得图谱支持但不暴露图谱术语。
- PR/就绪度场景能展示与 diff、验证、证据包相关的可信关系。
- 高风险 PR 能暴露缺失需求、测试、审查或负责人链接。
- 用户可以确认、拒绝或覆盖自动链接。
- 过期链接不会被作为通过/就绪依据。
- 需求影响分析和发布就绪度复用同一图谱模型。

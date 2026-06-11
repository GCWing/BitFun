# BitFun 子模块设计：Artifact Graph

> 上游文档：[design.md](../design.md)
> 模块角色：把目标项目中的任务、需求、设计、代码、测试、评审、CI、发布、运行期和复盘资产建模为可追踪、可失效、可确认的关系层。

## 1. 模块定位

Artifact Graph 是复杂项目的后台关系层，不是 Fast Path 的默认用户界面。它的价值在于让 BitFun 在需要解释、准备 PR、发布、复盘或评估时，能回答“这次变更和哪些工程资产有关，证据是否新鲜，关系是谁确认的”。

P0 不要求目标项目已经有可链接 issue、spec、完整 review 系统或企业知识图谱。P0 只允许建立隐藏的最小关系投影，用于支撑 evidence refs 和 readiness：

```text
task -> diff -> verification -> evidence refs -> readiness summary
```

当用户进入 PR、release、incident、requirement impact 或团队治理场景时，再逐步显露图谱视图和人工确认队列。

对抗性审查后的关键判断是：Artifact Graph 不能退化成“把所有文档和代码向量化后做 RAG”。图谱必须表达可治理的工程对象和可失效的关系；RAG 只能作为候选召回手段，不能替代语义层、证据和确认状态。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [Atlassian Software Collection](https://www.atlassian.com/collections/software) | 工作项、文档、代码和团队上下文正在图谱化 |
| [Harness Software Delivery Knowledge Graph](https://www.harness.io/blog/knowledge-graphs-for-ai-software-delivery) | 知识图谱必须围绕用例最小建模，并以新鲜度和结果改善衡量价值 |
| [Kiro Specs](https://kiro.dev/docs/specs/) / [Steering](https://kiro.dev/docs/steering/) | spec、steering 和项目规则正在成为 AI 原生工程交付物 |
| [Rovo acceptance criteria checks](https://support.atlassian.com/rovo/docs/check-acceptance-criteria-in-a-code-review/) | PR 可以检查代码是否满足 linked work item 的验收标准 |
| [TraceLLM](https://arxiv.org/html/2602.01253v1) | LLM 可辅助 trace links，但需要置信度和人工确认 |

## 3. 范围与非目标

范围：

- 建模 artifact node、edge、evidence 和 confirmation status。
- 支撑 readiness、PR 审计视图、需求变更影响视图、release readiness 和 incident 回溯视图。
- 为 Risk Classifier、EvidencePack 和 Evaluation 提供可解释上下文。

非目标：

- 不替代目标项目已有的 Jira、Linear、GitHub、CI 或 observability。
- 不在 P0 构建完整企业知识图谱。
- 不把 LLM 推断链接视为事实。
- 不要求所有外部系统双向同步。
- 不把向量检索结果直接写成 confirmed graph edge。
- 不让普通任务用户先学习图谱概念。

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

- readiness linked evidence。
- PR EvidencePack linked artifacts。
- change impact candidates。
- required checks seed。
- stale review / stale link warnings。
- release readiness context。
- incident-to-test candidate links。

## 5. 核心流程

```text
collect task and project artifacts
  -> create hidden support nodes
  -> infer candidate edges
  -> attach provenance and confidence
  -> expose only when product context needs it
  -> require confirmation for high-risk low-confidence edges
  -> write confirmations back to graph
```

关键视图：

| 视图 | 显露条件 | 用途 |
|---|---|---|
| Readiness 支撑视图 | PR/review 场景 | 展示 diff、验证、risk、open gaps |
| 需求变更视图 | 明确 spec/API/acceptance 变更 | 展示受影响文件、测试、owner、发布风险 |
| 任务视图 | 长任务或团队协作 | 展示从 spec 到 plan、diff、verification 的工作链路 |
| Incident 回溯视图 | 运行期问题复盘 | 展示 incident、release、PR、test gap 和回归补充 |

## 6. 策略与治理

- **边质量优先**：链接少但可信，优先于链接多但不可验证。
- **后台优先**：Fast Path 下图谱只做支撑，不成为用户流程。
- **语义层优先**：先定义 artifact kind、relation、source、confidence 和 staleness，再考虑 RAG 或 embedding 扩展。
- **人工确认**：高风险低置信链接进入 pending 状态，不直接影响 pass/ready。
- **新鲜度管理**：diff、test、review、CI 状态变化会触发边 staleness 更新。
- **来源分级**：human confirmed > CI/test evidence > static analysis > LLM inference。
- **可删除但不可篡改**：错误链接用 rejected/expired 表达，不静默删除审计事实。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | `task -> diff -> verification -> evidence refs` 隐藏支撑投影 |
| P1 | readiness linked evidence、stale evidence、PR support view |
| P2 | issue/spec/review 链接、team PR audit view |
| P3 | requirement impact、release readiness、incident-to-test |
| P4 | 跨团队质量趋势、图谱查询和预测性风险提示 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 图谱边界扩张过快 | P0 只能支撑 Fast Path 和 readiness，不建设完整 SDLC |
| 图谱概念污染默认体验 | 普通任务只展示 summary，不展示 graph |
| 低质量链接影响判断可信度 | 所有边必须携带 confidence、evidence、staleness、confirmation status |
| LLM 链接幻觉 | LLM 只生成 candidate edge，高风险链接必须人工确认 |
| 外部系统同步成本过高 | 默认本地投影和按需导出，不要求双向实时同步 |
| 链接过期不可见 | 任何 diff、test、review、CI 变更都应能标记 stale |
| 团队不使用 | 先嵌入 readiness 和 reviewer 工作流，避免单独打开图谱工具 |

## 9. 成功标准

- Fast Path 可获得图谱支持但不暴露图谱术语。
- PR/readiness 场景能展示与 diff、verification、EvidencePack 相关的可信关系。
- 高风险 PR 能暴露缺失需求、测试、review 或 owner 链接。
- 用户可以确认、拒绝或覆盖自动链接。
- stale link 不会被作为 pass/ready 的依据。
- 需求影响分析和 release readiness 复用同一图谱模型。

# BitFun 生命周期工程子模块设计：Artifact Graph

> 上游文档：[design.md](../design.md)
> 模块角色：把目标项目中的需求、设计、代码、测试、评审、CI、发布、运行期和复盘资产建模为可追踪交付物图谱。

## 1. 模块定位

Artifact Graph 是 BitFun 面向目标项目的工程上下文层。它不追求一次覆盖全部 SDLC 对象，而是先把高频、高价值、可验证的 PR 证据链建起来，再逐步扩展到需求变更、发布和 incident 回溯。

P0 最小闭环不要求目标项目已经有可链接的 issue、spec 或完整 review 系统；它只需要能把本地变更、验证和 EvidencePack 放进可失效的关系投影：

```text
diff -> verification -> evidence_pack -> PR
```

P1 再把 `issue/spec/review` 链接接入同一图谱，避免为了证明 P0 价值而提前建设完整需求或评审集成。

所有图谱边必须有来源、置信度、新鲜度和确认状态。低置信自动链接不能直接作为审计结论。

对抗性审查后的关键判断是：Artifact Graph 不能退化成“把所有文档和代码向量化后做 RAG”。图谱必须表达可治理的工程对象和可失效的关系；RAG 只能作为候选召回手段，不能替代语义层、证据和确认状态。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [Atlassian Software Collection](https://www.atlassian.com/collections/software) | 工作项、文档、代码和团队上下文正在图谱化 |
| [Harness Software Delivery Knowledge Graph](https://www.harness.io/blog/knowledge-graphs-for-ai-software-delivery) | 知识图谱必须围绕用例最小建模，并以新鲜度和结果改善衡量价值 |
| [Kiro Specs](https://kiro.dev/docs/specs/) / [Steering](https://kiro.dev/docs/steering/) | spec、steering 和项目规则正在成为 AI 原生工程交付物 |
| [Rovo acceptance criteria checks](https://support.atlassian.com/rovo/docs/check-acceptance-criteria-in-a-code-review/) | PR 可以检查代码是否满足 linked work item 的验收标准 |
| [TraceLLM](https://arxiv.org/html/2602.01253v1) | LLM 可辅助 trace links，但需要置信度和人工确认 |
| [LLM-driven requirements change impact analysis](https://arxiv.org/html/2511.00262v1) | 需求变更可驱动影响对象召回，但需衡量召回、精度和人工检查成本 |

## 3. 范围与非目标

范围：

- 建模 artifact node、edge、evidence 和 confirmation status。
- 支撑 PR 审计视图、需求变更影响视图和 incident 回溯视图。
- 为 Risk Classifier 和 PR Gate 提供可解释上下文。

非目标：

- 不替代目标项目已有的 Jira、Linear、GitHub、CI 或 observability。
- 不在 P0 构建完整企业知识图谱。
- 不把 LLM 推断链接视为事实。
- 不要求所有外部系统双向同步。
- 不把向量检索结果直接写成 confirmed graph edge。

## 4. 输入、输出与数据模型

核心节点：

```ts
type ArtifactKind =
  | "issue"
  | "requirement"
  | "acceptance_criteria"
  | "spec"
  | "design_decision"
  | "plan"
  | "diff"
  | "code_symbol"
  | "test"
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
  confirmation_status: "auto" | "confirmed" | "rejected" | "expired";
}
```

核心输出：

- PR EvidencePack linked artifacts。
- change impact candidates。
- required checks seed。
- stale review / stale link warnings。
- release readiness context。
- incident-to-test candidate links。

## 5. 核心流程

```text
collect artifacts and events
  -> create or update nodes
  -> infer candidate edges
  -> attach provenance and confidence
  -> require confirmation for high-risk low-confidence edges
  -> expose views and graph queries
  -> write confirmations back to graph
```

关键视图：

| 视图 | 用途 |
|---|---|
| PR 审计视图 | 展示 PR 关联需求、diff、验证、review、risk、open gaps |
| 需求变更视图 | 展示受影响文件、测试、owner、发布风险和待确认链接 |
| 任务视图 | 展示从 spec 到 plan、diff、verification 的工作链路 |
| Incident 回溯视图 | 展示 incident、release、PR、test gap 和回归补充 |

## 6. 策略与治理

- **边质量优先**：链接少但可信，优先于链接多但不可验证。
- **语义层优先**：先定义 artifact kind、relation、source、confidence 和 staleness，再考虑 RAG 或 embedding 扩展。
- **人工确认**：高风险低置信链接进入 pending 状态，不直接影响 gate pass。
- **新鲜度管理**：diff、test、review、CI 状态变化会触发边 staleness 更新。
- **来源分级**：human confirmed > CI/test evidence > static analysis > LLM inference。
- **可删除但不可篡改**：错误链接用 rejected/expired 表达，不静默删除审计事实。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | `diff -> verification -> evidence_pack -> PR` 最小投影 |
| P1 | issue/spec/review 链接、required checks、stale review、PR audit view |
| P2 | requirement impact、release readiness、incident-to-test |
| P3 | 跨团队质量趋势、知识图谱查询和预测性风险提示 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 图谱边界扩张过快 | P0 只能围绕 PR EvidencePack 验证价值 |
| 低质量链接影响判断可信度 | 所有边必须携带 confidence、evidence、staleness、confirmation status |
| LLM 链接幻觉 | LLM 只生成 candidate edge，高风险链接必须人工确认 |
| 外部系统同步成本过高 | 默认本地投影和按需导出，不要求双向实时同步 |
| 链接过期不可见 | 任何 diff、test、review、CI 变更都应能标记 stale |
| 团队不使用 | 先嵌入 PR 描述和 reviewer 工作流，避免单独打开图谱工具 |

## 9. 成功标准

- 大多数 PR 可生成包含 diff、verification 和 EvidencePack 关系的最小图谱投影。
- 高风险 PR 能暴露缺失需求、测试、review 或 owner 链接。
- 用户可以确认、拒绝或覆盖自动链接。
- stale link 不会被作为 gate pass 的依据。
- 需求影响分析和 PR Gate 都复用同一图谱模型。

# BitFun 子模块设计：Requirement Impact Analysis

> 上游文档：[design.md](../design.md)
> 模块角色：在需求、验收标准、API contract、设计决策、发布或 incident 场景中，召回受影响的代码、测试、文档、owner、release 和运行指标候选。

## 1. 模块定位

Requirement Impact Analysis 是复杂项目和高风险生命周期场景能力，不是 P0 默认能力。它的产品价值是在用户明确处理 spec/API/release/incident，或 Adaptive Control 判断变更可能影响多个工程资产时，给出候选影响面、置信度、证据和验证建议。

它输出的是候选集合，不是完整事实。高风险、低置信链接必须人工确认，确认或拒绝结果写回 [Artifact Graph](../architecture/artifact-graph.md)，用于后续校准。

Fast Path 下，本模块不应打断用户；最多在 summary 中提示“可能需要需求/影响确认”的下一步。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [TraceLLM](https://arxiv.org/html/2602.01253v1) | LLM 可辅助建立 trace links，但需要人工确认和持续校准 |
| [LLM-driven requirements change impact analysis](https://arxiv.org/html/2511.00262v1) | 影响分析应衡量召回、精度和人工检查成本 |
| [Rovo acceptance criteria checks](https://support.atlassian.com/rovo/docs/check-acceptance-criteria-in-a-code-review/) | PR 可检查代码是否满足 linked work item 的验收标准 |
| [Kiro Specs](https://kiro.dev/docs/specs/) | spec 应作为 AI 原生交付物参与实现、测试和追踪 |
| MBSE traceability | 复杂系统中需求、模型、验证之间需要结构化追踪 |

设计约束：

- graph-first retrieval 优先于纯 LLM semantic recall。
- Project Profile 必须说明项目结构、owner、验证能力和未知区域。
- 高风险低置信影响项必须人工确认。
- 输出必须包含 confidence、evidence、recommended/required checks 和 residual risk。
- 确认/拒绝结果必须反馈到 Artifact Graph。
- 不把影响分析作为普通 PR 的默认前置步骤。

## 3. 范围与非目标

范围：

- 识别 changed requirement、acceptance criteria、API contract、design decision、release/incident signal。
- 从 Artifact Graph、code graph、static dependency、history 和 LLM semantic expansion 召回候选。
- 生成 recommended/required checks 和人工确认清单。
- 为 release readiness 和 incident-to-test 提供候选关系。

非目标：

- 不替代需求管理工具。
- 不保证完整影响面。
- 不直接修改代码或测试。
- 不把低置信 LLM 推断作为 pass/ready 依据。
- 不让用户在临时小工具场景处理需求追踪。

## 4. 输入、输出与数据模型

输入：

| 输入 | 示例 |
|---|---|
| Requirement diff | issue 描述、acceptance criteria、spec diff |
| Project Profile | 模块边界、owner、验证能力、未知区域 |
| API contract change | DTO、Tauri command、MCP tool schema、OpenAPI |
| Artifact Graph | requirement -> spec -> diff/test/review links |
| Code graph | file、symbol、imports、tests |
| History | past incidents、review findings、flaky tests |
| Runtime signal | incident、metric、trace/log link、alert |
| Human hints | owner、module、risk tags |

输出：

```ts
interface ImpactCandidate {
  artifact_id: string;
  artifact_type: string;
  confidence: number;
  evidence: EvidenceReference[];
  reason: string;
  recommended_checks: RequiredCheck[];
  required_checks: RequiredCheck[];
  confirmation: "required" | "optional" | "not_required";
  residual_risk?: string;
  user_visible_mode: "hidden_hint" | "summary" | "review_queue" | "release_gate";
}
```

## 5. 核心流程

```text
detect explicit spec/API/release/incident trigger
  -> classify change type and risk
  -> load project profile and unknown areas
  -> retrieve confirmed graph links
  -> expand through static dependency and history
  -> optional LLM semantic expansion
  -> rank candidates by confidence, risk, and inspection cost
  -> generate checks and confirmation queue
  -> update Artifact Graph
```

召回策略：

| 策略 | 作用 |
|---|---|
| Graph-first retrieval | 使用已确认 requirement/spec/file/test/review 链接 |
| Static expansion | 通过 imports、symbol reference、test mapping 扩展 |
| History expansion | 结合 incident、review finding、flaky test、hot file |
| LLM semantic expansion | 在语义相似但无结构链接时生成候选 |

## 6. 显露策略

| 场景 | 行为 |
|---|---|
| Fast Path 普通改动 | 不展示影响分析；最多进入下一步建议 |
| PR readiness 且有 linked issue/spec | 展示 summary 级候选和 recommended checks |
| 高风险 API/schema/contract 变更 | 进入 review queue，要求人工确认关键候选 |
| release readiness | 未确认高风险候选进入 residual risk |
| incident 回溯 | 输出 incident-to-test/regression candidates |

治理规则：

- **置信度分层**：confirmed link > static dependency > historical co-change > LLM candidate。
- **人工确认**：高风险低置信候选进入 confirmation queue。
- **反向学习**：确认、拒绝、遗漏项都写回图谱和 eval backlog。
- **Gate 集成**：未确认的高风险候选不能直接 pass；可进入 degraded 或 residual risk。
- **成本控制**：优先使用图谱和静态分析，LLM semantic expansion 只处理候选不足或歧义场景。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | 不作为默认能力；只保留数据模型和隐藏 hint |
| P1 | PR linked issue/spec 的 summary candidate 和 recommended checks |
| P2 | graph-first impact view、static dependency expansion、确认队列 |
| P3 | release readiness、incident-to-test、LLM semantic candidate |
| P4 | precision/recall calibration、长期需求追踪质量指标 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 影响分析拖慢普通开发 | Fast Path 不展示、不阻塞 |
| 高召回导致低价值候选过多 | 输出必须排序，并显示人工检查成本 |
| 高精度导致漏项 | 对 high-risk area 保留低置信候选和 degraded 状态 |
| LLM 语义幻觉 | LLM 结果只能作为 candidate，需要 evidence 和确认 |
| 影响面过期 | graph edge 需要 staleness；diff/test/review 变化触发更新 |
| 与 readiness 脱节 | 每个 high-risk candidate 必须映射 checks 或 residual risk |
| 团队不确认链接 | 确认动作必须嵌入 PR review、release 或 incident flow |

## 9. 成功标准

- 普通任务不被需求影响分析打断。
- 明确需求或 API 变更能生成可解释影响候选。
- 高风险候选有人工确认路径。
- 确认/拒绝结果写回 Artifact Graph。
- required/recommended checks 能覆盖主要影响类别。
- precision、recall、人工检查成本和误打断率可量化。

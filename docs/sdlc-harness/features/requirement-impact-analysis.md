# BitFun 生命周期工程子模块设计：Requirement Impact Analysis

> 上游文档：[design.md](../design.md)
> 模块角色：在需求、验收标准、API contract 或设计决策变更时，召回受影响的代码、测试、文档、owner、release 和运行指标。

## 1. 模块定位

Requirement Impact Analysis 负责把“需求变更影响了什么”转化为候选影响面、置信度、证据和 required checks。它输出的是候选集合，不是完整事实。高风险、低置信链接必须人工确认，确认或拒绝结果写回 Artifact Graph，用于后续校准。

P0 不做完整需求管理，而是围绕 PR EvidencePack 和 linked artifact 提供影响面提示。

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
- Project Profile 必须先说明项目结构、owner、验证能力和未知区域。
- 高风险低置信影响项必须人工确认。
- 输出必须包含 confidence、evidence、required checks 和 residual risk。
- 确认/拒绝结果必须反馈到 Artifact Graph。

## 3. 范围与非目标

范围：

- 识别 changed requirement、acceptance criteria、API contract、design decision。
- 从 Artifact Graph、code graph、static dependency、history 和 LLM semantic expansion 召回候选。
- 生成 required checks 和人工确认清单。

非目标：

- 不替代需求管理工具。
- 不保证完整影响面。
- 不直接修改代码或测试。
- 不把低置信 LLM 推断作为 gate pass 依据。

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
| Human hints | owner、module、risk tags |

输出：

```ts
interface ImpactCandidate {
  artifact_id: string;
  artifact_type: string;
  confidence: number;
  evidence: EvidenceReference[];
  reason: string;
  required_checks: RequiredCheck[];
  confirmation: "required" | "optional" | "not_required";
  residual_risk?: string;
}
```

## 5. 核心流程

```text
detect changed requirement/API/design
  -> classify change type
  -> load project profile and unknown areas
  -> retrieve confirmed graph links
  -> expand through static dependency and history
  -> optional LLM semantic expansion
  -> rank candidates by confidence and risk
  -> generate required checks
  -> ask human to confirm high-risk links
  -> update Artifact Graph
```

召回策略：

| 策略 | 作用 |
|---|---|
| Graph-first retrieval | 使用已确认 requirement/spec/file/test/review 链接 |
| Static expansion | 通过 imports、symbol reference、test mapping 扩展 |
| History expansion | 结合 incident、review finding、flaky test、hot file |
| LLM semantic expansion | 在语义相似但无结构链接时生成候选 |

## 6. 策略与治理

- **置信度分层**：confirmed link > static dependency > historical co-change > LLM candidate。
- **人工确认**：高风险低置信候选进入 confirmation queue。
- **反向学习**：确认、拒绝、遗漏项都写回图谱和 eval backlog。
- **Gate 集成**：未确认的高风险候选不能直接 pass；可进入 warn/degraded。
- **成本控制**：优先使用图谱和静态分析，LLM semantic expansion 只处理候选不足或歧义场景。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | PR linked artifact、required checks、人工确认清单 |
| P1 | graph-first impact view、static dependency expansion |
| P2 | LLM semantic candidate、precision/recall calibration |
| P3 | release readiness、incident-to-requirement 回溯 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 高召回导致低价值候选过多 | 输出必须排序，并显示人工检查成本 |
| 高精度导致漏项 | 对 high-risk area 保留低置信候选和 degraded 状态 |
| LLM 语义幻觉 | LLM 结果只能作为 candidate，需要 evidence 和确认 |
| 影响面过期 | graph edge 需要 staleness；diff/test/review 变化触发更新 |
| 与 PR Gate 脱节 | 每个 high-risk candidate 必须映射 required checks 或 residual risk |
| 团队不确认链接 | 确认动作必须嵌入 PR review 或 gate flow，而不是独立后台任务 |

## 9. 成功标准

- 需求或 API 变更能生成可解释影响候选。
- 高风险候选有人工确认路径。
- 确认/拒绝结果写回 Artifact Graph。
- required checks 能覆盖主要影响类别。
- precision、recall、人工检查成本可量化。

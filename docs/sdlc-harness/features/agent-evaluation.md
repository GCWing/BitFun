# BitFun 子模块设计：Agent Evaluation

> 上游文档：[design.md](../design.md)
> 模块角色：为 Agent 模式、工具接口、prompt、context、permission、Adaptive Control、Security Boundary、review 和模型组合提供长期评测体系。

## 1. 模块定位

Agent Evaluation 的目标不是只证明模型更会写代码，也不是只证明 Gate 更严格。它要回答：

```text
某次模型、prompt、tool schema、context policy、permission policy、control strategy 或 review strategy 改动，
是否真正提升了用户完成任务的速度、信心、安全性、质量和成本效率？
```

没有 oracle 的 eval 不能用于决策。BitFun 的评测体系必须区分模型能力、工具接口、上下文选择、策略约束、工作流、UI 打断和团队配置影响，避免把 benchmark 分数误认为产品质量。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [SWE-bench](https://github.com/swe-bench/SWE-bench) / [SWE-agent](https://arxiv.org/abs/2405.15793) | 真实 issue 与 Agent-Computer Interface 都影响软件工程任务成功率 |
| [SWE-Bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public) | 更真实、更长程、更复杂代码库暴露出评测集泄漏、任务多样性和测试可靠性问题 |
| [Agentless](https://arxiv.org/abs/2407.01489) | 简单、结构化、可解释 pipeline 是强基线 |
| [Terminal-Bench](https://arxiv.org/abs/2601.11868) / [Terminal-Bench 3.0](https://www.tbench.ai/) | 长程终端任务需要真实环境、可靠 oracle 和任务泄漏防护 |
| [OpenAI Agent Improvement Loop](https://developers.openai.com/cookbook/examples/agents_sdk/agent_improvement_loop) | trace、feedback、eval 和策略变更应形成持续改进闭环 |
| [RovoDev Code Reviewer](https://arxiv.org/html/2601.01129v1) | 在线评估要同时看 resolution、PR cycle time、人类评论负载和错误反馈 |

设计约束：

- 评测必须有明确 oracle。
- 不能用单一 benchmark 代表产品质量。
- 必须记录 token、wall-clock、tool calls、失败原因、安全事件和用户打断。
- 评测集必须防止任务泄漏和过拟合。
- 公开 benchmark、内部 golden set 和私有 holdout 必须分开管理。
- 评测必须覆盖 Fast Path、Adaptive Control、Security Boundary、EvidencePack、Readiness、Gate 和 Graph 的协同效果。
- 每个评测集必须有 Eval Card，记录来源、授权、污染风险、oracle、可复现环境、适用范围和退出条件。

## 3. 范围与非目标

范围：

- 评测 Plan、Debug、Review、Deep Review、Agentic、Mini App、remote、MCP 等模式。
- 评测 tool interface、context policy、hook policy、approval/security policy。
- 评测 Adaptive Control 是否误升级、漏提示或过度打断。
- 评测 Security Boundary 是否降低风险且不制造无意义确认。
- 支持 replay、A/B、regression、failure taxonomy。
- 将失败沉淀为 rule、skill、hook、test、benchmark 或 product policy。

非目标：

- 不声明通用模型能力排名。
- 不用离线 benchmark 替代线上产品指标。
- 不把 human preference 当作唯一 oracle。
- 不在缺少隔离环境时运行高风险自动化任务。
- 不用“更严格”天然代表“更好”。

## 4. 输入、输出与数据模型

输入：

| 输入 | 示例 |
|---|---|
| Task fixture | repo snapshot、issue、expected diff、test command、throwaway tool prompt |
| Project profile | 规则来源、验证能力、owner、未知区域和冲突状态 |
| Trace | model calls、tool calls、file edits、approvals、events |
| Policy version | prompt、tool schema、context policy、control profile、security policy |
| Oracle | test pass、diff expectation、review rubric、human acceptance、security expectation |
| Product metrics | time to first useful action、interruption count、prompt acceptance、false escalation |
| Cost | token、wall-clock、tool time、retry count |
| Dataset lineage | source issue、snapshot、license、privacy、leakage status、holdout label |

输出：

```ts
interface EvalResult {
  task_id: string;
  run_id: string;
  success: boolean;
  oracle: OracleResult;
  product_experience: ProductExperienceScores;
  quality_scores: Record<string, number>;
  control_scores: ControlStrategyScores;
  safety_events: SafetyEvent[];
  cost: CostSnapshot;
  failure_taxonomy: string[];
  trace_ref: EvidenceReference;
  policy_versions: Record<string, string>;
  dataset_lineage: DatasetLineage;
  reproducibility: ReproducibilitySnapshot;
}
```

Eval Card 最小字段：

| 字段 | 说明 |
|---|---|
| task_source | 真实 issue、PR、CI failure、incident、throwaway task、synthetic seed 或公开 benchmark |
| data_rights | 是否可用于内部训练、评测、分享或导出 |
| leakage_risk | 是否曾进入 prompt、文档、公开榜单或训练数据 |
| oracle | 测试、diff expectation、review rubric、human acceptance、安全期望或组合 oracle |
| environment | repo snapshot、依赖缓存、工具版本、系统权限和网络策略 |
| decision_scope | 可用于 blocking、advisory、regression、control calibration 或 exploration 的范围 |
| retirement_policy | 任务过期、泄漏、不可复现或业务不再相关时的退出规则 |

## 5. 核心流程

```text
select task set
  -> prepare isolated environment
  -> run baseline and candidate policy
  -> collect trace, control decisions, security events and evidence
  -> evaluate oracle and rubrics
  -> compare product experience, cost, quality and safety
  -> classify failures
  -> promote fixes into policy/test/skill/product changes
```

任务类型：

| 类型 | Oracle |
|---|---|
| Fast Path task | 是否快速完成有用动作、是否低打断、是否给出合理 summary |
| Project profiling | 是否识别正确结构、规则、验证能力和未知区域 |
| Adaptive Control | 是否选择合理 profile、是否误升级/漏升级 |
| Security Boundary | 是否拦截高风险动作、是否允许合理 break-glass、是否避免无效弹窗 |
| Mode compliance | 是否遵守只读、审批、review 约束 |
| Tool interface | 工具调用是否正确、输出是否被使用 |
| Real issue repair | 测试通过、diff 符合预期、无回归 |
| Deep Review | seeded defect coverage、finding precision、judge consistency |
| Change Readiness / PR Gate | required check precision、false ready/block、degraded handling |
| Hook policy | 权限、redaction、timeout、blocking semantics |

## 6. 策略与治理

- **Baseline 优先**：每个复杂 agent flow 都要与简单结构化 pipeline 比较。
- **体验与质量共同决策**：新策略不能只提高 high-risk 质量，却显著拖慢低风险任务。
- **Trace replay**：失败可在固定输入、固定工具版本和固定 policy 下回放。
- **Holdout 管理**：公开 benchmark、内部 golden set、私有 holdout 和线上回放集分开管理。
- **数据血缘**：每个 task 记录来源、授权、快照、污染状态和适用决策范围。
- **成本约束**：报告必须展示 token、耗时、工具调用、重试和提示次数。
- **安全隔离**：高风险工具、网络、文件系统和插件在 eval 中使用最小权限。
- **线上反馈回流**：review blocker、post-merge defect、override、break-glass、incident 写入评测 backlog。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | Fast Path、Security Boundary、Adaptive Control 的小型 golden set |
| P1 | trace replay、failure taxonomy、成本和打断指标对比 |
| P2 | Change Readiness、Risk Classifier、Deep Review、team policy eval |
| P3 | Artifact Graph、requirement impact、release/incident replay |
| P4 | 模型/策略 A/B、线上反馈回流、regression dashboard 和长期趋势 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 无 oracle 评测 | 每个任务必须声明 oracle，否则只能作为探索样本 |
| benchmark 过拟合 | 保留 holdout set，记录任务来源和泄漏风险 |
| holdout 被污染 | 进入 prompt、文档、人工调参材料或公开榜单的样本必须降级 |
| 公开榜单幻觉 | 公开 benchmark 只用于参考，产品决策必须结合目标项目回放和线上指标 |
| 只看成功率忽略成本 | token、耗时、tool calls、提示次数必须和质量一起比较 |
| 只看强质量忽略体验 | time to first useful action、interruption rate、false escalation 必须进入决策 |
| 混淆模型与工程策略贡献 | model、prompt、tool schema、context、security、control 版本必须分开记录 |
| 人工评分不稳定 | review rubric 需要结构化，并记录 reviewer variance |
| eval 与真实工作脱节 | 从真实 PR、issue、CI failure、incident 和临时任务中抽样 |

## 9. 成功标准

- 关键 prompt、tool schema、context policy、security policy 或 control profile 改动可用固定任务集回放。
- 失败能归类到模型、工具、上下文、策略、安全边界或产品交互问题。
- Evaluation 能解释质量提升是否伴随 token、耗时或用户打断增加。
- Fast Path、Security Boundary、Adaptive Control 和 PR readiness 都有专属 eval。
- 线上缺陷、reviewer blocker、break-glass 和 false escalation 能沉淀为新任务。
- 每个用于决策的任务集都有 Eval Card、数据血缘和污染状态。

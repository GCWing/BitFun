# BitFun 全软件生命周期工程能力与质量保护外部调研

> 范围：围绕 AI IDE、Agentic SDLC、代码评审、Hook/Event、交付物图谱、质量治理和评测体系整理外部产品、论文、标准与趋势信号。
> 用途：作为设计文档的外部证据池。主设计文档不依赖本文叙述，只在文末保留必要参考资料。

## 1. 产品趋势

| 产品/方向 | 核心能力 | 对 BitFun 的启发 |
|---|---|---|
| [OpenAI Codex](https://openai.com/index/introducing-codex/) / [Codex Cloud](https://developers.openai.com/codex/cloud) / [Codex CLI](https://developers.openai.com/codex/cli) / [Codex Hooks](https://developers.openai.com/codex/hooks) | 云端任务、CLI、GitHub review、sandbox、approval、hook、日志/测试证据、hook trust review | 证据包、审批策略、PR 门禁、云/本地隔离、hook lifecycle、trust review 和 eval improvement loop 需要统一产品化 |
| [GitHub Copilot coding agent](https://docs.github.com/en/copilot/concepts/agents/cloud-agent/about-cloud-agent) | issue 到 PR、Actions 后台执行、PR review | 需要 PR/CI 深度集成，并明确 AI review 与 human approval 的治理边界 |
| [Atlassian Software Collection](https://www.atlassian.com/collections/software) / [Rovo Dev](https://www.atlassian.com/software/rovo-dev) | Jira、Confluence、Bitbucket、Pipelines、DX、PR review、acceptance criteria check | 交付物图谱应成为核心能力，而非仅围绕编辑器和代码文件 |
| [Linear](https://linear.app/) | 产品开发系统、AI workflow、PRD 到 PR | 产品开发上下文和任务系统正在成为 Agent 原生工作入口 |
| [Harness](https://www.harness.io/) / [Harness AI](https://developer.harness.io/docs/platform/harness-ai/overview) / [Software Delivery Knowledge Graph](https://www.harness.io/blog/knowledge-graphs-for-ai-software-delivery) | CI/CD、测试、AppSec、SRE、成本优化、软件交付知识图谱 | 质量、安全、发布和运行期需要纳入同一交付系统，但知识图谱应从最小高价值场景开始，保持新鲜度和可验证价值 |
| [Claude Code](https://github.com/anthropics/claude-code) / [Hooks](https://code.claude.com/docs/en/hooks) | 终端 Agent、代码库上下文、事件 hook、多 Agent review | prompt 规则应升级为可执行 hook、policy 和事件总线，并明确阻塞/非阻塞语义 |
| [OpenCode Plugins](https://opencode.ai/docs/plugins/) / [SDK](https://opencode.ai/docs/sdk/) / [Server API](https://opencode.ai/docs/server/) | JS/TS plugin、`tool.execute.before/after`、`session.*`、`permission.*`、`file.*`、`lsp.*`、`shell.env`、custom tools、SSE event stream | 可提供独立 OpenCode API 兼容层，但底层应先建设 BitFun 自己的 Hook/Event Bus |
| [Kiro Specs](https://kiro.dev/docs/specs/) / [Steering](https://kiro.dev/docs/steering/) / [Hooks](https://kiro.dev/docs/hooks/) | spec-driven development、workspace/team steering、agent hooks | spec、steering、agent rules 和 hook 配置应成为核心交付物和主动配置，而非仅作为 prompt 背景材料 |
| [Cursor Bugbot](https://cursor.com/blog/building-bugbot) | PR 级 logic bug、performance、security review | AI 生成速度提高后，PR 级质量回路会成为 IDE 外延能力 |
| [Qodo Code Review](https://docs.qodo.ai/code-review) | 多 Agent review、规则执行、ticket/security compliance | Deep Review 可扩展为 PR 门禁和合规检查 |
| [LangChain Harness Engineering](https://www.langchain.com/blog/improving-deep-agents-with-harness-engineering) | 固定模型下优化 agent 外部工程层显著提升 benchmark | 围绕 agent 的 prompt、context、tool、policy 和 workflow 需要版本化、评测和 A/B |

## 2. 研究和基准趋势

| 研究/基准 | 信号 | 设计启发 |
|---|---|---|
| [SWE-bench](https://github.com/swe-bench/SWE-bench) | 真实 GitHub issue 正成为代码 Agent 评测基础 | BitFun 需要真实 issue 黄金集和长期回归集 |
| [SWE-Bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public) | 针对更长程、更真实、更复杂代码库评估 Agent，并强调评测集泄漏、任务多样性和测试可靠性问题 | BitFun 评测不能只依赖公开 SWE-bench，需要内部 holdout、复杂项目和环境可复现性 |
| [SWE-agent](https://arxiv.org/abs/2405.15793) | Agent-Computer Interface 影响修复能力 | 工具 schema、终端反馈、错误呈现和文件浏览本身是能力杠杆 |
| [Agentless](https://arxiv.org/abs/2407.01489) | 简单、可解释的定位/修复/验证 pipeline 可达到强基线 | 不宜默认采用全自治，结构化 pipeline 应作为质量任务默认形态 |
| [Agentic AI in the SDLC](https://arxiv.org/abs/2604.26275) | 提出 Agentic SDLC 参考架构，并汇总 SWE-bench Verified 和生产力研究信号 | BitFun 的定位应从代码生成扩展到可审计的 Agentic SDLC 控制面 |
| [Terminal-Bench](https://arxiv.org/abs/2601.11868) / [Terminal-Bench 3.0](https://www.tbench.ai/) | 真实终端任务覆盖软件工程、ML、安全、数据科学等场景，并持续推进更难 benchmark | BitFun 适合构建终端任务回放和工具轨迹评测，但评测必须防止任务泄漏和 benchmark 过拟合 |
| [RovoDev Code Reviewer](https://arxiv.org/html/2601.01129v1) | 大规模在线评估显示 AI review 可缩短 PR 周期，但缺少上下文时会产生错误或不可执行反馈 | Deep Review 必须具备上下文完整性、finding 生命周期、反证和预算控制 |
| [AI-enhanced requirements traceability](https://sercuarc.org/wp-content/uploads/2025/09/Legesse_AI_Enhanced_Requirements_Traceability_Using_MBSE_LLM_Complex_Systems.pdf) | LLM 可辅助需求追踪，但需要高置信推荐和人工确认 | 需求到代码追踪应保留 human validation |
| [TraceLLM](https://arxiv.org/html/2602.01253v1) / [LLM-driven requirements change impact analysis](https://arxiv.org/html/2511.00262v1) | LLM 可辅助需求追踪和变更影响分析，但输出仍需成本、召回、精度和人工确认约束 | 需求变更影响面分析应输出候选集合、置信度和验证工作量，而非宣称完整自动结论 |
| [Testing with AI Agents](https://arxiv.org/abs/2603.13724) | AI 已大量参与测试生成，但测试质量需要结构化衡量 | 测试质量保护需要关注质量、稳定性和变异杀伤，而非仅增加测试数量 |
| [NIST SP 800-218A](https://csrc.nist.gov/pubs/sp/800/218/a/final) | 将生成式 AI 和基础模型纳入 SSDF 生命周期实践 | AI 参与开发后，安全开发框架需要覆盖模型、工具、数据、权限和供应链风险 |

## 3. 标准与治理趋势

| 标准/方向 | 信号 | 设计启发 |
|---|---|---|
| [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/concepts/semantic-conventions/) | traces、metrics、logs、profiles 和 resources 需要统一语义命名 | Quality Data Plane 应定义 stable semantic attributes，避免每个模块自造事件字段 |
| [CDEvents](https://cdevents.dev/docs/primer/) | CI/CD 事件强调声明式、松耦合、跨工具互操作 | BitFun 的 lifecycle event 应是 canonical fact，不应变成点对点命令调用 |
| [SLSA Provenance](https://slsa.dev/spec/v0.1/provenance) / [in-toto](https://slsa.dev/blog/2023/05/in-toto-and-slsa) | 构建和供应链证据需要说明 where、when、how，而不是只保存日志 | EvidencePack 应支持 provenance/attestation 引用，为 release readiness 和审计预留接口 |
| [OWASP LLM Top 10](https://owasp.org/www-project-top-10-for-large-language-model-applications/) | prompt injection、敏感信息、供应链、过度代理和模型拒绝服务都是 LLM 应用风险 | Hook/Event、plugin、tool、memory、external adapter 必须默认最小权限、redaction、timeout 和 budget |
| AI coding 成本治理 | 高级模型、AI review、CI/Actions 资源和长上下文都会形成显性成本 | Deep Review、Eval、Hook 和 Agent run 必须将 token、耗时、缓存命中和降级原因作为核心指标 |

## 4. 对抗性审查后的趋势判断

外部趋势共同指向三点：

1. AI IDE 的边界正在向 PR、CI、需求、发布和运行期延伸，单纯代码编辑器不再足以承载完整工程控制面。
2. Agent 能力的可靠性不只由模型决定，还由 hook、policy、evidence、context、permission、eval 和 feedback loop 决定。
3. 全链路工具不宜先建设完整平台形态，应从高频、可验证、可嵌入现有流程的 PR 证据链切入，再逐步扩展到需求、发布和运行期。
4. 先进形态正在从“多 Agent 执行”转向“项目画像、交付物图谱、事件标准、证据可信度、成本预算和人工责任”的组合治理。
5. Benchmark 分数无法直接证明产品质量；真实项目的 holdout、trace replay、oracle、成本和安全事件才是可演进质量保护能力的核心评估资产。
6. Hook、plugin、MCP 和 agent rules 正在把项目配置从静态说明变成主动执行面；先进产品需要把 trust review、权限声明、hash/version、超时和审计作为基础能力。

## 5. 参考资料

- OpenAI: [Codex](https://openai.com/index/introducing-codex/), [Codex agent loop](https://openai.com/index/unrolling-the-codex-agent-loop/), [Codex sandboxing](https://developers.openai.com/codex/concepts/sandboxing), [Codex hooks](https://developers.openai.com/codex/hooks), [Agent improvement loop](https://developers.openai.com/cookbook/examples/agents_sdk/agent_improvement_loop)
- GitHub: [Copilot coding agent](https://docs.github.com/en/copilot/concepts/agents/cloud-agent/about-cloud-agent), [Copilot code review](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review)
- Atlassian: [Software Collection](https://www.atlassian.com/collections/software), [Rovo Dev](https://www.atlassian.com/software/rovo-dev), [Acceptance criteria checks](https://support.atlassian.com/rovo/docs/check-acceptance-criteria-in-a-code-review/), [RovoDev Code Reviewer paper](https://arxiv.org/html/2601.01129v1)
- Linear: [Product development system](https://linear.app/)
- Harness: [AI software delivery platform](https://www.harness.io/), [Harness AI overview](https://developer.harness.io/docs/platform/harness-ai/overview), [Software Delivery Knowledge Graph](https://www.harness.io/blog/knowledge-graphs-for-ai-software-delivery)
- Anthropic: [Claude Code](https://github.com/anthropics/claude-code), [Claude Code Review](https://code.claude.com/docs/en/code-review), [Claude Code hooks](https://code.claude.com/docs/en/hooks)
- OpenCode: [Plugins](https://opencode.ai/docs/plugins/), [SDK](https://opencode.ai/docs/sdk/), [Server API](https://opencode.ai/docs/server/)
- Kiro: [Specs](https://kiro.dev/docs/specs/), [Hooks](https://kiro.dev/docs/hooks/), [Steering](https://kiro.dev/docs/steering/)
- PR review systems: [GitHub Copilot code review](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review), [Cursor Bugbot](https://cursor.com/blog/building-bugbot), [Qodo Code Review](https://docs.qodo.ai/code-review)
- Standards and metrics: [DORA](https://dora.dev/), [SPACE](https://queue.acm.org/detail.cfm?id=3454124), [DevEx](https://queue.acm.org/detail.cfm?id=3595878), [OpenTelemetry semantic conventions](https://opentelemetry.io/docs/concepts/semantic-conventions/), [CDEvents](https://cdevents.dev/docs/primer/), [SLSA provenance](https://slsa.dev/spec/v0.1/provenance), [in-toto and SLSA](https://slsa.dev/blog/2023/05/in-toto-and-slsa), [OWASP LLM Top 10](https://owasp.org/www-project-top-10-for-large-language-model-applications/), [NIST SP 800-218A](https://csrc.nist.gov/pubs/sp/800/218/a/final)
- Research: [SWE-bench](https://github.com/swe-bench/SWE-bench), [SWE-Bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public), [SWE-agent](https://arxiv.org/abs/2405.15793), [Agentless](https://arxiv.org/abs/2407.01489), [Agentic AI in the SDLC](https://arxiv.org/abs/2604.26275), [Terminal-Bench](https://arxiv.org/abs/2601.11868), [Testing with AI Agents](https://arxiv.org/abs/2603.13724), [TraceLLM](https://arxiv.org/html/2602.01253v1), [AI-enhanced requirements traceability](https://sercuarc.org/wp-content/uploads/2025/09/Legesse_AI_Enhanced_Requirements_Traceability_Using_MBSE_LLM_Complex_Systems.pdf), [LLM-driven requirements change impact analysis](https://arxiv.org/html/2511.00262v1)

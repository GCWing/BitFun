# BitFun 自适应工程开发与控制体验外部调研

> 范围：围绕 AI coding agent、repo instructions、权限与沙箱、可选 code review、hook/plugin、交付物图谱、质量治理和评测体系整理外部产品、论文、标准与趋势信号。
> 用途：作为设计文档的外部证据池。主设计文档只提炼必要产品判断，本文保留较完整参考资料。

## 1. 产品趋势

| 产品/方向 | 核心能力 | 对 BitFun 的启发 |
|---|---|---|
| [OpenAI Codex](https://openai.com/index/introducing-codex/) / [Codex Cloud](https://developers.openai.com/codex/cloud) / [Codex CLI](https://developers.openai.com/codex/cli) | 云端任务、CLI、本地/云端执行、AGENTS.md、sandbox、approval、日志/测试证据 | 用户体验应先围绕任务、计划、diff 和批准展开；执行安全与质量治理需要拆开 |
| [Codex approvals/security](https://developers.openai.com/codex/agent-approvals-security) / [Codex hooks](https://developers.openai.com/codex/hooks) | approval mode、sandbox、trusted commands、hook lifecycle、trust review | 安全边界要独立常开；hook 是主动执行面，不应默认可信 |
| [GitHub Copilot coding agent](https://docs.github.com/en/copilot/concepts/agents/cloud-agent/about-cloud-agent) | issue 到 PR、Actions 后台执行、PR review、agent session | async agent 的核心体验仍是任务、计划、变更和 review，而不是把用户拉进治理术语 |
| [GitHub Copilot repository instructions](https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/add-custom-instructions/add-repository-instructions) | 支持 repository instructions、path instructions、AGENTS.md | 项目规则应优先读取现有资产，并按路径和上下文渐进加载 |
| [GitHub Copilot code review](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review) | AI review 提供 comment 和建议 | AI review 默认应是低摩擦 advisory，不天然等同 blocking approval |
| [Claude Code](https://github.com/anthropics/claude-code) / [permissions](https://code.claude.com/docs/en/permissions) / [sandboxing](https://www.anthropic.com/engineering/claude-code-sandboxing) | 终端 Agent、权限配置、sandbox、allow/deny rules | 产品需要用技术隔离减少弹窗，同时保留用户可理解的放行路径 |
| [CodeRabbit configuration](https://docs.coderabbit.ai/reference/configuration) / [path instructions](https://docs.coderabbit.ai/configuration/path-instructions) | review profile、path-specific instructions、rules | 审查强度和路径规则应可配置，默认 profile 不应过度激进 |
| [GitLab Duo custom instructions](https://docs.gitlab.com/user/gitlab_duo/customize_duo/review_instructions/) / [warn mode](https://docs.gitlab.com/user/application_security/policies/merge_request_approval_policies/) | review instructions、approval policies、warn mode | 强策略应先 advisory/warn 校准，再进入 required/blocking |
| [Kiro Specs](https://kiro.dev/docs/specs/) / [Steering](https://kiro.dev/docs/steering/) / [Hooks](https://kiro.dev/docs/hooks/) | spec-driven development、workspace/global/team steering、inclusion mode、agent hooks | 项目知识需要作用域、优先级和加载时机；复杂上下文不应 always-on |
| [Jules](https://jules.google/) | 选择 repo/branch、云端计划、diff、用户批准 | 异步 coding agent 的高体验入口是计划和 diff approval，不是强流程前置 |
| [Atlassian Software Collection](https://www.atlassian.com/collections/software) / [Rovo Dev](https://www.atlassian.com/software/rovo-dev) | Jira、Confluence、Bitbucket、Pipelines、PR review、acceptance criteria check | 复杂项目需要连接任务、文档、代码、CI 和团队上下文，但应按需显露 |
| [Harness](https://www.harness.io/) / [Harness AI](https://developer.harness.io/docs/platform/harness-ai/overview) / [Software Delivery Knowledge Graph](https://www.harness.io/blog/knowledge-graphs-for-ai-software-delivery) | CI/CD、测试、AppSec、SRE、成本优化、软件交付知识图谱 | 知识图谱应从最小高价值场景开始，保持新鲜度和可验证价值 |
| [OpenCode Plugins](https://opencode.ai/docs/plugins/) / [SDK](https://opencode.ai/docs/sdk/) / [Server API](https://opencode.ai/docs/server/) | JS/TS plugin、hook、custom tools、SSE event stream | 可提供兼容层，但底层必须由 BitFun 自己的 permission、policy 和 event model 约束 |
| [Cursor Bugbot](https://cursor.com/blog/building-bugbot) / [Qodo Code Review](https://docs.qodo.ai/code-review) | PR 级 logic bug、security、compliance review | PR review 是重要扩展，但不应成为所有任务的默认入口 |
| [LangChain Harness Engineering](https://www.langchain.com/blog/improving-deep-agents-with-harness-engineering) | 固定模型下优化 agent 外部工程层显著提升 benchmark | prompt、context、tool、policy 和 workflow 是能力杠杆，需要评测和 A/B |

## 2. 研究和基准趋势

| 研究/基准 | 信号 | 设计启发 |
|---|---|---|
| [SWE-bench](https://github.com/swe-bench/SWE-bench) | 真实 GitHub issue 正成为代码 Agent 评测基础 | BitFun 需要真实 issue 黄金集和长期回归集 |
| [SWE-Bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public) | 更长程、更真实、更复杂代码库暴露评测集泄漏、任务多样性和测试可靠性问题 | 评测不能只依赖公开榜单，需要内部 holdout、复杂项目和环境可复现性 |
| [SWE-agent](https://arxiv.org/abs/2405.15793) | Agent-Computer Interface 影响修复能力 | 工具 schema、终端反馈、错误呈现和文件浏览本身是能力杠杆 |
| [Agentless](https://arxiv.org/abs/2407.01489) | 简单、可解释的定位/修复/验证 pipeline 可达到强基线 | 不宜默认采用全自治或强流程；结构化 pipeline 应作为基线 |
| [Agentic AI in the SDLC](https://arxiv.org/abs/2604.26275) | Agentic SDLC 需要从架构、证据、生产力和治理同时评估 | BitFun 可扩展到 SDLC，但必须以产品体验和可验证价值逐步推进 |
| [Terminal-Bench](https://arxiv.org/abs/2601.11868) / [Terminal-Bench 3.0](https://www.tbench.ai/) | 真实终端任务覆盖软件工程、ML、安全、数据科学等场景 | 需要终端任务回放和工具轨迹评测，防止任务泄漏和 benchmark 过拟合 |
| [RovoDev Code Reviewer](https://arxiv.org/html/2601.01129v1) | 在线评估显示 AI review 可缩短 PR 周期，但缺少上下文会产生错误反馈 | Deep Review 必须有上下文完整性、finding 生命周期、反证和预算控制 |
| [TraceLLM](https://arxiv.org/html/2602.01253v1) / [LLM-driven requirements change impact analysis](https://arxiv.org/html/2511.00262v1) | LLM 可辅助需求追踪和变更影响分析，但输出仍需成本、召回、精度和人工确认约束 | 需求变更影响面分析应输出候选集合、置信度和人工检查成本 |
| [Testing with AI Agents](https://arxiv.org/abs/2603.13724) | AI 已大量参与测试生成，但测试质量需要结构化衡量 | 测试质量保护要关注质量、稳定性和变异杀伤，而非仅增加测试数量 |
| [NIST SP 800-218A](https://csrc.nist.gov/pubs/sp/800/218/a/final) | 将生成式 AI 和基础模型纳入 SSDF 生命周期实践 | AI 参与开发后，安全开发框架需要覆盖模型、工具、数据、权限和供应链风险 |

## 3. 标准与治理趋势

| 标准/方向 | 信号 | 设计启发 |
|---|---|---|
| [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/concepts/semantic-conventions/) | traces、metrics、logs、profiles 和 resources 需要统一语义命名 | Quality Data Plane 应定义 stable semantic attributes，避免每个模块自造事件字段 |
| [CDEvents](https://cdevents.dev/docs/primer/) | CI/CD 事件强调声明式、松耦合、跨工具互操作 | BitFun 的 lifecycle event 应是 canonical fact，不应变成点对点命令调用 |
| [SLSA Provenance](https://slsa.dev/spec/v0.1/provenance) / [in-toto](https://slsa.dev/blog/2023/05/in-toto-and-slsa) | 构建和供应链证据需要说明 where、when、how | EvidencePack 应支持 provenance/attestation 引用，为 release readiness 和审计预留接口 |
| [OWASP LLM Top 10](https://owasp.org/www-project-top-10-for-large-language-model-applications/) | prompt injection、敏感信息、供应链、过度代理和模型拒绝服务都是 LLM 应用风险 | Hook/Event、plugin、tool、memory、external adapter 必须默认最小权限、redaction、timeout 和 budget |
| DORA / SPACE / DevEx | 速度、稳定性、协作和开发者体验需要联合衡量 | 指标体系不能只看质量门禁，要同时看速度、打断、信心、安全和成本 |
| AI coding 成本治理 | 高级模型、AI review、CI/Actions 资源和长上下文都会形成显性成本 | Deep Review、Eval、Hook 和 Agent run 必须将 token、耗时、缓存命中和降级原因作为核心指标 |

## 4. 对抗性审查后的趋势判断

外部趋势共同指向六点：

1. 默认体验正在走向快速执行、计划、diff、批准和轻量 review，而不是所有任务先进入强质量流程。
2. 项目知识正在产品化为 repo/path/team instructions、steering、AGENTS.md、hook 和 plugin，但这些主动配置必须经过信任和权限边界。
3. AI review 和 Gate 有价值，但先进产品普遍提供 review profile、comment/advisory、warn mode 或 required/blocking 分级。
4. 安全与质量必须分层：prompt injection、network、secret、MCP、hook、shell、跨目录写和删除风险，即使在 Fast Path 也不能静默放行。
5. 复杂项目能力仍然重要，但 Graph、EvidencePack、Requirement Impact 和 Release Readiness 应作为按需显露的后台能力。
6. Benchmark 分数无法直接证明产品质量；真实项目的 holdout、trace replay、oracle、成本、安全事件和用户打断指标才是可演进能力的核心评估资产。

## 5. 参考资料

- OpenAI: [Codex](https://openai.com/index/introducing-codex/), [Codex agent loop](https://openai.com/index/unrolling-the-codex-agent-loop/), [Codex approvals/security](https://developers.openai.com/codex/agent-approvals-security), [Codex sandboxing](https://developers.openai.com/codex/concepts/sandboxing), [Codex hooks](https://developers.openai.com/codex/hooks), [Agent improvement loop](https://developers.openai.com/cookbook/examples/agents_sdk/agent_improvement_loop)
- GitHub: [Copilot coding agent](https://docs.github.com/en/copilot/concepts/agents/cloud-agent/about-cloud-agent), [Copilot repository instructions](https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/add-custom-instructions/add-repository-instructions), [Copilot code review](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review)
- Anthropic: [Claude Code](https://github.com/anthropics/claude-code), [Claude permissions](https://code.claude.com/docs/en/permissions), [Claude Code Review](https://code.claude.com/docs/en/code-review), [Claude Code hooks](https://code.claude.com/docs/en/hooks), [Claude Code sandboxing](https://www.anthropic.com/engineering/claude-code-sandboxing)
- CodeRabbit and GitLab: [CodeRabbit configuration](https://docs.coderabbit.ai/reference/configuration), [CodeRabbit path instructions](https://docs.coderabbit.ai/configuration/path-instructions), [GitLab Duo custom instructions](https://docs.gitlab.com/user/gitlab_duo/customize_duo/review_instructions/), [GitLab approval policies](https://docs.gitlab.com/user/application_security/policies/merge_request_approval_policies/)
- Atlassian: [Software Collection](https://www.atlassian.com/collections/software), [Rovo Dev](https://www.atlassian.com/software/rovo-dev), [Acceptance criteria checks](https://support.atlassian.com/rovo/docs/check-acceptance-criteria-in-a-code-review/), [RovoDev Code Reviewer paper](https://arxiv.org/html/2601.01129v1)
- Linear and Jules: [Linear](https://linear.app/), [Jules](https://jules.google/)
- Harness: [AI software delivery platform](https://www.harness.io/), [Harness AI overview](https://developer.harness.io/docs/platform/harness-ai/overview), [Software Delivery Knowledge Graph](https://www.harness.io/blog/knowledge-graphs-for-ai-software-delivery)
- OpenCode and Kiro: [OpenCode Plugins](https://opencode.ai/docs/plugins/), [OpenCode SDK](https://opencode.ai/docs/sdk/), [OpenCode Server API](https://opencode.ai/docs/server/), [Kiro Specs](https://kiro.dev/docs/specs/), [Kiro Hooks](https://kiro.dev/docs/hooks/), [Kiro Steering](https://kiro.dev/docs/steering/)
- PR review systems: [Cursor Bugbot](https://cursor.com/blog/building-bugbot), [Qodo Code Review](https://docs.qodo.ai/code-review)
- Standards and metrics: [DORA](https://dora.dev/), [SPACE](https://queue.acm.org/detail.cfm?id=3454124), [DevEx](https://queue.acm.org/detail.cfm?id=3595878), [OpenTelemetry semantic conventions](https://opentelemetry.io/docs/concepts/semantic-conventions/), [CDEvents](https://cdevents.dev/docs/primer/), [SLSA provenance](https://slsa.dev/spec/v0.1/provenance), [in-toto and SLSA](https://slsa.dev/blog/2023/05/in-toto-and-slsa), [OWASP LLM Top 10](https://owasp.org/www-project-top-10-for-large-language-model-applications/), [NIST SP 800-218A](https://csrc.nist.gov/pubs/sp/800/218/a/final)
- Research: [SWE-bench](https://github.com/swe-bench/SWE-bench), [SWE-Bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public), [SWE-agent](https://arxiv.org/abs/2405.15793), [Agentless](https://arxiv.org/abs/2407.01489), [Agentic AI in the SDLC](https://arxiv.org/abs/2604.26275), [Terminal-Bench](https://arxiv.org/abs/2601.11868), [Testing with AI Agents](https://arxiv.org/abs/2603.13724), [TraceLLM](https://arxiv.org/html/2602.01253v1), [LLM-driven requirements change impact analysis](https://arxiv.org/html/2511.00262v1)

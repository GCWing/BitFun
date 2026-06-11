# BitFun 可配置开发体验与工程治理外部调研

> 范围：围绕 AI 编码智能体、仓库指令、权限与沙箱、可选代码审查、hook/plugin、交付物图谱、质量治理和评测体系整理外部产品、论文、标准与趋势信号。
> 用途：作为产品需求和架构设计的外部证据池。产品需求文档只提炼必要产品判断，本文保留较完整参考资料。

## 1. 产品趋势

| 产品/方向 | 核心能力 | 对 BitFun 的启发 |
|---|---|---|
| [OpenAI Codex](https://openai.com/index/introducing-codex/) / [Codex Cloud](https://developers.openai.com/codex/cloud) / [Codex CLI](https://developers.openai.com/codex/cli) | 云端任务、CLI、本地/云端执行、AGENTS.md、沙箱、审批、日志/测试证据 | 用户体验应先围绕任务、计划、diff 和批准展开；执行安全与质量治理需要拆开 |
| [Codex approvals/security](https://developers.openai.com/codex/agent-approvals-security) / [Codex hooks](https://developers.openai.com/codex/hooks) | 审批模式、沙箱、可信命令、hook 生命周期、信任审查 | 安全边界独立常驻；hook 按主动执行面管理信任状态 |
| [GitHub Copilot 编码智能体](https://docs.github.com/en/copilot/concepts/agents/cloud-agent/about-cloud-agent) | issue 到 PR、Actions 后台执行、PR 审查、智能体会话 | 异步智能体的核心体验围绕任务、计划、变更和审查组织 |
| [GitHub Copilot 仓库指令](https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/add-custom-instructions/add-repository-instructions) | 支持仓库指令、路径指令、AGENTS.md | 项目规则应优先读取现有资产，并按路径和上下文渐进加载 |
| [GitHub Copilot 代码审查](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review) | AI 审查提供评论和建议 | AI 审查默认应是低摩擦建议模式，不天然等同阻断审批 |
| [Claude Code](https://github.com/anthropics/claude-code) / [权限](https://code.claude.com/docs/en/permissions) / [sandboxing](https://www.anthropic.com/engineering/claude-code-sandboxing) | 终端 Agent、权限配置、沙箱、allow/deny 规则 | 产品需要用技术隔离减少弹窗，同时保留用户可理解的放行路径 |
| [CodeRabbit configuration](https://docs.coderabbit.ai/reference/configuration) / [路径指令](https://docs.coderabbit.ai/configuration/path-instructions) | 审查强度、路径级指令、规则 | 审查强度和路径规则可配置，默认模式降低噪音 |
| [GitLab Duo 自定义指令](https://docs.gitlab.com/user/gitlab_duo/customize_duo/review_instructions/) / [警告模式](https://docs.gitlab.com/user/application_security/policies/merge_request_approval_policies/) | 审查指令、审批策略、警告模式 | 强策略应先建议模式/警告校准，再进入强制要求/阻断 |
| [Kiro Specs](https://kiro.dev/docs/specs/) / [Steering](https://kiro.dev/docs/steering/) / [Hooks](https://kiro.dev/docs/hooks/) | spec 驱动开发、工作区/全局/团队 steering、加载范围模式、智能体 hooks | 项目知识需要作用域、优先级和加载时机；复杂上下文按条件加载 |
| [Jules](https://jules.google/) | 选择仓库/分支、云端计划、diff、用户批准 | 异步编码智能体的高体验入口是计划和 diff 审批 |
| [Atlassian Software Collection](https://www.atlassian.com/collections/software) / [Rovo Dev](https://www.atlassian.com/software/rovo-dev) | Jira、Confluence、Bitbucket、Pipelines、PR 审查、acceptance criteria 检查 | 复杂项目需要连接任务、文档、代码、CI 和团队上下文，但应按需显露 |
| [Harness](https://www.harness.io/) / [Harness AI](https://developer.harness.io/docs/platform/harness-ai/overview) / [Software Delivery Knowledge Graph](https://www.harness.io/blog/knowledge-graphs-for-ai-software-delivery) | CI/CD、测试、AppSec、SRE、成本优化、软件交付知识图谱 | 知识图谱应从最小高价值场景开始，保持新鲜度和可验证价值 |
| [OpenCode Plugins](https://opencode.ai/docs/plugins/) / [SDK](https://opencode.ai/docs/sdk/) / [Server API](https://opencode.ai/docs/server/) | JS/TS plugin、hook、自定义工具、SSE 事件流 | 可提供兼容层，但底层必须由 BitFun 自己的权限、策略和事件模型约束 |
| [Cursor Bugbot](https://cursor.com/blog/building-bugbot) / [Qodo Code Review](https://docs.qodo.ai/code-review) | PR 级逻辑缺陷、安全、合规审查 | PR 审查是团队和高风险场景的重要扩展 |
| [LangChain Harness Engineering](https://www.langchain.com/blog/improving-deep-agents-with-harness-engineering) | 固定模型下优化智能体外部工程层显著提升基准测试表现 | prompt、上下文、工具、策略和工作流是能力杠杆，需要评测和 A/B |

## 2. 研究和基准趋势

| 研究/基准 | 信号 | 设计启发 |
|---|---|---|
| [SWE-bench](https://github.com/swe-bench/SWE-bench) | 真实 GitHub issue 正成为代码 Agent 评测基础 | BitFun 需要真实 issue 黄金集和长期回归集 |
| [SWE-Bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public) | 更长程、更真实、更复杂代码库暴露评测集泄漏、任务多样性和测试可靠性问题 | 公开榜单、内部保留集、复杂项目和环境可复现性需要组合评估 |
| [SWE-agent](https://arxiv.org/abs/2405.15793) | 智能体-计算机接口影响修复能力 | 工具结构、终端反馈、错误呈现和文件浏览本身是能力杠杆 |
| [Agentless](https://arxiv.org/abs/2407.01489) | 简单、可解释的定位/修复/验证流程可达到强基线 | 不宜默认采用全自治或强流程；结构化流程应作为基线 |
| [Agentic AI in the SDLC](https://arxiv.org/abs/2604.26275) | Agentic SDLC 需要从架构、证据、生产力和治理同时评估 | BitFun 可扩展到 SDLC，但必须以产品体验和可验证价值逐步推进 |
| [Terminal-Bench](https://arxiv.org/abs/2601.11868) / [Terminal-Bench 3.0](https://www.tbench.ai/) | 真实终端任务覆盖软件工程、ML、安全、数据科学等场景 | 需要终端任务回放和工具轨迹评测，防止任务泄漏和基准测试过拟合 |
| [RovoDev Code Reviewer](https://arxiv.org/html/2601.01129v1) | 在线评估显示 AI 审查可缩短 PR 周期，但缺少上下文会产生错误反馈 | 深度审查必须有上下文完整性、问题生命周期、反证和预算控制 |
| [TraceLLM](https://arxiv.org/html/2602.01253v1) / [LLM-driven requirements change impact analysis](https://arxiv.org/html/2511.00262v1) | LLM 可辅助需求追踪和变更影响分析，但输出仍需成本、召回、精度和人工确认约束 | 需求变更影响面分析应输出候选集合、置信度和人工检查成本 |
| [Testing with AI Agents](https://arxiv.org/abs/2603.13724) | AI 已大量参与测试生成，但测试质量需要结构化衡量 | 测试质量保护要关注质量、稳定性和变异杀伤，而非仅增加测试数量 |
| [NIST SP 800-218A](https://csrc.nist.gov/pubs/sp/800/218/a/final) | 将生成式 AI 和基础模型纳入 SSDF 生命周期实践 | AI 参与开发后，安全开发框架需要覆盖模型、工具、数据、权限和供应链风险 |

## 3. 标准与治理趋势

| 标准/方向 | 信号 | 设计启发 |
|---|---|---|
| [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/concepts/semantic-conventions/) | 轨迹、指标、日志、画像和资源需要统一语义命名 | 质量数据面应定义稳定语义属性，避免每个模块自造事件字段 |
| [CDEvents](https://cdevents.dev/docs/primer/) | CI/CD 事件强调声明式、松耦合、跨工具互操作 | BitFun 生命周期事件应保持规范事实和松耦合互操作 |
| [SLSA Provenance](https://slsa.dev/spec/v0.1/provenance) / [in-toto](https://slsa.dev/blog/2023/05/in-toto-and-slsa) | 构建和供应链证据需要说明来源、时间和生成方式 | 证据包应支持来源/证明引用，为发布就绪度和审计预留接口 |
| [OWASP LLM Top 10](https://owasp.org/www-project-top-10-for-large-language-model-applications/) | prompt 注入、敏感信息、供应链、过度代理和模型拒绝服务都是 LLM 应用风险 | Hook/Event、plugin、工具、memory、外部适配器必须默认最小权限、脱敏、超时和预算 |
| DORA / SPACE / DevEx | 速度、稳定性、协作和开发者体验需要联合衡量 | 指标体系同时覆盖速度、打断、信心、安全、质量和成本 |
| AI 编码成本治理 | 高级模型、AI 审查、CI/Actions 资源和长上下文都会形成显性成本 | 深度审查、评测、Hook 和智能体运行必须将 token、耗时、缓存命中和降级原因作为核心指标 |

## 4. 对抗性审查后的趋势判断

外部趋势共同指向六点：

1. 默认体验正在走向快速执行、计划、diff、批准和轻量审查。
2. 项目知识正在产品化为仓库/路径/团队指令、steering、AGENTS.md、hook 和 plugin，但这些主动配置必须经过信任和权限边界。
3. AI 审查和门禁有价值，但先进产品普遍提供审查强度、评论/建议模式、警告模式或强制要求/阻断分级。
4. 安全与质量必须分层：prompt 注入、网络、凭据、MCP、hook、shell、跨目录写和删除风险在快速路径中也需要明确授权和可审计记录。
5. 复杂项目能力仍然重要，但图谱、证据包、需求影响和发布就绪度应作为按需显露的后台能力。
6. 基准测试分数无法直接证明产品质量；真实项目的保留集、轨迹回放、判定标准、成本、安全事件和用户打断指标才是可演进能力的核心评估资产。

## 5. 参考资料

- OpenAI: [Codex](https://openai.com/index/introducing-codex/), [Codex 智能体 loop](https://openai.com/index/unrolling-the-codex-agent-loop/), [Codex approvals/security](https://developers.openai.com/codex/agent-approvals-security), [Codex sandboxing](https://developers.openai.com/codex/concepts/sandboxing), [Codex hooks](https://developers.openai.com/codex/hooks), [Agent improvement loop](https://developers.openai.com/cookbook/examples/agents_sdk/agent_improvement_loop)
- GitHub: [Copilot 编码智能体](https://docs.github.com/en/copilot/concepts/agents/cloud-agent/about-cloud-agent), [Copilot 仓库指令](https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/add-custom-instructions/add-repository-instructions), [Copilot 代码审查](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review)
- Anthropic: [Claude Code](https://github.com/anthropics/claude-code), [Claude 权限](https://code.claude.com/docs/en/permissions), [Claude Code Review](https://code.claude.com/docs/en/code-review), [Claude Code hooks](https://code.claude.com/docs/en/hooks), [Claude Code sandboxing](https://www.anthropic.com/engineering/claude-code-sandboxing)
- CodeRabbit 与 GitLab: [CodeRabbit configuration](https://docs.coderabbit.ai/reference/configuration), [CodeRabbit 路径指令](https://docs.coderabbit.ai/configuration/path-instructions), [GitLab Duo 自定义指令](https://docs.gitlab.com/user/gitlab_duo/customize_duo/review_instructions/), [GitLab 审批策略](https://docs.gitlab.com/user/application_security/policies/merge_request_approval_policies/)
- Atlassian: [Software Collection](https://www.atlassian.com/collections/software), [Rovo Dev](https://www.atlassian.com/software/rovo-dev), [Acceptance criteria 检查](https://support.atlassian.com/rovo/docs/check-acceptance-criteria-in-a-code-review/), [RovoDev Code Reviewer paper](https://arxiv.org/html/2601.01129v1)
- Linear 与 Jules: [Linear](https://linear.app/), [Jules](https://jules.google/)
- Harness: [AI software delivery platform](https://www.harness.io/), [Harness AI overview](https://developer.harness.io/docs/platform/harness-ai/overview), [Software Delivery Knowledge Graph](https://www.harness.io/blog/knowledge-graphs-for-ai-software-delivery)
- OpenCode 与 Kiro: [OpenCode Plugins](https://opencode.ai/docs/plugins/), [OpenCode SDK](https://opencode.ai/docs/sdk/), [OpenCode Server API](https://opencode.ai/docs/server/), [Kiro Specs](https://kiro.dev/docs/specs/), [Kiro Hooks](https://kiro.dev/docs/hooks/), [Kiro Steering](https://kiro.dev/docs/steering/)
- PR 审查系统: [Cursor Bugbot](https://cursor.com/blog/building-bugbot), [Qodo Code Review](https://docs.qodo.ai/code-review)
- 标准与指标: [DORA](https://dora.dev/), [SPACE](https://queue.acm.org/detail.cfm?id=3454124), [DevEx](https://queue.acm.org/detail.cfm?id=3595878), [OpenTelemetry semantic conventions](https://opentelemetry.io/docs/concepts/semantic-conventions/), [CDEvents](https://cdevents.dev/docs/primer/), [SLSA provenance](https://slsa.dev/spec/v0.1/provenance), [in-toto and SLSA](https://slsa.dev/blog/2023/05/in-toto-and-slsa), [OWASP LLM Top 10](https://owasp.org/www-project-top-10-for-large-language-model-applications/), [NIST SP 800-218A](https://csrc.nist.gov/pubs/sp/800/218/a/final)
- 研究: [SWE-bench](https://github.com/swe-bench/SWE-bench), [SWE-Bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public), [SWE-agent](https://arxiv.org/abs/2405.15793), [Agentless](https://arxiv.org/abs/2407.01489), [Agentic AI in the SDLC](https://arxiv.org/abs/2604.26275), [Terminal-Bench](https://arxiv.org/abs/2601.11868), [Testing with AI Agents](https://arxiv.org/abs/2603.13724), [TraceLLM](https://arxiv.org/html/2602.01253v1), [LLM-driven requirements change impact analysis](https://arxiv.org/html/2511.00262v1)

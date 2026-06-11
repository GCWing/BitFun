# BitFun 自适应工程开发与控制体验总览

> 范围：BitFun 加载外部软件工程后的产品体验、上下文理解、执行安全、渐进质量治理和复杂项目支撑能力。
> 用途：作为拆分后的入口文档。主设计回答产品体验和架构边界，实施计划回答阶段落地，子模块文档回答局部 contract。

## 文档结构

| 文档 | 角色 | 主要内容 |
|---|---|---|
| [research/external-research.md](research/external-research.md) | 调研文档 | 外部产品、论文、标准和趋势信号 |
| [design.md](design.md) | 主设计 | 默认快速开发体验、Adaptive Control、复杂项目来源、架构边界和产品化原则 |
| [implementation-plan.md](implementation-plan.md) | 实施计划 | Fast Path、Contextual Assurance、Team Governance、复杂生命周期能力的阶段落地 |
| [architecture/security-boundary.md](architecture/security-boundary.md) | 安全边界 | prompt injection、hook/MCP/network/secret/shell 等执行安全底线和 break-glass 规则 |
| [features/adaptive-control-profile.md](features/adaptive-control-profile.md) | 自适应控制 | 任务、操作、环境、项目和团队配置如何共同决定提示、验证、审查和阻塞强度 |
| [architecture/evidence-pack.md](architecture/evidence-pack.md) | 证据包设计 | EvidencePack owner、状态、生命周期、风险接受和 PR/Review/replay 投影 contract |
| [governance/metrics-spec.md](governance/metrics-spec.md) | 指标规格 | 开发效率、安全提示、质量治理和阶段退出指标的公式、分母、窗口和 owner |
| [governance/self-governance-notes.md](governance/self-governance-notes.md) | 自身治理说明 | 记录 BitFun 仓库自身作为内部验证项目暴露出的文档、边界和治理问题 |

## 核心判断

BitFun 的默认产品体验不应是强质量流程，而应是简洁、快速、低摩擦的 Agentic Development。用户打开一个项目后，BitFun 首先要帮助用户更快理解、修改、运行和交付，而不是要求用户先理解 EvidencePack、Gate、Artifact Graph 或完整 SDLC 治理。

复杂项目和强治理能力仍然重要，但它们必须按上下文逐步显露：

- 当用户做临时脚本、demo、文档或小工具时，BitFun 应优先保持 Fast Path，只给必要提示。
- 当变更触及核心路径、权限、网络、数据迁移、发布或团队 PR 时，BitFun 应升级到 Contextual Assurance，给出验证、风险和 reviewer 建议。
- 当项目或组织配置启用统一管控时，BitFun 才进入 Team Governance 或 Guarded/Regulated 模式，展示 EvidencePack、required checks、Gate、risk acceptance 和审计。
- 执行安全永远独立于质量治理。prompt 注入、恶意 hook、MCP、network、secret、跨目录写入、删除和发布凭据等风险，即使在 Fast Path 也必须被识别、隔离、提示或要求明确授权。

因此，Harness 不是 BitFun 的产品定位，也不是默认用户体验。Harness 只指 BitFun 内部用于受控执行、证据校验、策略控制和持续评估的支撑能力。主线应从“质量保护平台”调整为：

```text
Fast Path by default
  -> Contextual Assurance when risk appears
  -> Team Governance when project or organization asks for it
  -> Security Boundary always on
```

## 阅读建议

1. 先读调研文档，确认市场正在从单点 AI IDE 走向 repo instructions、path rules、sandbox、async agent 和可选 review/governance。
2. 再读主设计，确认 BitFun 的默认体验、控制强度分层和复杂项目来源。
3. 需要落地顺序时读实施计划。
4. 需要实现 contract 时再读 Adaptive Control、Security Boundary、EvidencePack、QDP、Risk、Gate 等子模块。

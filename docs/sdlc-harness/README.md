# BitFun 全软件生命周期工程能力与质量保护总览

> 范围：BitFun 加载外部软件工程后的目标形态、外部调研、设计体系和实施计划。
> 用途：作为拆分后的入口文档。设计与计划已分离，避免将战略/架构判断和执行路线混在同一篇报告中。

## 文档结构

| 文档 | 角色 | 主要内容 |
|---|---|---|
| [research/external-research.md](research/external-research.md) | 调研文档 | 外部产品、论文、标准和趋势信号 |
| [design.md](design.md) | 设计文档 | BitFun 加载目标项目后的领域模型、逻辑架构、模块边界和风险治理 |
| [implementation-plan.md](implementation-plan.md) | 实施计划 | 阶段性交付件、成果验收、过程风险、关键里程碑和质量保护方案 |
| [architecture/evidence-pack.md](architecture/evidence-pack.md) | 证据包设计 | EvidencePack owner、状态、生命周期、风险接受和 PR/Gate 投影 contract |
| [governance/metrics-spec.md](governance/metrics-spec.md) | 指标规格 | 看护指标的公式、分母、窗口、owner 和阶段退出参考 |
| [governance/self-governance-notes.md](governance/self-governance-notes.md) | 自身治理说明 | 记录 BitFun 仓库自身作为内部验证项目暴露出的文档、边界和治理问题 |

## 核心判断

BitFun 的定位是面向外部目标项目的 Agentic Engineering 工具。用户加载一个软件工程后，BitFun 应能够识别项目画像、连接生命周期交付物、收集执行和验证证据、进行风险分级、给出质量门禁，并把评审、发布和运行期反馈回流成可复用资产。Harness 只作为设计文档中的术语边界，用于指代其中面向质量保护、受控执行、策略控制和持续评估的一组能力；详细定义见 [design.md](design.md) 的“能力分类与术语边界”。

BitFun 自身已有 Agent Runtime、工具调用、Git/终端、MCP/LSP、Deep Review 和本地工程验证基础，这些是产品实现基础，不是主设计的治理对象。主设计围绕“如何让 BitFun 更好支持外部项目的工程理解、质量保护和风险治理需求”展开；BitFun 自身仓库暴露出的文档缺口、模块边界不清或规则不一致问题，应进入独立自身治理文档。

首个可验证价值点仍应收敛在目标项目的 PR EvidencePack、Risk Classifier 和 lightweight gate 上；Project Profile、Artifact Graph、OpenCode 兼容、需求影响分析和 Agent Evaluation 都围绕这个最小闭环逐步扩展，避免过早扩展设计边界。EvidencePack 是独立的证据投影 contract，不应被 Gate、Quality Data Plane 或 PR 文本各自重新定义。

## 阅读建议

1. 先阅读调研文档，了解外部趋势和参考信号。
2. 再阅读设计文档，确认 BitFun 面向目标项目的长期产品形态、架构边界和风险约束。
3. 最后阅读实施计划，确认阶段性交付件、里程碑、过程风险和质量保护方案。
4. 需要评估 BitFun 仓库自身治理问题时，再阅读自身治理说明。

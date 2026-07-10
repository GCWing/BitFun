# BitFun 可配置开发体验追踪矩阵

> 范围：把产品需求、架构设计、功能规格、执行阶段和测试方法连起来，避免关键体验只停留在单份文档中。
> 上游文档：[product-requirements.md](product-requirements.md)、[design.md](design.md)、[implementation-plan.md](implementation-plan.md)

| 关键点 | 产品需求 | 设计承接 | 功能规格 | 执行阶段 | 测试方法 |
|---|---|---|---|---|---|
| 默认低摩擦 | PRD-01、PRD-02 | 快速路径轻量、体验投影层 | 配置化策略画像、项目画像 | P0 | 质量保障要求较低的探索性改动、无配置项目、文档改动任务回放；统计首次有用动作耗时和低风险任务完成率 |
| 内部策略不外露 | PRD-03、PRD-13 | 体验投影层、用户语言稳定 | 配置化策略画像 | P-1、P0 | 检查桌面、命令行、远程、PR 摘要是否只展示任务状态、原因和下一步，不展示内部枚举 |
| 弱提示和弹窗降噪 | PRD-14 | 提示先弱后强、弹窗触发约束 | 配置化策略画像、质量数据面 | P0、P1 | 同类提示合并/延后用例；统计用户打断率、弹窗触发率、重复提示抑制率 |
| 执行安全常驻 | PRD-04 | 安全边界独立 | 安全边界 | P0 | shell、网络、凭据、删除、跨目录写、hook/MCP 用例；验证 allow/ask/deny/应急放行记录 |
| 沙箱边界可见 | PRD-19 | 执行位置和沙箱等级分层 | 安全边界、质量数据面、证据包 | P0、P1、P3 | 本地 shell、远程 SSH、ACP、MCP、MiniApp、插件运行时主机、浏览器/桌面和云端任务用例；验证执行位置、沙箱等级组合、降级原因、授权范围和替代路径 |
| 阶段性用户收益 | PRD-20 | 阶段收益编排、质量一致性检查、阶段风险副作用分析 | 实施计划、指标规格、质量数据面 | P-1、P0-P4 | P0/P1 回放低风险任务、安全确认、远程/不可支持状态和主动配置发现；PR/团队场景作为“不显露、不阻断”的负向用例。P2 起再回放 PR/团队正向闭环 |
| 远程开发适配 | PRD-11、PRD-12、PRD-15 | 本地/远程能力边界、项目集成面 | 项目画像、风险分类器、安全边界 | P0、P1 | SSH、容器、远程工作区、云端任务用例；验证执行位置、路径映射、端口/网络、不可支持状态和替代路径 |
| 团队配置治理 | PRD-05、PRD-06 | 配置层级、组织策略优先级 | 配置化策略画像、PR 门禁 | P2 | `.bitfun`、AGENTS、CODEOWNERS、CI、路径规则冲突用例；验证来源、优先级和覆盖限制 |
| PR 就绪度 | PRD-07 | 证据按需投影、PR 场景显露 | PR 门禁、证据包 | P1、P2 | P1 可 shadow/advisory 生成就绪度摘要；P2 回放准备 PR、受保护分支、不可运行 CI 用例，验证就绪度摘要、未验证项、证据引用和非默认阻断 |
| 复杂项目追溯 | PRD-08 | 交付物图谱、证据可追溯 | 证据包、交付物图谱、需求影响分析 | P3 | 需求变更、发布、事故复盘用例；验证链接来源、新鲜度、人工确认和过期处理 |
| 主动配置信任 | PRD-09 | 项目规则与主动配置分层 | Plugin Runtime Host 与 OpenCode 兼容适配、安全边界 | P0、P2 | hook、plugin、MCP、自定义工具回放用例；验证来源、hash、权限、触发条件和信任状态 |
| 扩展契约 | PRD-16 | Plugin Runtime Host、Event Manifest、Tool ABI、UI descriptor、插件效果候选、内核权威状态 | Plugin Runtime Host 与 OpenCode 兼容适配、质量数据面 | 产品架构 P0、SDLC P-1/P1 | 产品架构 P0 回放 Desktop settings/command + CLI diagnostics 的 OpenCode-compatible plugin 垂直切片；SDLC P1 回放 availability/diagnostics、过期 epoch、重复 event 和候选效果丢弃 |
| 能力/效果模型 | PRD-09、PRD-16、PRD-17 | 能力/效果模型统一、未声明能力受限、策略不写死工具名 | 安全边界、Plugin Runtime Host 与 OpenCode 兼容适配、质量数据面 | P-1、P0、P1 | tool、MCP、skills、插件、hook、内置工具回放用例；验证能力声明、目标对象、数据类别、信任来源、副作用候选和未知能力受限 |
| 工具复写安全 | PRD-17 | 工具复写显式授权、复写表按项目执行域生效 | Plugin Runtime Host 与 OpenCode 兼容适配、安全边界 | P2 | 内置工具复写契约、同名工具候选、权限撤销、hash 变化、远程执行主机变化用例；验证复写不跨项目且仍经安全边界 |
| 关键用例体验 | PRD-18 | 体验投影层、页面/面板布局、跨入口语义一致 | 产品需求、配置化策略画像、质量数据面 | P0、P1、P2 | P0/P1 回放质量保障要求较低的探索性改动、文档改动、安全确认、远程容器和主动配置发现；P2 回放 PR 就绪和受管路径 |
| 产品指标闭环 | PRD-10 | 质量数据面、评测与学习 | 指标规格、智能体评测 | P0-P4 | 指标采样回放、策略 A/B、保留集；验证速度、打断、安全、质量和成本联合解释 |

## 文档边界

| 文档 | 回答的问题 | 不承担的问题 |
|---|---|---|
| [product-requirements.md](product-requirements.md) | 产品目标、用户画像、平台入口差异、关键边界、功能需求、成功指标 | 不定义模块内部数据结构和实现顺序 |
| [design.md](design.md) | 架构边界、领域模型、配置层级、模块职责和硬约束 | 不重复用户画像、产品规格和阶段任务 |
| [implementation-plan.md](implementation-plan.md) | 阶段用户收益、必要技术前置、延期边界、验收条件和过程风险 | 不替代产品需求或模块契约 |
| [research/review-product-experience-benchmark.md](research/review-product-experience-benchmark.md) | 记录统一 Review 合入后的竞品证据、体验差距、候选优化和明确非目标 | 不自动形成正式 PRD、技术方案、阶段承诺、指标或埋点 |
| [product-requirements-agent-workflow-adjustment.md](product-requirements-agent-workflow-adjustment.md) | 提出 workflow、Review、并发 GUI 和成本控制的候选产品调整议题 | 不作为正式 PRD 编号、指标口径、阶段承诺或门禁规则；采纳前必须回填到权威文档 |
| [agent-workflow-staged-plan.md](agent-workflow-staged-plan.md) | 将 workflow、审查、并发和成本控制映射到真实用户场景 | 不定义独立阶段路线，不定义新的核心对象模型；通用任务生命周期、scheduler 和队列状态归 Agent Kernel，Harness 只通过 provider/plan/step 参与编排；DeepReview 的 reviewer 调度只服务 L3 严格审查 |
| [review-verify-implementation-guardrails.md](review-verify-implementation-guardrails.md) | 记录两次 Review 收敛 PR 的已合入行为基线，并约束后续优化不重新扩张概念 | 不定义新的 workflow DSL、任务 scheduler、PR gate 执行器或 Verify 门禁；后续候选需求仍需回填权威 PRD 和实施计划 |
| 子模块文档 | 单模块输入、输出、状态、边界场景和成功标准 | 不重新定义产品定位 |
| [governance/metrics-spec.md](governance/metrics-spec.md) | 指标公式、分母、窗口、负责人和解释边界 | 不直接作为阻断策略 |

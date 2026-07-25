# bitfun-product-capabilities

**路径**: src/crates/assembly/product-capabilities
**描述**: Product capability pack contracts。拥有 provider 中性的产品能力组装事实。

## 核心类型

- `ProductCapabilityId` — 能力 ID 枚举（CodeAgent, DeepReview, DeepResearch, MiniApp, Canvas, VoiceInput）
- `ProductCapabilityPack` — 能力包（ID、所需服务、工具组、harness）
- `ProductCapabilityRegistry` — 能力注册表（const 包列表 + 查询方法）
- `ProductCapabilityAssembly` — 能力组装结果（能力/特性组/服务需求/工具计划/harness）
- `ProductAssemblyPlan` — 产品组装计划（profile + 能力集 + 组装 + 扩展能力）
- `ProductRuntimeAssembly` — 运行时组装（for_profile / product_full 快捷构造）
- `ProductRuntimeParts` — 运行时部件（plan + services + harness registry + plugin runtime）
- `ProductAssembler` — 产品组装器（assemble 方法进行完整校验）
- `DeliveryProfile` — 交付 profile 枚举（ProductFull, Desktop, Cli, Server, Remote, Acp, Web, MobileWeb, Sdk）
- `ProductFeatureGroup` — 特性组（Basic, Git, Mcp, BrowserWeb, ComputerUse, ImageAnalysis, MiniApp, Canvas, AgentControl）
- `ProductAssemblyInput` — 组装输入
- `ProductAssemblyError` — 组装错误枚举
- `ProductServiceCapabilityRequirement/Availability/Status` — 服务能力要求
- `ProductExtensionCapabilitySet` — 扩展能力（插件运行时）
- `ProductCoreDependencyMode` — 核心依赖模式

## 预定义能力包

- `CODE_AGENT_CAPABILITY_PACK` — 代码 agent（FS/Workspace/SessionStore/Events/Clock/Terminal）
- `DEEP_REVIEW_CAPABILITY_PACK` — 深度审核（Workspace/Git/Events）
- `DEEP_RESEARCH_CAPABILITY_PACK` — 深度调研（Workspace/Network/Events）
- `MINIAPP_CAPABILITY_PACK` — 小程序（FS/Workspace/Events）
- `CANVAS_CAPABILITY_PACK` — Canvas（FS/Workspace/SessionStore/Events）
- `VOICE_INPUT_CAPABILITY_PACK` — 语音输入（无服务要求）

## 功能

产品能力组装 crate。定义产品能力的 modulization 方案：每种能力（CodeAgent、DeepReview 等）声明所需 runtime services、tool provider groups 和 harness providers。通过 `DeliveryProfile` 选择不同 profile 的能力集，`ProductAssembler` 校验服务可用性并生成组装结果。用于 CLI、Desktop、ACP 等不同交付形态的能力适配。

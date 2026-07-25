# bitfun-harness

**路径**: src/crates/execution/harness
**描述**: Harness workflow contracts and registry for BitFun。Provider 中性的工作流描述符和注册表。

## 核心类型

- `HarnessWorkflow` — 工作流枚举（Sdd, DeepReview, DeepResearch, MiniApp, FunctionAgent）
- `HarnessCapability` — 工作流能力枚举（Plan, Execute, ReviewGate, Artifact, PostProcessor）
- `HarnessProvider` trait — 工作流提供者（plan + execute 异步方法）
- `HarnessRegistry` — 工作流注册表（按 workflow/provider_id 查询）
- `HarnessRegistryBuilder` — 注册表构建器（install_provider + build）
- `HarnessProviderDescriptor` — Provider 描述符（const 编译期定义）
- `DescriptorHarnessProvider` — 从描述符构造的 provider（legacy_facade 快捷构造）
- `HarnessInput` — 工作流输入（workflow + goal）
- `HarnessPlan` — 工作流执行计划（steps 列表）
- `HarnessStep`, `HarnessStepKind` — 执行步骤（LegacyFacade, AgentRuntime, ToolRuntime 等）
- `HarnessOutcome`, `HarnessOutcomeStatus` — 执行结果
- `HarnessError` — 错误枚举（UnsupportedWorkflow, UnsupportedExecution）
- `HarnessPlanningContext`, `HarnessExecutionContext` — 上下文
- `HarnessRegistryBuildError` — 注册表构建错误
- `HarnessId` — 字符串 ID 包装

## 功能

工作流契约 crate。定义工作流（如 DeepReview、DeepResearch）的执行模型：规划（plan）和执行（execute）两个阶段。提供 const 描述符和运行时 provider 两种注册方式。被 product-capabilities 用于定义产品能力的工作流提供者。当前 execute 在 legacy_facade 路径上返回 UnsupportedExecution，具体执行留在产品路径。

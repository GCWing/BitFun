# bitfun-tool-packs

**路径**: src/crates/execution/tool-provider-groups
**名称**: bitfun-tool-packs（Cargo.toml 中 package name 为 bitfun-tool-packs，lib name 为 bitfun_tool_packs）

**描述**: BitFun concrete tool-pack owner crate。具体工具包 owner crate。

## 核心类型

- `ToolPackFeatureGroup` — 特性组枚举（Basic, Git, Mcp, BrowserWeb, ComputerUse, ImageAnalysis, MiniApp, Canvas, AgentControl）
- `ToolProviderGroupPlan` — 工具提供者组计划（provider_id + feature_groups + tool_names）
- `ToolProviderGroupPlanSelectionError` — 选择错误（UnknownToolProviderGroup）

## 预定义工具组计划

- `core.basic` — 基础文件工具（LS, Read, Write, Edit, Delete, Glob, Grep, ExecCommand 等 14 个）
- `core.agent` — Agent 控制工具（Task, AgentWait, Skill, AskUserQuestion, TodoWrite, CreatePlan 等 14 个）
- `core.canvas` — Canvas 工具（CreateCanvas, ReadCanvas, UpdateCanvas, PatchCanvas）
- `core.session` — Session 管理工具（SessionControl, SessionMessage, SessionHistory, Cron）
- `core.integration` — 集成工具（WebSearch, WebFetch, Git, MCP 工具, ComputerUse, InitMiniApp 等 15 个）

## 预定义特性组映射

| 特性组 | 涉及的 provider groups |
|--------|----------------------|
| basic | core.basic |
| agent-control | core.agent, core.session, core.integration |
| canvas | core.canvas |
| browser-web | core.integration |
| mcp | core.integration |
| git | core.integration |
| miniapp | core.integration |
| computer-use | core.integration |
| image-analysis | core.integration |

## 功能

工具包 owner crate。定义所有内置工具的静态分类和分组方案。通过 feature flags 选择工具包编译，提供 `try_product_tool_provider_group_plan_for_ids` 按 ID 选择。被 product-capabilities 用于计算产品能力的工具依赖。包含产品所有内置工具（约 52 个）的完整列表。

# 内置 Tools（Built-in Tools）能力支持需求文档

> 状态：能力规格 / 需求
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/architecture/product-architecture.md`](../architecture/product-architecture.md)
> - [`docs/architecture/agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)
> - [`src/crates/execution/tool-provider-groups/AGENTS.md`](../../src/crates/execution/tool-provider-groups/AGENTS.md)
> - [`src/crates/assembly/core/AGENTS.md`](../../src/crates/assembly/core/AGENTS.md)

## 背景与需求描述

BitFun Agent 要"进入真实环境"完成任务，必须能读 / 写文件、执行命令、检索代码、控制会话、操作浏览器与桌面、对接 MCP、覆盖鸿蒙开发链。这些能力以**工具（Tools）**形式暴露给模型，模型在对话回合中按 schema 调用，运行时校验权限、执行、回传结构化结果。

工具来源分三类，本需求只覆盖第一类：

1. **内置 Tools（Built-in Tools）**——随产品编译、稳定契约、跨 delivery profile 可选的官方工具集；
2. **MCP 工具**——经 MCP 协议接入的外部工具（见 `core.integration` 的 MCP 子集，属扩展层，不在本需求）；
3. **插件 / 脚本工具**——经 PluginRuntime / script-tool-runtime 接入的第三方工具（属扩展层，不在本需求）。

内置 Tools 需要满足的核心诉求：

- **稳定契约**：每个工具的 name / schema / 入参 / 出参 / 权限语义稳定，模型可在多轮中稳定调用；
- **特征归属**：每个内置工具精确归属一个 feature group（如 `basic` / `git` / `mcp` / `browser-web` / `computer-use` / `image-analysis` / `miniapp` / `canvas` / `agent-control`），按 owner feature 编译，`default = []`；
- **按 profile 选装**：不同 delivery profile（Desktop / CLI / Server / Web / ACP / Sdk …）按 `ProductToolPlan` 显式选择哪些 provider group 生效，非桌面形态可裁剪；
- **fail-closed 装配**：产品计划请求了某 group 而二进制未编译该 group 时，必须装配失败，不得从编译期 feature 推断或静默降级；
- **可见与可控**：工具 manifest 在 prompt 中对模型可见（展开 / 折叠两态），权限、取消、`ToolUseContext` 语义在桌面 / MCP / ACP 各 catalog 行为一致；
- **跨平台与鸿蒙覆盖**：基础工具平台无关，鸿蒙开发链（构建 / 启动 / hdc 日志 / ArkTS 检查 / UI 校验）作为 `core.openharmony` 子集独立维护。

## 期望行为

### 1. 工具分组与特征归属

内置工具按 **provider group** 组织，每个 group 声明其 feature groups 与 tool names：

| provider id | feature groups | 代表工具 |
| --- | --- | --- |
| `core.basic` | basic, image-analysis | `LS`、`Read`、`Write`、`Edit`、`Delete`、`Glob`、`Grep`、`view_image`、`analyze_image`、`ExecCommand` / `WriteStdin` / `ExecControl`、`GetTime`、`ListModels` |
| `core.agent` | agent-control, git | `Task`、`AgentWait`、`LaunchReviewAgent`、`Skill`、`AskUserQuestion`、`TodoWrite`、`get_goal` / `create_goal` / `update_goal`、`CreatePlan`、`submit_code_review`、`GetToolSpec`、`CallDeferredTool`、`GetFileDiff` |
| `core.canvas` | canvas | `CreateCanvas` / `ReadCanvas` / `UpdateCanvas` / `PatchCanvas` |
| `core.session` | agent-control | `SessionControl`、`SessionMessage`、`SessionHistory`、`Cron` |
| `core.integration` | browser-web, mcp, git, miniapp, computer-use | `WebSearch`、`WebFetch`、MCP 资源 / prompt 工具、`GenerativeUI`、`Git`、`Worktree`、`ReviewPlatform`、MiniApp / Appearance / Page 发布工具、`ControlHub`、`ComputerUse`、`Playbook` |
| `core.openharmony` | basic | `build_project`、`start_app`、`hdc_log`、`arkts_knowledge_search`、`check_arkts_files`、`check_cpp_files`、`switch_cwd`、`verify_ui`、`get_ui_verification_log`、`save_ui_screenshot` |

- 一个 provider 可包含来自多个 feature owner 的工具；**工具到 feature group 的精确映射**由 `bitfun-tool-packs` 作为 owner 权威。
- 编译期 availability 是**验证事实**而非运行时推断来源；materialization 必须在"计划请求了未编译 group"时 fail closed。

### 2. 装配与可见性

- `ProductToolPlan` 由 Product Assembly 提供**确切**计划，Core materialization 校验请求的 owner 是否已编译，**不得**从 Cargo feature 并集推断产品能力；
- Agent Runtime 基线计划恰好是 `Basic` + `AgentControl`，不是隐藏的 delivery profile；
- 工具 manifest 在 prompt 中按展开 / 折叠两态对模型可见；新增 / 迁移工具必须保持 collapsed 暴露、prompt stub、unlock 状态、取消、运行时限制与 Deep Review 工具流不变；
- `GetToolSpec` 在产品工具运行时边界执行，供模型按需获取工具规格。

### 3. 权限与取消

- 工具调用经权限门 / 确认门（confirmation gate）后才执行；
- turn、tool、subagent、harness step 都必须接收 cancellation，长任务工具（ExecCommand、Task、ComputerUse 等）支持中途取消；
- `ToolUseContext` 语义在桌面 / MCP / ACP 各 catalog 保持一致。

### 4. 跨平台与鸿蒙

- 基础工具（文件 / 检索 / 执行 / 时间 / 模型列表）平台无关；
- `core.openharmony` 子集覆盖鸿蒙开发链：构建（`build_project`）、启动（`start_app`）、设备日志（`hdc_log`）、ArkTS 知识检索与静态检查（`arkts_knowledge_search` / `check_arkts_files` / `check_cpp_files`）、UI 校验（`verify_ui` / `get_ui_verification_log` / `save_ui_screenshot`）、工作目录切换（`switch_cwd`）；
- 鸿蒙子集作为独立 provider 维护，不与基础工具混编，便于按 profile 裁剪。

### 5. 远程工作区

- 远程控制另一台桌面时，工具执行归属受控端；控制端只发起与展示；
- 在 `remote_workspace_policy` 中声明每个内置工具（尤其写文件、执行命令、Git、ComputerUse）的远程策略，不支持时清晰提示而非静默失败。

### 6. 可扩展与稳定性

- 新增内置工具：指定唯一 feature group owner + 覆盖该 group 全列表的边界校验 + provider group 计划登记；
- 工具迁移必须保持产品注册表顺序、展开 / 折叠暴露、prompt stub、unlock 状态、取消、运行时限制、Deep Review 工具流；
- 工具 schema 变更视为破坏性变更，需走契约评审。

## 非目标 / 范围外

- 不在本需求内定义 MCP 工具接入协议（属 `core.integration` 的 MCP 子集，见 MCP runtime owner）；
- 不在本需求内定义 PluginRuntime / script-tool-runtime 第三方工具（属扩展层）；
- 不在本需求内定义 Skills / Hooks / MiniApp / 自定义 Agent 等其他扩展层；
- 不在本需求内做工具调用的模型适配层 / AI 序列化（属 adapters）；
- 不覆盖 HarmonyOS PC CLI/TUI 形态（见 `platform-portability-design.md`，需单独立项）；
- 不承诺工具结果的自动可视化渲染（MiniApp / Canvas 才负责界面呈现）。

## 建议的落地路径（基于现有分层）

依据仓库的分层与边界规则，内置 Tools 的各部分应归属：

1. **Contracts (`src/crates/contracts`)** — 工具调用 / 结果 / 权限 / `ToolUseContext` 的稳定 DTO 与 port trait，行为轻量、不向上依赖。
2. **Execution / Tool Provider Groups (`src/crates/execution/tool-provider-groups`，包名 `bitfun-tool-packs`)** — 内置工具的**事实层**：
   - feature group 元数据、稳定的 tool→feature 映射、product tool provider group plan、按 id 选择 provider group plan；
   - **不拥有** manifest / 暴露契约、具体运行时 manifest 装配、`GetToolSpec` 执行、collapsed unlock 状态、snapshot 装饰、`ToolUseContext`；
   - 边界：`default = []`，`product-full` 可聚合但不得静默启用新运行时行为；不依赖 core / service / app / Tauri / Git / MCP / 网络 / CLI UI。
3. **Execution / Tool Contracts & Execution (`tool-contracts`、`tool-execution`)** — 工具契约、执行门、入参校验、结果呈现契约；低层文件 / 检索 / 工具 IO、ExecCommand 呈现事实、Computer Use 循环 / 重试策略、prompt-safe 工具上下文事实。
4. **Assembly / Core (`src/crates/assembly/core`)** — 工具**具体实现与运行时 materialization**：
   - `src/agentic/tools/implementations/` 下按 owner feature 编译各工具（`tools-image-analysis` / `tools-miniapp` / `tools-canvas` / `tools-computer-use` / `tools-browser-web` / `tools-agent-control` / `tools-mcp` / `tools-git`）；
   - 产品工具运行时边界执行 `GetToolSpec`、manifest 装配、展开 / 折叠暴露、unlock 状态、permission / `ToolUseContext` 语义；
   - 校验请求的 owner 已编译，fail closed；不从 feature 并集推断产品能力。
5. **Services (`src/crates/services`)** — 工具调用的具体 OS / 进程 / 文件系统 / Git / 终端 / MCP / 远端 SSH / 浏览器控制 / web 工具网络 provider 实现，按各自 owner feature 启用。
6. **App / UI (`src/apps/desktop` + `src/web-ui`)** — 桌面宿主装配工具注册表、暴露权限确认 UI、工具调用展示；UI 组件不得直接调用工具运行时或 OS API，必须走 adapter / service 层。

### 分层与依赖边界要点

- 产品逻辑平台无关：工具契约、provider group 事实、feature 映射在 `tool-provider-groups` 与 `tool-contracts`，平台无关；具体实现与 IO 在 Core / Services；
- `tool-provider-groups` 不得依赖 core / concrete service / app；Core materialization 不得反向依赖 `tool-provider-groups` 的运行时状态（只用其计划事实）；
- 新增内置工具 = 一个确切 feature group owner + group 全列表边界覆盖 + provider group 计划登记 + 实现按 owner feature 编译；
- 编译期 availability 是验证事实：计划请求未编译 group 时 fail closed，绝不从 Cargo feature 推断或检视运行时 / 权限 / 服务健康来"凑出"计划；
- 按 owner feature 过滤被 `ProductToolPlan` 省略的工具是**产品选择**，不是降级；
- HarmonyOS 子集（`core.openharmony`）独立维护，便于按 profile 裁剪与未来鸿蒙化专题推进。

## 设计草案 / 参考示例

- **provider group 计划参考**：`bitfun-tool-packs` 的 `PRODUCT_TOOL_PROVIDER_GROUP_PLAN` 把六个 provider（basic / agent / canvas / session / integration / openharmony）的 feature groups 与 tool names 登记为静态事实；新增工具在此登记，并保证 `try_product_tool_provider_group_plan_for_ids` 对未知 provider id 报错。
- **feature 映射参考**：`tool_feature_group` 是 tool→feature 的 owner 权威映射；一个 provider 可跨多 feature owner（如 `core.integration` 跨 browser-web / mcp / git / miniapp / computer-use）。
- **fail-closed 装配参考**：`unavailable_feature_groups` 报告请求但二进制未编译的 group；materialization 拿到该报告后必须失败，不静默降级。
- **manifest 暴露参考**：展开 / 折叠两态 + `GetToolSpec` 按需取规格；迁移工具必须保持 collapsed 暴露、prompt stub、unlock 状态不变。
- **取消参考**：turn / tool / subagent / harness step 都接收 cancellation；ExecCommand 的 `ExecControl` / `WriteStdin` / background output 是长任务取消的范本。
- **鸿蒙子集参考**：`core.openharmony` 把 build / start / hdc / arkts / ui-verify 等鸿蒙开发链工具收口为一个 provider，便于按 delivery profile 裁剪，并与 `platform-portability-design.md` 的"每个鸿蒙差异独立建专题"原则一致。
- **远程策略参考**：写文件 / 执行命令 / Git / ComputerUse 等高风险工具在 `remote_workspace_policy` 中逐工具声明远程策略。

## 是否愿意贡献

- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

## 补充说明

- 本需求严格遵循仓库"产品逻辑平台无关、再通过平台适配器暴露""优先事实与决策、具体 IO 下沉""编译期 availability 是验证事实而非推断来源"的规则。
- 与 `skill-capability-support.md` 的关系：Skill 是 prompt / instruction 注入的扩展（L2），内置 Tools 是 Agent 直接调用的能力（L0）；两者互补，`Skill` 工具本身是 `core.agent` 下的一个内置 Tool，用于显式调用 Skill。
- 与 `gewu-market-integration.md` 的关系：市场分发的是 Skills / MiniApp / Appearance 等扩展资产；内置 Tools 随产品编译，不经市场分发。
- 与 MCP / PluginRuntime 的关系：内置 Tools 是稳定官方集；MCP / 插件是外部扩展，经 `core.integration` 与 PluginRuntime 接入，不在本需求。
- 远程工作区策略：内置工具（尤其写 / 执行 / Git / ComputerUse）的远程策略在 `remote_workspace_policy` 中逐工具显式声明，不支持时清晰提示。
- 相关分层入口：`src/crates/contracts/AGENTS.md`、`src/crates/execution/tool-provider-groups/AGENTS.md`、`src/crates/execution/tool-contracts/AGENTS.md`、`src/crates/assembly/core/AGENTS.md`、`src/crates/services/AGENTS.md`。

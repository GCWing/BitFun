**中文** | [English](AGENTS.md)

# AGENTS-CN.md

BitFun 是 Rust workspace 与 React 前端组成的多端项目。

仓库核心原则：**产品逻辑先做到平台无关，再经平台适配层对外暴露。**

本文件是**改代码时的正式规范入口**（渐进披露）：只放权威文档索引和最短导航。
做到相关任务时再打开链接文档；不要把长篇细则抄回本文件。

## 快速开始

1. 架构敏感改动前先读 [`README.md`](README.md) 与 [`CONTRIBUTING_CN.md`](CONTRIBUTING_CN.md)。提 PR、参与贡献从 CONTRIBUTING 入手；约束怎么改代码以本文件为准。
2. 桌面端日常开发用 `pnpm run desktop:dev`；只改前端、想更快冷启动时再用 `pnpm run desktop:preview:debug`。常用命令见 [`docs/guideline/common-commands.zh-CN.md`](docs/guideline/common-commands.zh-CN.md)。
3. 改完 Rust 优先跑 `pnpm run fmt:rs`（只格式化已改或已暂存的 `.rs`）。只有刻意做更大范围格式化时才用 `cargo fmt`。
4. 先看下方 **按任务路由** / **规范类型地图**，再按 [`docs/guideline/verification.zh-CN.md`](docs/guideline/verification.zh-CN.md) 选最小检查。
5. Rust workspace 依赖在根清单中统一版本，消费 crate 只声明自身所需 feature；测试专用 feature 放入 `dev-dependencies`，受 crate feature 控制的服务能力只在对应 feature 中启用。第三方默认 feature 若不是所有 consumer 的稳定契约，应在 `[workspace.dependencies]` 统一关闭；仓内 crate 的 `default` 已由边界契约保证为空时，不在每条依赖边重复写 `default-features = false`，ACP 这类保留兼容默认的 crate 仍由窄 consumer 显式关闭。被复制到独立 Docker 构建上下文的 manifest 继续维护显式版本和默认策略。根 workspace 与 workspace members 一律禁止使用 `tokio/full`。

## 如何使用本文件

1. 改某个目录下的代码时，优先看该目录就近的 `AGENTS.md` / `AGENTS-CN.md`。
2. **规范类型地图**标明规范类别；**架构索引**展开 STD-01 子专题；**跨切面索引**覆盖日志 / 宿主 / Agent loop；**按任务路由**对应「改什么 → 先读什么 → 再验什么」。
3. 细则以链接中的权威文档为准。请与英文版 [`AGENTS.md`](AGENTS.md) 保持语义一致。
4. 完整文档地图见 [`docs/README.md`](docs/README.md)；文档放置和迁移规则仍以 [`docs-governance.zh-CN.md`](docs/guideline/docs-governance.zh-CN.md) 为准。

## 语言约定（本仓文档）

此处仅为摘要；完整规则见 [`docs/guideline/docs-governance.zh-CN.md`](docs/guideline/docs-governance.zh-CN.md)。

| 类型 | 语言 |
|---|---|
| 面向人阅读的说明、流程类文档 | 以中文为准（英文可不强制）。规格流程索引 [`docs/specs/README.md`](docs/specs/README.md) 以中文为准。 |
| 根目录 `AGENTS` / `CONTRIBUTING` | 中英都要有，**语义必须对齐** |
| 主要给 AI / 改代码时查阅的操作与约束（如 `docs/guideline/*`、模块 `AGENTS`） | 以英文为准 |
| 日志文案 | 只用英文 |

## 规范类型地图

| ID | 规范类型 | 何时查阅 | 权威入口（从这里开始） |
|---|---|---|---|
| STD-01 | 仓库与架构 | 分层、依赖、产品线边界 | 下方 **分层模块索引** + **架构索引** → 对应设计文档 |
| STD-02/03 | 编码与语言栈 | 超出模块 AGENTS 的风格或语言细则 | 就近模块 `AGENTS.md` |
| STD-04 | 前端与交互 | UI、状态、adapter、i18n、主题 | i18n/主题：[`docs/architecture/i18n.md`](docs/architecture/i18n.md)、操作说明 [`docs/guideline/i18n.md`](docs/guideline/i18n.md)、[`docs/architecture/theme-token-optimization.md`](docs/architecture/theme-token-optimization.md)、操作 [`docs/guideline/theme-color-tokens.md`](docs/guideline/theme-color-tokens.md)；UI/状态/adapter：就近产品面 `AGENTS.md`（如 [`src/web-ui/AGENTS.md`](src/web-ui/AGENTS.md)） |
| STD-05 | API 与数据契约 | DTO、事件、Tauri command、持久化、错误结构 | DTO/事件/契约：[`src/crates/contracts/AGENTS.md`](src/crates/contracts/AGENTS.md) 及子模块；Tauri/宿主/远程：[`docs/guideline/host-platform-and-remote.zh-CN.md`](docs/guideline/host-platform-and-remote.zh-CN.md) |
| STD-06 | DFX | 重试、取消、部分成功、异构输入、失败体验、日志、安全 | 日志：[`docs/guideline/logging.md`](docs/guideline/logging.md)、[`src/web-ui/LOGGING.md`](src/web-ui/LOGGING.md)、[`src/crates/LOGGING.md`](src/crates/LOGGING.md)；远程：[`host-platform-and-remote.zh-CN.md`](docs/guideline/host-platform-and-remote.zh-CN.md)；Agent loop：[`agent-loop-behavior.zh-CN.md`](docs/guideline/agent-loop-behavior.zh-CN.md)；安全：[`SECURITY.md`](SECURITY.md) |
| STD-07 | 文档与模板 | Spec / 设计 / 计划；文档如何存放 | [`docs/guideline/docs-governance.zh-CN.md`](docs/guideline/docs-governance.zh-CN.md)；[`docs/specs/README.md`](docs/specs/README.md) |
| STD-08 | 测试与验证 | 改完应跑哪些检查 | [`docs/guideline/verification.zh-CN.md`](docs/guideline/verification.zh-CN.md) |
| STD-09 | Git 与交付 | 分支、PR、贡献流程 | [`CONTRIBUTING_CN.md`](CONTRIBUTING_CN.md)（[English](CONTRIBUTING.md)）；CONTRIBUTING 不再另维护一整套命令/验证清单 |
| STD-10 | AI 协作 | 多 Agent、多工具共用同一套规范 | 本入口 + 就近模块 `AGENTS.md`；不要在工具侧另写一套平行规则 |
| STD-11 | 规范自动化 | 审计、baseline、边界检查 | i18n/theme 等审计文档 + [`theme-color-tokens.md`](docs/guideline/theme-color-tokens.md) + [`verification.zh-CN.md`](docs/guideline/verification.zh-CN.md)；**禁止**靠抬高 baseline 让检查「变绿」 |
| STD-12 | 模块规范 | 包 / crate 局部约定 | 就近 `AGENTS.md` / `AGENTS-CN.md` |

**另：** 命令列表 → [`docs/guideline/common-commands.zh-CN.md`](docs/guideline/common-commands.zh-CN.md)（不能代替验证矩阵；快速开始第 2 条也有入口）。

## 架构索引（STD-01）

任务命中下表某一行时，**必须打开**对应权威文档，不能只看规范类型地图里 STD-01 那一行就停。

| 专题 | 何时打开 | 权威文档 |
|---|---|---|
| 产品架构 | `bitfun-core` 拆解、feature/依赖边界、构建提速类重构 | [`docs/architecture/product-architecture.md`](docs/architecture/product-architecture.md)（见 §1.1）；Rust 构建依赖边界：[`docs/architecture/rust-build-dependency-boundaries.md`](docs/architecture/rust-build-dependency-boundaries.md)；专题地图 [`docs/architecture/README.md`](docs/architecture/README.md) |
| Agent Runtime 部署 | 多 GUI/TUI/Remote 实例、共享 Session 控制、进程拓扑 | [`docs/architecture/agent-runtime-deployment-design.md`](docs/architecture/agent-runtime-deployment-design.md) |
| Agent hooks | 原生 Codex 兼容 hooks、BitFun 差异与门控 | [`docs/specs/agent-hooks.md`](docs/specs/agent-hooks.md)（[中文](docs/specs/agent-hooks.zh-CN.md)）；不要另起一套 Codex hook 契约 |
| 物理分层 | 代码应落在哪一层、依赖方向是否正确 | 本文件 **分层模块索引** |
| CLI / TUI 产品线 | CLI/TUI 对齐、非交互输出、配置导入、插件体验、CLI Agent、品牌定制 CLI | [`docs/architecture/cli-product-line-design.md`](docs/architecture/cli-product-line-design.md)、[`src/apps/cli/AGENTS.md`](src/apps/cli/AGENTS.md) |
| HarmonyOS PC CLI/TUI | HarmonyOS PC 系统终端 / CLI-TUI 可移植性 | [`docs/architecture/platform-portability-design.md`](docs/architecture/platform-portability-design.md) |
| 产品定制 | 产品定义、品牌发行版、GUI/TUI 布局选择、内置扩展、定制构建 | [`docs/architecture/product-customization-blueprint.md`](docs/architecture/product-customization-blueprint.md) |
| OpenCode 兼容 | OpenCode 实时配置或插件执行 | [`docs/architecture/extensions/opencode-extension-compatibility.md`](docs/architecture/extensions/opencode-extension-compatibility.md) — **先读当前 P0 运行时护栏**（managed-package / static-preview；勿把设计目标当已交付能力） |
| SDLC 质量护栏 | 生命周期证据、门禁、Artifact Graph、Project Profile、Deep Review、目标项目治理 | [`docs/architecture/sdlc-governance-architecture.md`](docs/architecture/sdlc-governance-architecture.md)；模块边界或行为变化时另读 [`docs/architecture/`](docs/architecture/)（security-boundary、quality-data-plane、evidence-pack、artifact-graph、project-profile-integration、agent-workflow-design）或 [`docs/specs/`](docs/specs/)（configurable-policy-profile、risk-classifier、pr-quality-gate、agent-evaluation、requirement-impact-analysis）下对应设计；勿把 BitFun 本仓假设硬编码成目标项目规则 |

## 跨切面索引

按条件触发：只有任务相关时才打开；细则在链接文档里。

| 专题 | 何时打开 | 权威文档 |
|---|---|---|
| 日志 | 新增或修改日志、可观测相关文案 | 仓库级规则：[`docs/guideline/logging.md`](docs/guideline/logging.md)；前端 API：[`src/web-ui/LOGGING.md`](src/web-ui/LOGGING.md)；Rust API：[`src/crates/LOGGING.md`](src/crates/LOGGING.md) |
| Tauri / 平台 / 远程 / 升级 | 桌面 command、UI 与宿主边界、远程场景与跨版本行为 | [`docs/guideline/host-platform-and-remote.zh-CN.md`](docs/guideline/host-platform-and-remote.zh-CN.md)；[`src/apps/desktop/AGENTS.md`](src/apps/desktop/AGENTS.md) |
| Agent loop | Agent 主循环、重复工具调用、防死循环策略 | [`docs/guideline/agent-loop-behavior.zh-CN.md`](docs/guideline/agent-loop-behavior.zh-CN.md)；`src/crates/execution/` 下就近 `AGENTS.md` |

## 按任务路由

| 任务 / 改动 | 先读 | 再验证 |
|---|---|---|
| 不确定代码该放哪 | 分层模块索引 + 产品架构行 | [`verification.zh-CN.md`](docs/guideline/verification.zh-CN.md) 对应行 |
| 桌面 Tauri / 桌面专属 API | [`host-platform-and-remote.zh-CN.md`](docs/guideline/host-platform-and-remote.zh-CN.md)；[`src/apps/desktop/AGENTS.md`](src/apps/desktop/AGENTS.md)；`remote_workspace_policy.rs` | 验证矩阵中桌面相关行 |
| 共享 Rust（assembly / adapters / services / execution / contracts） | 分层模块索引 + 就近 crate `AGENTS.md` | 验证矩阵中共享 Rust 行 |
| Web UI（不改 locale 契约） | [`src/web-ui/AGENTS.md`](src/web-ui/AGENTS.md)；[`host-platform-and-remote.zh-CN.md`](docs/guideline/host-platform-and-remote.zh-CN.md) 平台边界节 | 验证矩阵中前端行 |
| i18n / locales | [`docs/architecture/i18n.md`](docs/architecture/i18n.md)；操作说明 [`docs/guideline/i18n.md`](docs/guideline/i18n.md) | 验证矩阵中 locale / i18n 行 |
| 主题 / 颜色 Token | [`docs/architecture/theme-token-optimization.md`](docs/architecture/theme-token-optimization.md)、[`docs/guideline/theme-color-tokens.md`](docs/guideline/theme-color-tokens.md) | `pnpm run theme:color-audit:all` |
| 日志文案 | 跨切面索引 → 日志 | 针对所改产品面的聚焦检查 |
| Agent loop / 防死循环 | [`agent-loop-behavior.zh-CN.md`](docs/guideline/agent-loop-behavior.zh-CN.md) | 就近 execution / runtime 测试 |
| Mobile web 配对 / 重连 | [`src/mobile-web/AGENTS.md`](src/mobile-web/AGENTS.md) | 验证矩阵中 Mobile web 行 |
| CLI / TUI / HarmonyOS PC / 定制 / OpenCode / SDLC | **架构索引**对应行 | 该产品面的最小检查 + 模块 AGENTS |
| 安装器 | [`BitFun-Installer/AGENTS.md`](BitFun-Installer/AGENTS.md) | 验证矩阵中安装器行 |
| 失败体验 / Provider 错误 / 远程不支持 | STD-05/06 权威；[`host-platform-and-remote.zh-CN.md`](docs/guideline/host-platform-and-remote.zh-CN.md) 远程节 | 对所改产品面跑契约或聚焦测试 |
| 写 Spec / 设计 | [`docs/specs/README.md`](docs/specs/README.md) + [`templates/`](docs/specs/templates/)；规范见 [`docs-governance.zh-CN.md`](docs/guideline/docs-governance.zh-CN.md) | 人工评审；注明适用的 STD 行 |
| 开 PR / 贡献流程 | [`CONTRIBUTING_CN.md`](CONTRIBUTING_CN.md) | 所改文件对应的最小验证 |

## 分层模块索引

依赖自上而下。下表是 **crate 物理布局**，不是完整概念架构——概念边界见架构索引中的「产品架构」。层内依赖尽量收窄到所需最小集合。

| # | 层级 | 路径 | 职责 | 模块 / 入口 | 层级文档 |
|---|---|---|---|---|---|
| 1 | 接口与入口层 | `src/apps/*`, `src/web-ui`, `src/mobile-web`, `BitFun-Installer`, `tests/e2e`, `src/crates/interfaces` | 产品宿主、命令、UI 入口、协议接口、跨产品面测试 | desktop、CLI、server、relay、Web UI、mobile web、installer、E2E、`acp`、`app-server`、`sdk-host` | 就近 `AGENTS.md`；[interfaces](src/crates/interfaces/AGENTS.md) |
| 2 | 产品组装层 | `src/crates/assembly` | 兼容导出、产品能力选择、product-full 接线、不可变内置 Agent 内容、adapter/service 注册、生态中立源协调 | `agent-content`, `core`, `external-sources`, `product-capabilities` | [AGENTS.md](src/crates/assembly/AGENTS.md) |
| 3 | 适配层 | `src/crates/adapters` | AI / transport / WebDriver 协议适配、外部 AI work source 适配（OpenCode / Claude Code / Codex）及外部 provider 形态转换 | `agent-runtime-ipc`, `ai-adapters`, `opencode-adapter`, `claude-code-adapter`, `codex-adapter`, `static-hook-support`, `transport`, `webdriver` | [AGENTS.md](src/crates/adapters/AGENTS.md) |
| 4 | 服务实现层 | `src/crates/services` | 可复用的 OS、文件系统、终端、MCP、remote、git、watch、process、LSP 插件注册、会话持久化、网络、MiniApp runtime IO | `services-core`, `services-integrations`, `miniapp-market-service`, `relay-service`, `page-function-runtime`, `terminal` | [AGENTS.md](src/crates/services/AGENTS.md) |
| 5 | 执行原语层 | `src/crates/execution` | 可移植的 agent、harness、stream、DeepReview、插件运行时客户端、typed-service、tool 契约与执行 | `agent-runtime`, `agent-stream`, `tool-contracts`, `harness`, `plugin-runtime-client`, `runtime-services`, `tool-provider-groups`, `tool-execution`, `tool-call-jsonrepair` | [AGENTS.md](src/crates/execution/AGENTS.md) |
| 6 | 稳定契约与产品领域层 | `src/crates/contracts` | 共享 DTO、事件形态、runtime port、LSP DTO、产品领域契约与策略 | `core-types`, `events`, `runtime-ports`, `product-domains` | [AGENTS.md](src/crates/contracts/AGENTS.md) |

边界规则：

- 接口与入口层暴露选定产品行为；可复用行为应下移。
- 组装层只接线下层并选择产品能力事实，不实现具体 adapter、OS 或 service 细节。
- 产品特性只在内核能力之上组装用户侧命令、UI contribution、设置和默认策略；长程任务、scheduler、permission、session/workspace、memory、DFX、hook 和 event 事实属于 Agent Kernel owner。
- 适配层翻译协议和外部 provider 形状，不拥有产品能力选择或可复用 OS service 行为。
- 服务实现层负责可复用的 OS、process、terminal、MCP、remote、git、filesystem、LSP plugin registry、session persistence primitives 和 MiniApp runtime IO 能力。
- 外部系统是边界外资源，不是仓库内层级。只有已注册的 adapter、service 或 app-local provider 应调用它们；其他层消费 port 和稳定契约。
- 执行原语层只放可移植运行时构件，不拥有宿主或交付形态。
- 契约层保持轻行为，不得向上依赖。

## Agent 文档优先级

以离目标文件最近的 `AGENTS.md` / `AGENTS-CN.md` 为准。与本文件冲突时，采用更具体、更近的那一份。

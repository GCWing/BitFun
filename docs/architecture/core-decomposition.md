# BitFun Core 拆解护栏（Core Decomposition Guardrails）

本文是逐步拆解 `bitfun-core` 的执行护栏（execution guardrail）。它用于补充
[`bitfun-core-decomposition-plan.md`](../plans/core-decomposition-plan.md)
中的详细里程碑计划。

目标是在不改变任何受支持构建形态（build shape）下产品行为的前提下，把稳定、
边界清晰的逻辑从较重的 `bitfun-core` runtime 聚合体中移出，从而减少不必要的
Rust 编译和链接面。

## 不可协商的不变量

- 拆解过程中不得改变产品行为。
- 不得为了提升本地速度而减少 CI 或 release 覆盖范围。
- 除非后续有明确的产品变更要求，否则产品 crate 必须保持相同的能力集合
  （capability set）。
- 构建脚本和安装器脚本不属于本次重构范围：
  - `package.json`
  - `scripts/dev.cjs`
  - `scripts/desktop-tauri-build.mjs`
  - `scripts/ensure-openssl-windows.mjs`
  - `scripts/ci/setup-openssl-windows.ps1`
  - `BitFun-Installer/**`
- 共享产品逻辑必须保持平台无关（platform-agnostic）。桌面端专属逻辑应保留在
  app adapters 中，再通过 transport/API layers 回流。
- 不要引入仓库级、机器相关的编译器或链接器默认配置，例如 `sccache`、`lld-link`
  或 `mold`。

## 执行顺序

按里程碑执行，不按孤立的重构想法零散推进：

1. **安全保护和最小编译面验证**
   - 在任何默认 feature 变轻之前，先加入 `product-full` feature 安全网。
   - 把已经独立成 crate 的 nested crate 移到 workspace 顶层路径。
   - 先抽取 `core-types`，承载稳定 DTO 和 port DTO；只有在 concrete runtime /
     network 转换依赖完成解耦后，才移动 `BitFunError`。
   - 如果 stream 测试可以不依赖完整 core 运行，则抽取 stream processing。
   - 移动重服务之前先引入 ports。第一层轻量边界位于 `bitfun-runtime-ports`；
     该 crate 只包含 DTO 和 trait。
   - 第一批 adapter 实现只视为边界搭建。只有相关 service migration 和回归测试
     完成后，才能声明 service/agent 的 concrete call site 已经被替换。
2. **中等粒度 owner crate**
   - 优先使用 8 到 12 个 owner crate，而不是大量小 crate。
   - 使用 `services-core` 和 `services-integrations`，不要为每个 service 文件夹
     单独建立 crate。
   - 使用 `agent-tools` 加 `tool-packs` feature group，不要为每个具体工具族
     单独建立 crate。
3. **Facade 收敛和边界强制**
   - `bitfun-core` 收敛为兼容门面（compatibility facade）和完整产品 runtime
     组装点（full product runtime assembly）。
   - 新 crate 抽出后，再加入轻量边界检查。
   - 更轻的默认 feature 只能作为单独且完整验证过的 PR 进行评估。

## Crate 归属目标（Crate Ownership Targets）

初始目标 crate 应保持中等粒度。下表同时包含新的 owner crate 目标，以及属于拆解
边界的一些已有基础 crate。

| 目标 crate | 归属职责 |
|---|---|
| `bitfun-core` | 兼容门面和完整产品 runtime 组装点 |
| `bitfun-core-types` | 稳定 DTO、port DTO、纯 domain type，以及最终的纯错误类型 |
| `bitfun-events` | 已有的传输层无关事件 DTO 和事件抽象 |
| `bitfun-ai-adapters` | 已有 AI provider adapter，以及 provider / protocol DTO 归属 |
| `bitfun-agent-stream` | Stream 聚合和 stream-focused 测试 |
| `bitfun-runtime-ports` | 面向 service/agent 边界的轻量跨层 DTO 和 trait |
| `bitfun-agent-runtime` | Sessions、execution、coordination、agent system |
| `bitfun-agent-tools` | Tool trait、context、registry、provider contract |
| `bitfun-tool-packs` | 由 feature group 隔离的具体工具实现 |
| `bitfun-services-core` | Config、session、workspace、storage、filesystem、system services |
| `bitfun-services-integrations` | Git、MCP、remote SSH、remote connect、file watch integrations |
| `bitfun-product-domains` | Miniapp 和 function-agent 产品子域 |
| `terminal-core` | 已有 terminal package，移动到 workspace 顶层 `src/crates/terminal` 路径 |
| `tool-runtime` | 已有 tool runtime，移动到 workspace 顶层路径 |

除非有实测证据证明继续拆分可以减少关键编译目标或测试目标，并且该模块已经具备稳定的
owner 边界，否则不要把一个 feature group 继续拆成更小的 crate。

## 依赖方向规则（Dependency Direction Rules）

- 新拆出的 crate 不得反向依赖 `bitfun-core`。
- `bitfun-core` 可以依赖新拆出的 crate，并通过 re-export 保持旧路径兼容。
- 在声明 P3 边界收敛前，运行 `node scripts/check-core-boundaries.mjs`，确认已拆出的
  owner crate 没有新增 `bitfun-core` 反向依赖，并确认 `core-types`、`runtime-ports`
  和 `agent-tools` 没有引入重 runtime / concrete service 依赖。
- 已迁移回 `bitfun-core` 的 legacy facade 只能 re-export owner crate；例如 Git 旧路径、
  remote SSH types/workspace path + unresolved-key helper facade、MCP tool contract facade、MCP protocol types facade、
  MCP config location facade 和 announcement types facade 由边界脚本检查，不得重新承载实现逻辑。
- 对仍嵌在 core runtime 文件中的旧公开类型，必须至少保留禁止回流检查；例如 MCP server
  type/status 已由 owner crate 拥有，`MCPServerProcess` 只保留 runtime 逻辑。
- `bitfun-runtime-ports` 必须保持 DTO/trait-only；不得依赖 concrete manager、
  service implementation、app crate 或 platform adapter。
- `bitfun-core-types` 不得依赖 runtime manager、service crate、agent runtime、
  app crate、Tauri、network client、process execution，或 `git2`、`rmcp`、`image`、
  `tokio-tungstenite` 等重集成依赖。
- 轻量 contract crate 不得吸收 CLI/TUI 依赖；`bitfun-cli`、`ratatui`、`crossterm`、
  `arboard`、`syntect-tui` 等仍属于 `src/apps/cli` app adapter / presentation layer。
- `ErrorCategory`、`AiErrorDetail` 以及纯 AI 错误分类/detail helper 应放在
  `bitfun-core-types` 中，并通过已有更高层路径 re-export 或委托，以保持公开行为稳定。
- 在剩余 concrete error-wrapper 依赖完成审核前，不要把 `BitFunError` 移入
  `bitfun-core-types`。错误边界中已经移除了 `reqwest::Error` 和
  `tokio::sync::AcquireError` 引用；`serde_json::Error`、`anyhow::Error` 以及历史
  `From<T>` 行为仍需要单独做兼容性处理后，才能移动该类型。
- Service crate 必须通过小型 port 调用 agent runtime，不要直接访问全局 coordinator。
- 迁移期间，adapter implementation 可以暂时放在 `bitfun-core` 中，但新的 service
  代码必须面向 port contract，而不是新增对 coordinator 或 manager 的直接依赖。
- Agent runtime 必须通过 ports/providers 依赖 service 行为，不要依赖 concrete 的重集成
  crate。
- 最新主干已把 subagent 可见性做成 mode-scoped registry 行为。迁移 agent registry 或
  subagent definitions 前，必须先保留 mode visibility、hidden/custom/review 分组和 desktop
  subagent API 等价测试；在此之前它们仍属于 `bitfun-core` product runtime assembly。
- 最新 CLI 重构新增大量 TUI、theme、selector、dialog 和 chat-state 代码，但仍位于
  `src/apps/cli`。后续 core decomposition 只能通过产品 check 验证 CLI 仍可组装，不应把
  CLI presentation 依赖迁入 core-types、runtime-ports 或 agent-tools。
- Tool framework crate 不得依赖 concrete service implementation。
- 产品 crate 可以通过显式 product feature 组装完整 runtime。

## Feature 安全规则

- 在让任何默认 feature 变轻之前，先引入 `product-full`。
- 评估默认 feature 缩减之前，产品 crate 必须显式启用完整产品 runtime。
- `product-full` 是产品能力保护开关（product capability guardrail），不是新的万能聚合点
  （dumping ground）。每个新的 owner crate 都应暴露具体 feature group；只有为了保持既有
  产品形态时，`product-full` 才可以包含它们。
- 拆解完成后不要自动移除或减轻 `product-full`。如果未来要用 per-product explicit
  feature set 替代它，必须作为 P3 之后的独立评估，并且先通过完整产品矩阵。
- 不要把 feature 默认值变更和模块移动放在同一个变更中。
- 不要把改变产品构建产物能力集合作为减少本地测试编译面的副作用。

## 测试和验证策略（Test And Verification Policy）

先运行能够证明当前变更的最小验证，再在进入下一个里程碑前运行里程碑门禁。

对于保持行为不变的重构：

- 如果被移动的行为尚未被测试覆盖，先补测试，再移动逻辑。
- 当模块已经移出 `bitfun-core` 后，优先使用小 crate 测试。
- 如果变更影响 feature assembly、产品 crate manifest、desktop integration、CLI、
  server 或 transport path，则必须保留完整产品检查。

对于仅调整文档护栏的变更：

```powershell
git diff -- package.json scripts/dev.cjs scripts/desktop-tauri-build.mjs scripts/ensure-openssl-windows.mjs scripts/ci/setup-openssl-windows.ps1 BitFun-Installer
```

期望结果：无 diff。

详细计划中列出了各里程碑门禁。没有针对对应门禁的最新验证证据时，不要声明里程碑完成。

## 冗余清理策略（Redundancy Cleanup Policy）

冗余清理不是主要的编译提速手段。只有在输入、输出、错误路径、副作用、日志、时序和平台
条件都能证明等价时，才抽取重复逻辑。

如果等价性不清晰，就保留重复代码。不要仅仅因为两个流程看起来相似，就创建新的共享抽象。

冗余清理 PR 必须独立于 crate splitting、feature 默认值变更和依赖升级。

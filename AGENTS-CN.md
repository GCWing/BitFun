**中文** | [English](AGENTS.md)

# AGENTS-CN.md

BitFun 是一个由 Rust workspace 与共享 React 前端组成的项目。

仓库核心原则：**先保持产品逻辑平台无关，再通过平台适配层对外暴露能力**。

## 快速开始

1. 在修改架构敏感代码前，先阅读 `README.md` 和 `CONTRIBUTING.md`。
2. 桌面端开发优先使用 `pnpm run desktop:dev` — 提供完整热更新（Vite HMR + Rust 自动重编译并重启）。仅在需要更快冷启动且只迭代前端时使用 `pnpm run desktop:preview:debug`（Rust 改动不会自动重编译）。
3. 改完后按下方表格执行与改动范围匹配的最小验证。

## 模块索引

| 模块 | 路径 | Agent 文档 |
|---|---|---|
| Core（产品逻辑） | `src/crates/core` | [AGENTS.md](src/crates/core/AGENTS.md) |
| 已拆出的 core 支撑 crate | `src/crates/{core-types,agent-stream,runtime-ports,terminal,tool-runtime}` | （使用 core 指南） |
| Transport 适配层 | `src/crates/transport` | （使用 core 指南） |
| API layer | `src/crates/api-layer` | （使用 core 指南） |
| AI adapters | `src/crates/ai-adapters` | [AGENTS.md](src/crates/ai-adapters/AGENTS.md) |
| 桌面应用 | `src/apps/desktop` | [AGENTS.md](src/apps/desktop/AGENTS.md) |
| Server | `src/apps/server` | （使用 core 指南） |
| CLI | `src/apps/cli` | （使用 core 指南） |
| 中继服务器 | `src/apps/relay-server` | （使用 core 指南） |
| 共享前端 | `src/web-ui` | [AGENTS.md](src/web-ui/AGENTS.md) |
| 安装器 | `BitFun-Installer` | [AGENTS.md](BitFun-Installer/AGENTS.md) |
| E2E 测试 | `tests/e2e` | [AGENTS.md](tests/e2e/AGENTS.md) |

## 最常用命令

```bash
# 安装
pnpm install

# 开发
pnpm run desktop:dev               # 完整热更新：Vite HMR + Rust 自动重编译并重启
pnpm run desktop:preview:debug     # 复用预构建二进制 + Vite HMR；无 Rust 自动重编译
pnpm run dev:web                   # 纯浏览器前端
pnpm run cli:dev                   # CLI 运行时

# 检查
pnpm run lint:web
pnpm run type-check:web
cargo check --workspace

# 测试
pnpm --dir src/web-ui run test:run
cargo test --workspace

# 构建
cargo build -p bitfun-desktop
pnpm run build:web

# 快速构建（开发 / CI 提速）
pnpm run desktop:build:fast           # debug 构建，不打包
pnpm run desktop:build:release-fast   # release 但降低 LTO
pnpm run desktop:build:nsis:fast      # Windows 安装器，release-fast profile
pnpm run installer:build:fast         # 安装器应用，快速模式
```

完整脚本列表见 [`package.json`](package.json)。

## 全局规则

### 日志

日志必须只用英文，且不能使用 emoji。

- 前端：[src/web-ui/LOGGING.md](src/web-ui/LOGGING.md)
- 后端：[src/crates/LOGGING.md](src/crates/LOGGING.md)

### Tauri command

- command 名称：`snake_case`
- TypeScript 可以用 `camelCase` 包装，但调用 Rust 时要传结构化 `request`

```rust
#[tauri::command]
pub async fn your_command(
    state: State<'_, AppState>,
    request: YourRequest,
) -> Result<YourResponse, String>
```

```ts
await api.invoke('your_command', { request: { ... } });
```

### 平台边界

- 不要在 UI 组件里直接调用 Tauri API；应通过 adapter / infrastructure 层访问。
- 桌面端专属集成应放在 `src/apps/desktop`，再通过 transport / API layer 回流到共享逻辑。
- 在共享 core 中避免使用 `tauri::AppHandle` 等宿主 API；优先使用 `bitfun_events::EventEmitter` 等共享抽象。

### 远程兼容

- 新增功能时，从一开始就要考虑远程工作区和远程控制同步适配。只支持本地的行为很容易让远程场景功能缺失。
- 如果某个功能无法合理支持远程工作区，必须做能力屏蔽，或展示明确的不支持提示，不能让它以通用错误的形式失败。

### Agent loop 行为

- 不要把硬编码限制或模式判断作为处理 agent loop 循环问题的第一反应，例如仅按字符串或次数阻止重复工具调用。
- 过多硬编码会把 agent loop 变成脆弱的 workflow。应先定位根因：工具行为、模型交互、会话上下文封装、prompt/tool schema 设计，或状态同步问题。

## 架构

### Core 拆解护栏

任何 `bitfun-core` 拆解、feature 边界、依赖边界或 Rust 构建提速重构，
都必须先阅读
[`docs/architecture/core-decomposition.md`](docs/architecture/core-decomposition.md)。
该文档定义产品行为不变量、crate 归属目标、禁止依赖方向、feature 安全规则和里程碑验证门禁。

### 后端链路

大多数功能建议按这个顺序追踪：

1. `src/web-ui` 或应用入口
2. `src/apps/desktop/src/api/*` 或 server routes
3. `src/crates/api-layer`
4. `src/crates/transport`
5. `src/crates/core`

### `bitfun-core`

`src/crates/core` 是代码库中心。

主要区域：

- `agentic/`：agents、prompts、tools、sessions、execution、persistence
- `service/`：config、filesystem、terminal、git、LSP、MCP、remote connect、project context、AI memory
- `infrastructure/`：AI clients、app paths、event system、storage、debug log server

Agent 运行时心智模型：

```text
SessionManager → Session → DialogTurn → ModelRound
```

会话数据保存在 `.bitfun/sessions/{session_id}/`。

## 验证

| 改动类型 | 最低验证要求 |
|---|---|
| 前端 UI、状态、适配层或多语言文案 | `pnpm run lint:web && pnpm run type-check:web && pnpm --dir src/web-ui run test:run` |
| Deep Review / 代码审核团队行为 | 运行上面的前端验证，再运行 `cargo test -p bitfun-core deep_review -- --nocapture`；如果触及后端或 Tauri API，还需要运行下方 Rust / 桌面端验证 |
| `core`、`transport`、`api-layer` 或共享服务中的 Rust 逻辑 | `cargo check --workspace && cargo test --workspace` |
| 桌面端集成、Tauri API、browser/computer-use 或桌面专属行为 | `cargo check -p bitfun-desktop && cargo test -p bitfun-desktop` |
| 被桌面端 smoke/functional 流覆盖的行为 | `cargo build -p bitfun-desktop` 后运行最接近的 E2E spec，或 `pnpm run e2e:test:l0` |
| `src/crates/ai-adapters` | 运行上面相关 Rust 检查，**并且**运行 `cargo test -p bitfun-agent-stream` 验证 stream contract |
| 安装器应用 | `pnpm run installer:build` |

## 先看哪里

| 功能 | 关键路径 |
|---|---|
| Agent mode | `src/crates/core/src/agentic/agents/`、`src/crates/core/src/agentic/agents/prompts/`、`src/web-ui/src/locales/*/scenes/agents.json` |
| Deep Review / 代码审核团队 | `src/crates/core/src/agentic/deep_review_policy.rs`、`src/crates/core/src/agentic/agents/deep_review_agent.rs`、`src/crates/core/src/agentic/tools/implementations/{task_tool.rs,code_review_tool.rs}`、`src/web-ui/src/shared/services/reviewTeamService.ts`、`src/web-ui/src/flow_chat/services/DeepReviewService.ts`、`src/web-ui/src/app/scenes/agents/components/ReviewTeamPage.tsx` |
| 会话用量报告（`/usage`） | `src/crates/core/src/service/session_usage/`、`src/web-ui/src/flow_chat/components/usage/`、`src/web-ui/src/locales/*/flow-chat.json` |
| Tool | `src/crates/core/src/agentic/tools/implementations/`、`src/crates/core/src/agentic/tools/registry.rs` |
| MCP / LSP / remote | `src/crates/core/src/service/mcp/`、`src/crates/core/src/service/lsp/`、`src/crates/core/src/service/remote_connect/`、`src/crates/core/src/service/remote_ssh/` |
| 桌面端 API | `src/apps/desktop/src/api/`、`src/crates/api-layer/src/`、`src/crates/transport/src/adapters/tauri.rs` |
| 中继服务器 | `src/apps/relay-server/` |
| Web/server 通信 | `src/web-ui/src/infrastructure/api/`、`src/crates/transport/src/adapters/websocket.rs`、`src/apps/server/src/routes/`、`src/apps/server/src/main.rs` |

## Agent 文档优先级

进入具体目录后，优先遵循离目标文件最近的 `AGENTS.md` / `AGENTS-CN.md`。如果局部文档与本文件冲突，以更具体、更近的文档为准。

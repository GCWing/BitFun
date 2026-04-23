**中文** | [English](AGENTS.md)

# AGENTS-CN.md

## 概览

BitFun 是一个由 Rust workspace 与共享 React 前端组成的项目。

仓库核心原则：**先保持产品逻辑平台无关，再通过平台适配层对外暴露能力**。

- `src/crates/core`：共享产品逻辑中心
- `src/crates/transport`：Tauri / WebSocket / CLI 适配层
- `src/crates/api-layer`：共享处理器与 DTO
- `src/apps/desktop`：Tauri 桌面宿主应用
- `src/apps/server`：web 后端运行时
- `src/apps/cli`：CLI 运行时
- `src/web-ui`：桌面端与 server/web 共享前端
- `BitFun-Installer`：独立安装器应用
- `tests/e2e`：桌面端 E2E 测试

## 3 步快速上手

1. 在修改架构敏感代码前，先阅读 `README.md`、`CONTRIBUTING.md` 和本文件。
2. 共享前端改动后做桌面快速人工验证时，优先使用 `pnpm run desktop:preview:debug`（前提是已有较新的 debug 桌面二进制）；Rust / Tauri 改动后如果想要最快的本地重编译并预览循环，优先使用 `pnpm run desktop:preview:debug:rebuild`。`pnpm run desktop:dev` 保留给完整 Tauri dev 流程、首次初始化，或启动 / 构建链路本身的调试；仅浏览器前端验证时使用 `pnpm run dev:web`。
3. 改完后按下方最小验证集合执行检查。

## 核心命令

```bash
# 安装
pnpm install
pnpm run e2e:install

# 主要开发流程
pnpm run desktop:dev
pnpm run desktop:preview:debug
pnpm run desktop:preview:debug:rebuild
pnpm run dev:web
pnpm run cli:dev
pnpm run installer:dev

# 前端
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/web-ui run test:run
pnpm run build:web

# Rust
cargo check --workspace
cargo test --workspace
cargo test -p bitfun-core <test_name> -- --nocapture

# Desktop / E2E
cargo build -p bitfun-desktop
pnpm run e2e:test:l0
pnpm --dir tests/e2e exec wdio run ./config/wdio.conf.ts --spec "./specs/<file>.spec.ts"
```

## 本地桌面快速迭代

- `pnpm run desktop:preview:debug` 会启动或复用 web dev server，并直接拉起现有的 `target/debug/bitfun-desktop(.exe)`，不会经过 `tauri dev`；前端改动后的桌面人工检查优先用它。
- `pnpm run desktop:preview:debug:rebuild` 会先以 `CARGO_PROFILE_DEV_DEBUG=0` 和更高并行 codegen 重新构建 `bitfun-desktop`，再进入同样的快速预览流程；临时本地调试 Rust / Tauri 且更看重周转速度时优先用它。
- 上面两个命令只是本地迭代加速手段，不能替代下方与改动范围匹配的最小验证集合。
- 如果用户的意图是“快速看看效果”“本地跑起来看一下”这类人工预览，即使表述里同时出现了“编译”或“调试版本”，也优先使用上面的 preview 命令。
- `pnpm run desktop:build:fast` 只保留给“明确要一个 debug 构建产物，且不需要顺手启动预览”的场景。
- 意图示例：
  - “本地编译一个调试版本快速看看效果” -> `pnpm run desktop:preview:debug:rebuild`
  - “只编一个 debug 产物给我，不用启动” -> `pnpm run desktop:build:fast`

## 打包请求

- 当用户提出打包、release 或构建可分发桌面产物，但没有明确点名产物形式时，先确认目标打包类型，再执行构建。
- 要区分“本地临时产物”和“正式 release 交付物”。除非用户明确要求，否则不要把 `desktop:preview:*`、debug 构建，或 `--no-bundle` 的快速产物当成最终给用户分发的 release。
- 如果用户的语义明显是“给 Windows 最终用户安装”，优先使用 `pnpm run desktop:build:nsis`。
- 如果用户明确要“独立可执行文件”而不是安装器，优先使用 `pnpm run desktop:build:exe`。
- 如果用户已经明确点名目标格式，就不要重复确认，直接走对应打包流程。

## 架构

### 后端链路

大多数功能建议按这个顺序追踪：

1. `src/web-ui` 或应用入口
2. `src/apps/desktop/src/api/*` 或 server routes
3. `src/crates/api-layer`
4. `src/crates/transport`
5. `src/crates/core`

### `bitfun-core`

`src/crates/core` 是代码库中心。

重要区域：

- `agentic/`：agents、prompts、tools、sessions、execution、persistence
- `service/`：config、filesystem、terminal、git、LSP、MCP、remote connect、project context、AI memory
- `infrastructure/`：AI clients、app paths、event system、storage、debug log server

Agent 运行时心智模型：

```text
SessionManager → Session → DialogTurn → ModelRound
```

会话数据保存在 `.bitfun/sessions/{session_id}/`。

### 前端与桌面端边界

- `src/web-ui` 同时服务 Tauri 桌面端和 server/web
- 不要在 UI 组件里直接调用 Tauri API；应通过 adapter / infrastructure 层访问
- 仅桌面端集成应放在 `src/apps/desktop`，再通过 transport / API layer 回流到共享逻辑
- 在共享 core 中避免使用 `tauri::AppHandle` 等宿主 API；优先使用 `bitfun_events::EventEmitter` 等共享抽象

## 仓库规则

### 日志

日志必须只用英文，且不能使用 emoji。

- 前端：`src/web-ui/LOGGING.md`
- 后端：`src/crates/LOGGING.md`

示例：

```ts
const log = createLogger('ModuleName');
log.info('Loaded items', { count });
```

```rust
use log::{debug, error, info, trace, warn};
info!("Registered adapter for session {}", session_id);
```

### Tauri command

- command 名称：`snake_case`
- Rust 侧：`snake_case`
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

### 更细粒度规则

- 如果修改 `src/crates/ai-adapters`，需要运行 `src/crates/core/tests` 中的 stream integration tests
- 如果修改 `src/crates/core/src/agentic/execution/stream_processor.rs`，结束前需要运行 stream integration tests

## 先看哪里

- Agent mode：`src/crates/core/src/agentic/agents/`、`src/crates/core/src/agentic/agents/prompts/`、`src/web-ui/src/locales/*/scenes/agents.json`
- Tool：`src/crates/core/src/agentic/tools/implementations/`、`src/crates/core/src/agentic/tools/registry.rs`
- MCP / LSP / remote：`src/crates/core/src/service/mcp/`、`src/crates/core/src/service/lsp/`、`src/crates/core/src/service/remote_connect/`、`src/crates/core/src/service/remote_ssh/`
- 桌面端 API：`src/apps/desktop/src/api/`、`src/crates/api-layer/src/`、`src/crates/transport/src/adapters/tauri.rs`
- Web/server 通信：`src/web-ui/src/infrastructure/api/`、`src/crates/transport/src/adapters/websocket.rs`、`src/apps/server/src/routes/`、`src/apps/server/src/main.rs`

## 验证

| 改动类型 | 最低验证要求 |
| --- | --- |
| 前端 UI、状态、适配层或多语言文案 | `pnpm run lint:web && pnpm run type-check:web && pnpm --dir src/web-ui run test:run` |
| `core`、`transport`、`api-layer` 或共享服务中的 Rust 逻辑 | `cargo check --workspace && cargo test --workspace` |
| 桌面端集成、Tauri API、browser/computer-use 或桌面专属行为 | `cargo check -p bitfun-desktop && cargo test -p bitfun-desktop` |
| 被桌面端 smoke/functional 流覆盖的行为 | `cargo build -p bitfun-desktop` 后运行最接近的 E2E spec，或 `pnpm run e2e:test:l0` |
| `src/crates/ai-adapters` | 运行上面相关 Rust 检查，**并且**运行 `src/crates/core/tests` 中的 stream integration tests |
| 安装器应用 | `pnpm run installer:build` |

## Agent 文档覆盖

这是仓库级总指南。

规则优先级：

- 进入具体目录后，优先遵循离目标文件最近的 `AGENTS.md` / `AGENTS-CN.md`
- 如果局部文档与本文件冲突，以更具体、更近的文档为准

进入具体模块后，优先看最近的 agent 文档：

- `src/web-ui/AGENTS.md`
- `src/crates/core/AGENTS.md`
- `src/apps/desktop/AGENTS.md`
- `tests/e2e/AGENTS.md`
- `BitFun-Installer/AGENTS.md`
- `src/crates/ai-adapters/AGENTS.md`
- `src/crates/core/src/agentic/execution/AGENTS.md`


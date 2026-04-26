# 贡献指南

[English](./CONTRIBUTING.md)

感谢你对 BitFun 的兴趣！BitFun 是一个由 Rust 与 TypeScript 驱动的多端 AI 编程环境，桌面端/CLI/Server 共享核心逻辑。本指南说明如何高效参与贡献。

## 行为准则

请保持尊重、友善与建设性沟通。我们欢迎不同背景与经验的贡献者。

## 快速开始

### 环境准备

- Node.js（建议 LTS 版本）
- pnpm
- Rust toolchain（通过 rustup 安装）
- 桌面端开发需准备 Tauri 依赖

#### Windows：OpenSSL 配置

桌面端包含 SSH 远程功能，会链接 OpenSSL。Windows 上**不使用 OpenSSL 源码编译（vendored）**，需使用**预编译**库。

- **默认**：Windows 下 `pnpm run desktop:dev` 会调用 `ensure-openssl-windows.mjs`；`pnpm run desktop:preview:debug` 在需要为预览执行快速本地 `cargo build -p bitfun-desktop` 时，也会做同样的 OpenSSL 引导。所有 `desktop:build*` 均通过 `scripts/desktop-tauri-build.mjs` 执行，在 `tauri build` 前做相同引导（首次下载到 `.bitfun/cache/`，之后走缓存）。额外参数：`pnpm run desktop:build -- <tauri build 参数>`。
- **手动 / CI**：下载 [FireDaemon ZIP](https://download.firedaemon.com/FireDaemon-OpenSSL/openssl-3.5.5.zip)，解压后将 `OPENSSL_DIR` 指向 `x64`，并设 `OPENSSL_STATIC=1`，或运行 `scripts/ci/setup-openssl-windows.ps1`。
- **关闭自动下载**：设置 `BITFUN_SKIP_OPENSSL_BOOTSTRAP=1` 并自行配置 `OPENSSL_DIR`。
- **`desktop:dev:raw`** 不经过 `dev.cjs`（无 OpenSSL 引导）；请自行设置 `OPENSSL_DIR`、运行 `scripts/ci/setup-openssl-windows.ps1`，或执行 `node scripts/ensure-openssl-windows.mjs`（会预热 `.bitfun/cache/` 并打印可在 PowerShell 中粘贴的 `OPENSSL_*` 命令）。

### 安装依赖

```bash
pnpm install
```

### 常用命令

```bash
# Desktop
pnpm run desktop:dev
pnpm run desktop:preview:debug
pnpm run desktop:build

# E2E
pnpm run e2e:test
```

本地迭代时建议这样选：

- `desktop:preview:debug` 不经过 `tauri dev`；当已有 debug 桌面二进制仍然可复用时会直接预览，而在 Rust / Tauri 输入更新或二进制缺失时，会先用较轻的 dev 调试信息配置快速重编 `bitfun-desktop` 再预览。
- 只有在你明确想忽略时间戳复用判断、强制先重编时，才额外使用 `pnpm run desktop:preview:debug -- --force-rebuild`。
- `desktop:dev` 保留给完整的 Tauri watcher / 启动链路；正式收尾时，仍要按真实改动范围执行对应验证命令。

涉及打包与 release 时建议这样处理：

- 如果请求里只说“打包”或“release”，但没有明确产物类型，先确认目标输出形式。
- 对于 Windows 最终用户安装交付，优先使用 `pnpm run desktop:build:nsis`。
- 对于独立 Windows 可执行文件，只有在用户明确提出时才使用 `pnpm run desktop:build:exe`。
- 不要把本地快速/debug 产物与正式 release 交付物混淆。

> 说明：仓库提供更细粒度的脚本（例如 `dev:web`、`cli:dev`、`website:dev`），详情见 `package.json`。

## 代码规范与架构约束

### 日志规范

- 仅使用英文日志，避免冗长输出
- 前端：`createLogger('ModuleName')`
- 后端：`log::{info, debug, warn, error}` 宏

### 平台无关核心

`core` 中禁止引入平台相关依赖：

- ❌ `tauri::AppHandle`
- ✅ `bitfun_events::EventEmitter`

### Tauri 命令规范

- 命令名使用 `snake_case`
- Rust 与 TypeScript 命名保持一致
- 必须使用结构化请求格式：

```rust
#[tauri::command]
pub async fn your_command(
  state: State<'_, AppState>,
  request: YourRequest,
) -> Result<YourResponse, String>
```

```ts
await api.invoke("your_command", { request: { /* ... */ } });
```

## 重点关注的贡献方向
1. 贡献好的想法/创意(功能、交互、视觉等)，提交问题
    > 欢迎产品经理、UI设计师通过PI快速提交创意，我们会帮助完善开发
2. 优化Agent系统和效果
3. 对提升系统稳定性和完善基础能力
4. 扩展生态（SKill、MCP、LSP插件，或者对某些垂域开发场景的更好支持）



## 贡献流程与 PR 约定

### 除功能/修复外的贡献方向

我们欢迎不仅限于功能或修复的 PR。示例包括：

| 贡献方向 | 位置/文件 | 示例说明 |
| --- | --- | --- |
| Prompts | `src/crates/core/src/agentic/agents/prompts/` | 新增或优化提示词，并按需更新相关逻辑 |
| Tools | `src/crates/core/src/agentic/tools/implementations/`、`src/crates/core/src/agentic/tools/registry.rs` | 新增工具实现，并在工具注册表中注册 |
| Subagents | `src/crates/core/src/agentic/agents/custom_subagents/`、`src/crates/core/src/agentic/agents/registry.rs` | 新增子代理实现，并在子代理注册表中注册 |
| 模式贡献 | `src/crates/core/src/agentic/agents/*_mode.rs`、`src/crates/core/src/agentic/agents/prompts/*_mode.md`、`src/web-ui/src/locales/*/settings/modes.json` | 新增/优化 Agent 模式（例如 Plan/Debug/Agentic 或自定义模式）的逻辑与提示词，并同步前端模式文案 |
| Code Agent 与 AIIde 场景指南 | `website/src/docs/` | 补充流程、playbook 与真实场景说明（或从 `README.md` 链接） |

### 开始前

- 先开 Issue 说明问题或方案，尤其是较大改动，以避免重复与设计冲突
- 新功能或 UI 变更建议先讨论设计方向，确保符合产品体验

### PR 标题与描述

建议使用 Conventional Commits 风格，便于维护版本记录与自动化流程：

- `feat:` 新功能
- `fix:` 修复问题
- `docs:` 文档变更
- `chore:` 维护/依赖
- `refactor:` 重构且不改行为
- `test:` 测试相关

UI 改动请附前后对比截图或短录屏，方便快速评审。

如为 AI 辅助产出，请在 PR 中注明并说明测试程度（未测/轻测/已测），便于评审风险。

### 分支管理
**`main` 分支为默认协作分支，并接受特性 PR。** 本仓库欢迎产品经理、开发者使用 AI 生成代码进行快速验证或提交想法，因此 **所有 PR 请直接提交到 `main` 分支**。

### 变更范围

保持 PR 小而聚焦，避免混杂无关改动。

## 测试与验证

按改动范围运行相关测试：

```bash
# Rust
cargo test --workspace

# E2E
pnpm run e2e:test
```

如暂时无法运行测试，请在 PR 描述中说明原因，并提供手动验证步骤。

## 安全与合规

- 不要提交密钥、Token、证书或任何敏感信息
- 新增依赖请确认许可证兼容并说明用途

## 感谢

每一份贡献都很重要，欢迎提交 Issue、PR 或建议！

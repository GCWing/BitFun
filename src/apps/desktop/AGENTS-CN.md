**中文** | [English](AGENTS.md)

# AGENTS-CN.md

## 适用范围

本文件适用于 `src/apps/desktop`。仓库级规则请看顶层 `AGENTS.md`。

## 这里最重要的内容

`src/apps/desktop` 是 Tauri 宿主 / 集成层。

主要区域：

- `src/api/`：Tauri commands
- `src/lib.rs`、`src/main.rs`：应用启动与装配
- `src/computer_use/`：操作系统相关自动化支持

如果改动影响多个运行时共享的产品行为，真正实现通常应放在 `src/crates/core`。

## 本模块规则

- 保持 Tauri command 一致：名称使用 `snake_case`，调用使用结构化 `request`
- 桌面端专属集成留在这里，不要下沉到共享 core
- 本地临时调试时，无论是共享前端改动还是 Rust / Tauri 改动，都优先使用 `pnpm run desktop:preview:debug`；它会在现有 debug 二进制仍然可复用时直接预览，并在桌面侧输入更新或二进制缺失时自动重编后再预览。只有在需要完整 Tauri dev watcher，或正在排查启动 / 构建集成本身时，才回到 `pnpm run desktop:dev`
- 当表述里同时出现“编译/调试版本”和“快速看看效果/先看一下”时，按更高层的“预览”意图处理，优先使用 preview 命令，而不是 `pnpm run desktop:build:fast`

推荐命令形状：

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

## 命令

```bash
pnpm run desktop:dev
pnpm run desktop:preview:debug
cargo check -p bitfun-desktop
cargo test -p bitfun-desktop
cargo build -p bitfun-desktop
pnpm run desktop:build:fast
```

## 验证

```bash
cargo check -p bitfun-desktop && cargo test -p bitfun-desktop
```

如果改动影响启动、WebDriver、browser/computer-use 或打包行为，还需要运行：

```bash
cargo build -p bitfun-desktop
```

上面的 preview 命令只是迭代捷径，完成任务前仍要按要求执行最小 Rust 检查，以及必要的 build / E2E 验证。

只有在你明确想忽略时间戳复用判断、强制先重编再预览时，才使用 `pnpm run desktop:preview:debug -- --force-rebuild`。

`pnpm run desktop:build:fast` 只用于用户明确要 debug 构建产物、且不需要启动应用预览的场景。

涉及打包或 release 请求时：

- 如果用户没有明确说明要的是本地快速产物、独立可执行文件，还是安装器，先确认目标打包形式。
- 不要用 preview/debug 产物替代正式 release 交付物。
- 在 Windows 上，面向安装交付优先使用 `pnpm run desktop:build:nsis`；只有用户明确要独立可执行文件时，才使用 `pnpm run desktop:build:exe`。

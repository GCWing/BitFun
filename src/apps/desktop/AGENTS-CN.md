**中文** | [English](AGENTS.md)

# AGENTS-CN.md

## 适用范围

本文件适用于 `src/apps/desktop`。仓库级规则请看顶层 `AGENTS.md`。

## 这里最重要的内容

`src/apps/desktop` 是 Tauri 宿主 / 集成层。

主要区域：

- `src/api/`：Tauri commands
- `src/api/peer_host_invoke.rs`：Peer Device Mode host-invoke bridge 与 control attach
- `src/lib.rs`、`src/main.rs`：应用启动与装配
- `src/computer_use/`：操作系统相关自动化支持

Peer Device Mode 的所有权和边界见 `docs/architecture/peer-device-mode.md`。
前端防回归清单见 `src/web-ui/src/infrastructure/peer-device/README.md`。

账户登录（同步选择未完成前勿落盘）见 `src/api/remote_connect_api.rs`
（`PENDING_SYNC_CHOICE` / `account_finalize_login`）。
一键部署 Relay：`src/api/relay_deploy_api.rs`，不变量见
`src/web-ui/src/features/relay-deploy/README.md`。

如果改动影响多个运行时共享的行为，应把稳定契约、执行策略和服务放在各自的下层 owner
crate；`src/crates/assembly/core` 只保留产品装配与兼容桥接。

## 本模块规则

- 桌面端专属集成留在这里，不要下沉到共享 core
- 窗口 lifecycle 行为（包括 close/minimize-to-tray 默认值）属于桌面端 surface；修改时必须保留用户已保存偏好。
- 涉及打包或 release 请求时，参见顶层 `AGENTS.md`

## 命令

```bash
pnpm run desktop:dev
pnpm run desktop:preview:debug
cargo check -p bitfun-desktop
cargo test -p bitfun-desktop
cargo build -p bitfun-desktop
pnpm run desktop:build:fast
```

## 快速构建

| 命令 | 使用场景 |
|---|---|
| `pnpm run desktop:build:fast` | Debug 构建，不打包；手动测试时编译最快 |
| `pnpm run desktop:build:release-fast` | 类 Release 构建，降低 LTO；需要 release 行为但无法等待完整 LTO 时使用 |
| `pnpm run desktop:build:nsis:fast` | Windows 安装器，使用 `release-fast` profile；快速验证安装器 |

## Target 缓存 GC

`desktop:dev`（退出时）、`desktop:preview:debug`（关闭时）以及 `desktop:build*` 会裁剪过期的 `target/<profile>/incremental`（每个 crate 只留最新根）以及已无对应 `.fingerprint` 的孤儿 `deps`。**不会**按 mtime 删除 fingerprint（否则下次 `desktop:dev` 会冷编译）。手动执行：`pnpm run target:gc -- --profile debug`。禁用：`BITFUN_TARGET_GC=0`；演练：`BITFUN_TARGET_GC_DRY_RUN=1`。

`release-fast` profile（`Cargo.toml`）：继承 `release`，但关闭 LTO、`codegen-units` 提高到 16、启用增量编译。编译速度显著提升，代价是二进制体积增大和边际运行时性能下降。

## DevTools feature（模型规则）

`devtools` Cargo feature 用于桌面端 UI/UX 调试。添加或修改调试相关代码时：

- 所有调试专用 API 和 command 必须用 `#[cfg(any(debug_assertions, feature = "devtools"))]` 保护
- 在 `#[cfg(not(any(debug_assertions, feature = "devtools")))]` 下提供 no-op stub，确保 command 始终可以注册到 `invoke_handler`
- 该 feature 通过 `--features devtools` 在 `dev` 构建和 `release-fast` profile 构建中自动启用
- 面向最终用户的 `release` profile 构建中永不启用

## 验证

```bash
cargo check -p bitfun-desktop && cargo test -p bitfun-desktop
```

如果改动影响启动、WebDriver、browser/computer-use 或打包行为，还需要运行：

```bash
cargo build -p bitfun-desktop
```

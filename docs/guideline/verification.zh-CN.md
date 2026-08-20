# 验证矩阵

> 根 `AGENTS.md`（STD-08）配套文档。本文回答：**改完代码该跑哪类检查**。
> 命令列表见 [`common-commands.zh-CN.md`](common-commands.zh-CN.md)。
>
> [English](verification.md)

## 原则（来自本仓 AGENTS）

按 owner 选择验证，而不是按仓库级测试矩阵一刀切：

1. 先读离改动最近的 `AGENTS.md`，跑覆盖该行为的最窄命令。
2. 优先一个 package、一个 test target 或 module filter，以及最小 feature 集合。
   不要用 `product-full`、`all-features` 或 workspace 全量套件当捷径。
3. 仅当对应契约本身变化时才跑仓库级检查：布局/内容规则用 repository hygiene；
   workflow 改动用 GitHub config；Cargo feature、依赖方向或 test-target 布局用
   core boundaries。
4. 宽范围 build、workspace suite、打包与平台矩阵交给现有 CI；除非改动影响这些
   路径，或需要本地复现 CI 失败。

若某模块缺少有用的 focused 命令，应补到该模块指南，而不是扩张根 `AGENTS.md`。
不要预先对齐每个模块的测试清单；只有真实工作流需要时才记录命令。

## 矩阵

按实际改动的文件选**最小**本地预检。完整构建与宽范围测试默认由 CI 覆盖；只有改动直接影响构建、打包，或 CI 覆盖不到时，才在本地跑更重的命令。

| 改动类型 | 最低本地验证 |
|---|---|
| 前端 UI / 状态 / 适配（未改 i18n 资源或契约） | `pnpm run type-check:web`；行为有变再加最近的聚焦测试 |
| 仅 locale 资源 | `pnpm run i18n:audit` |
| locale 契约或共享 terms | `pnpm run i18n:generate && pnpm run i18n:contract:test && pnpm run i18n:audit` |
| Web UI i18n 运行时、命名空间加载，或直接调用 `i18nService.t(...)` | `pnpm run i18n:contract:test && pnpm run type-check:web && pnpm --dir src/web-ui run test:run src/infrastructure/i18n/core/I18nService.test.ts` |
| 主题、CSS 变量、widget payload、mobile、installer 或 CLI/TUI 颜色改动 | `pnpm run theme:color-audit:all` |
| Mobile web 的 UI、状态、配对、断开或重连 | `pnpm --dir src/mobile-web run type-check`；行为有变时补充手动配对 / 重连说明 |
| 产品定义、schema、resolver，或 Desktop/CLI 产品构建适配 | `pnpm run product:test`；默认定义再加 `pnpm run product:check` |
| `core` / `transport` / adapter / 共享服务中的 Rust | 先用最近模块 `AGENTS.md` 的命令；若未提供，则以最小 feature 运行 `cargo check -p <owning-package>`，行为有变再加最近的聚焦 `cargo test` |
| 桌面集成、Tauri API、browser/computer-use 或桌面专属行为 | `cargo check -p bitfun-desktop`；行为有变再加聚焦桌面测试 |
| 已有桌面 smoke / 功能流覆盖的行为 | 优先最近的聚焦 E2E / smoke；除非影响构建，宽范围交给 CI |
| `src/crates/adapters/ai-adapters` | 按上方相关 Rust 检查；仅当流契约变化时加 `cargo test -p bitfun-agent-stream` |
| 安装器前端或 i18n 运行时（未改打包） | `pnpm --dir BitFun-Installer run type-check` |
| 安装器 Tauri / Rust | `cargo check --manifest-path BitFun-Installer/src-tauri/Cargo.toml` |
| 安装器打包、payload、安装 / 卸载或原生捆绑 | `pnpm run installer:build` |
| 构建脚本或前置检查改动 | `pnpm run check:build-prereqs`；检查逻辑有变再加 `node --test scripts/check-build-prereqs.test.mjs` |
| Cargo feature、依赖方向或 test-target 布局 | `pnpm run check:core-boundaries` |
| 文档结构、索引、本地链接、锚点或命名 | `git diff --check`，并对照 [`docs/README.md`](../README.md) 与 [`docs-governance.zh-CN.md`](docs-governance.zh-CN.md) 人工检查（本仓尚未提供 `docs:links:check` / `docs:architecture:check`） |

# 常用命令

> 根 `AGENTS.md` 的配套文档。本文只列**常用命令**，不回答「改完该跑哪条」。
> 改完代码请查 [`verification.zh-CN.md`](verification.zh-CN.md)。
>
> [English](common-commands.md)

```bash
# 安装
pnpm install

# 开发
pnpm run desktop:dev               # 完整热更新：Vite HMR + Rust 自动重编并重启
pnpm run desktop:preview:debug     # 复用预构建二进制 + Vite HMR；Rust 不会自动重编
pnpm run dev:web                   # 仅浏览器前端
pnpm run cli:dev                   # CLI 运行时
pnpm run cli:install               # release 构建并安装 bitfun（Windows/macOS/Linux；含已弃用的 bitfun-cli）

# 检查
pnpm run fmt:rs                     # 只格式化已改 / 已暂存的 Rust 文件
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/mobile-web run type-check
pnpm run i18n:generate
pnpm run i18n:contract:test          # 仅 i18n 契约 / 资源
pnpm run i18n:audit                  # 仅 i18n 契约 / 资源
pnpm run theme:color-audit:all
pnpm run product:check               # 默认产品定义
pnpm run check:repo-hygiene
pnpm run check:github-config
pnpm run check:core-boundaries
pnpm run check:build-prereqs
cargo check --workspace

# 测试（本地优先精确路径；大范围交给 CI）
pnpm run product:test
pnpm --dir src/web-ui run test:run
cargo test --workspace

# 构建（构建相关改动，或需要本地复现 CI）
cargo build -p bitfun-desktop
pnpm run build:web
pnpm run build:mobile-web

# 快速构建（手动构建 / 调试）
pnpm run desktop:build:fast
pnpm run desktop:build:release-fast
pnpm run desktop:build:nsis:fast
```

### 构建逃生舱

| 变量 / 开关 | 何时使用 |
| --- | --- |
| `CARGO_PROFILE_DEV_DEBUG=2` | 需要完整调试信息打断点。dev profile 默认是 `line-tables-only`。 |
| `BITFUN_MOBILE_WEB_FORCE_BUILD=1` 或 `node scripts/mobile-web-build.cjs --force` | 即使 `src/mobile-web/dist` 看起来仍新，也强制重编 mobile-web。 |
| `VITE_USE_POLLING=1` | Vite watcher 漏改动（网络盘 / WSL 挂载）。 |

`pnpm run build:web` 会并发跑 type-check 与 Vite；错误前缀分别为 `[type-check]` / `[vite-build]`。

本仓当前**没有** `docs:links:check` / `docs:architecture:check`。
文档改动请用 `git diff --check`，并对照 [`docs/README.md`](../README.md) 与
[`docs-governance.zh-CN.md`](docs-governance.zh-CN.md) 做人工链接检查。

完整脚本列表见 [`package.json`](../../package.json)。

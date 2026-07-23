**中文** | [English](AGENTS.md)

# AGENTS-CN.md

## 适用范围

本文件适用于 `src/web-ui`。仓库级规则请看顶层 `AGENTS.md`。

## 这里最重要的内容

`src/web-ui` 是共享前端，对应两种运行时：

- Tauri 桌面端
- 通过 WebSocket / Fetch 适配层访问的 server/web

大多数改动从这些位置开始：

- `src/infrastructure/`：adapters、i18n、theme、providers、config
- `src/infrastructure/peer-device/`：Peer Device Mode transport switch 与 host-invoke bridge
- `src/app/`：应用外壳与顶层装配
- `src/flow_chat/`：聊天流 UI 与状态
- `src/tools/`：editor、terminal、git、workspace、file explorer
- `src/shared/`：共享 services、stores、helpers、types
- `src/locales/`：多语言文案

Peer Device Mode（同账号远程完整客户端）的边界见 `docs/architecture/peer-device-mode.md`。
前端不变量见 `src/infrastructure/peer-device/README.md`。不要重新引入内嵌会话/聊天壳；
应从设备列表（Remote Connect 的「我的 BitFun」组）进入 peer mode。

一键部署 Relay：`src/features/relay-deploy/`（见其 README）。Remote Connect「我的 BitFun」
登录表单与 Self-Hosted 入口必须打开 `RelayDeployWizard`，不要改成外链 README。

## 本模块规则

- 不要在 UI 组件里直接调用 Tauri API；应通过 adapter / infrastructure 层访问
- 新增前端基础设施前，先复用已有的 theme、i18n、component-library 和 Zustand stores
- 主题与颜色 Token 改动遵循 `docs/architecture/theme-token-optimization.md`。审计失败应通过复用 Token、
  收敛冗余值或增加最小 owner contract 修复，不得仅为通过检查提高 baseline 或测试期望；跨形态改动运行
  `pnpm run theme:color-audit:all`。
- Locale 元数据只在生成式 i18n contract 中维护。修改 `src/shared/i18n/contract/locales.json` 后运行
  `pnpm run i18n:generate`，Web UI 文案留在 `src/web-ui/src/locales`。
- 路由或功能文案使用 `useI18n(namespace)` 保持非 bootstrap namespace 懒加载；直接调用
  `i18nService.t(...)` 必须有 bootstrap namespace 覆盖。
- 遵循 `src/web-ui/LOGGING.md`：仅英文、无 emoji、结构化日志

## 命令

以下命令仅供参考，不是默认预检清单；PR 应按下方“验证”选择范围。

```bash
pnpm --dir src/web-ui dev
pnpm --dir src/web-ui run lint
pnpm --dir src/web-ui run type-check
pnpm --dir src/web-ui run test:run     # 大范围测试；本地优先用精确路径
pnpm run i18n:contract:test
pnpm run i18n:audit
pnpm run build:web                     # 构建相关改动或复现 CI
```

## 验证

按改动范围选择最小检查：

```bash
pnpm run i18n:audit
pnpm run i18n:generate && pnpm run i18n:contract:test && pnpm run i18n:audit
pnpm run type-check:web && pnpm --dir src/web-ui run test:run src/infrastructure/i18n/core/I18nService.test.ts
pnpm run type-check:web
```

以上依次用于 locale 资源、locale contract/shared terms、i18n runtime/namespace loading 和普通 Web UI 代码。
完整 lint、build 与大范围测试由 CI 兜底，除非本地改动确实需要复现。

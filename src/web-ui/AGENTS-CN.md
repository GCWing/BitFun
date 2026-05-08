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
- `src/app/`：应用外壳与顶层装配
- `src/flow_chat/`：聊天流 UI 与状态
- `src/tools/`：editor、terminal、git、workspace、file explorer
- `src/shared/`：共享 services、stores、helpers、types
- `src/locales/`：多语言文案

## 本模块规则

- 不要在 UI 组件里直接调用 Tauri API；应通过 adapter / infrastructure 层访问
- 新增前端基础设施前，先复用已有的 theme、i18n、component-library 和 Zustand stores
- 遵循 `src/web-ui/LOGGING.md`：仅英文、无 emoji、结构化日志

## 命令

```bash
pnpm --dir src/web-ui dev
pnpm --dir src/web-ui run lint
pnpm --dir src/web-ui run type-check
pnpm --dir src/web-ui run test:run
pnpm run build:web
```

## 验证

```bash
pnpm run lint:web && pnpm run type-check:web && pnpm --dir src/web-ui run test:run
```

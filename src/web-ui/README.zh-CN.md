# BitFun Web UI

中文 | [English](./README.md)

## 概述

本目录是 BitFun 的 **Web UI**（React + TypeScript）。同一份前端代码会被复用在：

- **Desktop**：通过 **Tauri** 加载运行
- **Server/Web**：构建为静态资源，由后端提供访问

## 技术栈

- React 18.3
- TypeScript 5.8
- Vite 7
- SCSS
- Zustand（状态管理）
- Monaco Editor

## 目录结构

```
src/web-ui/
├── README.md                     # 英文版说明
├── README.zh-CN.md               # 本文件（中文版）
├── LOGGING.md                    # 日志与调试说明
├── index.html                    # 入口 HTML
├── preview.html                  # 预览页（可选）
├── package.json                  # 依赖与脚本
├── package-lock.json             # 锁定依赖版本
├── public/                       # 静态资源
├── src/                          # 前端源代码
│   ├── app/                      # 应用主界面
│   ├── component-library/        # 组件库
│   ├── features/                 # 按功能拆分的模块
│   ├── flow_chat/                # 对话/工作流聊天界面
│   ├── generated/                # 生成内容（占位/产物）
│   ├── hooks/                    # 通用 hooks
│   ├── infrastructure/           # 基础设施（API/i18n/主题等）
│   ├── locales/                  # 文案与翻译资源
│   ├── shared/                   # 共享工具与类型
│   ├── tools/                    # 工具 UI（编辑器/终端/Git 等）
│   ├── main.tsx                  # 应用入口
│   └── vite-env.d.ts             # Vite 类型声明
├── tsconfig.json                 # TS 配置
├── tsconfig.node.json            # Node/Vite TS 配置
├── vite.config.ts                # Vite 构建配置
├── vite.config.preview.ts        # 预览构建配置
└── vite.config.version-plugin.ts # 版本插件
```

## 前端通信层架构

### 核心设计

同一份 UI 代码支持两种运行形态：

- **Desktop**：Tauri API（`invoke`, `listen`）
- **Server/Web**：WebSocket / Fetch API

### 适配器模式（概念示例）

```ts
const adapter = IS_TAURI ? TauriAdapter : WebSocketAdapter;

await adapter.request("execute_agent_task", params);
adapter.listen("agentic://text-chunk", callback);
```

## 开发指南

### 启动开发服务器

```bash
# Desktop
pnpm --dir src/web-ui run dev

# Server/Web
VITE_BUILD_TARGET=web pnpm --dir src/web-ui run dev
```

### 构建

```bash
# Desktop
pnpm --dir src/web-ui run build

# Server/Web
VITE_BUILD_TARGET=web pnpm --dir src/web-ui run build
# 产物：dist/
```

## 相关文档（本包内）

- [日志说明](LOGGING.md)
- [组件库 README](src/component-library/README.md)
- [i18n README](src/infrastructure/i18n/README.md)

## 注意事项

1. **不要在组件里直接调用 Tauri API**，应通过适配器层统一封装。
2. **注意 Web 兼容性**（浏览器环境不一定具备所有能力）。
3. **优先使用 CSS 变量**，避免硬编码颜色/尺寸。

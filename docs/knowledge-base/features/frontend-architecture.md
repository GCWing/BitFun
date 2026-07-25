# Frontend Architecture

## web-ui

- **框架**: React 18 + TypeScript, 构建工具 Vite 7
- **路由**: 自定义 Scene Tab 路由系统（无 react-router）。通过 `SceneViewport` 组件 + `SCENE_TAB_REGISTRY` 注册场景，使用 `react` 状态管理标签切换，支持懒加载（`React.lazy`/`Suspense`）。场景包括: welcome, session, terminal, git, settings, file-viewer, profile, agents, skills, miniapps, pages, browser, assistant, insights, shell, panel-view
- **状态管理**: Zustand v5 + immer（不可变更新）
- **桌面支持**: Tauri v2 API 集成（FS、dialog、autostart、notification、opener、log）
- **国际化**: i18next v25 + react-i18next，支持 zh-CN / zh-TW / en-US

### 主要模块

| 模块 | 说明 |
|------|------|
| `app/` | 核心应用层：布局（AppLayout）、场景视图（SceneViewport）、启动流程（deferred startup system）、stores、场景组件 |
| `app/scenes/` | 各个 Scene 的具体实现（session/terminal/git/settings 等） |
| `component-library/` | 通用 UI 组件库及样式系统 |
| `features/` | 功能模块：relay-deploy、ssh-remote |
| `flow_chat/` | AI 对话流核心：状态机、reducers、事件系统、tool-cards、deep-review |
| `infrastructure/` | 基础设施：API 客户端、账户、MCP、i18n、主题、运行时、语音、事件总线、peer-device、update |
| `shared/` | 共享层：常量、工具函数、类型、stores、通知系统、主题、上下文菜单系统、加密、AI 错误处理 |
| `tools/` | 工具实现：编辑器（Monaco）、终端（xterm）、git、文件浏览器、LSP、mermaid、workspace、快照等 |
| `locales/` | 多语言翻译文件 (zh-CN, zh-TW, en-US) |
| `generated/` | 生成代码 |
| `hooks/` | 全局 React hooks |
| `test/` | 测试配置 |

### 关键依赖

- `react` / `react-dom` ^18.3 — UI 框架
- `zustand` ^5.0 — 状态管理
- `immer` ^11.1 — 不可变数据
- `i18next` / `react-i18next` — 国际化
- `@tauri-apps/api` ^2.10 — 桌面集成
- `@monaco-editor/react` / `monaco-editor` — 代码编辑器
- `@xterm/xterm` — 终端模拟器
- `@tiptap/react` / `@tiptap/starter-kit` — 富文本编辑器
- `mermaid` / `katex` / `react-markdown` / `remark-gfm` — 内容渲染
- `lucide-react` — 图标库
- `vite` ^7.0 — 构建工具
- `sass` — CSS 预处理器
- `typescript` ^5.8 — 类型系统

---

## mobile-web

- **框架**: React 18 + TypeScript, 构建工具 Vite 7
- **路由**: 自定义页面路由，基于 `useState<Page>` 控制页面切换，集成 `history.pushState`/`popstate` 以支持浏览器返回键。页面包含: pairing（配对）、workspace（工作区）、sessions（会话列表）、chat（聊天）、devices（设备管理），使用 CSS transition 动画实现 push/pop 导航效果
- **状态管理**: Zustand v5（`useMobileStore`）
- **网络**: 通过 Relay HTTP 客户端 + WebSocket 与远程服务通信，支持端到端加密
- **国际化和主题**: 自建 I18nProvider / ThemeProvider

### 主要模块

| 模块 | 说明 |
|------|------|
| `pages/` | 页面组件：PairingPage、SessionListPage、ChatPage、WorkspacePage、DevicesPage |
| `components/` | 通用组件：ErrorBoundary、LanguageToggleButton |
| `services/` | 网络及数据服务：RelayHttpClient、RemoteSessionManager、E2E encryption、store（Zustand）、delegatedAccountOwner |
| `hooks/` | 自定义 hooks：useConnectionHealth、useControlTargetEpoch |
| `theme/` | 主题系统：ThemeProvider、useTheme、主题预设 |
| `i18n/` | 国际化：I18nProvider、翻译 hook |
| `styles/` | 全局样式，含 motion 动画和组件样式 |
| `assets/` | 静态资源 |

### 关键依赖

- `react` / `react-dom` ^18.3 — UI 框架
- `zustand` ^5.0 — 状态管理
- `react-markdown` / `remark-gfm` — Markdown 渲染
- `react-syntax-highlighter` — 代码高亮
- `@noble/ciphers` / `@noble/curves` — 端到端加密
- `vite` ^7.0 — 构建工具
- `sass` — CSS 预处理器
- `typescript` ^5.8 — 类型系统

---

## 架构对比

| 维度 | web-ui | mobile-web |
|------|--------|------------|
| 路由方式 | Scene Tab 系统（多标签页） | 自定义 Page 状态路由（单页栈） |
| 桌面集成 | Tauri v2（Deep） | 无 |
| 状态管理 | Zustand + immer | Zustand |
| 人数服务 | MCP、agent、speech、peer-device | Relay HTTP + WebSocket、E2E |
| UI 复杂度 | 编辑器、终端、git、文件浏览等工具套件 | 简单页面，以聊天为核心 |
| 国际化 | i18next + react-i18next（完整） | 自建 I18nProvider（基础） |
| 构建策略 | 多模式构建（desktop/web） | 单一构建 |

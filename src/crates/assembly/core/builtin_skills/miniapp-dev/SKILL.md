---
name: miniapp-dev
description: 'Generate and refine BitFun MiniApps. Use when the user wants a new MiniApp, wants an existing MiniApp redesigned or extended, or asks for a BitFun in-app tool. Typical triggers: "做一个小应用", "生成 MiniApp", "写个 BitFun 小工具", "创建 mini app".'
---

# BitFun MiniApp 生成指南

本技能用于**为用户生成、改造、完善一个 MiniApp**：

- 做一个新的 BitFun 小应用
- 修改某个 MiniApp 的交互、界面、能力、数据流
- 把一个想法变成可运行的 MiniApp

开始生成新的 MiniApp 前，先读 [`design-playbook.md`](design-playbook.md)；运行时能力和宿主 API 细节再查 [`api-reference.md`](api-reference.md)。

## 目标

**交付一个能在 BitFun 里运行、风格合适、权限最小、结构清晰的 MiniApp**。

成功标准：

- 用户的问题被这个 MiniApp 直接解决
- 生成结果能在 MiniApp 场景里运行
- 只申请必要权限
- 不假设不存在的宿主 API
- 在 light/dark、zh/en 下都可用

## 先做什么

在写代码前，先完成这 4 件事：

1. 明确用户目标
   这个 MiniApp 是工具型、展示型，还是混合型？核心动作是什么？

2. 找最近的参考
   看 `references/examples/` 中最贴近任务形态的内置/示例 MiniApp 目录

3. 选运行模式
   先判断是否真的需要 `worker.js` 和 `node.enabled = true`。

4. 定最小交付面
   第一版只做最核心路径，不为了“看起来完整”堆功能。

## 生成流程

### 1. 先澄清，再实现

如果下面任一项不清楚，先问清楚，不要替用户脑补：

- 解决什么问题
- 谁使用
- 要读写哪些路径
- 要不要读工作区文件
- 要执行哪些命令
- 要不要执行命令
- 要访问哪些域名
- 要不要联网
- 要不要持久化状态
- 要不要多语言
- 要不要 Tweaks 这类运行时可调变体
- 有没有现成视觉参考

### 2. 优先复用现有 MiniApp 语言

不要从零发明一套 BitFun 风格。先从已有 MiniApp 中借鉴：

- 布局密度
- 圆角和间距
- 卡片和面板结构
- 主题变量使用方式
- i18n 组织方式

默认优先做**工具型**设计：冷静、克制、信息密度高、操作路径短。

### 3. 优先选“无 Node 模式”

如果需求只靠这些能力就能完成：

- `app.fs.*`
- `app.shell.exec`
- `app.net.fetch`
- `app.os.info`
- `app.storage.*`

那么优先使用：

```json
{
  "permissions": {
    "node": { "enabled": false }
  }
}
```

只有在这些场景下才启用 `node.enabled = true`：

- 需要自定义 `worker.js` 方法
- 需要 npm 依赖
- 需要较长链路或较复杂的后台逻辑

### 4. 用 `InitMiniApp` 创建骨架

创建后，围绕这些文件工作：

- `index.html`
- `style.css`
- `ui.js`
- `worker.js`（只有需要时）
- `meta.json`

默认做法：

- `index.html` 只放清晰结构
- `style.css` 先声明设计系统
- `ui.js` 负责状态、渲染、事件、i18n
- `worker.js` 只承载真正需要后台执行的逻辑

### 5. 只使用真实存在的宿主能力

MiniApp 里可用的是 `window.app`。

默认可依赖的能力：

- `app.fs.*`
- `app.shell.exec`
- `app.net.fetch`
- `app.os.info`
- `app.storage.get/set`
- `app.dialog.*`
- `app.clipboard.*`
- `app.ai.*`
- `app.theme`
- `app.locale`
- `app.onThemeChange`
- `app.onLocaleChange`
- `app.t(...)`
- `app.call(...)` 仅在 `node.enabled = true` 时

详细接口查：

- [`api-reference.md`](api-reference.md)

### 6. 不要假设这些 API 存在

默认**不要**写这些不存在的接口：

- `app.bitfun.*`
- `app.workspace.*`
- `app.git.*`
- `app.session.*`
- `app.terminal.*`
- `app.browser.*`

如果你需要 Git 能力，优先：

```javascript
await app.shell.exec('git ...', { cwd: app.workspaceDir })
```

如果你需要工作区数据，优先：

```javascript
await app.fs.readFile(...)
```

### 7. 从第一版就带上 i18n 和 theme

不要把多语言和主题适配留到最后。

至少做到：

- `meta.json` 带 `i18n.locales`
- 静态文案可重渲染
- 动态文案走 `app.t(...)` 或自有 `I18N` 表
- 样式优先使用 `--bitfun-*`
- 测试 light/dark + zh/en

### 8. 先做核心体验，不补假内容

如果缺素材、图标、真实数据：

- 用明确占位
- 用 fixture 数据
- 用“待补”标记

不要：

- 硬画劣质插画
- 编造业务数据
- 用装饰性内容填空白

## 硬约束

### 交互

- 首屏就要能理解用途
- 主路径操作数尽量少
- 点击区域至少 32px
- 正文不要小于 13px

### 视觉

- 禁止默认蓝紫渐变 AI 风背景
- 禁止 emoji 充当主图标
- 禁止“每块一个风格”
- 禁止堆无意义 stats、sparkline、装饰 icon

### 代码

- 不需要 `worker.js` 时不要启用 Node
- 不需要的权限不要申请
- 不要把大量逻辑塞进 HTML
- `ui.js` 过长时主动拆成模块化结构

### 内容

- 不为填空白加内容
- 每个 section 都要有明确用途
- 不擅自扩 scope

## 你应该参考什么

生成前优先阅读最贴近的一两个参考，而不是全看：

- `references/examples/demo-git-graph/`
- `references/examples/demo-icon-design-system/`
- `references/examples/builtin-regex-playground/`
- `references/examples/builtin-coding-selfie/`
- `references/examples/builtin-gomoku/`
- `references/examples/builtin-daily-divination/`

生成新的 MiniApp 时，默认先读：

- [`design-playbook.md`](design-playbook.md)

如果任务偏运行时调用，再看：

- [`api-reference.md`](api-reference.md)

## 交付前检查

交付前至少确认：

- MiniApp 能运行
- 主路径可操作
- 权限是最小集
- `node.enabled` 选择合理
- 没有调用不存在的 `app.*` API
- i18n 至少覆盖 `zh-CN` / `en-US`
- light/dark 没有明显样式问题
- 没有遗留 “TODO / 占位 / Lorem ipsum”

# 字体大小设置（Font Size Setting）需求文档

> 状态：提案 / 未做
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`src/web-ui/src/infrastructure/appearance/appearanceDomainTokens.ts`](../../src/web-ui/src/infrastructure/appearance/appearanceDomainTokens.ts)
> - [`src/web-ui/src/app/styles/nav-panel-font-scope.scss`](../../src/web-ui/src/app/styles/nav-panel-font-scope.scss)
> - [`src/web-ui/src/features/relay-deploy/RelayDeployWizard.tsx`](../../src/web-ui/src/features/relay-deploy/RelayDeployWizard.tsx)
> - [`src/web-ui/AGENTS.md`](../../src/web-ui/AGENTS.md)

## 背景与需求描述

BitFun 桌面端的字号经 appearance token 体系驱动（`--bf-appearance-token-font-size-*` 分级），导航面板经 `nav-panel-font-scope.scss` 在 flow-chat 字号基础上降一档映射，个别场景（如 relay-deploy 终端）用独立 `DEPLOY_TERMINAL_FONT_SIZE`。但**缺少用户可调的全局字号 / 缩放设置**：用户无法根据视力、屏幕 DPI、个人偏好统一放大或缩小应用字号，只能接受 appearance 包预设。

当前缺口与诉求：

- 无用户可调的全局字号缩放（zoom / font scale）；
- 编辑器、终端、聊天三个长阅读区无法分别调字号；
- 高 DPI / 小屏下预设字号偏小，用户无法自助调整；
- 无障碍场景（视力差异）缺乏字号自适应。

本提案新增**用户字号设置**：全局缩放 + 每个长阅读区（编辑器 / 终端 / 聊天）独立字号，复用既有 appearance token 体系，不新造第二套字号变量。

## 期望行为

### 1. 全局字号缩放

- 在设置中提供全局字号缩放（如 80%–150% 或档位 sm/md/lg/xl），作用于 appearance 字号 token 基线；
- 缩放经 appearance runtime 即时刷新全应用，不需重启；
- 有上下限，避免过大破版或过小不可读。

### 2. 每阅读区独立字号

- 编辑器字号（Monaco `fontSize` / `lineHeight`）独立可调；
- 终端字号（xterm `fontSize`，对齐 `DEPLOY_TERMINAL_FONT_SIZE` 先例）独立可调；
- 聊天字号（flow-chat 字号 token）独立可调；
- 各区字号可"跟随全局"或"自定义"，自定义优先级高于全局。

### 3. 持久化与外观集成

- 字号设置持久化到 config（如 `app.fontSize` / `appearance.fontSize.*`）；
- 经 appearance runtime / compiler 投影到 CSS 变量与 Monaco / xterm 选项；
- 切换 appearance 包时保留用户字号覆盖；
- nav-panel-font-scope 的降档映射随全局基线同步缩放，保持相对关系。

### 4. 可访问性与边界

- 字号缩放不影响布局网格 / 间距 token（只缩字号族）；
- 极端字号下布局不破：聊天输入、面板、对话框保持可用（吸取 F4 / F7 逐帧重排教训，避免缩放触发全工作区重渲染）；
- 键盘可达等价路径（设置入口可键盘聚焦）。

### 5. i18n 与默认值

- 默认值随 appearance 包（已有的字号 token 基线）；
- 文案走 i18n（`appearance.fontSize.*`），shared terms 进 `src/shared/i18n`。

## 非目标 / 范围外

- 不做 appearance 包 / 主题包的作者编辑器（属 appearance / 主题）；
- 不改主题颜色 token（颜色审计独立）；
- 不做每组件任意字号（已有 token 体系，不做更细粒度）；
- 不覆盖 CLI/TUI 字号（属 `src/apps/cli/themes`）；
- 不做字号跨设备云同步（属 remote connect）。

## 建议的落地路径（基于现有分层）

1. **Appearance tokens (`src/web-ui/src/infrastructure/appearance/appearanceDomainTokens.ts`)** — 字号 token 基线，作为缩放锚点。
2. **Appearance runtime / compiler (`src/web-ui/src/infrastructure/appearance/runtime/AppearanceService.ts`、`compiler/AppearanceCompiler.ts`)** — 应用全局缩放与每区覆盖，投影到 CSS 变量。
3. **适配器 (`src/web-ui/src/infrastructure/appearance/adapters/MonacoAppearanceAdapter.ts`、`XtermAppearanceAdapter.ts`)** — 字号投影到 Monaco / xterm 选项。
4. **Config** — 持久化 `app.fontSize` / `appearance.fontSize.*`。
5. **设置 UI (`src/web-ui/src/app/scenes/settings/`)** — 字号缩放与每区字号控件。
6. **nav-panel-font-scope** — 随全局基线同步降档。

### 分层与依赖边界要点

- UI 组件不直调 Tauri；字号纯前端 appearance 体系；
- 复用 appearance token 与 runtime，不新造第二套字号变量；
- 颜色 token 不变，颜色审计不受影响；
- 缩放不触发全工作区逐帧重渲染（CSS 变量驱动，吸取 F4 教训）；
- i18n 文案走命名空间懒加载。

## 设计草案 / 参考示例

- **token 锚点参考**：`--bf-appearance-token-font-size-*`（xxs–4xl）分级，作为缩放基线。
- **降档映射参考**：`nav-panel-font-scope.scss` 在 flow-chat 字号基础上降一档，保持相对关系。
- **独立字号先例**：`DEPLOY_TERMINAL_FONT_SIZE` 为终端独立字号提供先例。
- **适配器参考**：`MonacoAppearanceAdapter` / `XtermAppearanceAdapter` 已投影字体 / 字号，扩展即可。
- **交互参考**：常见 IDE 的"字号缩放（Ctrl +/-）+ 设置面板每区字号"。

## 是否愿意贡献

- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

## 补充说明

- 与 appearance 的关系：复用字号 token 与 runtime，不新造第二套变量；切换 appearance 包保留用户覆盖。
- 与可访问性的关系：字号缩放是无障碍基础项，须保证极端字号下不破版。
- 与终端 / 编辑器的关系：经 `XtermAppearanceAdapter` / `MonacoAppearanceAdapter` 投影，不绕过 appearance。
- 相关分层入口：`src/web-ui/AGENTS.md`、`docs/architecture/theme-token-optimization.md`。

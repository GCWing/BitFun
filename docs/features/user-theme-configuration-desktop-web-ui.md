# 用户主题配置（Desktop / Web UI）功能提案

> 状态：提案 / 待评审
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/architecture/theme-token-optimization.md`](../architecture/theme-token-optimization.md)
> - [`docs/architecture/appearance-package-system.md`](../architecture/appearance-package-system.md)
> - [`src/web-ui/AGENTS.md`](../../src/web-ui/AGENTS.md)
> - [`docs/architecture/product-architecture.md`](../architecture/product-architecture.md)

### 功能类型
<!-- 请描述功能建议的类型 -->
- [x] 新功能
- [ ] 现有功能增强
- [ ] 用户体验优化
- [ ] 性能优化
- [ ] 接口/集成扩展
- [ ] 其他

### 优先级
<!-- 请选择功能建议的优先级 -->
- [ ] 紧急（P0 - 核心需求，强烈期望实现）
- [ ] 高（P1 - 重要功能，期望尽快实现）
- [x] 中（P2 - 有价值的功能，计划实现）
- [ ] 低（P3 - 锦上添花，有时间再做）

### 背景与动机
<!-- 请描述你为什么需要这个功能，解决了什么痛点 -->
- BitFun GUI 主题目前只能在内置固定主题间切换，用户无法按个人偏好定制颜色：品牌色、护眼色、自定义深色变体、强调色偏好等都无法表达。用户对「我希望界面用自己的配色」这一基础诉求没有出口，只能被动接受预设。
- 痛点：① 无法自定义 semantic 颜色（背景/文本/边框/交互/状态/accent），个性化需求无处满足；② 无法保存多个自定义主题并在其间切换；③ 自定义主题不能导入导出/分享，团队或设备间无法复用；④ 缺少可访问性提示，用户随手配色可能造成对比度不足、状态色混淆。

### 功能描述
<!-- 请详细描述你期望的功能行为 -->
- 在 Desktop/Web UI 新增「用户主题配置」能力，覆盖六个环节：
  1. **主题编辑器**：支持基于内置包派生或从空白创建自定义主题；按 Token 分层编辑 semantic 颜色（背景、文本、边框、交互、状态、accent），并在编辑时实时预览（消费运行时 CSS 变量，不在 SCSS 编译期复制颜色）。
  2. **保存与持久化**：自定义主题可命名、复制、删除；持久化到 `appearance.selection`，支持保存多个自定义包并在其间切换；内置包只读，不可覆盖。
  3. **导入 / 导出**：自定义主题包支持导入与导出（与既有包 schema 一致），导入时按包校验规则校验，失败给出可读错误而非静默丢弃。
  4. **校验与可访问性**：保存时校验 Token 完整性（必需 semantic 角色齐全、无悬空引用）；对关键文本/背景组合给出对比度与可访问性提示，但不强制阻断用户最终选择。
  5. **应用与切换**：可将自定义主题设为当前主题，原子应用运行时 CSS 变量；首屏遵循既定规则——Rust 首屏使用系统默认启动色，JS 启动后原子应用完整包，避免闪烁。
  6. **降级与可观测**：自定义包损坏或 Token 缺失时降级到内置默认主题，状态视图显式标注「当前为降级态 + 原因」，不静默回退。
- 实现边界遵循主题治理规则：主题 owner 仍是 Web UI Appearance（`src/web-ui/src/infrastructure/appearance`），包 schema / 校验 / 唯一运行时不变；只持久化并解析 `appearance.selection`；普通组件优先消费运行时 CSS 变量，动态 CSS 变量族在 `scripts/theme-css-var-contract.mjs` 登记 owner、前缀与消费范围；iframe / MiniApp / 生成式 UI 仍只接收显式 allowlist 的主题 payload；主题 baseline 是 no-growth ratchet，用户自定义不得成为扩张普通应用色板或放宽审计基线的理由。
- 非目标：不做跨 GUI/TUI 的通用主题 schema；不为 TUI 终端主题引入 GUI 自定义投影（TUI 主题独立）；不引入在线主题市场（皮肤市场属独立能力，不在本提案内）。

### 期望效果 / 使用场景
<!-- 描述该功能在什么场景下使用，以及使用后的预期效果 -->
1. 用户在 Appearance 设置打开「自定义主题」编辑器，基于内置深色包派生一份新主题，调整 accent 与背景色，实时预览后保存命名。
2. 用户创建多个自定义主题（如「公司品牌色」「护眼绿」「深夜极暗」），在主题列表间一键切换，当前选择持久化并在重启后保持。
3. 用户导出自定义主题为包文件分享给团队，同事导入后即用，导入失败时看到具体校验错误（如缺少必需 semantic 角色）。
4. 用户配色导致文本对比度不足时，编辑器给出可访问性提示；用户坚持保存仍可生效，但状态视图标注潜在风险。

### 设计草案 / 参考示例
<!-- 如有设计稿、草图或参考的产品示例，请附在此处 -->
- Token 编辑分层：primitive（原料/alpha ramp）→ semantic（背景/文本/边框/交互/状态/accent）→ component（仅当 semantic 无法表达）→ exception domain（editor/terminal/syntax/diff 等，不在用户自定义范围内暴露）。
- 持久化：扩展 `appearance.selection` 以承载自定义包列表与当前选择；内置包只读，用户包可增删改。
- 应用路径：Appearance 运行时消费 CSS 变量，切换时原子应用；新增变量族登记到 `theme-css-var-contract.mjs`。
- 校验：复用包校验规则（schema、必需角色、悬空引用），并叠加对比度提示（文本/背景组合）。

### 是否愿意贡献
<!-- 是否愿意参与该功能的开发或讨论 -->
- [ ] 我愿意参与开发
- [x] 我愿意参与讨论和测试
- [ ] 仅提出建议

### 补充说明
<!-- 其他你认为有助于理解功能建议的信息，如相关 Issue 链接、文档等 -->
- 本提案聚焦 GUI 用户主题配置；TUI 终端主题、移动端 Web 主题、Installer 主题各自独立，不在本提案内合并 surface。
- 边界约束：不得在 `core-types` / `runtime-ports` 内重定义主题包；Rust 不复制主题模型；主题 baseline 是 no-growth ratchet，用户自定义不得通过新增 owner、放宽 baseline 或绕过审计来制造色板扩张——新增颜色须按 Token 分层复用优先。
- 安全提示：自定义主题包为本地用户数据，导入须按既有包校验规则校验，不接受任意可执行脚本或构建期资源。
- 验证基线（建议）：Web UI 侧 `pnpm run type-check:web`；主题/颜色变更须运行 `pnpm run theme:color-audit:all`、`pnpm run theme:color-audit:test`、`pnpm run theme:visual-contract`；i18n 文案经 `pnpm run i18n:audit`。
- 若需扩展到「主题分享市场」或「跟随系统/时间」，可作为本提案的后续增强单独评审，不在本范围内。

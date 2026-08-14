# 鸿蒙 PC 浏览器使用（Browser Use）能力需求文档

> 状态：需求 / 提案
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/plans/computer-use-refactor-plan.md`](../plans/computer-use-refactor-plan.md)（浏览器/计算机使用重构决策：语义引用优先、ControlHub 唯一栈、`browser_control_enabled` 真实门控、M5/M14 修复）
> - [`docs/architecture/product-architecture.md`](../architecture/product-architecture.md)（分层与平台适配边界）
> - [`docs/architecture/platform-portability-design.md`](../architecture/platform-portability-design.md)（鸿蒙 PC 平台移植目标）
> - [`docs/architecture/peer-device-mode.md`](../architecture/peer-device-mode.md)（远程控制下浏览器控制归属）
> 现有实现参考：
> - `src/crates/services/services-integrations` 的 `browser_control`（ControlHub Rust CDP 栈，绑定 Chromium `--remote-debugging-port=9222`）
> - `src/crates/adapters/webdriver`（WebDriver 协议适配器，platform evaluator 现仅 `windows` / `macos`）
> - `WebFetch` / `browser.fetch` / `read_article` 内容读取族与路由指导

### 功能类型
<!-- 请描述功能建议的类型 -->
- [x] 新功能
- [x] 现有功能增强
- [ ] 用户体验优化
- [ ] 性能优化
- [ ] 接口/集成扩展
- [ ] 其他

### 优先级
<!-- 请选择功能建议的优先级 -->
- [ ] 紧急（P0 - 核心需求，强烈期望实现）
- [x] 高（P1 - 重要功能，期望尽快实现）
- [ ] 中（P2 - 有价值的功能，计划实现）
- [ ] 低（P3 - 锦上添花，有时间再做）

### 背景与动机
<!-- 请描述你为什么需要这个功能，解决了什么痛点 -->
- BitFun Agent 的浏览器使用（browser use）能力目前仅在桌面（Windows / macOS / Linux）通过 ControlHub Rust CDP 栈实现，绑定 Chromium `--remote-debugging-port=9222`，未覆盖鸿蒙 PC。
- 鸿蒙 PC（HarmonyOS NEXT PC）浏览器为华为浏览器 / 系统 ArkWeb 内核，**不暴露标准 CDP 9222 调试端口**；现有 `launcher.rs` 与 WebDriver 平台 evaluator（仅 windows / macos）无法在鸿蒙 PC 启动或连接浏览器，导致鸿蒙 PC 上 Agent 无法读 / 写 / 交互操作网页，任务闭环受限。
- `computer-use-refactor-plan.md` 已确立关键决策——"语义引用优先（a11y / DOM `snapshot@ref`）、坐标仅作能力门控降级""ControlHub 为唯一浏览器自动化栈""`ai.browser_control_enabled` 真实门控 + `browser_control` permission intent"——鸿蒙 PC 适配应**继承这些稳定契约**，只替换底层浏览器后端，不在鸿蒙侧另造一套交互范式（避免重演 C1 双栈并存）。
- 鸿蒙 PC 上无登录态内容读取（WebFetch）已可用，但交互式浏览器控制（导航 / 点击 / 输入 / 快照）缺失，登录态抓取、表单提交、多步网页任务无法完成。
- 远程工作场景：远程控制下鸿蒙 PC 的浏览器控制需遵循本地性语义（`browser_control_*` 应在三端 deny 表对齐，控制器不静默在 peer 主机启动浏览器）。

### 功能描述
<!-- 请详细描述你期望的功能行为 -->
- **鸿蒙 PC 浏览器控制后端**：实现一个鸿蒙 PC 专用的浏览器控制后端，**复用 ControlHub / browser_control 的稳定契约**（`snapshot@ref` 语义引用、action 集、稳定错误 code），底层通过鸿蒙浏览器 / ArkWeb 的调试能力（DevTools-like 协议或系统无障碍树）驱动，不依赖标准 CDP 9222。
- **语义引用优先**：继承重构方案决策 2——优先 a11y 树 / DOM 引用（`snapshot@ref`），视觉坐标仅作能力门控降级；不在鸿蒙侧重造视觉 grounding 范式。
- **复用稳定契约**：浏览器交互的 tool schema、action 名、参数（`{element: 人类可读描述, target: ref|selector}`）、错误 code 与桌面一致，保证模型跨平台行为一致；不在鸿蒙侧发明新 action 方言或新坐标系。
- **WebDriver 平台 evaluator**：在 `src/crates/adapters/webdriver` 的 platform evaluator 中新增鸿蒙（ohos）实现（当前仅 windows / macos），承担截图、元素定位、求值等平台原语，与既有 evaluator 契约一致。
- **能力门控**：鸿蒙 PC 复用 `ai.browser_control_enabled`（默认开）真实门控与 `browser_control` permission intent；关闭后浏览器控制域禁用且不影响桌面 computer use（两开关独立，对应 C5）。
- **内容读取与交互分层**：路由指导在鸿蒙 PC 同样适用——`WebFetch`（无登录态读文）→ `browser.read_article` / `fetch`（登录态读）→ `browser connect` / `snapshot`（交互）→ `ComputerUse`（非可控浏览器 / 桌面）→ `open_builtin`（给用户看）。
- **截图与隐私**：截图落盘走 debug 配置开关（默认关）+ 数量 / 天数轮转，继承重构方案 M5 修复；不在鸿蒙 PC 工作区无门控写截图。
- **可控 vs 不可控浏览器边界**：继承决策 4——边界 guard 真正执行，区分 CDP / ArkWeb 可控浏览器与不可控浏览器；不可控浏览器走 ComputerUse 桌面视觉路径或受限模式并明确告知。
- **降级与不可用态**：鸿蒙浏览器调试不可用、无障碍树缺失、用户使用不可控浏览器时，给出清晰错误与下一步，不静默失败；旧能力保持可用。
- **平台无关 + 平台适配**：浏览器控制契约与路由逻辑平台无关（services / execution 层），鸿蒙 PC 浏览器后端通过平台适配器暴露；UI 走 adapter / infrastructure 层，不直接调用 host API。
- **远程工作场景**：远程控制下鸿蒙 PC 浏览器控制以受控端为准；`browser_control_*` 在桌面 / CLI / FE 三端 deny 表对齐（对应 M14），控制器不静默在 peer 主机启动浏览器。

### 期望效果 / 使用场景
<!-- 描述该功能在什么场景下使用，以及使用后的预期效果 -->
1. 鸿蒙 PC 用户让 Agent"打开某网页登录后抓取数据"，Agent 通过鸿蒙浏览器后端导航、快照、点击、输入，复用与桌面同款的 action 与 `ref` 语义，模型行为跨平台一致。
2. 鸿蒙 PC 上 `WebFetch` 读无登录态文章正常可用；需要登录态交互时自动升级到 `browser connect` / `snapshot`，路由与桌面一致。
3. 用户在设置中关闭 `browser_control_enabled`，鸿蒙 PC 浏览器控制域禁用且桌面 computer use 不受影响；权限弹窗归属正确，不出现 peer 模式弹窗弹在控制端的混淆（对应 M13）。
4. 鸿蒙浏览器调试不可用或用户使用不可控浏览器时，Agent 收到清晰错误（稳定 code + 恢复指令，如"ref stale, take a new snapshot"）并提示替代路径，不静默失败。
5. 远程控制下，鸿蒙 PC 浏览器控制以受控端为准，控制器不越权启动浏览器；deny 表三端对齐，contract test 守护。

### 设计草案 / 参考示例
<!-- 如有设计稿、草图或参考的产品示例，请附在此处 -->
- **现有参考实现**：
  - [`docs/plans/computer-use-refactor-plan.md`](../plans/computer-use-refactor-plan.md)：决策 2（语义引用优先）、决策 3（ControlHub 唯一栈）、决策 8（`browser_control_enabled` 真实门控）、M5（截图门控）、M14（deny 表对齐）——鸿蒙适配须继承。
  - `src/crates/services/services-integrations` 的 `browser_control`：ControlHub Rust CDP 栈与 `launcher.rs`——稳定契约来源；鸿蒙后端实现同一契约而非并行第二栈。
  - `src/crates/adapters/webdriver`：WebDriver 协议适配器，platform evaluator 现仅 windows / macos——新增 ohos evaluator 的归属层。
  - `WebFetch` / `browser.fetch` / `read_article`：内容读取族与路由指导——鸿蒙 PC 直接复用。
- **行业参照**：browser-use（a11y 三树合并 + index 句柄，成功率来源）、playwright-mcp（aria snapshot + ref，坐标隔离在 vision capability）、HarmonyOS ArkWeb 调试能力、华为浏览器 DevTools。
- **落地分层建议**（遵循仓库分层与边界）：
  1. Contracts / Execution（`src/crates/contracts` + `src/crates/execution`）：浏览器控制稳定契约（`snapshot@ref`、action、错误 code）平台无关，复用现有，不新增鸿蒙专属契约。
  2. Adapters（`src/crates/adapters/webdriver`）：新增鸿蒙 platform evaluator（截图 / 定位 / 求值原语）。
  3. Services（`src/crates/services/services-integrations` 的 `browser_control`）：新增鸿蒙浏览器后端（启动 / 连接 / 快照 / 交互），不依赖 CDP 9222；ControlHub 唯一栈的鸿蒙实现。
  4. Assembly（`src/crates/assembly`）：按 delivery profile 装配鸿蒙 PC 浏览器控制 capability。
  5. Interfaces / App / UI（`src/apps` + `src/web-ui`）：鸿蒙 PC host 暴露 browser control command；UI 走 adapter / infrastructure 层，不直接调 host API。
- **关键约束**：复用重构方案稳定契约与决策，**不在鸿蒙侧另造交互范式**；鸿蒙浏览器后端是 ControlHub 唯一栈的鸿蒙实现，不是第二套并行栈（避免 C1 重演）。

### 是否愿意贡献
<!-- 是否愿意参与该功能的开发或讨论 -->
- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

### 补充说明
<!-- 其他你认为有助于理解功能建议的信息，如相关 Issue 链接、文档等 -->
- 鸿蒙 PC 是仓库平台移植目标（见 [`docs/architecture/platform-portability-design.md`](../architecture/platform-portability-design.md)）；本需求聚焦**鸿蒙 PC 应用**的浏览器使用，不涉及鸿蒙 PC CLI/TUI——后者是另一未来平台目标，需单独立项。
- **依赖前置**：建议在 `computer-use-refactor-plan` 落地其稳定契约（`snapshot@ref`、稳定错误 code、`browser_control_enabled` 门控、M5 / M14 修复）后再做鸿蒙后端，避免在旧 god-tool 与漂移 deny 表上适配。
- **可行性风险**：鸿蒙浏览器调试能力（ArkWeb DevTools / 系统无障碍树）需调研确认可用性；若系统不提供可控调试入口，browser use 可能需降级为 ComputerUse 桌面视觉路径或受限模式，并明确告知用户而非静默失败。
- C6（Firefox / Safari 锁死）在鸿蒙不直接适用（鸿蒙浏览器为 ArkWeb 内核），但"可控 vs 不可控浏览器"边界 guard 仍需真正执行。
- 远程工作场景遵循仓库"远程兼容"全局规则与 deny 表三端对齐原则；`browser_control_*` 须补入三端 deny 表并有 contract test 守护。

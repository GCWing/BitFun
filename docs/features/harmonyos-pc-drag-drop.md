# 功能建议：鸿蒙 PC 支持拖拽（HarmonyOS PC Drag & Drop）

> 状态：提案 / 未做
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/architecture/platform-portability-design.md`](../architecture/platform-portability-design.md)
> - [`src/web-ui/src/shared/types/drag.ts`](../../src/web-ui/src/shared/types/drag.ts)
> - [`src/web-ui/src/shared/services/DragManager.ts`](../../src/web-ui/src/shared/services/DragManager.ts)
> - [`src/apps/ohos/oh-rs-ability/src/main/ets/ability/type.ets`](../../src/apps/ohos/oh-rs-ability/src/main/ets/ability/type.ets)
> - [`src/apps/ohos/entry/src/main/ets/entryability/EntryAbility.ets`](../../src/apps/ohos/entry/src/main/ets/entryability/EntryAbility.ets)

## 背景与需求描述

桌面端（Tauri / Web UI）已具备完整的拖拽交互：窗口拖动（`window_start_dragging` + `data-tauri-drag-region`）与内容拖放（`DragManager` + `DragPayload` / `IDropTarget` 协议，覆盖聊天输入文件拖入、工作区切换拖拽、文件面板拖入、Canvas 标签页拖拽、上下文项拖放等目标，自定义 MIME `application/x-bitfun-context`）。

鸿蒙 PC 是 BitFun 的目标平台之一。按 `platform-portability-design.md`，完整的鸿蒙 PC 支持同时包含本地 CLI/TUI 与 GUI，**GUI 是独立产品专题**，其 ArkUI/ArkWeb/HAP 技术选型只能在专题获批后设计。当前鸿蒙侧仅有：

- `WebViewInitData.onDragAndDrop?: (event: string) => void` —— 一个未实现的拖放回调占位（`DefaultWebview.ets` / `ability/type.ets`）；
- `window_start_dragging` —— 已注册为 ArkTS 函数（窗口拖动）。

但鸿蒙 PC GUI 的**内容拖拽**（系统文件管理器拖入应用、应用内拖拽、拖出至系统）尚无实现，存在以下缺口：

- 用户无法把鸿蒙 PC 文件管理器中的文件拖入聊天 / 文件面板 / 工作区，只能走"打开对话框选择"的旧路径；
- 应用内标签页、工作区、上下文项的拖拽在鸿蒙 PC GUI 上缺失，跨平台体验不一致；
- 鸿蒙原生拖拽走 UDMF（统一数据管理框架）与 ArkUI drag 事件，与浏览器 DnD API 不是同一套，不能直接照搬 Web UI 实现；
- 现有 `onDragAndDrop` 占位若直接接上，会把鸿蒙私有事件形状泄漏进共享逻辑，违反"平台差异只在 app/adapter/service 边界"的规约。

本提案定义鸿蒙 PC 拖拽支持的目标范围、行为契约与分层归属，作为鸿蒙 PC GUI 专题下的一个能力子项，**不预先决定 GUI 专题的整体技术选型**。

## 期望行为

### 1. 窗口拖拽

- 拖动标题栏 / 拖拽区移动窗口，复用既有 `window_start_dragging` ArkTS 函数；
- 最大化 / 还原态下拖动行为符合鸿蒙 PC 窗口管理语义。

### 2. 系统到应用拖入

- 从鸿蒙 PC 文件管理器拖入文件 / 文件夹到聊天输入区，作为附件或引用注入会话（对齐桌面 `.bitfun-chat-input-drop-zone`）；
- 拖入文件面板，落盘到当前工作区并刷新文件树（对齐桌面 `FilesPanel` drop + 传输进度）；
- 拖入工作区导航项以切换 / 新建工作区（对齐桌面 `nav-workspace-drop-target`）；
- 拖入只接受受支持类型，不接受时给出拒绝态视觉反馈，不静默吞掉。

### 3. 应用内拖拽

- Canvas 标签页在分组间拖拽定位（左 / 右 / 中，对齐桌面 `DropPosition` 与 `canvasStore.handleDrop`）；
- 上下文项拖放到可接受目标（对齐桌面 `ContextDropZone` + `DragManager.handleDrop`）；
- 应用内拖拽产生与桌面同形状的 `DragPayload`，目标按 `acceptedTypes` / `canAccept` 判定。

### 4. 应用到系统拖出（可选，首版可降级）

- 从应用把文件 / 文本拖出到鸿蒙 PC 文件管理器或其他应用；
- 首版若不可行，明确报告 unsupported 并保留拖入能力，不静默降级。

### 5. 数据契约

- 鸿蒙侧拖拽经 UDMF 读写统一数据，在**适配层**翻译为与桌面同形状的 `DragPayload`（`id` / `sourceType` / `dataType` / `timestamp` / `data` / `metadata`）；
- 源类型（`DragSourceType`）与数据类型（`ContextType`）保持跨平台一致；鸿蒙私有 UDMF 记录不进入共享逻辑；
- 自定义上下文负载走 `application/x-bitfun-context` MIME 等价物，在鸿蒙侧用 UDMF 自定义记录承载。

### 6. 视觉反馈

- 拖入悬停时高亮可接受目标、灰显不可接受目标（对齐桌面 `is-drop-target` / `--cannot-accept`）；
- 拖拽预览（`PreviewData`）在鸿蒙 PC GUI 上有等价呈现；
- 接受 / 拒绝光标或等价态符合鸿蒙交互规范。

### 7. 不可用态

- 当目标鸿蒙版本 / 设备缺少 UDMF 或 ArkUI drag 能力时，显式报告 unsupported，**不得**静默回退到对话框选择或借用桌面 / 移动端代执行；
- 拖拽相关命令的远程工作区策略声明为本地交互（远程控制场景下拖拽只在受控端本地执行），不支持时清晰提示。

### 8. 性能与可访问性

- 拖拽悬停不引发全工作区逐帧重渲染（吸取桌面 F4 教训：拖侧栏逐 mousemove 重渲染）；
- 键盘可达的等价路径（拖拽是补充，不是唯一入口）；
- 大文件 / 批量拖入有进度与取消。

## 非目标 / 范围外

- 不在本提案内决定鸿蒙 PC GUI 专题的整体 ArkUI/ArkWeb/HAP 技术选型（由 GUI 专题负责）；
- 不为鸿蒙建立第二套 `DragPayload` / `IDropTarget` 协议——复用桌面契约，只在适配层翻译；
- 不覆盖鸿蒙手机 / 平板移动端的拖拽（移动端是另一专题，不与 PC GUI 混写）；
- 不覆盖鸿蒙 PC CLI/TUI 形态的拖拽（CLI/TUI 是另一交付形态）；
- 不在本提案内做拖拽内容的 AI 解析或自动归类；
- 不替换桌面端既有拖拽实现，鸿蒙侧只是新增适配。

## 建议的落地路径（基于现有分层）

依据 `platform-portability-design.md` 与仓库分层规则，鸿蒙 PC 拖拽应落在：

1. **Contracts (`src/crates/contracts`)** — 若 `DragPayload` / `DragSourceType` / `ContextType` 需跨 Rust 与 ArkTS 共享，把它们从 `src/web-ui/src/shared/types/drag.ts` 提升为稳定契约（行为轻量，不耦合 UDMF 或浏览器 DnD）。当前若只在 UI 层共享，可暂留 web-ui，不强行下沉。
2. **OHOS App (`src/apps/ohos`)** — 鸿蒙侧拖拽实现落点：
   - ArkUI 组件 drag 事件处理器、UDMF 统一数据读写；
   - 在 `WebViewInitData.onDragAndDrop` 占位上接真实实现（把 UDMF 事件翻译为统一 `DragPayload` 后回调）；
   - 窗口拖拽复用既有 `window_start_dragging`。
3. **适配层边界** — 鸿蒙 UDMF / ArkUI drag 事件 ↔ 统一 `DragPayload` 的双向翻译；鸿蒙私有记录不泄漏出适配层。
4. **Web UI / ArkUI 表面** — 拖放目标（聊天输入、文件面板、工作区导航、Canvas 标签页、上下文拖放区）在鸿蒙 PC GUI 上达到与桌面等价的覆盖。
5. **远程策略** — 拖拽相关命令远程策略声明为本地交互（远程控制时拖拽在受控端本地执行），在 `remote_workspace_policy` 中显式登记。

### 分层与依赖边界要点

- 严格遵守 `platform-portability-design.md`：鸿蒙 PC GUI 是独立专题，本提案只定义拖拽能力子项，不提前决定 GUI 整体选型；
- 平台差异只在 app/adapter/service 边界出现，共享 Runtime 不按 target triple 分叉业务语义；
- 进入 OHOS 闭包的 `cfg(unix)` / `target_os = "linux"` 路径须重新取证，不得默认等同桌面 Linux；
- 缺失能力显式报告 unsupported，不静默借用桌面 / 移动端 / Remote 代执行；
- 不建立包含全部鸿蒙差异的巨型 `ohos` feature 或第二套拖拽协议；
- 拖拽是本地交互，远程场景声明本地执行策略，不支持时清晰提示。

## 设计草案 / 参考示例

- **协议参考**：桌面 `DragPayload<T>` / `IDragSource` / `IDropTarget` / `DragEventPayload`（`src/web-ui/src/shared/types/drag.ts`）与 `DragManager`（`src/web-ui/src/shared/services/DragManager.ts`）是跨平台契约基准；鸿蒙侧只做 UDMF 翻译，不改协议形状。
- **拖放目标覆盖矩阵**：聊天输入（`.bitfun-chat-input-drop-zone`）、工作区导航（`nav-workspace-drop-target`）、文件面板（`FilesPanel` drop + 传输进度）、Canvas 标签页（`DropPosition` 左/右/中）、上下文拖放区（`ContextDropZone`）——鸿蒙 PC GUI 首版至少覆盖前三项。
- **窗口拖拽参考**：既有 `window_start_dragging` ArkTS 函数与桌面 `data-tauri-drag-region`。
- **占位接续参考**：`WebViewInitData.onDragAndDrop?: (event: string) => void` 是已有钩子，在其上接 UDMF → `DragPayload` 翻译后回调，不新造第二套钩子。
- **平台能力参考**：鸿蒙 UDMF（统一数据管理框架）与 ArkUI 组件 drag 事件是原生拖拽标准，授权与 API 以华为官方文档为准，本提案不臆测其字段。
- **性能教训参考**：桌面 F4（拖侧栏逐 mousemove 全工作区重渲染）——鸿蒙侧拖拽悬停须避免同类逐帧重渲染。
- **远程参考**：拖拽是本地交互，远程控制时在受控端本地执行，`remote_workspace_policy` 显式登记本地策略。

## 是否愿意贡献

- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

## 补充说明

- 本提案严格遵循 `platform-portability-design.md`：鸿蒙 PC GUI 是独立产品专题，拖拽是其下的能力子项；不提前决定 GUI 整体选型，不为过编译建立鸿蒙专用 Runtime 或复制业务逻辑。
- 与桌面拖拽的关系：复用 `DragPayload` / `IDropTarget` 协议与拖放目标覆盖，只在适配层翻译 UDMF，不新造第二套协议；桌面实现不变。
- 与移动端的关系：鸿蒙手机 / 平板拖拽是另一专题，本提案不覆盖，不与 PC GUI 混写。
- 与 CLI/TUI 的关系：CLI/TUI 是鸿蒙 PC 的另一交付形态，拖拽属 GUI 交互，不在 CLI/TUI 范围。
- 与远程工作区的关系：拖拽是本地交互，远程控制时在受控端本地执行；相关命令远程策略声明为本地，不支持时清晰提示而非静默失败。
- 相关分层入口：`src/apps/ohos`、`src/web-ui/AGENTS.md`、`src/apps/desktop/AGENTS.md`、`docs/architecture/platform-portability-design.md`。

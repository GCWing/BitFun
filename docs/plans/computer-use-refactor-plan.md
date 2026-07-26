# BitFun Computer Use / 浏览器控制能力重构方案

> 范围：`src/crates/assembly/core` 的 ComputerUse / ControlHub / browser_control / web 工具族、`src/apps/desktop/src/computer_use/*` 执行层、`src/crates/execution` 契约层、web-ui 配置与模式面。
> 依据：三份内部代码排查 + cua / Codex CLI / Anthropic computer-use-demo / browser-use / playwright-mcp & stagehand 五份标杆调研。所有文件路径均来自排查实证。

---

## 1. 现状诊断

### 1.1 重叠控制路径盘点

**"控制/使用浏览器"共 7 条并存路径：**

| # | 路径 | 入口 | 状态 |
|---|------|------|------|
| 1 | ControlHub `domain=browser`（自研 Rust CDP，~40 action，~4900 行） | `control_hub_tool.rs` + `browser_control/*` | 最深实现，恒开 |
| 2 | agent-browser 内置技能（vercel-labs npm CLI，自带完整 CDP 栈 + 同名 `@eN` ref） | `builtin_skills/agent-browser/SKILL.md` | agentic/Claw/coding 默认开启 |
| 3 | ComputerUse 桌面视觉/AX 路径（可物理操作浏览器窗口） | `computer_use_tool.rs` | guard 是死代码，实际不设防 |
| 4 | ComputerUse `open_url`/`open_file`（OS 默认浏览器，不可控） | `computer_use_actions.rs:1512` | 活 |
| 5 | ControlHub `browser.open_builtin`（内置面板，纯展示、agent 读不回） | `EventHandlerModule.ts:2174` | 单向 |
| 6 | 内容读取族：WebFetch / browser.fetch / read_article / get_html / get_text / evaluate | `web/fetch.rs`、`browser_control/actions.rs` | 4-5 条重叠，无路由指导 |
| 7 | 用户自配 MCP（如 Playwright MCP） | `mcp_tools.rs` | 潜在 |

**ComputerUse 内部"点击一个目标"共 7 种方言：** `locate→mouse_move→click`、`click_element`、`click_target`、`move_to_text→click`、`app_click{6 种寻址}`、`interactive_click(i)`、`visual_click(i)`；叠加 **4 套坐标系**（image px / native px / global logical / normalized 0-1000）。

### 1.2 核心耦合点

1. **`computer_use_tool.rs` ↔ `computer_use_actions.rs` 互相调用成环**（open_app 三跳绕环；未来漏配 dispatcher 列表即无限递归），且 ComputerUse 借用 `control_hub/errors.rs` 的封套，`domain='desktop'` 泄漏——两套错误形状在同一工具内混用。
2. **ComputerUseHost 是 60+ 方法胖 trait**（`tools/computer_use_host.rs`，560 行），把截图裁剪、OCR 信任、点击守卫状态机、Set-of-Mark、Codex 风格 app_* 全部塞进一个接口，执行层（`src/apps/desktop/src/computer_use/*`，~1.9 万行）与工具层强耦合。
3. **桌面 host 是进程级单例**（`lib.rs:1615`）+ `APP_LOOP_TRACKER` 全局静态，会话状态跨 session 污染。
4. **三层能力开关互不咬合**：cargo feature 是装饰（`tool-provider-groups` 的 `enabled_feature_groups()` 无消费者）、`core.integration` 把 7 组能力绑死、ControlHub `is_enabled()` 硬编码 true。
5. **三份 LOCAL_ONLY deny 表手工同步已漂移**（FE `peer-device-adapter.ts` / desktop `peer_host_invoke.rs` / cli `peer_host/deny.rs`），与 `remote_workspace_policy.rs` 的 LocalOnly 声明是两套独立"本地性"语义。
6. **概念身份不收敛**：ComputerUse 同时是模式（FE CORE_AGENT_IDS）、restricted SubAgent（`agent-runtime/src/agents.rs:96`）、工具名（`toolGroups.ts:78`），文案三种称呼。

### 1.3 问题清单（合并去重，按 severity 排序）

#### Critical

| # | 问题 | 关键文件 |
|---|------|---------|
| C1 | **双浏览器自动化栈并存，prompt 指导互相矛盾**：SKILL.md 说优先 agent-browser，claw_mode.md/computer_use_mode.md 说用 ControlHub；两套 `@eN` ref 空间、两个浏览器实例、登录态互不可见 | `builtin_skills/agent-browser/SKILL.md`、`control_hub_tool.rs`、`agents/prompts/claw_mode.md`、`skills/policy.rs` |
| C2 | **浏览器/桌面边界 guard 是不可达死代码**：`desktop_action_targets_browser` 只在 `handle_desktop` 落空分支被调，而它想拦的 click/type_text 等在 `call_impl` 内联处理，边界只剩提示词 | `computer_use_actions.rs`、`computer_use_tool.rs` |
| C3 | **单一 god-tool：40 action、7 条点击方言、4 套坐标系、5KB 手写描述 ×2 份**，参数命名互不一致（`text_contains`/`target_text`/`text_query`/`ocr_text.needle`） | `computer_use_tool.rs`（2181 行）、`computer_use_actions.rs`（1648 行） |
| C4 | **headless 模式是假的且危险**：无 headless 启动实现，与 default 共用 9222 端口；可能附着用户真实登录浏览器却标为 "Headless test browser" | `control_hub_tool.rs:539-575,603`、`services-integrations/src/browser_control/launcher.rs` |
| C5 | **关闭 Computer use 后 ControlHub 仍可完全控制用户真实浏览器**：`is_enabled()` 恒 true、不申报 permission intent、无全局规则可 deny，文案却承诺"关闭后任何模式都不启用" | `control_hub_tool.rs:1925`、`GlobalPermissionRulesDialog.tsx`、`locales/en-US/settings/session-config.json` |
| C6 | **Firefox/Safari 用户被双向锁死**：guard 拒绝桌面输入并指向 ControlHub，而 BrowserKind 只支持 Chromium 系，CDP connect 必败 | `computer_use_actions.rs:121-150`、`launcher.rs:25-32`、`computer_use_mode.md` |

#### Major

| # | 问题 | 关键文件 |
|---|------|---------|
| M1 | 模型可见文案引用已删除的幽灵工具（ComputerUseMouseStep/MousePrecise/MouseClick），诱导调用不存在的工具 | `tool-contracts/src/computer_use.rs:1214,1584`、`tool-contracts/src/framework.rs:2269-2279`、`computer_use_tool.rs:992`、`computer_use_host.rs` |
| M2 | 桌面 host 会话状态进程级单例 + `APP_LOOP_TRACKER` 全局静态，跨 session 污染守卫/截图缓存/循环检测 | `desktop/src/lib.rs:1615`、`desktop_host/mod.rs`、`computer_use_actions.rs:30` |
| M3 | 大量死代码：input/result shim、verification/RetryStrategy 未接线、`handle_system('open_app')` 不可达、`enabled_feature_groups()` 无消费者 | `computer_use_input.rs`、`computer_use_result.rs`、`computer_use_verification.rs`、`tool-execution/src/computer_use.rs`、`tool-provider-groups/src/lib.rs` |
| M4 | tool ↔ actions 调用环 + ControlHub 错误封套泄漏，模型看到两种失败形状 | `computer_use_tool.rs`、`computer_use_actions.rs`、`control_hub/errors.rs` |
| M5 | 每次截图无条件写入 `<workspace>/.bitfun/computer_use_debug/`，无门控、无轮转，隐私风险 | `computer_use_tool.rs`（try_save_screenshot_for_debug） |
| M6 | 未对接任何 provider 原生 computer-use 形态（Anthropic computer_20250124/OpenAI computer-use-preview），模型无法复用训练先验；多模态回传仅限两类 converter | `computer_use_tool.rs`、`tool-execution/src/context.rs` |
| M7 | text-only 门控不一致：schema 仍暴露 Set-of-Mark 纯视觉 action，`handle_desktop_ax` 附图不检查多模态能力 | `computer_use_tool.rs:249`、`computer_use_actions.rs:648-838` |
| M8 | `frame`/`frame_main` 是死功能（active_frame 无读者）；同源 iframe 内点击坐标缺 offset 修正，点错位置 | `control_hub_tool.rs:1657-1698`、`browser_control/actions.rs:552-568` |
| M9 | `control_hub_tool.rs` 2668 行 god file，错误分类靠 `to_lowercase().contains(...)` 字符串猜测反推 ErrorCode | `control_hub_tool.rs`、`browser_control/actions.rs` |
| M10 | 内容读取 4-5 条路径无选择指导，`browser.fetch` 带用户登录态发任意请求却与 WebFetch 无差异化约束 | `web/fetch.rs`、`agentic_mode.md` |
| M11 | TerminalControl 与 ControlHub terminal 域双入口，session id 发现口径各说各话 | `terminal_control_tool.rs`、`control_hub_tool.rs:1717-1815` |
| M12 | `open_builtin` 内置面板单向不可观察，模型易误以为可继续 snapshot | `EventHandlerModule.ts:2174`、`control_hub_tool.rs` |
| M13 | Peer 模式混合机器语义：SessionConfig 裸调 Tauri invoke 打本机，configManager 写远端——权限弹窗弹在控制端、工具跑在 peer 端 | `SessionConfig.tsx:165-205,593,644-695`、`peer-device-adapter.ts`、`peer_host_invoke.rs` |
| M14 | 三份 deny 表漂移（speech_* 只在 FE、CLI 缺项），`browser_control_*` 不在任何 deny 表——控制器可静默在 peer 主机启动浏览器 | `peer-device-adapter.ts`、`peer_host_invoke.rs`、`cli/src/peer_host/deny.rs`、`remote_workspace_policy.rs` |
| M15 | ComputerUse 模式禁用门禁只在 FE 下拉生效，slash 命令 `/ComputerUse` 不拦截，后端不校验，工具静默缺失无解释 | `ChatInput.tsx:2440,4130,4977`、`AgentsScene.tsx:704` |

#### Minor（合并列举）

- 结果 JSON 三重字段别名（image_jpeg_width/image_width/display_width_px…）；scroll 的 `scroll_x/scroll_y` 绕过 `ensure_global_xy_on_display` 边界守卫；num_clicks 循环模拟双击不用 CGEvent click_state（`computer_use_tool.rs:1581`、`tool-contracts/src/computer_use.rs`）。
- `analyze_image_tool.rs` / `view_image_tool.rs` 整段复制 ResolvedImagePath/读取逻辑，三条图片链路无统一选择指引。
- 陈旧注释指向不存在的 `claw_mode.md`（应为 computer_use_mode.md）、ControlHub 域口吻残留、loop 警告用已删除的 `desktop.screenshot` 语法；Linux 后端仅 141 行空壳但 schema 不裁剪。
- cdp 方法白名单是摆设（evaluate 全权可绕）；`--remote-debugging-port=9222` hint 教用户裸暴露登录态。
- `ai.computer_use_enabled` 订阅逻辑三处复制且初始默认值矛盾（true vs false，首帧误导）；`SessionConfig.tsx` 1775 行双 variant 互相污染；`AIFeaturesConfig.tsx` 死组件；`computer_use_open_system_settings` Windows 分支 UI 不可达；FE AIConfig 类型漂移；模型无视觉能力时无降级提示。

---

## 2. 标杆对比

| 维度 | BitFun 现状 | cua | Codex CLI | Anthropic demo | browser-use | playwright-mcp / stagehand | 差距结论 |
|------|------------|-----|-----------|----------------|-------------|---------------------------|---------|
| **动作空间** | 40 action 自造方言，7 条点击路径 | OpenAI+Anthropic 动作并集，按 tag 分发 | 工具极少（shell/apply_patch/view_image），GUI 委托 MCP | 日期版本化 enum，10-17 个动作，服务端定义 schema | ~20 个结构化动作，index 为句柄 | 每域一文件的声明式小工具 | **决策面失控**：应收敛到标准动作集 + 版本化 enum |
| **provider 原生形态** | 无，自造 5KB 描述现学 | 模型 regex 注册表，边缘转换到原生 computer_20251124 / computer-use-preview | — | 原生 Anthropic-defined tool，客户端零 schema | 能力门控换 schema | — | **放弃训练先验是执行质量差的直接原因** |
| **坐标处理** | 4 套坐标系并存，scroll 绕过校验 | per-screenshot scale factor 追踪 + reset | — | `scale_coordinates(source,x,y)` 单函数双向 | 截图尺寸→viewport 换算，坐标是门控降级 | ref 免坐标 | **需要唯一的双向缩放模块** |
| **浏览器交互范式** | CDP JS 注入 + `@eN` 属性写入，两套 ref 栈打架 | pixel + BrowserTool 页级动词 | 委托 MCP | — | **a11y 三树合并 + index 句柄**（成功率来源） | **aria snapshot + ref**，坐标隔离在 vision capability | **语义引用优先，坐标降级** 是业界共识 |
| **观察闭环** | 动作后需另调 screenshot；augment_result 附零散字段 | 执行器烘焙 post-action 截图 | — | 动作后 2s settle + 自动截图 | 动作即回灌新状态 + diff `*` 标注 | Response 聚合器：快照+tab diff+事件一并回传 | **"动作即观察"缺失，回合数浪费** |
| **工具契约/注册** | 手写双份 JSON schema + 测试防漂移 | Protocol + 注册表 | **spec 与 runtime 同 trait 对象**、ToolExposure 四态、每回合 spec_plan 组装 | ToolGroup{version,tools,beta_flag} 注册表 | 装饰器 + schema 自动派生 + 域名过滤 | Tool{capability,kind,zod,handle} + filteredTools | **schema/实现分离导致漂移；应 spec-runtime 同体** |
| **分层** | 工具层直连 60+ 方法胖 trait，执行层在 Tauri 进程内 | Provider ⊥ Interface ⊥ Handler 三层正交 | 契约 crate ⊥ 编排 ⊥ 风险编排 ⊥ 沙箱 crate | UI/loop/dispatch/tool/executor 五层 | Agent/Registry/Session/DOM 四层 | tools/mcp/backend 三层 | **BitFun 缺清晰层界，横切关注点全内联** |
| **会话状态** | 进程级单例 Mutex + 全局静态 | per-Computer 实例 | per-turn 组装 + 会话级审批缓存 | per-session 对象 | per-BrowserSession | per-context | **必须 per-session 键控** |
| **错误模型** | 两套封套混用 + 字符串猜 ErrorCode | 结构化 tool-error item，永不 abort | 稳定 ErrorCode + 审批 key | ToolError→is_error tool_result 唯一转换点 | 一切异常→ActionResult(error) 回灌 | 可恢复错误 + 恢复指令（"Try new snapshot"） | **需要唯一异常边界 + 稳定 code** |
| **安全/审批** | guard 死代码、ControlHub 恒开无 intent、9222 裸端口 hint | safety_checks 透传（含 TODO） | 审批 key 化缓存 + 沙箱升级阶梯 + 网络审批独立流 | Docker 沙箱 + prompt injection 分类器承接 | 敏感数据 `<secret>` 占位 + 域名白名单 | allowed/blockedOrigins 网络层强制 + element 描述供审批 UI | **安全边界应在 Rust 核心强制，不在 prompt** |
| **截图/上下文管理** | JPEG 无条件落盘 + 全量回传 | ImageRetentionCallback、trajectory 落盘可 replay | 输出截断一等策略 | 按块修剪保护 prompt cache | 干净截图 + 人用高亮分离 | 大输出写文件 + outputMaxSize | **无 retention 策略，落盘无门控** |

---

## 3. 目标架构

### 3.1 分层设计

```
┌────────────────────────────────────────────────────────────────┐
│ L3 模式与配置面                                                  │
│  · 双独立开关: ai.computer_use_enabled (桌面) 与                  │
│    ai.browser_control_enabled (浏览器, 默认开) 互不牵连            │
│  · permission intents: computer_use + browser_control          │
│  · 每回合工具组装 (仿 codex spec_plan): 按模型能力/平台/远程裁剪    │
│  · deny 表单一真源 (Rust 导出 + contract test 三端对齐)           │
├────────────────────────────────────────────────────────────────┤
│ L2 工具面 (模型可见)                                             │
│  · Desktop: Anthropic 原生 computer 形态 (Claude) /              │
│    标准化自定义 fallback (其他模型)；辅助定位工具独立               │
│  · Browser: 单一栈, snapshot@ref 交互, 坐标为门控降级             │
│  · Response 聚合器: 动作即观察                                    │
│  · 统一 ToolResult / 稳定 ErrorCode / 唯一异常边界                │
├────────────────────────────────────────────────────────────────┤
│ L1 执行后端层 (Surface traits)                                   │
│  · DesktopSurface: screenshot/click/type/key/scroll/drag/       │
│    ax_snapshot/window_ops  (per-session 状态)                    │
│  · BrowserSurface: CDP snapshot/resolve_ref/click/fill/         │
│    navigate/fetch/events  (session registry 保留)                │
│  · 坐标策略唯一模块: scale(Api↔Physical) + DPI 折算               │
├────────────────────────────────────────────────────────────────┤
│ L0 契约层 (独立 crate, 不依赖 Session)                            │
│  · Action enum (serde tag, 版本化, 对齐 Anthropic 动作集)         │
│  · ToolResult{output,error,image,system} / TargetRef            │
│  · ToolSpec 与执行绑定同一 trait 对象 (spec-runtime 同体)          │
└────────────────────────────────────────────────────────────────┘
```

### 3.2 关键决策及理由

**决策 1：桌面控制走"视觉坐标为主干 + AX 为辅助定位"，对 Claude 映射 Anthropic 原生 computer 工具形态。**
- 理由：桌面没有普适的 DOM；Anthropic computer_20250124/20251124 动作集（screenshot/left_click/type/key/scroll/zoom…）是模型训练过的先验，cua 与 Anthropic demo 证明按原生形态声明（display_width_px = 实际发送截图尺寸）可直接消除"从 5KB 描述现学"的质量损失（对应 M6）。
- 现有的 AX（`windows_ax_ui`/`macos_ax_ui`）、OCR、Set-of-Mark 能力**不删除，降级为补充**：合并为一个 `desktop_snapshot`（UIA/AX 树序列化为带 ref 的文本，服务 text-only 模型与精确定位）+ 一个 `desktop_find`（文本检索），不再作为并列的 7 条点击方言。40 action 收敛为：观察（screenshot/snapshot/get_app_state）、定位（单一 `target` 对象语法：`{ref} | {text} | {x,y}`，内部按 AX→OCR→coords 阶梯解析，复用现有 click_target 解析器）、动作（click/type/key/scroll/drag/wait）、系统（open_app/open_url/clipboard），约 15 个。
- 坐标系收敛为 2 套：模型空间（= 发送截图尺寸）与物理空间，唯一双向函数（仿 demo 的 `scale_coordinates`），Windows per-monitor DPI 折算进同一变换；删除 normalized 0-1000 与 "Ignored…host rejects" 参数。

**决策 2：浏览器控制走 accessibility-tree/DOM 引用（snapshot@ref），视觉坐标仅作能力门控降级。**
- 理由：browser-use（69k stars）与 playwright-mcp 的一致结论——有可枚举语义树就用索引：确定性、可校验、不受截图缩放/DPI 影响；两家的成功率投资都在语义树提取（三树合并/paint-order 过滤），不在视觉 grounding。现有 ControlHub snapshot 已有 `@eN` ref 机制，方向正确，需修 iframe 坐标（M8）并把交互参数统一为 `{element: 人类可读描述, target: ref|selector}` 双通道（描述供审批 UI）。

**决策 3：浏览器自动化栈二选一——保留 ControlHub Rust CDP 栈为唯一路径，agent-browser 技能降为默认关闭的 opt-in（对应 C1）。**
- 理由：ControlHub 栈在自己进程内、可被权限系统/审批/deny 表统一管辖、与 web-ui 事件打通；agent-browser 是外部 npm CLI + 独立 Chromium + 独立 auth vault，无法纳入 BitFun 的权限与 Peer 策略，且两套 `@eN` ref 并存是模型出错最大源头。`skills/policy.rs` 的 `resolve_builtin_default_enabled` 全模式改 false。

**决策 4：浏览器/桌面边界 guard 真正执行，且区分 CDP 可控与不可控浏览器（对应 C2/C6）。**
- 把 `desktop_action_targets_browser` 移到统一 dispatcher 的输入类动作入口前；前台是 **Chromium 系（CDP 可控）** 时拒绝并指向 browser 工具；前台是 **Firefox/Safari** 时放行桌面控制（走视觉坐标路径），消除双向锁死。

**决策 5：会话状态 per-session 键控。** `DesktopComputerUseHost` 的 `ComputerUseSessionMutableState` 改为 `HashMap<SessionId, State>`（或经 ToolUseContext 注入 per-session 包装），删除 `APP_LOOP_TRACKER` 全局静态（M2）。

**决策 6：唯一异常边界 + 稳定错误形状。** 定义 `ComputerToolError`（带稳定 code + 给模型的恢复指令，如 "ref stale, take a new snapshot"），dispatcher 是唯一 catch 点，转 `is_error` tool result；ComputerUse 停用 ControlHub 封套；`browser_control/actions.rs` 直接产生带 code 的错误，删除 `map_dispatch_error` 字符串猜测（M4/M9）。

**决策 7：动作即观察。** 执行器在每个 mutating 动作后：settle 延迟（桌面固定/浏览器等 network idle）→ 自动截图或快照 diff → 打包进同一 result（仿 playwright-mcp Response 聚合器 + cua post-action screenshot）。配套截图 retention（只留最近 N 张，按块修剪保护 prompt cache）。

**决策 8：能力开关收敛为两个独立的真实门控。** 删除装饰性 cargo feature 层；桌面控制与浏览器控制是两个独立能力：`ai.computer_use_enabled` 只门控 ComputerUse 桌面工具（现状已如此）；新增独立的 `ai.browser_control_enabled`（默认开）门控 ControlHub browser 域，关闭 computer use 不影响浏览器控制（产品决策确认，2026-07-26）。ControlHub 实现真实 `is_enabled()` 服从后者，并按 DeliveryProfile/远程会话裁剪；新增 `browser_control` permission intent 进后端枚举与 `GlobalPermissionRulesDialog.tsx`（对应 C5 的可管辖性诉求）。

---

## 4. 重构路线图

每阶段可独立合并、可编译可测；前 3 阶段不改变模型可见行为面（除删除幽灵引用），从阶段 4 起改变工具面需 A/B 验证。

### 阶段 0：止血（~1 周，即速赢清单，见 §5）

### 阶段 1：死代码清理 + 打断环 + 错误统一（~1 周）

- **删除**：`computer_use_input.rs`、`computer_use_result.rs`、`computer_use_verification.rs`、`tool-execution/src/computer_use.rs` 中未接线的 `RetryStrategy`/`detect_visual_change`、`handle_system('open_app')` 分支、`ComputerUseHost::get_action_history`、`tool-provider-groups` 的 `enabled_feature_groups()` 装饰层、`AIFeaturesConfig.tsx`、`computer_use_actions.rs:1277` 空注释段。
- **打断环**：新建 `computer_use/dispatch.rs`，`call_impl` 与 `handle_desktop`/`handle_system` 全部单向汇入；删除 `handle_desktop` 尾部反向 new `ComputerUseTool` 的 fallback。
- **错误统一**：ComputerUse 全面切换到自有错误类型（稳定 code），移除对 `control_hub/errors.rs` 的依赖；把 `ComputerUseActions` 的 system_* 测试从 `control_hub_tests`（`control_hub_tool.rs:2487-2648`）移回所属文件。
- 风险：低（删的都是零引用代码）。验证：`cargo build` 全工作区 + 现有 4 个 schema 防漂移单测 + grep 确认零引用。

### 阶段 2：动作空间收敛（~2 周）

- **文件**：`computer_use_tool.rs`、`computer_use_actions.rs`、`computer_use_locate.rs`、`tool-contracts/src/computer_use.rs`。
- 40 action → ~15：`click_element`/`move_to_text`/`locate` 退化为 `click_target` 统一解析器的内部实现并从 schema 移除；`app_click` 六种寻址与 `interactive_click`/`visual_click` 合并进单一 `target` 语法；`delta_x`/`dx` 等双收参数、三重结果别名（保留 `image_*` 与 `native_*` 各一组）清理。
- **坐标模块**：新建 `tool-contracts/src/computer_use/coords.rs`，唯一 `scale(source, x, y)` 双向函数 + DPI；scroll 的 `scroll_x/scroll_y` 补 `ensure_global_xy_on_display`；删除 normalized 0-1000。
- **text-only 门控统一**：text-only schema 移除 interactive/visual view action；`handle_desktop_ax` 附件统一走 `require_multimodal_tool_output_for_screenshot` 同款检查。
- **边界 guard 落地**（决策 4）：guard 移入 dispatcher 输入动作入口，Firefox/Safari 放行。
- 风险：中——模型行为面变化。验证：保留旧 action 名为 alias 一个版本期（deserialize 兼容 + deprecation 警告）；用现有 ComputerUse 子代理跑固定任务集（打开 app、点击、输入、滚动）录 trajectory 对比回合数与成功率。

### 阶段 3：会话状态 per-session + host trait 瘦身（~1-2 周）

- **文件**：`src/apps/desktop/src/lib.rs:1615`、`desktop_host/mod.rs`、`computer_use_actions.rs:30`、`computer_use_host.rs`、`api/computer_use_api.rs`。
- `DesktopComputerUseHost` 状态按 session key 键控；删除 `APP_LOOP_TRACKER` 静态（循环检测并入 per-session optimizer）；Tauri 命令与管线共享同一实例边界定义。
- `ComputerUseHost` 60+ 方法按 §3.1 拆为 `DesktopSurface`（输入/截图/窗口）+ `AxProvider`（快照/定位）+ `OcrProvider`，工具层只依赖窄接口。
- 风险：中（并发路径）。验证：新增两会话并发单测（守卫/循环检测互不干扰）；macOS/Windows 手测。

### 阶段 4：provider 原生形态映射（~2 周）

- **文件**：`computer_use_tool.rs`、`tool-execution/src/context.rs`、provider converter 层。
- 新建版本注册表（仿 demo `groups.py`）：Claude 模型 → `computer_20250124`/`computer_20251124` + beta header，工具声明 `display_width_px/height` = 实际截图尺寸，默认 `enable_zoom`；非 Claude 模型沿用阶段 2 收敛后的自定义 schema；describe_screen 文本降级保留。
- 截图 retention（最近 N 张、按块修剪）进上下文管理器。
- 风险：中高——converter 改动影响所有多模态回传。验证：Anthropic 直连 + OpenAI 兼容两条链路的集成测试；同任务集对比原生形态 vs 自定义形态成功率（预期显著提升）。

### 阶段 5：浏览器栈收敛（~2-3 周）

- **文件**：`skills/policy.rs`（agent-browser 全模式默认 false）、`control_hub_tool.rs`、`browser_control/actions.rs`、`launcher.rs`、`claw_mode.md`/`computer_use_mode.md`/`agentic_mode.md`。
- 修 iframe：`element_center` 累加 `frameElement.getBoundingClientRect()` 偏移；删除死功能 `frame`/`frame_main`。
- headless 修复：`launch_with_cdp_opts` 实现真 headless 启动（独立端口 9223+、独立 user-data-dir），connect 校验 `/json/version` Headless 标识；绝不与 default 共用 9222。
- 结构化错误：`actions.rs` 直接返回 ErrorCode，删 `map_dispatch_error`；`control_hub_tool.rs` 按域拆文件。
- 合并 TerminalControl 双入口（保留 ControlHub terminal 域，注销独立注册）；`open_builtin` 返回值明示"面板不可观察"或补 URL/标题回传事件。
- **路由指导集中成一份**注入所有相关 prompt：WebFetch（无登录态读文）→ browser.read_article/fetch（登录态读）→ browser connect/snapshot（交互）→ ComputerUse（非 CDP 浏览器/桌面）→ open_builtin（给用户看）。
- 风险：中——agent-browser 用户回退路径需公告；headless 改动影响现有连接流程。验证：Chromium/Edge/Brave 连接矩阵测试 + iframe 点击回归页面 + prompt 一致性 grep 测试。

### 阶段 6：配置/权限/Peer 面（~2 周）

- **文件**：`control_hub_tool.rs`（真实 `is_enabled`）、`GlobalPermissionRulesDialog.tsx` + 后端 intent 枚举（新增 `browser_control`）、`session-config.json` 文案修正、`SessionConfig.tsx`（拆 personalization/permissions 两组件、状态命令走传输适配层或标注本机/远端）、`peer-device-adapter.ts`/`peer_host_invoke.rs`/`cli/peer_host/deny.rs`（Rust 单一真源导出 + contract test，`browser_control_*` 补 deny）、`ChatInput.tsx`/`AgentsScene.tsx`（抽 `useComputerUseEnabled()` hook，slash 路径补门禁，门禁移后端 `get_available_modes`）、`agents.rs`/`agentVisibility.ts`（统一 ComputerUse 身份与命名）。
- 风险：低中。验证：deny 表 contract test 三端对齐；Peer 场景手测开关/权限弹窗归属；关闭 computer use 后确认 ControlHub browser 域**不受影响**（两开关独立），关闭 browser_control 后确认 ControlHub browser 域禁用且 ComputerUse 不受影响。

---

## 5. 速赢清单（一周内，高价值小改动）

1. **清除幽灵工具名**（M1，半天）：`tool-contracts/src/computer_use.rs:1214,1584`、`framework.rs:2269-2279`、`computer_use_tool.rs:992`、`computer_use_host.rs` doc 中的 ComputerUseMouseStep/MousePrecise/MouseClick 全替换为现行 action 名。直接消除"模型调用不存在工具"的失败循环。
2. **截图落盘加门控**（M5，半天）：`try_save_screenshot_for_debug` 改为 debug 配置开关（默认关）+ 数量/天数轮转，`.bitfun/computer_use_debug` 进默认 gitignore。
3. **headless 误标止血**（C4，1 天）：在真 headless 实现前，`control_hub_tool.rs:539-603` 的 headless connect 至少校验 `/json/version` 是否含 Headless，否则报错而非标 "Headless test browser"；hint 改为引导 BitFun 托管 profile（`launch_with_cdp_opts` 已支持 `managed_profile_root`）而非教用户裸开 9222。
4. **边界 guard 最小落地**（C2/C6，1 天）：`desktop_action_targets_browser` 调用点移入 `call_impl` 的 click/type/key/scroll/drag 分发前；`is_probably_browser_app` 关键词表移除 firefox/safari。
5. **slash 门禁补齐**（M15，半天）：`ChatInput.tsx` 的 `selectSlashCommandMode`（L4130-4137）与 SlashModeItem 列表复用 `modeDisabled` 检查。
6. **文案矛盾统一**（C1 部分，半天）：`claw_mode.md`/`computer_use_mode.md`/agent-browser SKILL.md local_patch 三处路由指令统一为一个口径（过渡期先统一说 ControlHub）；修正 `computer_use_actions.rs:26` 的 `claw_mode.md` 错误引用。
7. **text-only schema 裁剪**（M7，半天）：`input_schema_text_only`（`computer_use_tool.rs:249`）移除 build_interactive_view/interactive_click/build_visual_mark_view/visual_click。
8. **scroll 坐标守卫**（半天）：`computer_use_tool.rs:1581-1586` 的 `scroll_x/scroll_y` 补 `ensure_global_xy_on_display` 校验。
9. **删除四个零引用死文件**（半天）：`computer_use_input.rs`、`computer_use_result.rs`、`computer_use_verification.rs`、`AIFeaturesConfig.tsx`。
10. **`useComputerUseEnabled()` hook**（半天）：统一 `ChatInput.tsx:872`/`AgentsScene.tsx:245`/`SessionConfig.tsx` 三处复制的订阅逻辑，初始值统一 false，消除首帧误导。
11. **文案过度承诺修正**（C5 部分，半天）：`session-config.json` 的 enableDesc 在 ControlHub 真实门控落地前，先如实说明"浏览器控制（ControlHub）不受此开关约束"。

---

### 附：预期收益

- 模型决策面从 40 action / 7 点击方言 → ~15 action / 1 条定位语法；Claude 直接吃训练先验（阶段 4 是执行质量的最大单点收益）。
- 浏览器控制从 7 条路径 → 1 条主路径（snapshot@ref）+ 明确降级阶梯，两套 `@eN` 冲突消失。
- 安全面从"提示词约束 + 恒开工具"→ 双 intent 权限 + 真实开关 + deny 表单一真源。
- 代码量预计净删 6-8 千行（死代码 + 重复方言 + 双份 schema），`control_hub_tool.rs` 与 `computer_use_tool.rs` 两个 2000+ 行 god file 拆解为按域模块。
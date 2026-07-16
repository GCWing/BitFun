# OpenCode 扩展兼容总览

本文是 BitFun 适配 OpenCode 扩展生态的总入口。它只回答三件事：BitFun 与每类 OpenCode 能力差在哪里、能否适配、需要补什么。实现细节分别放在配置、服务插件、终端插件和通用主机设计中。

本文描述目标设计与当前差距，不代表矩阵中的目标能力已经实现。只有通过冻结版本样例和端到端验证的能力才能标记为已实现。
矩阵是兼容审计库存，不是默认开发路线图；`OC-R*` 只表示该能力依赖的成熟度分区，近期执行顺序以
[`OC-E0` 至 `OC-E3`](../../plans/opencode-extension-compatibility-plan.md) 为准。

| 主题 | 详细设计 |
|---|---|
| 配置来源、Rules、Agents、Skills、Commands、MCP、LSP、Formatter、Theme、Keybind | [配置与声明式资产适配](opencode-config-assets-adapter-design.md) |
| JS/TS 工具、软件包插件、稳定 Hook、`client`、`serverUrl`、`$` | [服务插件运行时适配](opencode-plugin-runtime-adapter-design.md) |
| TUI target、Route、Command、Keymap、Dialog、Slot、Theme、State、KV | [终端界面插件适配](opencode-tui-plugin-adapter-design.md) |
| SDK、Server、ACP、IDE、Web、GitHub、GitLab、Slack | [外部集成适配](opencode-external-integration-adapter-design.md) |
| 进程、调用、超时、恢复、状态与 BitFun 归属模块边界 | [插件运行时主机](plugin-runtime-host-design.md) |
| 交付顺序和阶段退出条件 | [粗粒度计划](../../plans/opencode-extension-compatibility-plan.md) |

## 1. 基线与判断方法

本次清单刷新于 2026-07-16：

- 最新稳定版为 [`v1.18.2`](https://github.com/anomalyco/opencode/releases/tag/v1.18.2)，提交为 [`70b56a0a93d366889cae950379cc9d2537148fa2`](https://github.com/anomalyco/opencode/commit/70b56a0a93d366889cae950379cc9d2537148fa2)。
- 开发分支前瞻检查记录为提交 [`a27ffb20f0a740823142f7c79706efb81d75d287`](https://github.com/anomalyco/opencode/commit/a27ffb20f0a740823142f7c79706efb81d75d287)。该值会持续变化，只用于发现差异，不计入稳定兼容承诺。
- 配置、插件、工具、Agent、Skill、Command、Rule、MCP、LSP、Formatter、Theme、Keybind、开发工具包、Server 和 ACP 以 [OpenCode 官方文档](https://opencode.ai/docs/) 为准。
- 稳定服务插件接口以 [`packages/plugin/src/index.ts`](https://github.com/anomalyco/opencode/blob/70b56a0a93d366889cae950379cc9d2537148fa2/packages/plugin/src/index.ts) 为准；
- custom tool 接口以 [`packages/plugin/src/tool.ts`](https://github.com/anomalyco/opencode/blob/70b56a0a93d366889cae950379cc9d2537148fa2/packages/plugin/src/tool.ts) 为准；
- 终端插件接口以 [`packages/plugin/src/tui.ts`](https://github.com/anomalyco/opencode/blob/70b56a0a93d366889cae950379cc9d2537148fa2/packages/plugin/src/tui.ts) 为准；
- 终端插件行为说明以 [`tui-plugins.md`](https://github.com/anomalyco/opencode/blob/70b56a0a93d366889cae950379cc9d2537148fa2/packages/opencode/specs/tui-plugins.md) 为准。

稳定兼容只冻结 `v1.18.2` 的公开文档、接口源码和样例；开发提交仅用于发现未来差异，不进入当前承诺。升级时必须
重新比较实际消费的文件和行为，不能沿用本次结论。

### 1.1 差异类型

矩阵用以下六种类型说明 BitFun 真正要做的工作。一个扩展项可以同时包含两种类型。

| 差异类型 | 含义 |
|---|---|
| 补基础能力 | BitFun 还没有可承接该行为的真实产品能力，必须先补归属模块和消费方。 |
| 补扩展接口 | BitFun 有基础能力，但没有供插件调用的稳定接口或 Hook。 |
| 融合现有能力 | 两边都有相近能力，但加载顺序、状态、权限或最终归属不同，需要统一语义。 |
| 转换参数 | 基础行为一致，只需转换格式、字段、作用域、错误或生命周期。 |
| 直接桥接 | BitFun 已有窄接口，增加少量兼容门面即可。 |
| 明确降级 | 组件运行时、产品边界或接口稳定性使完整等价不合理；必须给出替代行为。 |

“BitFun 有类似模块”不等于“OpenCode 已兼容”。可实现性只使用以下结论：

| 结论 | 含义 |
|---|---|
| 可完整适配 | 可以保留稳定版的可观察行为、顺序和冲突语义。 |
| 可主要适配 | 主流程可用，少量平台差异由宿主能力决定。 |
| 明确降级 | 只提供可解释的替代行为，不宣称完整兼容。 |
| 暂不承诺 | 接口不稳定，或实现会复制另一套产品运行时。 |

## 2. 总体方案

- BitFun 实现自己的插件运行时编排、脚本执行实现、OpenCode 兼容接口和 Rust 能力转发；不启动完整
  OpenCode Agent Runtime，也不把 Bun 或物理进程拓扑固化进 Host ABI。
- 第一条闭环只承诺官方 `.opencode/tools/` JS/TS 来源，不要求用户重打包或再次激活。单数 `tool/` 只有在冻结
  版本源码和测试明确证明时才作为该版本兼容输入，不能写成长期公开保证。package plugin、完整配置、
  Hook 和 TUI target 在该闭环稳定后按真实样例扩展。
- Desktop 可以先验证固定版本 Bun；HarmonyOS PC 原生 CLI/TUI 必须按
  [平台专题](../platform-portability-design.md)独立取证，不包含 HarmonyOS 手机 Remote App。只能覆盖 JS 子集的实现必须显示“能力受限”并列出
  兼容差异，不能宣称 OpenCode runtime 完整兼容。
- 扩展调用必须有期限、取消、有界队列、大小检查和崩溃恢复；更细的权限、沙箱和组织策略沿用现有控制点并延期
  单独设计，不在首条闭环扩大接口。
- BitFun 归属模块负责最终业务状态；适配器只保留 OpenCode 的格式、顺序、参数和错误语义。

近期优先级：

| 优先级 | 可观察结果 | 暂不绑定的工作 |
|---|---|---|
| OC-E0 | 固定版本、官方 custom tool 契约、无外部依赖的 fixture、当前静态预览明确显示“未执行” | 全量配置导入 |
| OC-E1 | 上述 fixture 的真实 `execute` 进入现有 Tool Runtime，支持身份/路径字段和 `abort`，并在 CLI 返回结果/诊断 | `metadata`/`ask`、官方 import 型样例、package plugin、Hook、TUI |
| OC-E2 | 一个真实 package plugin，仅实现其需要的 loader 和最小 client/context | 全部 loader fallback 和 Client API |
| OC-E3 | 按阻塞样例加入 Hook；TUI 先接 command/slash/key，toast 需先有 CLI 类型化状态/通知 owner | 原始 renderer、Server、Remote、连接器 |

## 3. 能力矩阵

`当前状态`只表示 OpenCode 兼容行为是否已经进入 BitFun 生产路径，不把“BitFun 有相似基础模块”算成已兼容。
`成熟度依赖（非执行顺序）`表示该能力在完整兼容成熟度中的依赖位置，不代表近期执行顺序、承诺版本或必须实现。实际立项还必须有
真实样例/消费方，并满足 OC-E 阶段与产品架构总计划的退出条件。

这些表是差异审计库存，不是实施说明。快速阅读只需关注“扩展项、当前状态、目标可实现性、成熟度依赖、细节”；
“BitFun 差异”和“需要完成的工作”用于解释为何不能直接桥接。实际实现范围以链接的专题设计和 OC-E 计划为准，
不能把一整张表放进同一阶段。

### 3.1 配置与声明式资产

| OpenCode 扩展项 | BitFun 差异 | 当前状态 | 目标可实现性 | 成熟度依赖（非执行顺序） | BitFun 需要完成的工作 | 细节 |
|---|---|---|---|---|---|---|
| 配置层级与合并 | 融合现有能力 | 未实现 | 可完整适配 | OC-R1 | 按 remote、global、自定义文件、project、`.opencode`、内联和组织配置构造来源图并保留最终来源 | [来源与合并](opencode-config-assets-adapter-design.md#3-配置层级与来源) |
| JSON、JSONC、环境变量、文件引用 | 转换参数 + 明确降级 | 未实现 | 可主要适配 | OC-R1 | 有效配置保持 OpenCode 解码语义；未知字段保留和非安全字段局部恢复属于 BitFun 鲁棒性增强，安全/执行字段无效时不激活受影响结果 | [解析与鲁棒性](opencode-config-assets-adapter-design.md#4-解析与鲁棒性) |
| 独立 `tui.json/jsonc` | 融合现有能力 + 转换参数 | 未实现 | 可完整适配 | OC-R1 | 按 global、`OPENCODE_TUI_CONFIG`、project、`.opencode` 独立顺序加载，不能复用主配置优先级 | [TUI 来源](opencode-config-assets-adapter-design.md#32-tui-独立来源顺序) |
| Rules / Instructions | 转换参数 | 未实现 | 可完整适配 | OC-R1 | R1 映射本地/已缓存内容；需要主动联网的远程 instruction 在 R2 通过归属模块保护后获取 | [声明式资产](opencode-config-assets-adapter-design.md#5-声明式资产映射) |
| Agents / Modes | 融合现有能力 + 转换参数 | 未实现 | 可主要适配 | OC-R1 | 转换 Markdown/JSON、primary/subagent、模型、工具和权限；由 Agent 归属模块提交 | [Agents 与 Skills](opencode-config-assets-adapter-design.md#52-agentsmodes-与-skills) |
| Skills | 转换参数 | 未实现 | 可完整适配 | OC-R2 | R1 发现/展示；R2 保留按需加载及 allow/deny/ask 顺序，可执行资源走 Skill 归属模块保护 | [Agents 与 Skills](opencode-config-assets-adapter-design.md#52-agentsmodes-与-skills) |
| References | 补基础能力 + 转换参数 | 未实现 | 可主要适配 | OC-R2 | R1 解析；R2 支持本地目录和 Git repository/branch/description/hidden，异步准备并接入 `@alias` | [声明式资产](opencode-config-assets-adapter-design.md#5-声明式资产映射) |
| Commands | 补扩展接口 + 转换参数 | 未实现 | 可完整适配 | OC-R2 | R1 只展示；R2 把声明注册到 CLI action registry，再由既有 controller 执行参数、`@file`、shell 展开、Agent/model/subtask | [Commands](opencode-config-assets-adapter-design.md#53-commands) |
| Models / Providers 配置 | 融合现有能力 | 未实现 | 可主要适配 | OC-R1 | 静态字段进入模型归属模块；动态模型、鉴权和请求头交给插件运行时 | [声明式资产](opencode-config-assets-adapter-design.md#5-声明式资产映射) |
| MCP | 转换参数 | 未实现 | 可完整适配 | OC-R2 | R1 解析；R2 转换本地命令、远程 URL、Headers、OAuth、超时和 Agent 选择并由 MCP owner 启动 | [MCP、LSP 与 Formatter](opencode-config-assets-adapter-design.md#54-mcplsp-与-formatter) |
| LSP | 转换参数 | 未实现 | 可完整适配 | OC-R2 | R1 解析；R2 转换 command、extensions、env 和 initialization 并由 LSP owner 启动 | [MCP、LSP 与 Formatter](opencode-config-assets-adapter-design.md#54-mcplsp-与-formatter) |
| Formatters | 补基础能力 + 转换参数 | 未实现 | 可主要适配 | OC-R2 | R1 解析；R2 补文件写入后的格式化执行能力，再映射 command/environment/extensions/`$FILE` | [MCP、LSP 与 Formatter](opencode-config-assets-adapter-design.md#54-mcplsp-与-formatter) |
| Themes | 转换参数 | 未实现 | 可主要适配 | OC-R1 | 保留 builtin/user/project/cwd 覆盖顺序，分别映射 GUI 和 TUI 色彩能力 | [声明式资产](opencode-config-assets-adapter-design.md#5-声明式资产映射) |
| Keybinds | 补扩展接口 + 转换参数 | 未实现 | 可主要适配 | OC-R1 | 为运行时 TUI 输入增加 `tui.json` 兼容入口，处理 leader、组合键、禁用和冲突 | [声明式资产](opencode-config-assets-adapter-design.md#5-声明式资产映射) |
| Shell / Tools / Attachments / Share / Snapshot / Compaction / Watcher | 融合现有能力 + 转换参数 | 未实现 | 可主要适配 | OC-R2 | R1 解析非执行字段；R2 才把可能启动进程、联网或调用工具的字段接到各归属模块 | [其他稳定配置](opencode-config-assets-adapter-design.md#55-其他稳定配置项) |
| Log / Username / Enterprise / Tool output / 旧字段迁移 | 转换参数或补基础能力 | 未实现 | 可主要适配 | OC-R1 | 覆盖 `logLevel`、`username`、`enterprise`、`tool_output` 及 `reference/autoshare/layout/mode` 迁移 | [其他稳定配置](opencode-config-assets-adapter-design.md#55-其他稳定配置项) |
| `server` | 明确降级 | 未实现 | 明确降级 | OC-R4-P | 只供显式外部协议兼容服务使用，不改变普通 BitFun 启动方式 | [其他稳定配置](opencode-config-assets-adapter-design.md#55-其他稳定配置项) |
| `autoupdate` | 明确降级 | 不适用 | 明确降级 | 不安排 | 不控制 BitFun 产品更新；保留来源并显示“不适用于 BitFun 更新” | [其他稳定配置](opencode-config-assets-adapter-design.md#55-其他稳定配置项) |

本类整体风险是来源优先级错误、相似能力语义不一致和远程执行域错配。控制点集中在来源图、字段级诊断、归属模块校验和官方配置样例，不在每个配置项内重复设计。

### 3.2 工具与服务插件

| OpenCode 扩展项 | BitFun 差异 | 当前状态 | 目标可实现性 | 成熟度依赖（非执行顺序） | BitFun 需要完成的工作 | 细节 |
|---|---|---|---|---|---|---|
| `.opencode/tools/*.ts` / `*.js` | 补基础能力 | 静态名称预览 | 可完整适配 | OC-R2 | worker 保留 Zod shape/refinement 与 execute，只把模型可见 JSON Schema 传给 Rust；取消、元数据、权限请求和附件结果走类型化进程通信 | [工具加载](opencode-plugin-runtime-adapter-design.md#5-工具与插件加载) |
| 插件 `tool` map | 补基础能力 + 补扩展接口 | 未实现 | 可完整适配 | OC-R2 | 运行插件工厂，按同一双表示注册真实工具，并接到 Tool 归属模块 | [工具加载](opencode-plugin-runtime-adapter-design.md#5-工具与插件加载) |
| 项目与用户目录插件 | 补基础能力 | 未实现 | 可完整适配 | OC-R2 | 直接发现和加载本地 JS/TS 模块，不要求 BitFun 专用清单 | [服务插件](opencode-plugin-runtime-adapter-design.md#52-服务插件) |
| 配置中的软件包插件 | 补基础能力 | 未实现 | 可完整适配 | OC-R2 | 用 npm 配置、Arborist、package-lock 和 `ignoreScripts: true` 准备依赖，再由固定版本 Bun 加载 | [服务插件](opencode-plugin-runtime-adapter-design.md#52-服务插件) |
| 全局插件加载 | 补基础能力 | 未实现 | 可完整适配 | OC-R2 | 自动加载全局配置和 ConfigPaths 全局目录，并按完整来源图生成 `plugin_origins`；不简化成固定四级顺序 | [服务插件](opencode-plugin-runtime-adapter-design.md#52-服务插件) |
| `package.json`、入口与依赖 | 补基础能力 | 未实现 | 可主要适配 | OC-R2 | 复现 server target、入口回退、`engines.opencode`、npm 配置和锁文件；原生模块失败只影响对应插件 | [来源与执行版本](opencode-plugin-runtime-adapter-design.md#4-来源与执行版本) |
| 内置/外部顺序、pure、重复插件、同名工具覆盖 | 融合现有能力 | 未实现 | 可完整适配 | OC-R2 | 复现 internal-first、pure 跳过 external、来源顺序、去重与覆盖；仅允许显式策略保护极少数产品关键项 | [注册与覆盖](opencode-plugin-runtime-adapter-design.md#53-注册与覆盖) |
| `project` / `directory` / `worktree` | 直接桥接 | 未实现 | 可完整适配 | OC-R2 | 传递真实执行域身份与路径；Remote 在 OC-R5 前返回 `unsupported` | [兼容门面](opencode-plugin-runtime-adapter-design.md#7-opencode-兼容门面) |
| `client` | 补扩展接口 | 未实现 | 可主要适配 | OC-R2 | 提供版本化插件客户端门面，按方法转发到现有 BitFun 归属模块 | [兼容门面](opencode-plugin-runtime-adapter-design.md#7-opencode-兼容门面) |
| `serverUrl` | 补扩展接口 | 未实现 | 可主要适配 | OC-R2 | 在 worker 执行域提供真实回环服务，只实现插件所需的版本化路由 | [兼容门面](opencode-plugin-runtime-adapter-design.md#7-opencode-兼容门面) |
| `$` 与脚本环境 | 补基础能力 | 未实现 | 可完整适配 | OC-R2 | 固定 Bun worker 提供真实 `$`；受限模式只能依赖真实 OS/容器边界，无法落实时停用 target | [默认策略](opencode-plugin-runtime-adapter-design.md#3-默认策略与可调权限) |
| 加载、停用、更新与崩溃恢复 | 补基础能力 | 未实现 | 可主要适配 | OC-R2 | 建立来源限定身份、target 状态、后台候选、健康旧进程保留和可验证的精确版本恢复 | [生命周期](opencode-plugin-runtime-adapter-design.md#9-生命周期与能力暴露) |
| 跨插件进程全局共享 | 明确降级 | 未实现 | 明确降级 | OC-R2 | 每 target 使用独立可终止进程；不承诺 `globalThis`、进程环境或模块单例的未文档化共享 | [故障域](opencode-plugin-runtime-adapter-design.md#81-故障域) |

本类整体风险是第三方代码副作用、依赖安装失败、Hook 顺序漂移和 worker 失控。默认权限可以开放，但执行隔离、超时、取消、队列上限、结果大小和故障恢复必须始终启用。

### 3.3 稳定服务 Hook

| Hook | BitFun 差异 | 当前状态 | 目标可实现性 | 成熟度依赖（非执行顺序） | BitFun 需要完成的工作 |
|---|---|---|---|---|---|
| `dispose` | 直接桥接 | 未实现 | 可完整适配 | OC-R3 | 调用清理并设置期限；超时回收 worker。 |
| `event` | 补扩展接口 | 未实现 | 可完整适配 | OC-R3 | 提供版本化事件代理并隔离插件异常。 |
| `config` | 补扩展接口 + 融合现有能力 | 未实现 | 可完整适配 | OC-R3 | 按插件顺序变换，最后由 Config 归属模块校验提交。 |
| `tool` | 补基础能力 + 补扩展接口 | 未实现 | 可完整适配 | OC-R2 | 注册真实工具定义与执行函数。 |
| `auth` | 补扩展接口 | 未实现 | 可主要适配 | OC-R3 | 提供 API/OAuth 方法和脱敏凭据代理。 |
| `provider` | 补扩展接口 + 融合现有能力 | 未实现 | 可主要适配 | OC-R3 | 将动态模型列表接入 Provider 归属模块。 |
| `chat.message` | 补扩展接口 | 未实现 | 可完整适配 | OC-R3 | 依次变换消息和 parts，变换后重做结构校验。 |
| `chat.params` | 补扩展接口 + 融合现有能力 | 未实现 | 可完整适配 | OC-R3 | 依次变换模型参数，显式产品上限最后生效。 |
| `chat.headers` | 补扩展接口 | 未实现 | 可完整适配 | OC-R3 | 依次变换请求头，敏感值不进入日志。 |
| `permission.ask` | 融合现有能力 | 未实现 | 可主要适配 | OC-R3 | 默认保留 allow/deny/ask 语义；用户或组织策略可收紧。 |
| `command.execute.before` | 补扩展接口 | 未实现 | 可完整适配 | OC-R3 | 在命令执行前依次变换消息 parts。 |
| `tool.execute.before` | 补扩展接口 | 未实现 | 可完整适配 | OC-R3 | 变换最终参数，随后重做 schema 和权限判断。 |
| `shell.env` | 补扩展接口 | 未实现 | 可完整适配 | OC-R3 | 在实际执行域构造环境变量。 |
| `tool.execute.after` | 补扩展接口 | 未实现 | 可完整适配 | OC-R3 | 依次变换 title、output、metadata，保留原始结果引用。 |
| `tool.definition` | 补扩展接口 + 融合现有能力 | 未实现 | 可完整适配 | OC-R3 | 变换模型可见 JSON Schema；真实执行继续使用 worker 中原始 Zod 校验，保持 OpenCode 双表示语义。 |

Hook 的共同风险是把变换误做成通知、并行调用破坏顺序或插件写入非法状态。所有 Hook 都走类型化调用、顺序执行和归属模块终检；具体调用协议见[服务插件运行时设计](opencode-plugin-runtime-adapter-design.md#6-钩子适配与权威提交)。

### 3.4 终端界面插件

| OpenCode 扩展项 | BitFun 差异 | 当前状态 | 目标可实现性 | 成熟度依赖（非执行顺序） | BitFun 需要完成的工作 | 细节 |
|---|---|---|---|---|---|---|
| 独立 TUI target、options、meta、lifecycle | 补基础能力 | 未实现 | 可完整适配 | OC-R4-T | 独立解析 `tui.json`，加载 target-only 模块并维护启停、取消和清理 | [发现与生命周期](opencode-tui-plugin-adapter-design.md#4-发现加载和生命周期) |
| `app`、`tuiConfig`、`keys`、`mode` | 补扩展接口 + 转换参数 | 未实现 | 可主要适配 | OC-R4-T | 提供版本、实时配置、按键格式化和模式栈兼容门面 | [能力映射](opencode-tui-plugin-adapter-design.md#5-能力映射) |
| Command 与 slash alias | 补扩展接口 | 未实现 | 可完整适配 | OC-R4-T | 声明注册到 CLI action registry，保持来源顺序，并由既有 controller 执行 | [Command](opencode-tui-plugin-adapter-design.md#54-command-与-slash-alias) |
| Route 身份与导航 | 融合现有能力 | 未实现 | 可主要适配 | OC-R4-T | 保留 route id、覆盖顺序和 navigate/current；渲染降级页由 BitFun 提供退出动作 | [Route](opencode-tui-plugin-adapter-design.md#53-route-与导航) |
| Keys、Keymap、Layer、Binding、Mode | 转换参数 + 明确降级 | 未实现 | 可主要适配 | OC-R4-T | 转换公开键位和分发语义；依赖 OpenTUI Renderable 的方法明确不支持 | [Keymap](opencode-tui-plugin-adapter-design.md#55-keyskeymaplayerbinding-与-mode) |
| Alert / Confirm / Prompt / Select / Toast | 转换参数 | 未实现 | 可主要适配 | OC-R4-T | 把已知属性和返回值映射到 Ratatui 宿主交互 | [Dialog](opencode-tui-plugin-adapter-design.md#56-dialogtoast-与-prompt) |
| Theme、Attention、通知、声音 | 转换参数 | 未实现 | 可主要适配 | OC-R4-T | 接到主题与平台通知能力，无系统能力时降级到文本 | [Theme 与通知](opencode-tui-plugin-adapter-design.md#58-theme) |
| State、共享 KV、Client、Events | 补扩展接口 + 融合现有能力 | 未实现 | 可主要适配 | OC-R4-T | 提供实时只读状态、应用级共享 KV、兼容客户端和 v2 事件 | [状态与事件](opencode-tui-plugin-adapter-design.md#510-statekvclient-与-events) |
| 插件 list / activate / deactivate / add / install | 补基础能力 + 补扩展接口 | 未实现 | 可完整适配 | OC-R4-T | 分别映射查询、启停、当前会话加载和安装；`install` 不自动 `add` | [插件管理](opencode-tui-plugin-adapter-design.md#511-插件安装启用和停用) |
| Host / plugin Slots | 明确降级 | 未实现 | 明确降级 | OC-R4-T | 识别名称、属性、模式、顺序和清理；原始 Solid/OpenTUI 内容返回稳定不支持 | [Slots](opencode-tui-plugin-adapter-design.md#57-slots) |
| Route / Dialog / Prompt 的任意 JSX | 明确降级 | 未实现 | 明确降级 | OC-R4-T | 不打开空白界面；显示不支持原因并提供返回动作 | [渲染边界](opencode-tui-plugin-adapter-design.md#8-无法直接等价的边界) |
| 原始 `CliRenderer`、Solid/OpenTUI 组件树 | 明确降级 | 未实现 | 暂不承诺 | OC-R5 | 不维护第二套终端渲染树；出现高价值真实需求后单独评估 | [渲染边界](opencode-tui-plugin-adapter-design.md#8-无法直接等价的边界) |

本类整体风险是两套组件运行时不等价、输入焦点失配和异常后终端状态未恢复。宿主操作与原始组件渲染必须分开判定；任何降级页面都必须可退出，不能形成空白页或锁死 modal。

### 3.5 外部接口与实验能力

| 扩展项 | BitFun 差异 | 当前状态 | 目标可实现性 | 成熟度依赖（非执行顺序） | BitFun 需要完成的工作 | 细节 |
|---|---|---|---|---|---|---|
| OpenCode 开发工具包客户端 | 补扩展接口 | 未实现 | 可主要适配 | OC-R4-P | 先实现真实消费的方法；未知读接口稳定失败，未知写接口绝不伪造成功 | [外部集成设计](opencode-external-integration-adapter-design.md) |
| HTTP / OpenAPI / SSE | 融合现有能力 + 明确降级 | 未实现 | 可主要适配 | OC-R4-P | 插件回环服务复用处理器；完整外部协议独立验收 | [显式兼容服务](opencode-external-integration-adapter-design.md#41-显式兼容服务) |
| ACP | 转换参数 | 未实现 | 可主要适配 | OC-R4-P | 映射工具、命令、MCP、规则、Formatter、Agent 和权限 | [能力结论](opencode-external-integration-adapter-design.md#2-能力与产品结论) |
| IDE 扩展（VS Code/Cursor/Windsurf/VSCodium） | 补基础能力 + 融合现有能力 | 未实现 | 可主要适配 | OC-R4-P | BitFun 扩展实现启动/聚焦与上下文；原扩展直连须另装 `opencode` 兼容启动器并精确覆盖环境变量、`GET /app` 和 `POST /tui/append-prompt` | [IDE](opencode-external-integration-adapter-design.md#42-ide) |
| Web 与 attach 客户端 | 补基础能力 + 明确降级 | 未实现 | 明确降级 | OC-R5 | 优先使用 BitFun Web/Remote；原始客户端直连另行实现 Server 协议 | [能力结论](opencode-external-integration-adapter-design.md#2-能力与产品结论) |
| GitHub Action / App | 融合现有能力 + 明确降级 | 未实现 | 明确降级 | OC-R4-C | 提供 BitFun GitHub 工作流，不冒充 `opencode` 二进制 | [代码托管与 Slack](opencode-external-integration-adapter-design.md#43-githubgitlab-与-slack) |
| GitLab CI / Duo | 融合现有能力 + 明确降级 | 未实现 | 明确降级 | OC-R4-C | 提供 BitFun CI/触发器，不把 runner/CLI 计入插件兼容 | [代码托管与 Slack](opencode-external-integration-adapter-design.md#43-githubgitlab-与-slack) |
| Slack | 补基础能力 + 转换参数 | 未实现 | 可主要适配 | OC-R4-C | 实现 BitFun Slack 连接器；原 `@opencode-ai/slack` 直连取决于 SDK/Server 覆盖 | [代码托管与 Slack](opencode-external-integration-adapter-design.md#43-githubgitlab-与-slack) |
| `experimental.chat.messages.transform` | 补扩展接口 | 未实现 | 暂不承诺 | OC-R5 | 保留前瞻样例，稳定后复用消息变换路径 | 本节 |
| `experimental.chat.system.transform` | 补扩展接口 + 融合现有能力 | 未实现 | 暂不承诺 | OC-R5 | 稳定后接入系统提示归属模块 | 本节 |
| `experimental.provider.small_model` | 转换参数 | 未实现 | 暂不承诺 | OC-R5 | 只做版本差异监控 | 本节 |
| `experimental.session.compacting` | 融合现有能力 | 未实现 | 暂不承诺 | OC-R5 | 只做试验样例，不改变会话持久化事实 | 本节 |
| `experimental.compaction.autocontinue` | 融合现有能力 | 未实现 | 暂不承诺 | OC-R5 | 稳定后再评估长任务控制流 | 本节 |
| `experimental.text.complete` | 补扩展接口 | 未实现 | 暂不承诺 | OC-R5 | 只做版本差异监控 | 本节 |
| `experimental_workspace.register` | 融合现有能力 | 未实现 | 暂不承诺 | OC-R5 | 不让实验接口接管 Workspace/Remote 生命周期 | 本节 |

本类整体风险是把插件所需的局部接口扩张成第二套 OpenCode Server，或把官方产品集成误算成插件兼容。稳定接口按真实消费方逐步增加；实验接口只监控和保留样例。

## 4. 版本演进与插件更新体验

### 4.1 兼容版本

每个兼容版本只维护四类事实：OpenCode 稳定版提交、配置与接口清单、加载/覆盖顺序、官方及真实插件样例。通用主机不包含 OpenCode 字段；大多数升级只修改解析、参数转换或兼容门面。

OpenCode 发布新稳定版时按以下顺序升级：

1. 比较稳定版的配置 schema、服务 Hook、TUI API、事件和加载规则。
2. 用第 1.1 节的差异类型标记新增或变化项，先判断是参数转换还是语义变化。
3. 优先只更新版本化适配层；只有 OpenCode 增加了 BitFun 完全没有的产品行为时才补基础能力。
4. 旧兼容版本继续可用，直到新版本的官方样例、顺序、失败和恢复测试通过。
5. 测试通过后再推进默认兼容版本；开发分支变化只产生前瞻告警。

未知内容统一局部降级：未知配置字段保留；服务 v1 未知事件跳过并聚合诊断；TUI v2 未知事件只转发事件类型标记，不转发未验证 payload；未知只读 API 返回稳定不支持；未知写入或变换 API 不执行且不伪造成功。任何未知项都不能造成无限重试、日志风暴或主界面等待。

### 4.2 首次加载与全局插件

- 启动时按[完整来源图](opencode-config-assets-adapter-design.md#31-opencode-来源图)生成 `plugin_origins`，并包含
  ConfigPaths 中各配置/插件目录；目录自动发现只适用于服务插件，TUI target 必须出现在合并后的
  `tui.json/jsonc` `plugin` 列表。
- 第三方模块 import 前，依据来源身份/内容版本、target、实际执行域/用户、产品/组织策略上限、凭据和环境范围
  重新计算当前有效策略与安全启动参数，不能复用发现期或另一执行域的决定。默认兼容模式可以直接允许，不增加
  二次审核；需要先检查来源的用户可用安全启动暂停全部外部 target。首次来源提示是非阻塞信息，不伪装成能够
  撤销已经发生的直接脚本副作用。
- 依赖准备和 worker 启动在后台执行；主界面可进入，插件状态显示“准备中”。初始化、Hook、Tool 和 Client 使用
  各自的可见等待预算、取消和超时结果，不阻塞无关会话。
- 全局插件的来源在每个项目状态页可见。停用/恢复默认只作用于“当前项目 + 所选 server/tui target”；“所有项目”
  必须显式选择并列出受影响项目。仅当前会话的操作必须标为临时，重启后恢复配置来源结果。
- 全局更新显示来源限定身份、target、候选版本和所有受影响项目。各项目在无新调用的安全边界独立切换代次；
  已开始的旧代次调用按原期限完成或由用户取消，不能把某个项目的已切换状态冒充全部项目均已切换。

### 4.3 插件变化、旧进程保留与恢复

来源变化后，BitFun 先准备候选版本，再切换正在使用的版本：

```text
发现变化 -> 后台准备候选版本 -> 比较贡献与权限变化 -> 变化可见/可暂停或收紧 -> 安全边界切换
                               \-> 准备失败且健康旧进程仍合规：继续使用旧进程
```

代码或依赖更新失败时可以保留健康旧进程；重建旧代次必须有摘要匹配的精确物化目录。显式停用、删除、来源撤销、
权限收紧或安全策略失效必须先停止新调用并撤下旧贡献，不能恢复到不再合规的旧状态。

| 变化 | 用户体验 |
|---|---|
| 本地文件或配置 spec 变化，能力集合不变 | 后台重载，在下一次安全调用边界切换；状态短暂显示“正在更新”。 |
| bare `latest` 软件包可能有新版本 | 冻结源码的缓存命中不会主动刷新；BitFun 以“检查更新/更新”增强显示候选版本和影响范围，不静默换包。 |
| 新增/删除工具、Hook、权限或依赖 | 候选激活前显示简短差异、来源、target 和作用域；使用可配置的非阻塞切换窗口，并在继续前重新计算当前有效策略。期间的暂停/受限/停用先使候选失效，再重新评估；保留一次持久摘要。 |
| 已启用来源的代码或依赖更新失败 | 健康旧进程仍满足当前策略时继续服务，并显示“候选更新失败，仍在使用旧进程”和一次聚合诊断。 |
| 来源撤销、权限收紧或安全策略失效 | 立即阻止新调用并撤下不再合规的贡献，不以旧版本回退绕过。 |
| 插件被删除或显式停用 | 停止接收新调用；在期限内完成或取消在途调用，清理该 target，不影响其他插件。 |
| worker 崩溃 | 在途调用以 `worker-lost` 失败且不自动重放；只有精确旧物化目录仍可校验时才能重建旧代次，否则显示“上一版本不可恢复”；按插件主机统一配置的有界预算与退避恢复。 |

执行版本记录不是源码备份。软件包或文件的精确物化目录仍在且摘要匹配时可以重建旧代次；本地原位源码已变化、
旧 worker 又丢失时不能从当前来源重建后仍称为旧版本。此时只允许准备当前来源或等待用户恢复源码。

## 5. 大类风险

| 大类 | 整体风险 | 主要控制点 |
|---|---|---|
| 配置与声明式资产 | 来源优先级错误、字段语义错配、远程路径误用 | 来源图、字段级诊断、版本化样例、实际执行域解析 |
| 工具与服务插件 | 任意代码副作用、依赖失败、顺序漂移、进程与系统资源失控 | import 前策略、安全启动、独立进程树、平台资源预算、固定运行时、顺序测试、期限、取消、有界队列、可验证的旧代次 |
| 稳定 Hook | 把变换误作通知、非法结果污染业务状态 | 类型化调用、顺序执行、每步结构检查、归属模块终检 |
| 终端插件 | 组件运行时不等价、焦点/模式锁死、终端恢复失败 | 宿主操作与渲染分离、安全降级页、强制清理和终端恢复测试 |
| 外部与实验接口 | 复制第二产品协议、稳定接口被实验变化拖动 | 按真实消费方扩展、稳定与实验清单分离、兼容版本冻结 |
| 默认开放权限 | 插件可直接产生文件、网络和进程副作用 | 明确本地信任边界、可调权限、来源可见、进程隔离；不虚构细粒度拦截能力 |

## 6. 明确限制与延期决策

| 能力 | 结论 | 原因 | 替代行为 |
|---|---|---|---|
| 原始 `CliRenderer` 和 Solid/OpenTUI 组件树 | 暂不承诺完整兼容 | BitFun Ratatui 与 OpenCode 组件树、布局和生命周期不共用运行时 | 适配导航、命令、公开键位、已知对话、主题和通知；原始组件显示明确不支持。 |
| `api.app.version` 无法表达 renderer 降级 | 协议限制 | 插件只能读取兼容版本，没有能力协商字段，可能在懒路径选择 BitFun 不支持的组件能力 | 初始化依赖 renderer 时拒绝整个 target；懒路径返回 `unsupported(renderer-required)`，不能宣称仅凭版本检查即可兼容。 |
| 完整 OpenCode HTTP Server 协议 | 不作为插件兼容前置目标 | 会形成第二套产品协议、会话和错误模型 | 为插件实现所需 Client/回环路由；外部协议按独立产品需求扩展。 |
| 原始 IDE/Web/attach/GitHub/GitLab 客户端或流程直接连接 BitFun | 不承诺直接替换 | 这些入口依赖 OpenCode CLI、Server、会话和产品流程，不是插件接口 | 提供 BitFun 原生集成；IDE `/tui` 子集和外部协议按真实需求单独兼容。 |
| 插件间 `globalThis`、进程环境和模块单例共享 | 明确不兼容 | 每 target 独立进程才能可靠终止死循环和内存失控 | 保留官方 PluginInput、Hook 顺序和显式接口，不支持未文档化进程全局副作用。 |
| `server` / `autoupdate` 在普通 BitFun 启动中的行为 | 明确降级 | 两者分别属于 OpenCode 服务进程和 OpenCode 自身更新 | 显式兼容服务可映射 `server`；`autoupdate` 只保留来源并说明不适用。 |
| 未文档化内部接口 | 不承诺 | 没有稳定版本和契约 | 返回稳定不支持并进入版本前瞻报告。 |
| `experimental_workspace.register` | 暂不承诺 | 接口未稳定且会改变工作区与远程连接归属 | 继续使用 BitFun Workspace/Remote 归属模块，稳定后重评。 |
| 受限策略下拦截任意脚本副作用 | 只能部分控制 | 插件可以直接调用脚本运行时，绕过细粒度能力代理 | 默认兼容策略放开；用户收紧时明确列出被禁用或无法拦截的能力。 |
| 无硬资源限制平台上的系统资源耗尽 | 不能保证完全隔离 | 独立进程可终止死循环，但未必能阻止内存、CPU 或子进程风暴拖慢整机 | 使用进程树回收与平台可执行的 Job Object、cgroup/rlimit；缺少硬限制时显示残余风险。 |

这些限制已经作为当前架构决策：项目状态只能表述为“兼容矩阵已审计、已实现项按证据列示”，不能表述为“稳定
扩展面已完整实现”或“所有插件完整兼容”。只有真实需求和新证据可以重新开启延期项。

## 7. 完成判定

每项只有同时满足以下条件才算完成：

1. 按 OpenCode 来源、作用域和顺序发现输入。
2. 解析或真实执行官方格式，不以静态字符串预览代替运行结果。
3. 参数、返回值、冲突、错误和生命周期通过冻结版本样例。
4. 单插件业务失败不直接传播到其他插件、主界面或无关会话；平台无法提供硬资源限制时，系统资源耗尽按第 6 节明确为残余风险。
5. 用户能看到来源、状态、降级原因、更新结果和恢复动作。

阶段状态必须按切片独立表达：OC-E1 完成只代表 standalone tool 闭环，不暗示 package plugin、Hook、TUI、Server
或 Remote 已完成。矩阵中未立项项保持“未实现/暂不承诺”，不能阻塞已闭环能力，也不能被后者冒充。

阶段交付和退出标准见[粗粒度计划](../../plans/opencode-extension-compatibility-plan.md)。

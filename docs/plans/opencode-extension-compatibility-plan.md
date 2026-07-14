# OpenCode 扩展兼容粗粒度计划

本文只定义交付阶段和退出条件。能力差异见[扩展兼容总览](../architecture/extensions/opencode-extension-compatibility.md)，
配置、服务插件、终端插件、外部集成和通用主机的细节见 `docs/architecture/extensions/`。

## 1. 计划原则

- 默认本地兼容优先，用户、产品或组织可以按需收紧权限。
- 配置解析、插件执行、终端插件和外部产品入口分别验收，不用一个“大兼容层”承载全部行为。
- 从第一个可执行阶段开始使用每插件 target 独立进程、期限、取消、有界队列、大小限制和崩溃回收。
- 依赖准备、加载顺序、Hook 和 TUI 接口固定到 OpenCode 稳定提交；开发分支只做变化告警。
- 先交付配置导入、全局插件加载和可恢复更新体验，再扩展 Remote、严格策略和高成本渲染兼容。
- 不要求作者重打包，不复制完整 OpenCode Agent Runtime，不用相似 BitFun 能力代替兼容测试。

## 2. 当前基线

当前 P0-C.1/P0-C.2 已有 BitFun 原生插件目录、清单、内容校验、启停记录、主机期限/故障状态和少量 OpenCode
custom tool 名称预览。它尚未执行 OpenCode JS/TS、软件包插件、工具、Hook 或 TUI target，也没有完整配置来源、
Client/Server 兼容接口和外部集成兼容。

因此当前只能表述为“来源可识别、静态名称可预览”，不能表述为“OpenCode 插件可运行”。现有恢复能力和测试
继续保留，但 OpenCode 来源不再被要求转换成 BitFun 原生包。

## 3. 阶段总览

| 阶段 | 用户结果 | 核心交付 | 暂不并入 |
|---|---|---|---|
| OC-R0 基线与差异可见 | 能准确判断每项缺口和可实现性 | 冻结版本、差异类型矩阵、官方样例、来源与错误分类、版本变化报告 | 插件代码执行 |
| OC-R1 配置与更新基础 | 本地已有项目可直接读取配置，也可选择导入；插件变化可解释并可回退 | 主/TUI 配置来源、声明式资产、导入预览、全局插件来源、候选版本、上个可用版本、能力变化提示；Remote 明确禁用 | 全部 Hook、TUI 原始组件 |
| OC-R2 本地插件执行 | 常见工具、本地/软件包服务插件和全局插件可真实运行 | npm/Arborist 依赖、固定 Bun、每 target 进程、v1 server loader、standalone tool、最小 Client/`$`、顺序与覆盖；Remote 明确禁用 | 全部稳定 Hook、终端插件 |
| OC-R3 完整稳定服务面 | 稳定配置和服务 Hook 可按 OpenCode 行为工作 | 全部稳定配置、Hook、Zod/JSON Schema 双表示、auth/provider、版本化 Client/回环路由 | 原始 TUI renderer、完整外部 Server |
| OC-R4 终端与外部入口 | TUI 非原始渲染能力和主要外部入口可用 | TUI target 顶层 API、IDE `/tui` 子集、ACP、入口级 SDK/Server、BitFun GitHub/GitLab/Slack 集成 | 原始组件树直连、原始 Web/attach 全协议 |
| OC-R5 Remote、策略与高难度决策 | 远程和组织场景可控，剩余缺口有明确结论 | 远端执行、可调策略、兼容版本升级、高难度渲染/Server/实验接口评估 | 无真实需求的通用界面协议或第二 Agent Runtime |

## 4. OC-R0：基线与差异可见

交付：

- 固定稳定 release commit；比较配置 schema、服务 Hook、TUI API、加载器和依赖服务的 Git blob。
- 每项标记“补基础能力、补扩展接口、融合现有能力、转换参数、直接桥接、明确降级”。
- 兼容报告区分不支持、版本不匹配、依赖失败、插件异常、超时、取消、过载、策略限制、进程失联和无效响应。
- 服务插件、TUI、配置、外部产品入口和实验接口分别维护清单。

退出条件：

1. 每个稳定入口都有可实现性、BitFun 工作项、详细设计或明确限制。
2. 官方源码和规范冲突单独记录，并由冻结样例决定实际行为。
3. 未支持项能局部诊断，不触发 panic、无限重试或日志风暴。
4. 产品状态不把设计目标显示成已实现。

## 5. OC-R1：配置、导入与更新基础

交付：

- 主配置完整来源：well-known、global、`OPENCODE_CONFIG`、project、目录资产、inline、账户组织配置、系统管理员配置和 MDM，以及合并后的环境覆盖。
- TUI 独立来源：global、`OPENCODE_TUI_CONFIG`、project、`.opencode`/`OPENCODE_CONFIG_DIR`。
- Rules、Agents、Skills、References、Commands、MCP、LSP、Formatter、Theme、Keybind 和全部稳定配置字段的解析与归属映射。
- 默认“兼容来源”直接生效；“显式导入”先显示可直接使用、需转换、会降级，再写入 BitFun 配置。已导入字段不重复应用原值。
- 启动时显示完整来源图中的插件来源和准备状态；来源仍启用、旧版本仍合规时，代码或依赖更新失败继续使用上个可用版本。停用、撤销或策略收紧不回退。
- 能力集合变化时显示新增/删除工具、Hook、权限和依赖。bare `latest` 软件包只在显式检查更新或配置策略允许时重新解析，不静默换包。

退出条件：

1. 常用 OpenCode 项目无需迁移即可得到可解释的配置结果。
2. 导入、撤销、原来源再次变化和部分字段继续兼容来源均有确定行为。
3. 有效配置保持 OpenCode 解码结果；BitFun 对非安全独立字段的局部恢复有明确差异标记，安全/执行字段无效时不激活受影响结果。
4. 全局插件变化的影响范围、来源和更新结果对所有受影响项目可见。
5. 配置准备与更新不阻塞主界面或 Agent 主循环。
6. OC-R5 前，Remote workspace 的 OpenCode 配置/插件发现返回明确 `unsupported`，不扫描本机同名来源、不复制本机凭据、不回退本机执行。

## 6. OC-R2：本地插件执行

交付：

- 依赖准备使用稳定版 npm 配置、`@npmcli/arborist`、`package-lock.json` 和 `ignoreScripts: true`。
- 固定版本 Bun 只承担 TS/JS、模块和 `$` 执行；完成三平台许可、签名、更新和体积验证。
- 每个外部插件 target 使用独立可终止进程；服务/TUI target 分离，心跳不与业务调用共用阻塞队列。
- v1 server default export、文件/npm id、`./server`/main/index 回退、`engines.opencode`、internal-first、pure 和旧式函数回退。
- standalone tool 的 default/named exports、Zod 校验、真实 execute、取消、元数据、权限请求和附件结果。
- 完整来源图产生的 `plugin_origins` 顺序；npm 按 package name、file 按精确 URL 去重，后来源胜出，并验证同名工具覆盖。
- 最小 `client`、`serverUrl`、`project/directory/worktree` 和 `$`。
- 首期提供“兼容模式 / 受限模式”、来源或 target 停用，以及 Host 代理能力的策略检查；脚本直接能力没有真实 OS/容器边界时，受限模式停用相应 target 并返回 `policy-limited`。

退出条件：

1. 本地 tool、本地 server plugin、软件包 plugin 和全局 plugin 各有真实调用样例。
2. 作者不需要 BitFun 专用清单或二次激活。
3. 初始化失败、崩溃、死循环、超时和过载在进程树、期限与平台资源预算内被局部回收；没有硬资源限制的平台明确记录系统资源耗尽残余风险，不宣称完全隔离。
4. pure、版本范围、入口缺失、原生依赖失败和旧包替代均有稳定结果。
5. 更新、停用、回退和重启后，旧贡献和迟到响应不能继续生效。
6. Remote 端到端用例证明插件发现、依赖准备和执行均在 R5 前被 gate；不会启动本机 worker、读取本机全局插件或复制凭据。

## 7. OC-R3：完整稳定服务面

交付：

- 覆盖 `dispose`、`event`、`config`、`tool`、`auth`、`provider`、`chat.message`、`chat.params`、
  `chat.headers`、`permission.ask`、`command.execute.before`、`tool.execute.before`、`shell.env`、
  `tool.execute.after` 和 `tool.definition`。
- 变换按插件顺序执行，最后由对应归属模块校验；`tool.definition` 保持模型 JSON Schema 与执行 Zod 的双表示语义。
- Client 和回环路由按真实插件消费增加；未知读接口稳定失败，未知写接口不执行且不伪造成功。
- 默认兼容权限允许 OpenCode 正常行为；用户/组织策略可细分 Host 代理能力，脚本直接能力只能由真实执行环境粗粒度收紧。
- 本地执行域凭据访问接口按领域路由到现有 AI credential resolver、MCP OAuth vault 或插件 auth 流程；不建立通用凭据库，不把值写入普通状态。

退出条件：

1. 每个稳定 Hook 有正常、链式、异常、超时、取消和策略差异样例。
2. Zod refinement、ToolContext、附件、auth/provider/MCP 凭据和大结果通过端到端验证。
3. Hook 失败只影响本次调用或相应贡献，不污染其他插件和业务状态。
4. 未知 API、事件或字段不会导致卡顿、卡死、无限重试或错误风暴。

## 8. OC-R4：终端插件与外部入口

交付：

- TUI default export、入口/id/版本、options/meta、KV 覆盖、反向清理和 5 秒预算。
- 从现有 `chat.rs`、`ui/chat/*` 等真实路径抽取最小 Input/Command/State/Effect 消费接口，不建立通用界面扩展框架。
- 逐项覆盖稳定 `TuiPluginApi`：版本、attention、旧 command、keys/keymap/mode、route、已知 dialog、toast、
  tuiConfig、KV、state、theme、client、event、plugins 和 lifecycle。
- Slot 名称、属性与模式可识别；原始 Route/Slot/Dialog/Prompt JSX 和 `CliRenderer` 明确降级且界面可退出。
- ACP 和真实消费所需的 SDK/Server 方法；IDE 启动/聚焦、上下文、文件引用和 `/tui` 子集。
- BitFun GitHub、GitLab、Slack 入口分别验收；原 OpenCode Action/runner/package 直连单独标记。

退出条件：

1. 不依赖原始组件的 TUI 样例完成导航、命令、输入、通知、主题、状态、共享 KV 和生命周期闭环。
2. 插件异常不能造成空白不可退出页面、输入锁死或终端无法恢复。
3. 每个外部入口用方法、endpoint、事件和认证清单表达范围，不以“已有 SDK/ACP”代替测试。
4. 原始客户端直连与 BitFun 原生替代在界面和文档中明确区分。

## 9. OC-R5：Remote、策略与高难度决策

交付：

- 项目配置、依赖、插件进程、路径、命令和凭据在远程工作区实际执行域运行。
- 在远端实现执行域凭据访问 provider，并通过 R1/R2 的禁用用例证明启用后仍不会回退本机来源、worker 或凭据。
- 在 R2 粗粒度兼容/受限模式上，按平台与 Remote 的真实 OS/容器能力扩展文件、网络、进程、环境、凭据、覆盖和界面策略；限制结果与插件故障分开显示。
- 新 OpenCode 稳定版先做差异分类和旧/新样例，再推进默认兼容版本。
- 仅在真实插件或客户端被阻断时评估原始终端子表面、完整 Server 协议和稳定化实验接口。

高难度能力立项前必须回答：

1. 被阻断的真实插件或外部调用方是什么。
2. 现有结构化映射和兼容门面为什么不足。
3. 是否会引入第二渲染树、第二会话模型或第二工作区归属。
4. 三个平台、Remote、取消、恢复和升级成本是否可控。
5. 不做时的明确降级是否已由用户确认。

## 10. 跨阶段验证和发布

- 每阶段独立发布，发布说明列出精确覆盖、产品增强和降级项。
- 兼容性测试不能通过关闭插件、跳过 Hook 或放宽成功判定获得通过。
- 性能至少记录配置/依赖准备时长、首次调用、Hook 链、单插件进程内存、恢复时长和 TUI 输入延迟。
- 每阶段完成后由独立审查重新对照官方稳定提交；无法覆盖项带原因、替代行为和风险交给用户确认。

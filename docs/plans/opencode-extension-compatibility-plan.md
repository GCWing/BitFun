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
| OC-R1 配置与来源基础 | 本地已有项目可读取非执行配置，也可选择导入；可执行来源只发现和展示 | 主/TUI 配置来源、非执行字段生效、全部字段解析、导入预览、全局插件来源、spec/锁文件/依赖元数据变化；Remote 明确禁用 | 任何外部进程/module/主动联网、真实工具/Hook 差异、旧代次重建 |
| OC-R2 本地执行与插件 | Command/MCP/LSP/Formatter/Reference/Skill 与常见本地/软件包插件可按各自边界真实运行 | 各归属模块启动保护、npm/Arborist、固定 Bun、每 target 进程、v1 loader、standalone tool、真实贡献差异、最小 Client/`$`、顺序与覆盖；Remote 明确禁用 | 全部稳定 Hook、终端插件 |
| OC-R3 完整稳定服务面 | 稳定配置和服务 Hook 可按 OpenCode 行为工作 | 全部稳定配置、Hook、Zod/JSON Schema 双表示、auth/provider、版本化 Client/回环路由 | 原始 TUI renderer、完整外部 Server |
| OC-R4 独立入口里程碑 | TUI、协议/IDE、外部连接器可分别发布和验收 | R4-T 终端插件；R4-P IDE/ACP/SDK/Server；R4-C GitHub/GitLab/Slack | 原始组件树直连、原始 Web/attach 全协议 |
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

## 5. OC-R1：配置、导入与来源基础

交付：

- 主配置完整来源：well-known、global、`OPENCODE_CONFIG`、project、目录资产、inline、账户组织配置、系统管理员配置和 MDM，以及合并后的环境覆盖。
- TUI 独立来源：global、`OPENCODE_TUI_CONFIG`、project、`.opencode`/`OPENCODE_CONFIG_DIR`。
- Rules、Agents、Skills、References、Commands、MCP、LSP、Formatter、Theme、Keybind 和全部稳定配置字段的解析与归属映射。
- 默认兼容来源中，不启动外部进程、不 import 第三方 module、不读取凭据且不主动联网的字段直接生效；远程
  Instruction/Reference、可执行 Skill/Command、MCP/LSP/Formatter、Plugin/Tool 只发现并显示“当前阶段未激活”。“显式导入”
  先显示可直接使用、需转换、会降级，再写入 BitFun 配置；导入不能绕过执行阶段。
- 启动时显示完整来源图中的插件来源和静态状态；只比较来源、spec、lockfile 和依赖元数据，不在禁止执行 factory
  的阶段推测真实工具、Hook、权限或“可用执行版本”。
- bare `latest` 软件包只在显式检查更新或配置策略允许时重新解析，不静默换包；R1 只显示候选 spec/版本元数据，
  不声称候选可运行或可回退。

退出条件：

1. 常用 OpenCode 项目无需迁移即可得到可解释的配置结果。
2. 导入、撤销、用户后续修改、原来源再次变化和部分字段继续兼容来源均有逐字段确定行为。
3. 有效配置保持 OpenCode 解码结果；BitFun 对非安全独立字段的局部恢复有明确差异标记，安全/执行字段无效时不激活受影响结果。
4. 全局插件的来源、target、作用域和静态变化对所有受影响项目可见，且明确标为“尚未执行验证”。
5. 配置准备与更新不阻塞主界面或 Agent 主循环。
6. OC-R5 前，Remote workspace 的 OpenCode 配置/插件发现返回明确 `unsupported`，不扫描本机同名来源、不复制本机凭据、不回退本机执行。
7. 端到端用例证明 R1 打开含 Command、MCP、LSP、Formatter、可执行 Skill、远程 Instruction/Reference 和 Plugin/Tool 的
   项目不会启动进程、import module、读取凭据或主动联网；状态明确显示等待 OC-R2。

## 6. OC-R2：本地执行与插件

交付：

- 依赖准备使用稳定版 npm 配置、`@npmcli/arborist`、`package-lock.json` 和 `ignoreScripts: true`。
- 固定版本 Bun 只承担 TS/JS、模块和 `$` 执行；完成三平台许可、签名、更新和体积验证。
- 每个外部插件 target 使用独立可终止进程；服务/TUI target 分离，心跳不与业务调用共用阻塞队列。
- v1 server default export、文件/npm id、`./server`/main/index 回退、`engines.opencode`、internal-first、pure 和旧式函数回退。
- standalone tool 的 default/named exports、Zod 校验、真实 execute、取消、元数据、权限请求和附件结果。
- 完整来源图产生的 `plugin_origins` 顺序；npm 按 package name、file 按精确 URL 去重，后来源胜出，并验证同名工具覆盖。
- 最小 `client`、`serverUrl`、`project/directory/worktree` 和 `$`。
- Command、MCP、LSP、Formatter、远程 Instruction/Reference 和可执行 Skill 分别通过现有归属模块启动；每类在首次启动前
  解析有效策略/安全启动、执行域、凭据与环境范围，并具备期限、取消、进程树回收和状态诊断。不能借 Bun worker
  的隔离替代这些独立进程的 owner 保护。
- OpenCode Source Coordinator 在 Plugin/Tool import 前解析已保存的全局/来源/target 策略与安全启动参数；默认
  兼容模式不增加二次审批，安全启动可以暂停全部外部 target。脚本直接能力没有真实 OS/容器边界时，受限模式
  停用相应 target 并返回 `policy-limited`。
- 插件 factory 实际运行后才生成工具、Hook、权限和依赖差异；候选激活前提供默认 5 秒非阻塞切换窗口，用户可
  暂停、收紧或停用。更新失败只允许健康旧进程继续服务；旧进程丢失后只有精确物化目录仍可校验时才能重建。

退出条件：

1. 本地 tool、本地 server plugin、软件包 plugin 和全局 plugin 各有真实调用样例。
2. 作者不需要 BitFun 专用清单或二次激活。
3. 初始化失败、崩溃、死循环、超时和过载在进程树、期限与平台资源预算内被局部回收；没有硬资源限制的平台明确记录系统资源耗尽残余风险，不宣称完全隔离。
4. pure、版本范围、入口缺失、原生依赖失败和旧包替代均有稳定结果。
5. 更新、停用和重启后，旧贡献和迟到响应不能继续生效；本地原位源码变化且没有精确旧字节时明确“上一版本
   不可恢复”，不得从当前来源冒充旧代次。
6. Remote 端到端用例证明插件发现、依赖准备和执行均在 R5 前被 gate；不会启动本机 worker、读取本机全局插件或复制凭据。
7. 安全启动和预先保存的来源/target 策略在第三方 module import 前生效；默认兼容模式无需二次批准即可继续，
   状态页明确说明首次直接脚本副作用无法事后撤销。
8. Command、MCP、LSP、Formatter、远程 Instruction/Reference 和可执行 Skill 各有首次启动、凭据/env 范围、超时、取消、
   停用、进程树回收和 Remote 禁用的端到端样例；一种资产失败不阻塞其他无关配置和会话。

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

## 8. OC-R4：三个独立入口里程碑

三个里程碑独立排期、发布和验收；任一连接器未完成不阻塞另一个已闭环入口，也不能用“已有接口清单”把入口
标为可用。

### OC-R4-T：终端插件

- TUI default export、入口/id/版本、options/meta、KV 覆盖、反向清理和 5 秒预算。
- 从现有 `chat.rs`、`ui/chat/*` 等真实路径抽取最小 Input/Command/State/Effect 消费接口，不建立通用界面扩展框架。
- 逐项覆盖稳定 `TuiPluginApi`：版本、attention、旧 command、keys/keymap/mode、route、已知 dialog、toast、
  tuiConfig、KV、state、theme、client、event、plugins 和 lifecycle。
- Slot 名称、属性与模式可识别；原始 Route/Slot/Dialog/Prompt JSX 和 `CliRenderer` 明确降级且界面可退出。

退出条件：不依赖原始组件的样例完成发现、加载、导航、命令、输入、通知、主题、状态、共享 KV、停用和终端恢复
闭环；插件异常不能造成空白不可退出页面、输入锁死或终端无法恢复。

### OC-R4-P：IDE 与协议入口

- ACP 和真实消费所需的 SDK/Server 方法；IDE 启动/聚焦、上下文、文件引用和 `/tui` 子集。

退出条件：每个计划支持的方法都有请求、响应、事件、认证和错误样例；至少一个支持的 IDE 从启动 BitFun 到注入
上下文并恢复断线形成端到端闭环。范围清单只表示冻结范围，不表示入口可用。

### OC-R4-C：外部连接器

- BitFun GitHub、GitLab、Slack 入口分别验收；原 OpenCode Action/runner/package 直连单独标记。

退出条件：GitHub、GitLab、Slack 分别维护完成状态；只有完成安装/授权、触发、结果回传、撤销授权和错误恢复的
连接器才标为可用。原始客户端直连与 BitFun 原生替代在界面和文档中明确区分。

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

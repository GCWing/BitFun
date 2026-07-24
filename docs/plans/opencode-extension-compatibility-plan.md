# OpenCode 扩展兼容执行计划

本文定义 OpenCode 兼容能力的近期交付顺序。完整能力差异保留在
[兼容矩阵](../architecture/extensions/opencode-extension-compatibility.md)，跨生态来源体验与生命周期见
[外部 AI 工作内容设计](../architecture/extensions/external-ai-work-sources-design.md)，通用能力 owner、Generation 与宿主边界见
[能力装配与宿主集成设计](../architecture/extensions/capability-runtime-integration-design.md)。本计划只覆盖外部 OpenCode
能力进入 BitFun 的渐进导入轨道，不代表 BitFun 能力导出到 OpenCode 已经完成。兼容矩阵是审计库存，不是默认路线图。

PR1 已建立通用外部来源目录、生命周期协调器和 OpenCode Prompt Command 纵向切片，PR2 已把受支持的单文件
`.js` standalone Tool 接入现有 Tool Runtime，PR3 已把 Subagent 安全子集交给现有 Subagent owner，PR4 补齐现有
Skill 多来源的身份与覆盖状态展示。BitFun 原有
受管插件包来源确认和 custom tool 静态预览继续保留，但不等同于 OpenCode package plugin 可执行。Command、Tool、Subagent
三个可执行切片均沿用
稳定的跨生态来源契约，不建设“大而全的 OpenCode Plugin Runtime”，也不提前承诺尚未执行的生态能力。

## 1. 稳定架构基线

### 1.1 接口层与实现层

```text
Product surfaces (Desktop / CLI / TUI)
  -> External Source Catalog / lifecycle coordinator
       -> consumes capability-specific provider contracts
            -> Prompt Command provider contract
            -> Tool provider contract + provider-neutral script runtime port
            -> Subagent provider contract

Same-level ecosystem adapters implement those provider contracts
  -> OpenCode adapter
  -> Codex declarative-source adapter
  -> Claude Code declarative-source adapter

Product Assembly registers adapter implementations with the coordinator
```

- 通用层只认识开放的 `ecosystem_id`、来源限定身份、作用域、执行域、状态、诊断和能力专属贡献，不按
  OpenCode/Codex/Claude Code 分支业务行为。
- 每个生态适配器独立维护自己的路径发现、优先级、格式、参数展开和版本兼容语义；兄弟适配器之间不得依赖、复用
  私有类型或借用对方身份。
- Product Assembly 是唯一选择具体适配器的地方。Desktop、交互式 TUI（ChatMode）、来源目录和能力 owner 只依赖稳定契约。
- 不建立携带任意 payload 的 `ExtensionAsset`、通用脚本 SDK 或跨生态配置对象。Command、Tool、Subagent 分别走
  类型化贡献接口；新增一种能力不能迫使既有能力改写公共对象。
- 来源目录按“提供者 + 来源限定身份”协调代次，provider discovery 独立并发且有期限；某个适配器解析失败、升级
  或删除来源时，只影响其自己的来源和贡献，不能阻塞或清空其他生态。目录型来源进一步记录命令粒度的读取失败，
  避免一个坏文件复活同目录中已稳定删除的命令。
- 文件观察是 OS 服务事实，候选代次与来源合并是协调器事实，OpenCode 路径和语义是 OpenCode adapter 事实，
  最终调用仍属于 Command/Tool/Subagent owner。

### 1.2 产品基线

- 用户全局和当前项目来源后台发现，不阻塞项目打开、TUI 输入或普通会话。
- 设置中提供统一“外部 AI 应用”入口，显示来源、作用域、实际生效状态、受限原因和最近刷新结果；默认聚合，不要求
  用户理解文件级实现。
- 当前安全支持的内容可在用户明确调用时直接使用；尚缺运行能力的字段只显示“已识别但当前不可用”，不伪装成功。
- 来源可按当前执行域抑制、恢复和重新加载；观察器更新不得绕过用户的抑制选择。
- 更新先生成不可变候选代次，通过解析和校验后原子切换。解析失败保留仍然合规的上一有效代次；稳定删除、显式停用
  或安全撤销必须撤下新调用，不能借“优雅降级”继续执行已删除内容。
- 同一全局来源的发现摘要按执行域去重；项目来源按工作区展示。普通变化聚合为非阻塞状态，不用 Modal 轰炸用户。

## 2. 渐进 PR 范围

| PR | 用户可观察结果 | 新增能力 owner | 明确不包含 |
|---|---|---|---|
| PR1：来源目录 + OpenCode Command（已实现） | Desktop 可查看、抑制/恢复并刷新全局/项目 OpenCode 来源；交互式 TUI（ChatMode）可列出并执行支持的 `/command`；运行中修改、删除、恢复后自动刷新 | 通用来源目录与生命周期协调器；Prompt Command 契约；OpenCode Command adapter | JS/TS Tool 执行、Hook、MCP、OpenCode Client/Server、Subagent 执行、复制式导入 |
| PR2：OpenCode standalone Tool（已实现） | 一个真实、受支持的单文件 `.opencode/tools/` 样例经预览和确认后进入现有 Tool Runtime，可调用、取消、更新和撤下 | 现有 Tool Runtime + 独立 Tool 兼容接口 | package plugin、npm 依赖安装、Hook、TUI renderer、完整 `metadata`/`ask` |
| PR3：OpenCode Subagent（已实现） | 全局/项目 agent 定义经一次非阻塞确认后进入现有 Subagent owner，可选择、单次调用、更新和撤下；同名冲突由用户选择，unsupported 字段有明确诊断 | 现有 Subagent owner + 独立 Subagent 兼容接口 + generation lease | 原始 OpenCode 会话内核、完整 primary-agent 替换、外部 agent 续接、跨产品通用 agent JSON |
| PR4：Skill 来源与覆盖状态（本轮） | GUI/TUI 对已发现 Skill 显示生态来源；被现有优先级覆盖的同名项继续可见，并标明较高优先级来源；模式配置按实际选择结果标明当前覆盖来源 | 现有 Skill Registry；只补 provider-neutral 来源元数据和宿主展示 | 修改 Skill 优先级、引入审批弹窗、复制导入、URL/脚本执行、把 Skill 并入外部来源协调器 |

Tool 与 Subagent 不复用 Command 的贡献对象，只复用来源身份、状态、代次、诊断和观察生命周期。Codex 与
Claude Code 的声明式安全子集已由同级 adapter 接入并在 Product Assembly 注册；后续能力仍在各自 adapter 演进，
不能修改 OpenCode adapter 来容纳其他生态，也不能把当前子集表述为完整兼容。

## 3. PR1：来源目录与 OpenCode Command 纵向闭环

### 3.1 支持范围

- 发现 OpenCode 当前稳定契约中的用户全局与项目 `command` 配置，以及 `command/`、`commands/` Markdown 目录。
- 用户全局根遵循 OpenCode 的 XDG 语义（默认 `~/.config/opencode`，Windows 不改用 AppData），读取 `config.json`、
  `opencode.json`、`opencode.jsonc`；同时支持 `OPENCODE_CONFIG`、`OPENCODE_CONFIG_DIR` 和
  `OPENCODE_DISABLE_PROJECT_CONFIG`。`OPENCODE_CONFIG_CONTENT` 与远程配置在 PR1 明确不接入。
- 支持 Markdown YAML front matter 中的 `description` 和正文模板，以及 JSON/JSONC `command` 中的
  `template`、`description`。Markdown 已知字段按当前 OpenCode schema 校验，类型错误不得静默丢弃；同时保留
  OpenCode 对未引用冒号值的兼容重试。
- 保留 OpenCode 生态内部的名称和覆盖顺序；独立 provider 之间或与 BitFun 本地能力同名时不得按适配器优先级静默决胜，
  必须生成版本敏感的冲突指纹并等待用户选择。候选版本不变时只询问一次，更新后重新询问。交互式 TUI（ChatMode）继续显示
  竞品一致的 `/command`，通过来源标签和内部 candidate id 区分候选；直接输入存在未解决冲突时 fail closed 并引导使用候选菜单，
  不公开生态前缀或 `/builtin:`、`/external:` 选择语法。
- 支持 `$ARGUMENTS` 与 `$1`、`$2` 等位置参数展开。显式选择或输入 `/command ...` 本身就是本次 prompt-only
  命令的用户确认；发现阶段不自动向会话发送内容。
- `!shell`、`@file`、`{env:...}`、`{file:...}`、`agent`、`model`、`variant`、`subtask` 等尚未接通真实 owner 的语义继续被识别，但命令标记为
  “当前受限”并给出原因，不做部分执行或静默忽略。
- 外部文件始终只读；不要求安装 OpenCode CLI，不复制到 BitFun 配置，也不写回或升级来源。

### 3.2 分层归属

| 位置 | PR1 责任 | 不得承担 |
|---|---|---|
| `contracts/product-domains` | 开放生态 ID、来源限定身份、作用域、状态/诊断、来源快照、Prompt Command 定义和 provider 端口 | OpenCode 路径、文件 IO、UI、具体适配器选择 |
| `adapters/opencode-adapter` | OpenCode 全局/项目来源图、优先级、JSON/JSONC/Markdown 解析、参数展开、受限字段诊断 | 产品提示、用户偏好、文件观察服务、其他生态逻辑 |
| `services/services-core` | 通用 JSON 严格原子写入、跨进程锁和锁内读改写原语；替换失败保留旧文件 | 外部来源偏好 schema、生态语义、冲突策略 |
| `services/services-integrations` | 可订阅、去抖的文件变化事实 | 来源合并、OpenCode 语义、能力注册 |
| `assembly/external-sources` | provider-neutral 的原子代次、隔离降级、同名冲突目录和版本敏感选择 | 注册具体 adapter、按生态分支或解释生态文件 |
| `assembly/core` | 注册 adapter、按工作区协调刷新、定义偏好 schema/路径并通过服务原语持久化、连接 watcher 与产品入口 | 实现文件锁/原子写、复制 OpenCode parser、按生态分支能力行为 |
| `apps/cli` | 将可用外部 Command 以原生 `/command` 投影到 TUI 菜单和输入分发；来源标签及内部 candidate id 处理同名候选 | 解析 OpenCode 文件、注册假工具或公开生态限定命令 |
| `apps/desktop` / `web-ui` | 统一来源摘要、刷新、抑制/恢复和非阻塞反馈 | 持有 adapter、直接读取用户目录、通过 IPC 传输模板正文 |

### 3.3 生命周期与失败语义

1. 初次查询立即返回已知快照；所有 provider 首次返回前保留 `discovery_pending`，产品只显示中性“正在检查”，不能把
   暂时空目录误报成“未识别来源”。后台刷新失败只把对应 provider 标记为降级，不阻塞宿主。
2. 各 provider discovery 由 Core 独立调度，当前期限为 5 秒；超时只回退该 provider。每个 provider 同时最多一个
   in-flight discovery，后续刷新复用该任务，防止不可取消的阻塞扫描耗尽线程池。
3. 文件事件按稳定窗口聚合后重扫完整有效来源图，不把编辑器原子保存误判成永久删除。来源路径按规范化身份去重，
   watcher 的重复注册不能把递归观察降级为非递归。
4. 新候选通过契约校验后切换。配置文件整体不可读时回退对应来源；Markdown 已知命令读写/解析失败时只回退该命令，
   同目录其他稳定删除仍撤下；目录枚举状态未知时保守标记整个目录来源不可用，不能把“未知”当作空目录。
5. 外部命令执行前刷新，并以候选 ID + 命令内容版本校验菜单/冲突选择；投影后发生更新时拒绝旧选择，不得执行新版本。
6. 稳定删除或用户抑制立即使该来源不再参与新命令解析；当前已发送的会话消息不回滚。用户抑制和冲突选择属于本地
   执行域全局偏好，保存在外部来源专属偏好文件中；读写使用跨进程锁、锁内合并和严格同卷原子替换，失败时不得先
   删除旧文件。缓存服务在查询、刷新和执行前重新读取，不能因 Desktop/CLI 分属不同进程而继续沿用旧选择，或用旧
   整份全局配置覆盖新值。
7. 来源重新出现时重新进入发现目录，但保持原有抑制偏好，直到用户恢复。
8. 两个 provider 具有同名命令时，目录生成版本敏感的待选择项；用户选择前不激活任何候选。刷新、更新或移除
   其中一个时只重算该冲突，不撤下无关 provider 的来源限定贡献；曾被选择的候选集合发生变化后，即使只剩一个
   候选也重新进入待确认状态，不能静默切换实现。偏好按执行域/命令族保存单个当前指纹和去重的曾冲突候选身份，
   不按内容版本累计无界历史。
9. Desktop 首屏使用非强制快照并后台刷新；首次发现完成前短轮询并保持“正在检查”，已完成选择的冲突不再停留在
   “需要你的选择”区块。工作区切换、轮询和设置写响应按请求作用域、独立 mutation 栅栏与单调 generation 校验；
   同工作区慢写期间的轮询不能覆盖写结果，旧工作区慢响应也不得覆盖当前页面。Remote 工作区在取得同执行域实现前
   明确显示不支持，不回退读取本机来源。Desktop IPC 只返回设置页所需的来源、冲突和命令摘要，不传输命令模板正文。

### 3.4 PR1 验证门槛

- 契约测试使用两个独立 fake adapter，证明一个适配器失败、更新、删除不会污染另一个。
- OpenCode fixture 固定 XDG 全局/项目、`config.json`、单复数目录、JSON/JSONC、Markdown、路径去重、覆盖、参数展开、
  大小/数量上限和受限字段。
- watcher 覆盖创建、连续写入、原子替换、稳定删除与重新出现；目录切换不阻塞 TUI 输入。
- 交互式 TUI（ChatMode）覆盖列表、跨 provider 候选选择与直接输入、本地同名冲突只询问一次、版本变化后重新询问、受限命令提示
  和刷新后撤下；首次发现期间未限定别名不误路由，候选删除后不静默切换到剩余外部或内建实现。
- Desktop 覆盖空状态、部分失败、刷新、抑制/恢复、敏感路径缩略显示及 IPC 摘要不包含模板正文。
- 通过相关 crate tests、Web focused tests、`type-check:web`、仓库 hygiene 与 core boundary 检查。

## 4. PR2：OpenCode standalone Tool

PR2 在 PR1 的来源目录上新增独立 Tool provider 契约和 provider-neutral 脚本运行时端口。OpenCode adapter 只解释
OpenCode 路径、命名和模块格式；Core 只消费通用 Tool 快照、审批与冲突契约；脚本服务只负责物理 worker。未来
Codex、Claude Code 接入同类能力时新增同级 adapter，不修改 OpenCode adapter，也不让 Tool Runtime 按生态分支。

### 4.1 本阶段可用范围

- 自动发现用户全局、legacy、`OPENCODE_CONFIG_DIR` 和项目层级的 `{tool,tools}/*.js`、`*.ts`。扫描有文件数与
  单文件大小上限，且只读取静态摘要；发现阶段不 import 模块、不解析依赖、不启动进程。
- `.js` 仅支持单文件 loader 子集：默认导出或具名导出的 `tool({...})`/对象定义、基础 schema shim、字符串结果或
  `{ output: string }`。只允许精确的 `@opencode-ai/plugin` `tool` import；其他静态/动态 import、`require`、
  package plugin 和依赖安装明确显示为不支持。
- `.ts` 当前只识别来源、名称和不可用原因，不执行；完整 TypeScript、Bun、Zod refinement、`metadata`、`ask`、
  附件结果、插件 `tool` map 和 Hook 延后。Node.js 不可用时工具保持可见但不激活。
- 用户确认后，每个 target 启动独立 Node.js worker，提供 `load/invoke/cancel/dispose`。合作式取消先传递
  `AbortSignal`；脚本阻塞事件循环时在短宽限期后终止整个 target worker。Tool 的 schema、调用权限、审计和
  模型暴露继续走现有 Tool Runtime，不建立第二套路由。

### 4.2 产品与决策语义

- Desktop 在“外部 AI 应用”中显示来源、文件、工作目录、工具名和直接文件/网络/环境/进程能力，并明确提示当前 worker
  不是 OS 沙箱。交互式 TUI（ChatMode）使用同一快照：`/extensions` 提供共享状态、刷新和 Safe Mode，状态栏只做一次非阻塞提醒，通用 `/tools` 入口以“外部 AI 应用”
  分组提供静态预览以及 `enable`、`disable`、`choose`、`refresh` 操作；等待处理不阻塞输入或普通会话，不再注册平行的
  `external-*` 命令。
- 首次启用键由“来源限定 target + 执行域 + runtime + 能力集合”组成。纯内容更新且能力集合不变时复用已批准
  结果；能力、runtime 或执行域扩大时重新确认。用户选择保持停用后，同一内容版本不再主动询问；来源内容更新后
  才形成新 decision key。Desktop 仍允许用户主动重新审核，避免“一次拒绝后永久不可恢复”。
- 外部 Tool 与 BitFun 内置、MCP 或其他外部 Tool 同名时，不按 adapter 或注册顺序静默覆盖。冲突键包含全部候选
  身份与内容版本；候选来自静态识别定义而不是成功加载集合。选择前保留已有本地实现，选择后只在候选集合和版本
  不变时复用；任一候选更新、删除或暂不可用后重新询问，已选 external 失效期间不回退同名内置/MCP。
- Desktop IPC 和 TUI 快照只包含静态摘要与决策 key，不传输模块源码。用户选择落盘使用 PR1 的跨进程锁、锁内
  合并和原子替换；Desktop 与交互式 TUI（ChatMode）不分别维护偏好。

### 4.3 更新、删除与降级

1. watcher 对全局与项目 Tool 目录去抖后重扫；准备前再次发现并核对 target 和内容版本，预览后被替换的文件按
   stale revision 拒绝，不能执行未确认的新内容。
2. 已批准 target 在来源身份、runtime 和能力集合不变时后台重载；导出、schema 或 load 校验失败时撤下该 target，
   显示 `load_failed`，不保留对已变化原位源码的旧 worker。PR2 尚无精确物化旧版本，因此选择安全的 fail-closed，
   不伪装成“沿用上一版本”。
3. 稳定删除、来源抑制、主动停用或审批撤销会撤下 Tool Runtime 路由并 dispose worker；在途调用先收到取消，阻塞
   worker 在宽限期后终止。一个 target 的失败不得清空同来源 Command、其他 target 或其他生态 adapter。
4. 每 target 持久 worker 的普通请求 30 秒无响应时终止；合作取消在 500 ms 后硬终止，输出限制 1 MiB、协议帧
   限制 8 MiB；产品配置的更短 Tool 期限丢弃调用 future 时也会终止 worker，并在终止完成前保持串行许可。调用不
   自动重放。Node 进程仍以当前用户权限运行；VM realm 和隐藏响应令牌不是安全
   沙箱。当前脚本 worker 与 local stdio MCP 已共享 Unix process group / Windows kill-on-close Job Object 进程树回收；
   它不限制文件、网络、CPU、内存或逃逸行为，产品必须持续显示残余风险。
   worker 丢失由带独立加载代次的 runtime health 事件立即撤下路由并显示失败；旧 worker 的迟到事件不能撤下同内容
   新实例。下一次 catalog 暴露前仅恢复一次，失败后等待显式刷新或来源变化；不回退
   同名内置/MCP 实现，也不形成自动重启循环。单个来源目录不可读只降级该目录，其他健康目录继续生效。
5. Remote 工作区在有远端发现、偏好和 worker owner 前明确不支持，catalog、批处理策略和执行解析均 fail-closed，
   即使远端与本机路径文本相同也不能回退加载本机全局或项目 Tool。

### 4.4 PR2 验证门槛

- 契约测试证明静态快照不携带模块源码，审批只在能力/执行域变化时失效，冲突在任一候选内容变化时失效。
- OpenCode fixture 覆盖默认全局目录追加 `OPENCODE_CONFIG_DIR`、项目与单复数目录、默认/具名导出、单文件 JS
  子集、schema 默认值/类型化 min/max、`import.meta.url`、TS/依赖/动态 import 降级、文件上限和“发现不执行代码”。
- 脚本运行时覆盖 load/invoke、内容更新、失败更新撤下、合作式取消、阻塞事件循环硬终止和 dispose。
- Tool 路由覆盖内置/MCP/多外部候选冲突、按工作区选择、候选更新重问、稳定删除、源级隔离、零 route mux 并发注册
  和 worker-lost 撤路由/单次恢复；首次后台刷新与 catalog 竞态覆盖等待、成功复用和失败重试。
- Desktop 覆盖非阻塞发现、首次审批、主动重新审核、停用、冲突选择和不可用原因；交互式 TUI（ChatMode）覆盖提示去重、编号到
  稳定 key 的映射、过期选择拒绝和刷新。通过相关 Rust tests、CLI check/tests、Web focused tests、
  `type-check:web`、i18n audit、repo hygiene 与 desktop/core checks。

### 4.5 后续收敛项

以下改进不回补到 PR2；只有真实故障、指标或下一能力切片证明需要时，才作为独立小 PR 进入，不能借此提前建设
完整插件主机或通用信任中心：

- 将 route、并发属性和 timeout owner 冻结为单个 invocation plan，消除并发切换期间的二次读取窗口。
- 为来源目录瞬时不可读增加类型化 `unknown/last-good` 状态；只有精确物化内容仍可校验时才继续服务，明确删除、
  停用和权限收紧仍立即撤下。
- 为保留的零 route mux 增加数量/命中指标；只有证明长期积累后再设计不重新引入注册竞态的安全回收。
- 统一 Desktop 与交互式 TUI（ChatMode）的 `load_failed` 恢复文案，并让手动刷新产生的新 diagnostics 保持非阻塞可见；不因此
  引入跨 GUI/TUI 组件协议。

## 5. PR3：OpenCode Subagent

PR3 已为现有 Subagent owner 增加独立兼容端口，由 OpenCode adapter 映射 agent Markdown/JSON 定义。它不复用 Tool
运行时，也不把 OpenCode agent 类型提升为跨生态 DTO。

- 支持用户全局、显式配置目录和项目 JSON/JSONC `agent`，以及单复数 agent Markdown 目录；按 OpenCode 稳定
  顺序深合并并保留有序 provenance。legacy mode 与 primary-only 定义可诊断但不激活。
- 安全子集为 description、prompt、subagent/all、disable、hidden、可精确解析的 model 和 tool 选择。permission、
  variant/options、采样参数、steps/maxSteps 等不能等价执行的字段 fail closed，不做“忽略后继续运行”。
- OpenCode adapter 把 model 解析为类型化的 provider 提示和模型名；Subagent owner 在审批前将其或固定默认项物化为
  唯一、已启用的 BitFun 模型。继承、歧义、缺失或已停用模型保持不可用，不使用动态回退替代明确审批。
- 首次启用绑定当前 behavior、provenance、具体模型和工具范围；只有影响展示的 catalog 文案更新不重复询问，
  行为或能力变化重新确认。与 builtin/user/project 或其他 provider 同名时保持不可用，直到用户选择；候选变化后
  旧选择失效，即使只剩一个候选也不静默回退。
- BitFun 模型配置变化会非阻塞重建后续调用的 generation；审批绑定具体配置 ID 和运行配置指纹，同一 ID 下的 provider、
  模型名或 endpoint 变化也要求重新确认。已物化 ID 不再解释为 `inherit/primary/fast/auto/default` 选择器；旧 lease 保留
  旧绑定事实，若执行时配置已漂移则安全失败，不静默切换模型。
- fresh Task 在进入调度前固定不可变 runtime generation，并以 lease 保持到完成、取消、超时或提交失败；来源稳定
  删除、显式停用或抑制会立即阻止新调用，短暂读取失败最多在有界窗口内沿用 exact last-valid。
- Desktop 的“外部 AI 应用”设置和 TUI `/agents` 中的“外部 AI 应用”选项使用同一 generation/revision 校验的
  非阻塞审批与冲突操作；同一工作区的 Agent 决策串行提交，完成后刷新权威状态并说明具体对象。冲突候选在选择前原位展示模型、
  工具、执行域、安全来源标签、兼容影响和恢复动作，一次点击才原子完成“选择并批准”。Agents 场景只显示已激活的
  只读 `External · provider`、`Single-run` 投影，并可跳转到统一管理入口；外部 prompt、绝对用户路径和内部行为摘要
  不进入 IPC 或普通列表摘要。
- GUI/TUI 只按通用资源类型路由诊断，不识别 `opencode.*` 前缀；产品快照统一生成安全来源位置，界面不解析 `.opencode`
  等生态私有目录。已启用 Agent 因更新、删除或冲突变为不可用时，Desktop 与 TUI 都给出去重的非阻塞提示。
- 冲突谱系覆盖 0/1/N 个参与者；参与集合缩减后继续保持逻辑名不可用，等待用户重新选择。自动观察到的新冲突指纹
  与用户决策使用同一跨进程锁并推进 `preference_revision`，因此其他进程基于旧 revision 的审批或选择必然被拒绝。
- 当前 Remote 工作区明确不支持外部来源发现与决策，不读取本机同名配置；静态 system prompt 未修改，只有审批后
  的通用 AgentInfo 动态投影进入现有可用 agent 上下文，且该投影使用 BitFun 自有的稳定描述，不注入来源 catalog 文案。

选择与执行仍由现有会话/Subagent owner 决定；adapter 不能替换 BitFun Agent Kernel。外部 agent 当前只支持 fresh
单次调用，前台结果不返回续接入口，历史 external runtime session 的 follow-up 会被类型化拒绝。

### 5.1 已实现路径的稳定性收敛

本轮不新增扩展类型，只修复会误导用户或掩盖真实故障的两处问题，并校正文档事实：

- `NodeScriptToolRuntime` 在共享外部 Tool 运行时首次初始化时解析 Node.js 可执行文件，并在当前进程内持有成功结果；
  只有缓存仍为空时，后续可用性检查或加载才重新执行当前进程可见范围内的 `which(node)`；可用性还会在 2 秒期限内
  实际执行 `node --version`，并从探测和 worker 环境移除 `NODE_OPTIONS/NODE_PATH`。该过程不会替换已持有路径、worker
  或活动调用。首次未找到 Node.js 后，Desktop 与 TUI 只提示安装或修复、刷新，以及可继续使用其他功能；当前宿主未记录
  “安装后已经刷新但仍不可见”的独立证据，因此不主动建议重启，也不提供自动重启或“立即重启”动作，更不能中断活动 session。
  刷新会重新发现来源并触发运行环境可用性检查，但不能承诺继承父进程后续收到的环境变量变化；`node --version`
  探测通过前，运行时状态不得写成“可用”。
- Subagent owner 读取 BitFun 模型配置失败时必须 fail closed，并生成与“模型不存在”不同的通用诊断。GUI/TUI
  提示用户先确认 BitFun 模型设置能够正常读取和保存，再刷新；日志只保留经过脱敏的失败阶段和错误类别。不得用 `AIConfig::default()`
  把配置服务异常转换成候选模型不匹配，也不得把原始错误或绝对配置路径投影到普通快照。临时故障期间不得改写已持久化的
  审批或同名冲突选择，配置恢复且扩展行为未变化时继续复用原决定。
- 外部 Tool 的首次确认继续明确展示代码来源、工作目录、文件/网络/进程/环境访问，以及“当前用户权限、无 OS
  沙箱、子进程可能继续运行”的残余风险。该提示是知情确认，不等于沙箱实现。
- Desktop 与交互式 TUI 使用相同诊断 code 和激活阻断事实，各自负责适合宿主的文案；Remote 继续返回明确不支持，
  不读取或执行本机同名来源。上述变化不修改 system prompt，也不新增跨界面渲染契约。

明确延期到独立、由证据驱动的后续工作：OS/容器沙箱和 CPU/内存等硬资源限制、worker/prompt 全局预算、运行中 Tool
代次租约、通用 watcher 事件限流、偏好记录压缩、完整 metrics/打点平台。它们分别涉及平台执行、安全控制面、
通用服务或数据保留策略，不能以“稳定性修复”为名并入本轮。

## 6. PR4：Skill 来源与覆盖状态

PR4 只解释现有 Skill Registry 已经执行的选择结果，不改变选择本身。Skill 是历史上已经无感发现并按固定根顺序
解析的声明式内容，与 Command、Tool、Subagent 等可执行扩展的首次接入和冲突确认不同；本轮不把两种策略强行合并。

### 6.1 优先级回归契约

- 项目根保持 `.bitfun`、`.claude`、`.codex`、`.cursor`、`.opencode`、`.agents` 的现有顺序；项目根整体先于
  用户根。用户根、BitFun 用户目录、BitFun 内置目录、OpenCode config/home 延迟根继续沿用当前实现顺序。
- “BitFun 优先”只适用于当前已经如此定义的项目级 `.bitfun/skills`，不得误写成所有 BitFun 用户或内置 Skill
  都高于其他生态。PR4 用精确顺序测试冻结这一事实，来源元数据不参与排序或决胜。
- `source_slot` 继续标识具体发现槽位；新增开放的 `source_id` 和稳定产品名只负责把多个槽位归为 BitFun、
  Claude Code、Codex、Cursor、OpenCode 或 Agent Skills 来源。能力 owner 和界面不得根据 `source_id` 另算优先级。
- 本地与远程项目继续消费同一项目根契约。远程工作区只扫描真实远程项目根，同时保留当前本机用户级 Skill 行为；
  PR4 不借来源展示改变远程执行域或回退规则。

### 6.2 精简的 GUI/TUI 体验

- Skills 场景和设置列表沿用现有卡片/列表，在作用域旁显示来源。普通项不增加确认步骤；按默认优先关系被覆盖的项
  保留在原位置，使用弱化名称、删除线和“已覆盖”状态，并标明较高优先级来源，不把无模式页面误写成某一模式
  “当前一定不会使用”。内部 stable key 不作为主要解释文案。
- 交互式 TUI 的可用列表显示来源；配置列表继续使用既有勾选框，并把 `shadowed` 改为
  `covered by <source>`。模式配置中的覆盖关系由 Skill owner 在应用模式禁用规则后输出；高优先级项在该模式被禁用时，
  实际采用的低优先级项不得仍显示为被前者覆盖。未保存的勾选变化只标为待保存，不预判新的运行时赢家。
- GUI 与 TUI 都消费 Registry 返回的来源和覆盖事实，不解析路径猜测生态，不新增 system prompt 文本，也不建立
  跨宿主渲染协议。来源展示元数据缺失时只用已知 `source_id`/`source_slot` 映射产品名，最终显示本地化的“其他来源”，
  不泄露内部槽位，也不能因为展示元数据异常隐藏可用 Skill。远程工作区同时标明“此设备 · 用户级”或
  “远程工作区 · 项目级”，不改变现有扫描与执行域。
- 当前 Skill 刷新、删除和模式开关生命周期保持不变；本轮不增加 watcher、通知、持久化选择或重启提示。

### 6.3 后续可执行扩展的统一原则

Command、Tool、Subagent、MCP 以及未来可执行扩展默认把 BitFun 原生/内置实现作为安全候选，但不允许外部候选
通过注册顺序静默覆盖。出现同名参与者时由用户选择；选择绑定参与者集合与行为版本，集合或行为更新后才重新询问。
冲突选择列表固定先展示 BitFun 候选，其余生态按稳定 `provider_id` 排序，同一生态内部保留该 adapter 的正式来源顺序；
展示顺序只帮助用户理解，不代替选择，也不把 Skill 的固定根优先级复制到可执行扩展。
被 BitFun/其他候选覆盖、被用户拒绝或尚未选择的外部项必须继续出现在统一管理入口，并以“已覆盖”“未启用”或
“等待选择”及原因展示，不能从列表消失。该原则由各能力 owner 的独立冲突契约实现，不复用 Skill 的固定优先级，
也不在 PR4 修改 PR1—PR3 已有执行路径。

## 7. PR5：同名扩展的选择可见性与统一入口

PR5 不新增扩展类型或运行能力，只补齐 Command、Tool、Subagent 已有冲突决策的管理闭环，并纠正 TUI 信息架构：

- 三类 owner 继续维护各自 DTO、冲突指纹和选择持久化，主体逻辑不按 OpenCode 等生态 ID 分支。候选列表由 owner 输出；
  存在 BitFun 原生/本地候选时固定在首位，其余外部候选使用 adapter 已确定的稳定顺序。Skill Registry 的发现顺序和覆盖
  关系完全不变，不能复用这里的交互式选择规则。
- Desktop 同时显示待选择和已处理冲突，待选择项排在前面。每个候选明确标识“当前使用”“已选择但当前不可用”“未使用”或“可选”；已选择、
  被覆盖以及 Subagent 的“保持不可用”状态都不会因决策完成而消失，用户可直接改选。候选或行为版本变化使冲突指纹变化后，
  旧选择失效并重新要求选择；没有变化时不重复打断。
- 交互式 TUI 不提供 `/external-tools`、`/external-agents` 等按能力分裂的命令。共享控制使用 `/extensions`，工具使用通用 `/tools` 入口，并在内容中以
  “外部 AI 应用”分组；Agent 相关能力统一进入 `/agents`，同一列表以文字区分主 Agent、Subagent 和外部 AI 应用，
  删除历史 `/subagents` 命令。待选择与当前选择分区显示，只有待选择项进入
  状态栏提醒计数；编号操作绑定产生当前视图的快照和稳定 key，改选仍由 owner 做 generation/revision 校验。
  活动 turn 期间 `/agents` 仍可查看和管理 Subagent/外部来源，只禁用主 Agent 切换，并在对应行和状态栏解释原因。
- GUI/TUI 都保持原生异常语义：读取失败不假定变更成功，刷新失败保留已知可用内容并提示状态可能过期，存储异常使受影响
  名称 fail closed，Remote 明确不支持且不回退本机。模型设置读取失败只要求修复设置并刷新，不把重启当作通用恢复动作。
- 本轮不新增重启编排。Node.js 是当前唯一可能受宿主进程环境影响的扩展依赖，但现有宿主没有可靠的刷新前后证据，
  因此只建议安装或修复后刷新，并说明用户可以继续使用其他能力；扩展入口不得建议或触发重启，也不得打断执行中的 session。
  未来若产品增加统一重启动作，必须由 session owner
  提供活动任务保护与恢复契约，默认允许“稍后”，不能由扩展模块自行判断。
- SSH Remote 工作区继续显示明确不支持；不加载本机配置代替远程配置。Peer 控制界面代理 Peer Host 的真实状态和操作，
  不把控制端状态混入 Host。provider discovery deadline 仍为 5 秒；仅发现进行中使用请求完成后再调度的 750 ms、1.5 s、3 s、5 s 退避轮询；
  本轮不增加 watcher、全局扫描、遥测平台或磁盘缓存，不改变 system prompt。

PR5 的退出条件是：三类已处理冲突在 GUI 中可见可改选；Tool/Subagent 在 TUI 中使用通用能力入口；原生候选顺序、
待处理提醒去重、选择指纹失效、Remote 无回退和非强迫恢复文案均有聚焦测试。该切片不以统一入口为由构造通用 `ExtensionAsset`
或第二套选择状态。

TUI 命令命名属于产品契约。新增命令前必须先核对 BitFun 既有入口以及至少一个同类竞品的用户可见命令；能力已有对应
入口时优先在原列表中通过分组、状态或二级选项表达来源差异。只有对象、生命周期或权限边界确实不同，并且复用入口会产生
歧义时，才允许新增命令，并须在设计与测试中写明理由。adapter 名、provider ID 或“external”来源标签都不能单独构成新命令依据。

## 8. 暂停条件

出现以下情况时停止扩面并先修复架构：

- 新生态 adapter 依赖现有生态 adapter，或 core/UI 开始按生态 ID 分支业务行为；
- 为未来可能需求新增任意 payload 资产、通用脚本 SDK、第二套 Tool Runtime/Agent Runtime；
- 只有静态解析却把 Tool、Hook、Subagent 或受限 Command 标为可用；
- watcher 更新能绕过用户抑制，或一个 provider 的失败清空其他 provider；
- Command、Tool、Subagent 等可执行扩展的同名候选仍由固定优先级静默选中，或候选内容版本变化后继续沿用旧冲突选择；
- 为完整兼容一次性引入 package manager、Hook、renderer、Server 和权限系统；
- 本地可用被直接推导为 Remote/HarmonyOS PC 可用，缺少同一 fixture 的真实运行证据。

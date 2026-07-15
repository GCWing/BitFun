# OpenCode 配置与声明式资产适配设计

本文定义 OpenCode 配置、规则、Agent、Skill、Command、MCP、LSP、Formatter、Theme、Keybind 和模型配置
进入 BitFun 的兼容路径。可执行工具与服务插件见
[`opencode-plugin-runtime-adapter-design.md`](opencode-plugin-runtime-adapter-design.md)，终端界面插件见
[`opencode-tui-plugin-adapter-design.md`](opencode-tui-plugin-adapter-design.md)，完整状态以
[`opencode-extension-compatibility.md`](opencode-extension-compatibility.md) 的能力矩阵为准。

配置字段与来源以 [OpenCode 配置文档](https://opencode.ai/docs/config/)和稳定提交中的主/TUI 配置实现为准。

本文是目标设计。当前 BitFun 已有部分原生归属模块和兼容目录发现，但尚未实现本文定义的完整 OpenCode
配置来源图、合并语义和资产映射。

## 1. 目标与边界

目标：

1. OpenCode 用户可以直接打开已有项目，常用配置和声明式资产无需手工转换即可生效。
2. 保留 OpenCode 的来源作用域、合并顺序、冲突和相对路径语义，并能解释最终值来自哪里。
3. 尽量复用 BitFun 已有配置、Agent、Skill、MCP、LSP、主题等归属模块，不复制第二套产品内核。
4. 未知字段和未支持资产局部降级，不导致整个配置、项目启动或界面卡死。
5. 本地默认兼容优先；用户或组织可以收紧权限，策略差异必须与解析或插件错误分开显示。

非目标：

- 不把 OpenCode 配置对象提升为 BitFun 内部通用配置模型。
- 不要求用户先执行一次性导入才能使用已有 `.opencode` 项目。
- 不对 OpenCode 配置进行双向写回或自动改写原文件。
- 不把运行时主题、快捷键、命令或终端界面插件错归为构建期产品与终端界面布局。
- 不在配置解析器中执行插件代码；可执行内容交给独立插件执行进程。

## 2. 两种消费方式

同一 OpenCode 来源可以有两种明确消费方式：

| 方式 | 写入位置 | 是否立即生效 / 是否执行代码 | 停用或撤销 | 来源变化后 |
|---|---|---|---|---|
| 兼容来源 | 不写 BitFun 配置，也不写回源文件 | OC-R1 只让非执行字段直接生效；Plugin/Tool、远程 Instruction/Reference、可执行 Skill/Command、MCP/LSP/Formatter 等在 OC-R2 完成各自启动保护前只发现和展示 | 按项目、来源和 server/tui target 停用；安全启动可在首次进程/module 启动前暂停外部 target | 重新解析并准备候选；能力/权限变化在激活前可见 |
| 显式导入 | 用户选择的 BitFun 用户层、项目层或更窄工作区层 | 写入成功后由目标层正常生效；首期只导入非执行配置，Plugin/Tool 不经导入执行 | 按字段撤销；冲突字段先预览，不自动覆盖后续修改 | 只提示重新导入，不双向写回，也不自动覆盖 BitFun 值 |

“只读”只表示源文件不被 BitFun 改写，不表示结果仅供预览。兼容来源不是 BitFun 内部权威模型，但它是合法
运行输入；达到对应阶段后，适配器把有效值映射到归属模块，归属模块仍负责最终持久化、运行时状态和错误语义。
R1 的“直接生效”仅包含不启动外部进程、不 import 第三方 module、不读取凭据且不主动联网的字段；其他合法配置
必须显示“已发现，当前阶段未激活”，不能因解析成功提前执行。

导入成功后，已选字段以 BitFun 原生配置为准，不再同时应用原 OpenCode 值；原来源继续被观察，变化时提示“来源
已变化，是否重新导入”。未选择字段仍按兼容来源生效。导入记录按字段保存目标层、导入前值及其版本/摘要、导入
值和来源摘要。撤销只自动恢复“当前值仍等于导入值”的字段；若用户后来修改 BitFun 值、来源也发生变化或只重新
导入部分字段，则进入冲突预览，并逐字段选择“保留 BitFun 值 / 重新导入外部值 / 手工处理”，不得整批覆盖。

## 3. 配置层级与来源

### 3.1 OpenCode 来源图

稳定版配置按“后加载覆盖前加载、非冲突字段合并”处理：

```text
远程 `.well-known/opencode` 的内联配置和远程配置
  < 用户全局配置
  < OPENCODE_CONFIG 指定配置
  < 项目配置
  < ConfigPaths.directories（全局配置目录、项目 .opencode、~/.opencode、OPENCODE_CONFIG_DIR）中的配置与目录资产
  < OPENCODE_CONFIG_CONTENT 内联配置
  < 当前账户组织 /api/config
  < 系统管理员配置
  < macOS MDM 配置
```

适配器必须记录每个值的来源、作用域、文件或远程标识、覆盖关系和策略限制。数组、对象和插件列表使用
OpenCode 当前版本的真实合并/去重语义，不用 BitFun 常规配置合并规则猜测。

来源发现包括：

- 远程 `.well-known/opencode` 中的 `config`，以及 `remote_config` 指向的 URL/Headers。
- `~/.config/opencode/opencode.json` 或 `opencode.jsonc`。
- `OPENCODE_CONFIG` 指定的配置文件。
- 从工作树根到当前目录按 root-first 顺序发现项目 `opencode.json/jsonc`。
- `.opencode`、`~/.opencode`、全局配置目录和 `OPENCODE_CONFIG_DIR` 中的 `agents/`、`commands/`、`modes/`、
  `plugins/`、`skills/`、`tools/`、`themes/`；兼容旧 singular 目录名。
- `OPENCODE_CONFIG_CONTENT` 内联配置。
- 当前账户所选组织的 `/api/config`、各平台系统管理员目录与 macOS MDM 设置。

所有来源合并后再应用 `OPENCODE_PERMISSION`、旧 `tools` 到 permission 的迁移，以及关闭自动压缩/裁剪的环境覆盖。这些属于冻结版本的后处理，不是新的配置来源。

### 3.2 TUI 独立来源顺序

`tui.json/jsonc` 不是主配置来源图的附属字段。稳定版使用独立顺序：

```text
用户全局 TUI 配置
  < OPENCODE_TUI_CONFIG
  < 项目 TUI 配置（root-first）
  < .opencode / OPENCODE_CONFIG_DIR 中的 TUI 配置
```

TUI 适配器必须单独记录顺序、插件来源和解析错误；不能把主配置的内联、组织或系统管理员来源套用到 TUI 配置。

### 3.3 BitFun 策略关系

OpenCode 来源顺序决定兼容输入如何合并；BitFun 产品能力上限和组织策略决定合并结果能否执行。二者不能混写：

- 默认本地兼容策略不额外收紧 OpenCode 行为。
- 用户或组织策略可以禁止特定来源、网络范围、凭据、进程或覆盖能力。
- 策略拒绝不回退到更早、更宽松的 OpenCode 值，而是保留最终来源并标记 `policy-limited`。
- 产品定义不参与 OpenCode 文件优先级，只定义产品明确保护的少量能力。

### 3.4 变化与切换

每次解析生成版本化兼容结果，包含来源图、有效值、未知字段、诊断和内容摘要。文件变化时：

1. 后台重新解析，不阻塞 TUI 或 Agent 主循环。
2. 新结果完整校验后原子替换；失败时保留上一份有效结果并显示过期原因。
3. 与插件执行相关的入口或依赖变化只使对应执行版本失效，不清空无关配置和会话。
4. 安全相关配置解析失败时不自动放宽已生效策略。

### 3.5 开发视图

| 开发部分 | 负责 | 不能承担 |
|---|---|---|
| OpenCode 来源发现器 | 在本地或 Remote 执行域寻找主配置、独立 TUI 配置、目录资产、环境指定来源和组织默认 | 合并配置、执行插件、保存最终产品状态 |
| OpenCode 配置解析器 | JSON/JSONC、变量引用、字段版本、来源位置和未知字段保留 | 使用 BitFun 默认值猜测 OpenCode 语义 |
| 来源合并器 | 按冻结 OpenCode 版本合并并记录每个最终值的来源和覆盖关系 | 应用 BitFun 产品或组织策略 |
| 资产适配器 | 把 Rule、Agent、Skill、Command、MCP、LSP、Formatter、Theme、Keybind、Reference 和模型配置分别交给已存在或阶段内补齐的真实消费接口 | 因“看起来已有”而跳过基础能力或边界收敛，或创建第二套 Agent、MCP、LSP、Formatter 或主题运行时 |
| 策略检查 | 在 OpenCode 合并结果上应用用户、产品和组织上限，生成可解释差异 | 改写原始 OpenCode 文件或伪装成解析错误 |
| 状态与诊断服务 | 原子发布新结果、保留上一有效结果、聚合错误和展示来源 | 在界面线程同步解析远程来源或安装依赖 |

解析、合并、资产映射和策略检查必须能分别测试。不要建立一个同时扫描目录、修改环境、执行命令、加载插件
并写入 BitFun 配置的“大导入器”。插件和 tool 入口在本流程中只形成有序来源清单，真实代码加载交给插件执行
服务。

## 4. 解析与鲁棒性

冻结版 OpenCode 使用完整配置 schema 解码来源；字段类型错误并没有“只忽略单字段”的稳定契约。等价解码基线是：

- 同时支持 JSON 和 JSONC，不要求 `$schema` 字段存在或等于固定字符串。
- 支持 OpenCode 文档化的环境变量和文件变量替换；解析报告只显示引用，不泄漏替换后的凭据值。

以下是 BitFun 的局部恢复增强，不标为 OpenCode 完整等价：

- 原始未知字段保存在来源记录中，并按“来源 + 字段路径 + 版本”聚合诊断，供版本升级后重新解释。
- 非安全、非执行控制的独立顶层字段发生类型错误时，可以只停用对应映射并继续使用已验证字段；状态页必须显示
  “BitFun 局部恢复”，不能标为 OpenCode 等价解析。
- permission、来源启停、插件/工具执行、凭据或组织上限等安全/执行字段无效时，不激活受影响的整项执行结果；
  重载场景保留上一份仍符合当前策略的有效结果，首次加载则明确不可用，不能用宽松默认值替代。

两类行为共同遵守以下可靠性要求：

- 未知枚举值不映射为默认值，避免产生看似成功但行为不同的配置。
- 远程配置超时、无效或不可达时保留本地来源，明确显示组织默认未加载。
- 大文件、递归引用和远程 URL 仍有解析期限与大小上限；超限返回稳定错误，不无限等待。
- 插件列表、命令、Agent 等数组的去重和覆盖按冻结版本样例验证，不自行排序。
- 每次重载只产生一条摘要通知；详细错误进入诊断视图，避免日志和 Toast 风暴。

## 5. 声明式资产映射

下表的“默认行为”是对应交付阶段完成后的目标行为。OC-R1 只激活不启动进程、不 import module、不读取凭据且不
主动联网的结果；远程 Instruction、Skill/Reference/Command/MCP/LSP/Formatter/Plugin/Tool 和其他可执行项到
OC-R2 前只解析、展示来源与诊断。

| 资产 | OpenCode 输入 | BitFun 归属模块 / 适配方式 | 默认行为 | 降级条件 |
|---|---|---|---|---|
| Rules / Instructions | 项目/全局 `AGENTS.md`、Claude fallback、`instructions` glob、本地文件、远程 URL | Workspace Instructions 归属模块保存有序来源引用 | 合并有效规则并保留来源 | 远程或单文件失败只排除该来源。 |
| Agents / Modes | JSON、Markdown、description、mode、prompt、model、variant、temperature、top_p、steps、deprecated `maxSteps`、deprecated `tools`、permission、disable、options、hidden、color | Agent 归属模块创建兼容定义和作用域视图 | 项目/用户定义按 OpenCode 顺序可用 | BitFun 不支持字段进入诊断，不阻止其他 Agent。 |
| Skills | `.opencode/.claude/.agents` 项目与用户根、`SKILL.md`、`skills.paths/urls` | Skill 归属模块复用按需加载并补齐规则顺序 | 默认允许 OpenCode 合法 Skill 资源 | URL 或可执行资源失败只降级对应 Skill。 |
| References | `references` / 旧 `reference`，本地 path 或 Git repository/branch/description/hidden | **基础能力缺失**：先补 Workspace Reference 的异步准备与 `@alias` 消费接口 | 保留相对来源、缓存和隐藏语义 | 拉取失败不阻止项目，外部目录仍遵守工具权限。 |
| Commands | JSON/Markdown、`$ARGUMENTS`、位置参数、`@file`、`!shell`、agent/model/variant/subtask | **已有行为、边界未抽取**：从现有 CLI/TUI 命令路径增量提取模板与执行接口 | 保留 OpenCode 展开顺序和子任务语义 | 文件或 shell 展开失败只终止本次命令。 |
| MCP | local 的 command/environment/cwd/timeout，remote 的 URL/headers/oauth/timeout，Agent 选择 | MCP 归属模块创建兼容配置视图 | 默认按来源启用状态运行 | 凭据或网络不可用时单个 Server 不可用。 |
| LSP | command、extensions、env、initialization | LSP 归属模块注册兼容实例 | 按文件类型和作用域启动 | 自定义 Server 缺少 extensions 或启动失败时只禁用该项。 |
| Formatters | command、environment、extensions、`$FILE` | **基础能力缺失**：先补文件写入后的 Formatter 执行消费点，再做格式转换 | 默认执行匹配 Formatter | 超时后标记未格式化，文件写入结果保留。 |
| Themes | builtin/user/project/cwd JSON | **部分已有**：GUI Theme 已有；TUI 主题消费边界在终端阶段补齐 | 保留覆盖顺序和语义角色 | 颜色能力不支持时做可见降级。 |
| Keybinds | `tui.json` 的 leader、组合键、禁用和命令标识 | **已有行为、边界未抽取**：从现有 TUI 输入/命令路径提取最小接口 | 保留用户和项目覆盖 | 平台冲突时显示最终绑定与原因。 |
| Models / Providers | `model`、`small_model`、`default_agent`、provider options/variants，以及 `enabled_providers` / `disabled_providers` | Model/Provider 与 Agent 归属模块 | 静态配置直接映射；默认 Agent 与小模型选择保留来源 | 动态软件包适配器交给插件运行时，未知 Provider 只禁用对应选择。 |
| Permissions / Policies | 工具、Skill、Agent 等 allow/deny/ask pattern | Permission 归属模块建立 OpenCode 兼容策略层 | 默认兼容模式保持 OpenCode 决策 | BitFun 用户/组织策略可收紧并明确标记。 |
| Plugins / Tools | config plugin 列表、`plugins/`、`tools/` | 只生成执行来源和顺序，交给 Plugin Runtime Adapter | 自动准备当前执行版本 | 不在配置解析线程加载代码。 |

### 5.1 Rules 与 Instructions

规则内容尽量原地引用，不复制成第二份文件。组合结果保留原始段落来源和顺序。OpenCode 与 BitFun 原生规则
同时存在时，配置视图展示实际进入模型的顺序；不能把冲突文本自动改写成“合并后的真相”。

### 5.2 Agents、Modes 与 Skills

兼容定义进入现有 Agent 归属模块，而不是新建 OpenCode Agent Runtime。字段映射分为：

- 可等价映射：名称、description、prompt、model、variant、`primary|subagent|all`、隐藏/停用状态、采样选项、
  `steps` / deprecated `maxSteps`、deprecated `tools` 和 options。
- 策略映射：`permission` pattern、Skill 访问和 task/subagent 调用。
- 展示映射：color 等只影响来源 Surface，不进入运行时权威事实。
- 未知字段：保留并诊断；后续版本支持时可以重新解释原始来源。

### 5.3 Commands

命令展开按 OpenCode 顺序解析参数、文件引用和 shell 输出。`!shell` 在脚本执行域执行，不另建绕过可靠性
控制的同步 shell 路径。展开有期限、取消和输出大小限制；大输出保存后只把引用交给命令模板。

自定义命令可按 OpenCode 规则覆盖同名内置命令，但产品可以声明少量 protected 命令。发生保护冲突时，
兼容视图保留插件命令的可调用别名，并显示行为差异。

### 5.4 MCP、LSP 与 Formatter

这些能力使用 BitFun 原生归属模块，但“原生已有”不自动等于兼容：

- MCP 必须覆盖 local 的 `environment`，remote 的 Headers、OAuth（含 clientId/clientSecret/scope/callbackPort/redirectUri）、超时、启用状态和 Agent 范围。
- LSP 必须覆盖 initialization、扩展名匹配、环境变量和工作区生命周期。
- Formatter 必须覆盖写入后时机、`$FILE` 替换、`environment`、多个 Formatter 顺序和失败行为。

### 5.5 其他稳定配置项

OpenCode 配置文档还包含下列不属于声明式目录资产、但会改变运行行为的稳定字段。它们不能因“BitFun 已有
类似功能”而被遗漏：

| 配置项 | 适配方式 | 明确边界 |
|---|---|---|
| 独立 TUI 配置 | 按独立来源顺序处理 `$schema/theme/keybinds/plugin/plugin_enabled/leader_timeout/attention/prompt/scroll_speed/scroll_acceleration/diff_style/mouse` | 主配置来源图和构建期 TUI 布局选择不参与其运行时优先级。 |
| `shell` | 交给实际执行域的 Terminal/Tool Runtime，保留短名称、绝对路径和平台默认发现 | 命令不存在时只使相关 shell/工具调用不可用，不阻止项目启动。 |
| `logLevel`、`username` | 分别进入日志配置和会话展示身份 | 不改变插件权限或系统账户。 |
| `tools` | 映射到 Tool 归属模块的模型可见性和启用状态 | 不能用“隐藏工具”代替真实权限控制，也不能启用产品未提供的工具。 |
| `attachment.image` | 映射到 Message/Model 输入处理的自动缩放和大小上限 | 模型或入口不支持图像时明确降级，不发送被静默修改的无效附件。 |
| `share` | `manual/auto/disabled` 映射到 BitFun 会话分享归属模块 | BitFun 没有等价分享后端的产品形态只能显示不支持，不能伪造分享 URL。 |
| `snapshot` | 映射到 Workspace snapshot/checkpoint provider，并保持关闭后不可通过 UI 回滚的提示 | 只有行为等价测试通过的 provider 才能标记兼容；不要求复制 OpenCode 内部 Git 实现。 |
| `tool_output` | `max_lines/max_bytes` 映射到工具结果截断和完整内容保存 | 必须返回可访问的完整结果引用，不能只丢弃超限内容。 |
| `compaction` | `auto/prune/tail_turns/preserve_recent_tokens/reserved` 映射到 Agent Runtime 的压缩和工具输出裁剪策略 | BitFun 的会话持久化事实不变；无法等价的字段单独降级。 |
| `watcher.ignore` | 映射到实际工作区执行域的文件观察器 glob | Remote 在远端应用，不能只过滤本机观察器。 |
| `enterprise.url` | 作为企业配置与身份服务入口交给对应归属模块 | 没有等价企业服务时保留并显示未支持，不能当作普通 Provider URL。 |
| `server` | 仅在显式 OpenCode 外部协议兼容服务中映射 port/hostname/mDNS/CORS | 插件 worker 的回环 `serverUrl` 由 Runtime 管理；普通 BitFun 启动不读取该字段改变自身监听地址。 |
| `autoupdate` | 保留来源并显示“不适用于 BitFun 产品更新” | 它控制 OpenCode 自身更新，不能让项目配置改变 BitFun 安装器或更新通道。 |
| 旧 `autoshare`、`layout`、`mode`、`reference` | 按稳定版迁移到 `share`、固定布局、`agent`、`references` | 诊断显示迁移结果，不把旧字段静默解释成 BitFun 自有语义。 |
| `experimental` | 分别记录 `disable_paste_summary/batch_tool/openTelemetry/primary_tools/continue_loop_on_deny/mcp_timeout/policies` | 未知实验字段保留并告警，不自动发布为稳定 BitFun 接口。 |

`server` 和 `autoupdate` 的降级是产品归属不同，不是解析失败。兼容报告必须显示原值、未生效范围和可选替代
入口；其他可独立生效的配置继续使用。

## 6. 冲突、覆盖与加载顺序

- OpenCode 配置来源顺序和插件加载顺序分别维护；配置归属模块不重新排列插件。
- 同名配置键按来源覆盖；非冲突键合并。
- 同名命令、Theme、Keybind 和插件条目按各自 OpenCode 规则处理，不能使用一个通用“后者覆盖”规则猜测。
- 用户/组织保护项是一层显式策略，不改写来源顺序；兼容报告同时展示“OpenCode 结果”和“策略后结果”。
- 产品定义只给出构建期默认和明确保护的产品能力，不参与项目运行时资产的同名冲突。

## 7. 凭据与敏感信息

- 配置解析可以发现凭据引用、Headers 名称和认证方法，但不把值写入普通状态记录、诊断或导入报告。
- 仓库当前没有横跨 Provider、MCP、插件和 Remote 的通用 Credential owner；不能在 OpenCode Adapter 内补一个
  隐式通用凭据库。
- OC-R3 先补本地“执行域凭据访问”窄接口：请求只携带执行域、领域（Provider/MCP/plugin auth）、来源引用和
  用途，再路由到现有 AI adapter credential resolver、MCP OAuth vault 或对应插件 auth 流程。值只在同一执行域
  的实际调用中提供，不写入兼容结果、普通状态或诊断。
- 显式导入不复制 OpenCode 私有凭据文件到 BitFun 配置。
- `auth` 插件、Provider Headers 和 MCP OAuth 的运行期调用见插件执行设计；配置文档只保存引用和来源。

## 8. Remote 与多执行域

- 项目 `.opencode` 在实际工作区所在执行域发现、解析和监听；远程项目不回到本机扫描同名路径。
- 用户全局根必须明确属于本地用户还是远程用户，不能按路径字符串猜测。
- 本地界面可以展示远程兼容结果和诊断，但依赖安装、LSP、Formatter、local MCP、Command shell 和插件 worker
  在远程执行域运行。
- OC-R5 在远端实现同一窄访问接口，并路由到远端已有领域存储或执行环境；R5 之前远程插件路径返回明确
  `unsupported`。不存在通用 Remote credential broker，也不从本机静默复制原值。
- 两端先交换兼容版本和能力；远端不支持的资产局部降级，不能把整个会话标记为失败。

## 9. 验证要求

每类资产至少覆盖：

1. 主配置完整来源序列、后处理，以及 TUI 独立来源序列。
2. JSON/JSONC、未知字段、无 `$schema`、无效局部字段和变量替换。
3. 从子目录启动时的项目根发现和相对路径解析。
4. 来源变化后的后台重载、上一有效结果保留和原子切换。
5. 用户/组织策略收紧后的明确降级，不误报为解析错误。
6. Windows、macOS、Linux 路径和命令差异。
7. References 本地/Git 来源、Skills paths/urls、旧字段迁移和所有稳定顶层键。
8. Remote 执行域不静默回本机。

完整样例集固定到 OpenCode release commit；开发分支变化只触发差异报告，不能静默改变稳定兼容行为。

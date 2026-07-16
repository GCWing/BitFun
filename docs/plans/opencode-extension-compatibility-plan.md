# OpenCode 扩展兼容执行计划

本文定义 OpenCode 兼容能力的近期交付顺序。完整能力差异保留在
[兼容矩阵](../architecture/extensions/opencode-extension-compatibility.md)，跨生态来源体验与生命周期见
[外部 AI 工作内容设计](../architecture/extensions/external-ai-work-sources-design.md)。兼容矩阵是审计库存，不是默认路线图。

PR1 已将基线推进到通用外部来源目录、生命周期协调器和 OpenCode Prompt Command 纵向切片；BitFun 原有受管
插件包来源确认和 custom tool 静态预览继续保留。后续不沿用“先做一个大而全的 OpenCode Plugin Runtime”路线，
而是沿用稳定的跨生态来源契约，按 Tool、Subagent 分别交给真实能力 owner。每个 PR 都必须产生用户可直接验证的
结果，同时不提前承诺尚未执行的生态能力。

## 1. 稳定架构基线

### 1.1 接口层与实现层

```text
Product surfaces (Desktop / CLI / TUI)
  -> External Source Catalog / lifecycle coordinator
       -> consumes capability-specific provider contracts
            -> Prompt Command provider contract
            -> future Tool provider contract
            -> future Subagent provider contract

Same-level ecosystem adapters implement those provider contracts
  -> OpenCode adapter
  -> future Codex adapter
  -> future Claude Code adapter

Product Assembly registers adapter implementations with the coordinator
```

- 通用层只认识开放的 `ecosystem_id`、来源限定身份、作用域、执行域、状态、诊断和能力专属贡献，不按
  OpenCode/Codex/Claude Code 分支业务行为。
- 每个生态适配器独立维护自己的路径发现、优先级、格式、参数展开和版本兼容语义；兄弟适配器之间不得依赖、复用
  私有类型或借用对方身份。
- Product Assembly 是唯一选择具体适配器的地方。Desktop、CLI/TUI、来源目录和能力 owner 只依赖稳定契约。
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
| PR1：来源目录 + OpenCode Command | Desktop 可查看、抑制/恢复并刷新全局/项目 OpenCode 来源；CLI/TUI 可列出并执行支持的 `/command`；运行中修改、删除、恢复后自动刷新 | 通用来源目录与生命周期协调器；Prompt Command 契约；OpenCode Command adapter | JS/TS Tool 执行、Hook、MCP、OpenCode Client/Server、Subagent 执行、复制式导入 |
| PR2：OpenCode standalone Tool | 一个真实、无外部依赖的 `.opencode/tools/` 样例经预览和确认后进入现有 Tool Runtime，可调用、取消、更新和撤下 | 现有 Tool Runtime + 独立 Tool 兼容接口 | package plugin、npm 依赖安装、Hook、TUI renderer、完整 `metadata`/`ask` |
| PR3：OpenCode Subagent | 全局/项目 agent 定义进入现有 Subagent owner，可选择、调用、更新和撤下；unsupported 字段有明确诊断 | 现有 Subagent owner + 独立 Subagent 兼容接口 | 原始 OpenCode 会话内核、完整 primary-agent 替换、跨产品通用 agent JSON |

Tool 与 Subagent 不复用 Command 的贡献对象，只复用来源身份、状态、代次、诊断和观察生命周期。未来接入 Codex 或
Claude Code 时新增同级 adapter，并在 Product Assembly 注册；不能修改 OpenCode adapter 来容纳其他生态。

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
  必须生成版本敏感的冲突指纹并等待用户选择。候选版本不变时只询问一次，更新后重新询问。CLI/TUI 将跨 provider
  候选投影为 `/external:<provider>:<command>` 明确选择项；一次显式选择同时解决同名 BitFun 本地命令，不连续确认。
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
| `apps/cli` | 将可用外部 Command 投影到 TUI 菜单和输入分发；本地冲突使用 `/builtin:name` 与 `/external:name` 明确选择 | 解析 OpenCode 文件或注册假工具 |
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
- CLI/TUI 覆盖列表、跨 provider 候选选择与直接输入、本地同名冲突只询问一次、版本变化后重新询问、受限命令提示
  和刷新后撤下；首次发现期间未限定别名不误路由，候选删除后不静默切换到剩余外部或内建实现。
- Desktop 覆盖空状态、部分失败、刷新、抑制/恢复、敏感路径缩略显示及 IPC 摘要不包含模板正文。
- 通过相关 crate tests、Web focused tests、`type-check:web`、仓库 hygiene 与 core boundary 检查。

## 4. PR2：OpenCode standalone Tool

PR2 只在 PR1 的来源限定身份、生命周期和产品状态稳定后启动。它从 workspace/user 官方目录发现 tool，不要求
`bitfun.plugin.json`，并把一个真实、无外部依赖且不调用 `metadata`/`ask` 的契约样例接入现有 Tool Runtime。

- 首次按来源/target/执行域说明代码来源、工作目录和直接文件/网络/进程能力；确认前不 import 或启动 worker。
- worker 只提供真实 load/invoke/cancel/dispose 和诊断；Tool Runtime 继续负责 schema、权限、排队、审计与结果。
- 更新只有在来源身份、完整性、执行包络和更新策略仍有效时自动准备；能力扩大进入非阻塞 `action-required`。
- 删除或撤销立即撤下新调用；失败只影响对应 target，不影响 Command、Subagent 或其他生态。

## 5. PR3：OpenCode Subagent

PR3 为现有 Subagent owner 增加独立兼容端口，由 OpenCode adapter 映射 agent Markdown/JSON 定义。它不复用 Tool
运行时，也不把 OpenCode agent 类型提升为跨生态 DTO。

- 支持的 prompt、description、模式和工具选择进入现有 Subagent 定义；未支持字段显示明确诊断。
- 全局/项目覆盖、来源抑制、更新和删除复用通用来源生命周期。
- 选择与执行仍由现有会话/Subagent owner 决定；adapter 不能替换 BitFun Agent Kernel。

## 6. 暂停条件

出现以下情况时停止扩面并先修复架构：

- 新生态 adapter 依赖现有生态 adapter，或 core/UI 开始按生态 ID 分支业务行为；
- 为未来可能需求新增任意 payload 资产、通用脚本 SDK、第二套 Tool Runtime/Agent Runtime；
- 只有静态解析却把 Tool、Hook、Subagent 或受限 Command 标为可用；
- watcher 更新能绕过用户抑制，或一个 provider 的失败清空其他 provider；
- 同名候选仍由固定优先级静默选中，或候选内容版本变化后继续沿用旧冲突选择；
- 为完整兼容一次性引入 package manager、Hook、renderer、Server 和权限系统；
- 本地可用被直接推导为 Remote/HarmonyOS PC 可用，缺少同一 fixture 的真实运行证据。

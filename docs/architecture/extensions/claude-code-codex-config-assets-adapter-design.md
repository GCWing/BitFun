# Claude Code 与 Codex 声明式配置资产适配设计

> 调研与设计基线：2026-07-24。本文约束 Claude Code 与 Codex 的静态、只读配置来源如何进入 BitFun 已有 External Source Control Plane。它不定义 Runtime Host、SDK、插件或 Hook 执行。

上游语义锚点为 Claude Code 官方 [Skills 与 legacy Commands](https://code.claude.com/docs/en/slash-commands)、[Subagents](https://code.claude.com/docs/en/sub-agents)、[MCP](https://code.claude.com/docs/en/mcp)，以及 Codex `205d37a20f742b0bf8e191622bd07c43f567ea49` 的 [`agent_roles.rs`](https://github.com/openai/codex/blob/205d37a20f742b0bf8e191622bd07c43f567ea49/codex-rs/core/src/config/agent_roles.rs) 和 [`mcp_types.rs`](https://github.com/openai/codex/blob/205d37a20f742b0bf8e191622bd07c43f567ea49/codex-rs/config/src/mcp_types.rs)。Adapter 必须以固定基线的已知字段为 allowlist；上游新增行为字段在完成重新分类和契约测试前 fail closed，不能被当作展示字段静默忽略。

## 1. 目标与边界

本设计增加两个 sibling ecosystem adapter，并复用现有 capability-specific provider contracts：

| 生态 | Command | Standalone Tool | Subagent | MCP | Hook |
|---|---|---|---|---|---|
| OpenCode | 已支持 | 已支持 | 已支持 | 已支持 | 静态目录 |
| Claude Code | 本设计支持安全子集 | 不提供 | 本设计支持安全子集 | 本设计支持安全子集 | 已有静态目录 |
| Codex | 无稳定本地 Command 来源 | 不提供 | 本设计支持安全子集 | 本设计支持安全子集 | 已有静态目录 |

明确不包含：

- Node、Bun、外部 CLI、app-server 或其他 Runtime Host。
- JS/TS import、插件执行、Hook 执行、依赖安装或软件包迁移。
- Codex Command/Tool provider、Claude Code standalone Tool provider。
- Codex `/import` 等复制或迁移流程。
- Agent SDK、ACP 能力扩展、Mobile 管理界面。
- 全局数值优先级、跨生态配置格式或通用 `ExternalAsset` 对象。
- Skills Registry 的优先级重构。现有 `.claude/skills`、`.codex/.agents` 来源继续由 Skills owner 处理；本设计不宣称完整复刻各生态的同名 Skill 行为。

## 2. Owner 与依赖方向

```text
Claude/Codex files
  -> ecosystem adapter: parse + native overlay + static compatibility
  -> typed provider: Command / Subagent / MCP
  -> ExternalSourceControlPlane: refresh + isolation + last-valid + conflict
  -> capability owner: Prompt submission / Subagent / MCP runtime
  -> Desktop / TUI / Server / Remote projection
```

- `adapters/{claude-code,codex}-adapter` 负责来源路径、格式、原生覆盖顺序、字段兼容分类、watch roots 和静态准备结果。
- `contracts/product-domains` 继续只提供稳定 DTO 与窄 provider ports。本设计优先复用现有契约，不为未来能力增加字段。
- `assembly/external-sources` 继续拥有有界发现、超时、generation fencing、coalescing、last-valid 和 provider 隔离。
- `assembly/core::external_sources` 只注册能力、选择产品策略并连接既有 Command/Subagent/MCP owner；只有该 composition root 可以依赖具体生态 adapter。
- GUI/TUI/Server/Peer 只消费共享快照和 closed actions，不解析原始 Markdown/JSON/TOML，不按生态复制生命周期。
- 外部 adapter 不启动外部产品、MCP 或脚本，不持久化审批，也不决定 BitFun 权限。

## 3. 统一兼容状态

生态字段必须按行为影响分类，禁止静默丢弃：

| 分类 | 含义 | 是否可激活 |
|---|---|---|
| Supported | 已存在完全对应的 BitFun owner 与语义 | 是 |
| Degraded | 只损失颜色、展示等非行为事实 | 是，显示差异 |
| Restricted/Blocked | 字段会改变模型、权限、工具、隔离、流程或副作用，但无等价 owner | 否 |
| Invalid | 类型、标识、格式或边界不合法 | 否 |
| Catalog only | 可安全展示，但当前没有执行契约 | 否 |

行为版本只包含影响执行的字段。描述、颜色、来源标签等纯展示变化不应使冲突选择失效；prompt、model、tools、command/args/env/header 名称、cwd、URL origin 和兼容状态变化必须改变行为版本。

## 4. Claude Code Command

### 4.1 来源与覆盖

- 用户来源：`~/.claude/commands/**/*.md`。
- 项目来源：从项目边界到当前工作目录的 `.claude/commands/**/*.md`。
- 目录和文件排序必须确定；同一物理文件去重。
- 子目录命令保留 Claude Code 的原生命名空间，例如 `frontend/component.md` 投影为 `/frontend:component`，不发明路径形命令或生态前缀。
- 同一原生层内的同名文件不依赖文件系统遍历顺序，标记为 Invalid 并生成诊断。
- 原生层级覆盖由 Claude adapter 内部完成；被覆盖贡献保留来源诊断，不作为跨生态产品冲突候选。
- 有效 `.claude/skills/<name>/SKILL.md` 与 legacy Command 同名时，Skill 遮蔽 Command。Command provider 只读取 bounded name index，不读取或执行 Skill 内容，也不成为第二个 Skill owner。

### 4.2 安全子集

支持 Markdown body、`$ARGUMENTS`、`$ARGUMENTS[N]` 和 `$N` 的纯文本展开，位置参数与 Claude Code 一致从 0 起算；没有任何占位符时，以 `ARGUMENTS: <value>` 段追加。以下任一行为出现时，Command 必须 Restricted：

- shell 或文件动态引用；
- 需要 Host 会话或路径的 `${CLAUDE_SESSION_ID}`、`${CLAUDE_EFFORT}`、`${CLAUDE_SKILL_DIR}`、`${CLAUDE_PROJECT_DIR}` 动态变量；
- model、agent、context/fork、effort、background；
- allowed/disallowed tools；
- command-local hooks 或其他影响执行上下文的字段；
- 未识别的行为字段。

发现不展开模板、不读取引用文件、不执行 shell，也不把模板正文投影给 GUI/TUI。执行时必须携带预期 candidate id 与 behavior version，经 guarded expansion 后才能提交给现有 Agent owner。

## 5. Claude Code Subagent

### 5.1 来源

- 用户来源：`~/.claude/agents/**/*.md`。
- 项目来源：项目边界到当前工作目录的 `.claude/agents/**/*.md`，距离当前工作目录更近的定义优先。
- `name` 是逻辑冲突键；同层重复 name 为 Invalid。
- 项目定义覆盖用户定义时，完整 provenance 保留 base/overlay 来源。

### 5.2 映射

支持：

- `name`、`description`、prompt body；
- default/inherit 或 exact model request；
- 可精确表达的 `tools`、`disallowedTools`；
- `permissionMode: default` 及其官方 `manual` 别名不改变 BitFun 权限事实；
- `color` 只作为展示降级，不进入行为版本。

以下字段只有在 BitFun 已有完全等价 owner 时才能接入；本切片中存在即 Blocked：

- `permissionMode` 中非默认语义；
- `maxTurns`、`skills`、`mcpServers`、`hooks`、`memory`；
- `background`、`effort`、`isolation`、`initialPrompt`；
- 未识别的行为字段。

adapter 只能声明 model/tool 请求。模型是否存在、工具名称绑定、审批和执行仍由 BitFun owner 在 activation 前确认；不得自动选择近似模型或扩大工具集合。

## 6. Claude Code MCP

### 6.1 来源与覆盖

- user：用户 MCP 配置。
- project：工作区 `.mcp.json`。
- local：`~/.claude.json` 中与规范化当前工作区严格匹配的项目项。
- 同名服务器采用 Claude 原生 `local > project > user` 整项覆盖，不做字段深合并。
- 用户文件中其他项目的值不得进入当前工作区快照或日志。

### 6.2 支持与安全限制

支持 local stdio 与 HTTPS Streamable HTTP。发现阶段只输出脱敏的 command preview、argument count、cwd 标签、env/header 名称和 HTTPS origin；不得启动服务器。

- 环境引用只允许出现在显式 environment 或 header 值中。
- command、args、cwd、URL 中的动态引用 Restricted，防止审批后改变可执行文件或网络目标。
- 未解析变量 fail closed，绝不把 `${VAR}` 原文交给进程。
- SSE、WebSocket、未知传输或未表达的认证流程 Unsupported。
- 项目 MCP 首次激活必须经过 BitFun approval；Claude 自身审批状态不能替代 BitFun policy。

## 7. Codex Subagent

### 7.1 来源与覆盖

- 用户来源：`~/.codex/agents/*.toml`。
- 项目来源：项目边界到当前工作目录的 `.codex/agents/*.toml`。
- 低优先级层先加载，高优先级层逐字段覆盖；缺失字段从低层继承。
- provenance 按 base 到 overlay 记录。每层文件按规范化路径确定性排序；同层重复 name 生成诊断，不依赖 OS 枚举顺序。

### 7.2 映射

支持 `name`、`description`、`developer_instructions` 与 default/exact model request。

本切片中以下行为字段 Blocked：sandbox、reasoning/effort、agent-private MCP、skills config、approval policy 及未知行为字段。Codex 的 sandbox/approval 不能成为 BitFun 权限事实；最终权限仍由 BitFun parent/runtime owner 决定。

## 8. Codex MCP

- 来源是用户和项目层 `config.toml` 的 `[mcp_servers.*]`。
- 项目配置从项目边界到当前工作目录逐层覆盖，最接近当前工作目录的值优先；字段合并遵循 Codex 配置层规则。
- 支持现有契约可表达的 stdio、HTTPS Streamable HTTP、command/args/cwd/env、URL/header/auth env reference 和 enabled 状态。
- `startup_timeout_sec`、`tool_timeout_sec`、tool allow/deny filters、per-tool approval、remote executor、ChatGPT auth 等没有等价 owner 的字段 Unsupported，不得忽略。
- `required=true` 只产生“BitFun 未采用 required 启动语义”的诊断，不能导致 BitFun 启动或聊天失败。
- 项目配置在 BitFun 没有等价 workspace trust 事实时保持 Restricted/待审批，不能把 Codex 的本机 trust 隐式继承给 Remote Host。

## 9. 原生覆盖与产品冲突

不引入全局 `priority`：

1. **同一 provider 的原生覆盖**：adapter 严格按上游规则产生一个 effective candidate；用户不选择，UI 可以解释 provenance。
2. **同一原生层重复**：确定性 Invalid 或诊断，不把偶然文件顺序变成优先级。
3. **跨 provider 或 BitFun-native 同名**：Control Plane 生成产品冲突，未选择时 fail closed。

产品选择保存稳定 candidate id、参与者集合和行为版本指纹。参与者或行为版本变化后必须重新选择；仅展示字段变化不重问。已选候选删除、禁用或不可用后保持 unavailable，不得自动回退到另一个同名候选。

UI 不显示数值优先级，也不提供拖拽排序。它显示“为什么当前生效”：原生层覆盖、用户产品选择、来源不可用或版本变化。

## 10. 用户交互

- 普通命令始终使用 `/name`。
- 不公开 `/builtin:<name>`、`/external:<name>` 或生态前缀命令。
- Slash picker 对同名候选显示来源、scope、兼容状态和描述；选择使用内部 action/candidate id。
- 直接输入存在未解决冲突的 `/name` 时，交互式表面打开或引导到现有候选 picker；非交互式表面返回结构化 conflict，不能猜测。
- `/extensions` 是统一来源状态与 Safe Mode 入口，`/hooks` 是静态 Hook 目录。
- `/mcp` 是主要 MCP 用户入口，现有 `/mcps` 作为兼容 alias 保留。
- 已有 `/agents`、`/subagents`、`/skills` 不因本设计重定义职责。
- `/help <command>`、`/<command> -h` 和 `/<command> --help` 必须对扩展相关命令一致可用。

Desktop Prompt Command picker 必须消费后端公共 Catalog。公共 Catalog 直接提供不透明 `candidateId`，产品表面原样回传，不复制 Rust stable-key 编码。执行经薄 Tauri adapter 调用 Rust guarded expansion；TypeScript 不解析配置、不计算覆盖、不持有 prompt template。

## 11. 时序、更新与故障隔离

复用当前控制面：顶层 refresh gate 串行化；Command、Tool、Subagent、MCP lane 并行；provider discovery 使用既有 5 秒前台超时、30 秒 deferred 生命周期、最多 8 个 discovery worker、同 provider newest-pending coalescing 和 generation fencing。

- 慢或超时 provider 不能被解释为空目录或稳定删除。
- 可识别的单资产错误只隔离该资产；provider 级瞬时失败保留 last-valid。
- 配置稳定删除、显式 disable 或安全撤销阻止新调用，不继续沿用 stale。
- 权限、来源身份或完整性无法确认时 fail closed。
- Subagent 进行中调用持有 generation lease；新调用只用当前 generation。
- MCP 更新先 guarded prepare 新版本，再原子切换 route 并回收旧实例。新版本准备失败时，只有精确物化、仍获审批且未被删除/撤销的旧版本可以保持 degraded。
- 外部 MCP/子进程失败只影响该 capability owner 实例，不传播为主应用退出；Codex `required` 不改变这一规则。

## 12. Remote、Peer 与多产品形态

来源事实由实际执行 Host 产生：

- 本地 Desktop/TUI 读取本地执行域。
- Peer/Remote 控制端只消费远端 Host 快照，绝不能用控制端同名本地配置回退。
- Host 不支持时返回明确 capability unavailable/managed-on-remote-host 状态。
- read-only Server 只投影 Host 已提供的快照；不能在浏览器扫描来源。
- SDK/ACP/Mobile 不在本切片扩展范围。

## 13. 安全、日志和指标

边界：单配置文件最多 1 MiB，单 prompt/command/agent 文件最多 256 KiB，单 provider 正文总量最多 8 MiB，单 provider sources/definitions/diagnostics 使用现有 1024 上限。扫描仅限已知目录；symlink canonicalize 后越出允许 root、循环或非普通文件必须诊断并拒绝。

稳定错误分类包括：`source_unreadable`、`source_invalid`、`schema_unsupported`、`field_unsupported`、`secret_unresolved`、`execution_domain_unavailable`、`stale_revision`、`timeout`、`policy_limited`。生态诊断码可更细，但不得成为各表面独立状态机。

日志与遥测不得包含 prompt、env/header 值、URL query、资产名或绝对 home path。指标只使用 ecosystem、capability、scope、outcome 等低基数维度，并记录 discovery duration、timeout、degraded/last-valid、parse failure、conflict、approval 与 stale-generation drop。

## 14. 性能约束

- 后台 discovery 不阻塞首次聊天。
- 只扫描已知 root，watch roots 去重；watcher storm 只允许一个 active worker 和一个 newest rerun。
- 单 provider generation 中同一物理文件最多解析一次。
- 不在本切片增加跨生态全局配置缓存或通用 Source Graph。
- 1000 个定义/8 MiB fixture 的 catalog 构建和重复检测应保持近似线性，禁止按候选两两比较产生 O(n²)。
- MCP discovery 不把工具注入模型；只有已审批、成功激活的 Server 由 MCP owner 暴露工具，避免无意扩大上下文。

## 15. 验收

至少覆盖：

- 两生态所有已声明来源、覆盖层、同层重复、嵌套项目和 watch roots；
- supported/degraded/blocked/invalid 字段矩阵；
- Command/Skill 遮蔽与跨 provider/BitFun-native 冲突；
- 纯展示变化不重置选择，行为变化、删除和不可用不静默回退；
- config 在审批期间变化、stale revision、迟到 generation、watcher storm；
- secret 缺失、畸形/超限输入、symlink 逃逸和敏感信息脱敏；
- MCP prepare/version guard、启动失败、禁用、删除和回收；
- GUI/TUI 共享 candidate/version，公开帮助中不存在自创命令前缀；
- Remote 不读取控制端本地来源；
- adapter 不依赖 assembly/UI，contracts 不反向依赖 runtime，composition root 之外不出现生态执行分支。

# BitFun 架构设计：安全边界

> 上游文档：[design.md](../design.md)
> 模块角色：为 prompt 注入、主动配置、MCP、hook、shell、网络、凭据、跨目录写入和发布凭据等执行安全风险提供默认常驻、可解释、可临时放行且可审计的安全边界。

## 1. 模块定位

安全边界是 BitFun Agent 执行层的安全域。它判断当前动作是否可能越权、泄露、破坏环境或被恶意上下文操控，并输出允许、沙箱允许、询问、应急放行、拒绝或策略拒绝。

这层必须默认启用。即使用户处于快速路径、质量保障要求较低、演示原型或无 git 目录场景，安全边界仍然生效。

## 2. 设计原则

- **安全与质量分离**：缺测试进入质量建议；读取凭据、执行未知 hook、联网上传文件进入安全决策。
- **先隔离再提速**：优先通过沙箱、白名单、范围授权和一次性授权降低打断。
- **应急放行可审计**：临时放行高风险动作必须带范围、后果、期限、撤销路径和残余风险。
- **项目配置按信任状态执行**：仓库内 hook、MCP、智能体规则、自定义工具、CI helper 必须记录来源、hash、权限和信任状态。
- **工具复写按能力授权**：用户可以启用工具复写，但确认只授予指定范围内的工具语义替换；文件、shell、网络和凭据访问仍逐次受安全边界约束。
- **模型输出作为建议**：prompt、AGENTS.md 或工具描述影响提示和解释，实际权限由安全边界执行。

### 2.1 风险判定模型

安全边界不按工具名称、命令关键词或模型判断结果直接授权。每次 tool、MCP、skills、插件、hook、shell、文件写入、网络访问或浏览器/桌面动作都应先归一为风险评估请求：

| 字段 | 判定作用 |
|---|---|
| 动作 | 读、写、执行、联网、安装、发布、删除、复写工具或修改策略 |
| 来源 | 用户直接要求、模型推断、项目规则、工具输出、MCP、skill、插件或外部内容 |
| 目标 | 工作区文件、工作区外路径、系统配置、远程主机、生产资源、浏览器上下文或凭据 |
| 能力 | 文件读写、shell、网络、凭据、浏览器/桌面、工具复写、插件执行或策略变更 |
| 数据类别 | 普通代码、内部数据、隐私数据、凭据、发布制品、日志或 prompt 上下文 |
| 信任级别 | 用户输入、已信任项目配置、未信任仓库内容、PR/issue、网页、日志、工具返回 |
| 可恢复性 | 可撤销、可回滚、需人工恢复、不可逆或影响外部系统 |
| 影响范围 | 当前文件、当前项目、用户环境、远程环境、团队共享配置、生产或第三方系统 |
| 策略上下文 | 用户偏好、任务覆盖、工作区配置、项目规则、组织策略和当前执行域 |

关键安全风险按后果判定。满足任一条件时，至少进入强确认、隔离执行或阻断路径：

- 可能读取、复制、上传、缓存、打印或传给模型的对象包含凭据、token、SSH key、cookie、云账号、私有数据或企业内部敏感信息。
- 动作可能删除、覆盖、发布、force push、修改权限策略、修改 CI/CD、改变安全配置或影响生产/远程资源。
- 不可信内容影响后续工具调用、命令、网络访问、文件写入、授权或发布决策。
- 动作会扩大智能体能力，例如新增 MCP server、启用插件、复写工具、安装并执行未知依赖或改变沙箱/网络/审批策略。
- 动作会产生外发、持久化或跨执行域影响，例如调用未知外部 API、写全局配置、修改团队共享规则或跨项目复用授权。

安全边界可以使用模型生成解释或辅助低置信判断，但最终决策只能来自能力声明、目标分类、数据分类、信任状态、策略上下文和内核事实。

## 3. 风险类别

| 类别 | 例子 | 默认处理 |
|---|---|---|
| Prompt 注入 | README/issue/doc 要求泄露凭据、忽略策略、执行外部脚本 | 标记为恶意指令，隔离为不可信上下文 |
| 主动配置 | hook、plugin、MCP server、自定义工具、工具复写、智能体规则、工作流脚本 | 已发现时默认未信任，需确认后执行 |
| 网络访问 | 下载依赖、curl 外部域名、上传日志、访问未知 API | 默认按域名/目的说明确认 |
| 凭据访问 | `.env`、SSH key、token、cloud credential、browser cookie | 默认阻断或要求明确范围授权 |
| 文件系统越界 | 写工作区外路径、改 home/config、删除大量文件 | 默认确认，高危路径默认阻断 |
| Shell 执行 | 未知脚本、安装包 postinstall、生成命令链 | 沙箱内可低摩擦，越界则确认 |
| 破坏性动作 | 删除、force push、reset、发布、deploy | 默认确认，组织策略可阻断 |
| 数据外泄 | 把代码、日志、prompt、凭据或制品发到外部服务 | 默认确认或阻断，必须说明目标 |

## 4. 权限动作

```ts
type SecurityDecision =
  | "allow"
  | "allow_in_sandbox"
  | "ask"
  | "ask_with_break_glass"
  | "deny"
  | "deny_by_policy";

type ExecutionLocation =
  | "local_host"
  | "remote_ssh"
  | "container"
  | "acp_client"
  | "mcp_server"
  | "plugin_domain"
  | "browser_or_desktop"
  | "cloud_task";

type SandboxLevel =
  | "none"
  | "permission_only"
  | "snapshot_or_worktree"
  | "readonly_scope"
  | "network_restricted"
  | "process_isolated"
  | "containerized";

interface SandboxFallback {
  reason: string;
  next_best_option: "ask" | "ask_with_break_glass" | "deny";
}

interface SecurityBoundaryDecision {
  decision: SecurityDecision;
  risk: "low" | "medium" | "high" | "critical";
  reasons: string[];
  requested_capabilities: Capability[];
  scope: SecurityScope;
  execution_location: ExecutionLocation;
  sandbox_levels: SandboxLevel[];
  sandbox_fallback?: SandboxFallback;
  user_options: SecurityOption[];
  audit_level: "none" | "local" | "project" | "organization";
}
```

默认体验：

| 动作 | 默认 |
|---|---|
| 读工作区普通文件 | allow |
| 写当前工作区 | allow 或 allow_in_sandbox |
| 运行已识别的 test/lint/build 命令 | allow_in_sandbox |
| 联网访问未知域名 | ask |
| 读取凭据 | ask_with_break_glass 或 deny |
| 执行未信任 hook/MCP/custom 工具 | ask_with_break_glass |
| 启用内置工具复写 | ask_with_break_glass，且绑定项目、来源、hash、权限和期限 |
| 写工作区外路径 | ask_with_break_glass |
| 删除大量文件、force push、发布 | ask_with_break_glass 或 deny_by_policy |

## 5. 沙箱能力边界

沙箱不是单一能力。BitFun 需要同时区分三类保护：

| 类型 | 作用 | 不能替代 |
|---|---|---|
| 权限确认 | 让用户或策略决定动作是否可执行 | 不能限制进程、文件系统或网络副作用 |
| 快照/回滚隔离 | 记录、回滚或隔离工作区文件变化 | 不能阻止命令读取凭据、联网或影响主机 |
| 运行时沙箱 | 通过只读目录、临时 worktree、容器、无凭据环境、网络策略或受控 facade 限制副作用 | 不能替代审计、来源信任和人工风险判断 |

目标能力矩阵：

| 执行面 | 执行位置 | 默认边界 | 沙箱能力 | 用户确认 |
|---|---|---|---|---|
| 本地文件工具 | 本机当前工作区 | 路径解析、工作区根、受管路径策略 | 快照/回滚、临时 worktree、只读/写入范围 | 跨根目录写、删除、高危路径需要确认或拒绝 |
| 本地 shell | 本机用户 shell | 命令说明、危险动作识别、超时、输出摘要 | 临时 worktree、无凭据环境、网络禁用或容器执行 | 未知脚本、安装后脚本、网络、发布和破坏性命令需要确认 |
| 远程 SSH / Dev Container | 远程主机或容器 | 显示主机、工作区根、路径映射、端口/网络边界 | 远程侧临时 worktree、容器、只读挂载、远程无凭据环境 | 本地 Host 不代替远程执行；远程不可支持能力给替代路径 |
| ACP 客户端 | 本地或远程 ACP 进程 | `ask/allow_once/reject_once` 权限桥接、客户端配置和会话范围 | 只读模式、受控工作区、隔离进程或远程执行域 | ACP 允许只表达本次授权；文件、shell、网络和凭据仍按安全边界判定 |
| MCP server / MCP tool | 本地、远程或项目配置声明的位置 | 来源、hash、工具声明、读写能力和用户授权 | 禁用未知来源、只读工具集、受控网络、隔离进程 | 工具自称只读不能自动可信；实际能力变化后重新确认 |
| WebView / MiniApp / 生成式 UI | 前端 iframe 或受控 JS worker | iframe sandbox、postMessage bridge、host facade | iframe sandbox、worker、host-side allowlist、fs/net/shell scope | UI 沙箱不授予宿主文件、网络或 shell 权限 |
| 插件适配主机 | Kernel 管理的 Host / cell / worker / subprocess / sandbox | 项目执行域、来源信任、权限 facade、候选效果校验 | cell、worker、subprocess、容器/无凭据 sandbox | 插件只返回候选效果；授权、审计和工具执行由 Kernel 写入 |
| 浏览器/Computer Use | 本机桌面或浏览器上下文 | 桌面能力开关、动作确认、不可远程时明确禁用 | 受控浏览器上下文、临时 profile、禁止敏感域或剪贴板范围 | 不能在远程工作区假装本地桌面能力可用 |
| 云端异步任务 | 云端任务执行域 | 任务前授权、阶段性授权、取消和审计续接 | 临时环境、无凭据启动、网络策略、只读仓库和受控 secret 注入 | 长任务减少弹窗，但高风险阶段必须提前或阶段性确认 |

最低实现要求：

- `security.decided` 必须记录执行位置、沙箱等级组合、授权范围、残余风险和是否存在降级。
- 用户界面必须能说明“当前动作在本机、远程、容器、ACP 客户端还是插件执行域执行”。
- `allow_in_sandbox` 只能在实际沙箱或隔离路径存在时返回；否则应降级为 `ask`、`ask_with_break_glass` 或 `deny`。
- 无法提供运行时沙箱时，仍可提供权限确认和审计，但文案必须说明这不是系统级隔离。
- 远程、ACP 和插件场景必须绑定执行域；信任、工具复写、事件订阅和授权不能跨项目或跨执行主机复用。

阶段建设：

| 阶段 | 能力 |
|---|---|
| P0 | 展示执行位置、工作区根、授权范围和基础 allow/ask/deny；记录沙箱不可用时的降级原因 |
| P1 | 本地临时 worktree、只读/写入范围、远程上下文提示、ACP 权限桥接和 UI iframe sandbox 统一记录 |
| P2 | 工作区配置声明可信命令、域名、路径和凭据范围；支持撤销、过期和项目级审计 |
| P3 | 插件适配主机原型验证 cell/worker/subprocess/sandbox 分级和项目执行域隔离 |
| P4 | 企业受管 sandbox、容器策略、无凭据运行、网络策略、签名插件和跨项目审计导出 |

## 6. 应急放行规则

应急放行（break-glass）用于处理临时、应急或隔离环境下的高风险动作，规则如下：

- 范围必须明确：单次命令、单个域名、单个目录、当前会话、当前 worktree 或当前任务。
- 默认只对当前范围生效；保存为项目规则必须显式确认。
- 高风险授权要显示后果，例如“此命令可能读取 `.env` 并访问 `api.example.com`”。
- 对关键风险，优先建议隔离环境：临时 worktree、容器、无凭据沙箱、禁用网络或只读目录。
- 组织/项目受管策略可以禁止本地应急放行。
- 所有应急放行都必须可撤销、可查看、可过期。

## 7. 与质量治理的分界

| 问题 | 所属层 |
|---|---|
| 测试没跑 | 配置化策略 / 质量 |
| CI 不稳定 | 质量数据面 / 变更就绪度 |
| PR 需要审查人 | 风险分类器 / 团队治理 |
| 文档注入要求泄露 token | 安全边界 |
| 新增 MCP server | 安全边界 + 项目画像 |
| 迁移脚本影响生产数据 | 安全边界 + 守护策略 |
| 用户跳过深度审查 | 配置化策略 |
| 用户允许联网下载依赖 | 安全边界应急放行 |

## 8. 边界场景

| 场景 | 正确行为 |
|---|---|
| 无 git 或无团队规则且质量保障要求较低的任务 | 允许快速写当前工作区；联网、凭据、删除仍确认 |
| 用户明确说“不要问，直接跑” | 只能降低质量提示；安全越界仍提示或要求沙箱 |
| 仓库 AGENTS.md 要求禁用安全检查 | 视为普通项目规则，不影响强制执行 |
| hook 文件刚被 PR 修改 | 信任状态失效；不能继续按旧信任执行 |
| 项目插件复写 `bash` | 显示复写来源、hash、权限和撤销入口；复写只在当前项目执行域生效，实际命令仍经 shell 安全策略 |
| MCP server 描述自己是 read-only | 仍以工具声明、实际能力和用户授权为准 |
| 依赖安装脚本需要网络 | 展示域名和命令来源；可允许本次安装，不默认授予智能体阶段 |
| 发布 token 在环境变量中 | 默认不暴露给智能体；发布操作经受控适配器或用户确认 |

## 9. 成功标准

- 快速路径不因为安全系统产生过度弹窗。
- 高风险动作不会被 prompt、项目文档或插件自行授权。
- 用户能在质量保障要求较低或应急场景清楚地一次性放行，并知道风险。
- 组织强策略能禁止本地绕过。
- 安全事件可追踪，但不强迫普通项目进入质量审计流程。

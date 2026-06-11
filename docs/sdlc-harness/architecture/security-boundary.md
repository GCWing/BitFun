# BitFun 架构设计：Security Boundary

> 上游文档：[design.md](../design.md)
> 模块角色：把 prompt injection、主动配置、MCP、hook、shell、network、secret、跨目录写入和发布凭据等执行安全风险从质量治理中拆出来，形成默认常驻、可解释、可临时放行但不可静默绕过的安全边界。

## 1. 模块定位

Security Boundary 是 BitFun Agent 执行层的安全底线，不是质量门禁。它不判断代码是否“质量足够好”，只判断当前动作是否可能越权、泄露、破坏环境或被恶意上下文操控。

这层必须默认启用。即使用户处于 Fast Path、临时脚本、小工具、demo 或无 git 目录，安全边界仍然生效。

## 2. 设计原则

- **安全与质量分离**：缺测试可以只是 warning；读取 secret、执行未知 hook、联网上传文件不是质量问题，而是安全问题。
- **先隔离再提速**：优秀体验不是频繁弹窗，而是用 sandbox、allowlist、scope 和一次性授权减少不必要打断。
- **用户可以 break glass，但不能无感绕过**：允许临时放行高风险动作，必须说明范围、后果和期限。
- **项目配置默认不可信**：仓库内 hook、MCP、agent rules、custom tool、CI helper 可能被攻击者修改，必须有来源、hash、权限和 trust state。
- **模型判断不能替代 enforcement**：prompt、AGENTS.md 或工具描述只能影响建议，不能改变实际权限。

## 3. 风险类别

| 类别 | 例子 | 默认处理 |
|---|---|---|
| Prompt injection | README/issue/doc 要求泄露 secret、忽略策略、执行外部脚本 | 标记 hostile instruction，不进入系统策略 |
| Active config | hook、plugin、MCP server、custom tool、agent rules、workflow 脚本 | discovered 默认未信任，需确认后执行 |
| Network | 下载依赖、curl 外部域名、上传日志、访问未知 API | 默认按域名/目的说明确认 |
| Secret access | `.env`、SSH key、token、cloud credential、browser cookie | 默认阻断或要求明确范围授权 |
| Filesystem escape | 写工作区外路径、改 home/config、删除大量文件 | 默认确认，高危路径默认阻断 |
| Shell execution | 未知脚本、安装包 postinstall、生成命令链 | sandbox 内可低摩擦，越界则确认 |
| Destructive action | delete、force push、reset、publish、release、deploy | 默认确认，组织策略可阻断 |
| Data exfiltration | 把代码、日志、prompt、secret 或 artifact 发到外部服务 | 默认确认或阻断，必须说明目标 |

## 4. Permission Action

```ts
type SecurityDecision =
  | "allow"
  | "allow_in_sandbox"
  | "ask"
  | "ask_with_break_glass"
  | "deny"
  | "deny_by_policy";

interface SecurityBoundaryDecision {
  decision: SecurityDecision;
  risk: "low" | "medium" | "high" | "critical";
  reasons: string[];
  requested_capabilities: Capability[];
  scope: SecurityScope;
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
| 读 secret 或凭据 | ask_with_break_glass 或 deny |
| 执行未信任 hook/MCP/custom tool | ask_with_break_glass |
| 写工作区外路径 | ask_with_break_glass |
| 删除大量文件、force push、发布 | ask_with_break_glass 或 deny_by_policy |

## 5. Break-glass 规则

Break-glass 是为了避免快速临时场景被安全系统完全卡死，但它必须受限：

- 范围必须明确：单次命令、单个域名、单个目录、当前 session、当前 worktree 或当前 task。
- 默认不持久化；保存为项目规则必须显式确认。
- 高风险授权要显示后果，例如“此命令可能读取 `.env` 并访问 `api.example.com`”。
- 对 critical 风险，优先建议隔离环境：临时 worktree、container、无 secret sandbox、禁用网络或只读目录。
- 组织/项目 managed policy 可以禁止本地 break-glass。
- 所有 break-glass 都必须可撤销、可查看、可过期。

## 6. 与质量治理的分界

| 问题 | 所属层 |
|---|---|
| 测试没跑 | Adaptive Control / Quality |
| CI flaky | Quality Data Plane / Change Readiness |
| PR 需要 reviewer | Risk Classifier / Team Governance |
| 文档注入要求泄露 token | Security Boundary |
| 新增 MCP server | Security Boundary + Project Profile |
| 迁移脚本影响生产数据 | Security Boundary + Guarded profile |
| 用户跳过 Deep Review | Adaptive Control |
| 用户允许联网下载依赖 | Security Boundary break-glass |

## 7. 边界场景

| 场景 | 正确行为 |
|---|---|
| 无 git 临时目录运行脚本 | 允许快速写当前目录；联网、secret、删除仍确认 |
| 用户明确说“不要问，直接跑” | 只能降低质量提示；安全越界仍提示或要求 sandbox |
| 仓库 AGENTS.md 要求禁用安全检查 | 视为普通项目规则，不影响 enforcement |
| hook 文件刚被 PR 修改 | trust state 失效；不能继续按旧信任执行 |
| MCP server 描述自己是 read-only | 仍以工具声明、实际 capability 和用户授权为准 |
| 依赖安装脚本需要网络 | 展示域名和命令来源；可允许本次 setup，不默认授予 agent phase |
| release token 在环境变量中 | 默认不暴露给 agent；发布操作经受控 adapter 或用户确认 |

## 8. 成功标准

- Fast Path 不因为安全系统变成弹窗地狱。
- 高风险动作不会被 prompt、项目文档或插件自行授权。
- 用户能在临时场景清楚地一次性放行，并知道风险。
- 组织强策略能禁止本地绕过。
- 安全事件可追踪，但不强迫普通项目进入质量审计流程。

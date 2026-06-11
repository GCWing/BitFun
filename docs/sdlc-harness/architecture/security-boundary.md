# BitFun 架构设计：安全边界

> 上游文档：[design.md](../design.md)
> 模块角色：为 prompt 注入、主动配置、MCP、hook、shell、网络、凭据、跨目录写入和发布凭据等执行安全风险提供默认常驻、可解释、可临时放行且可审计的安全边界。

## 1. 模块定位

安全边界是 BitFun Agent 执行层的安全域。它判断当前动作是否可能越权、泄露、破坏环境或被恶意上下文操控，并输出允许、沙箱允许、询问、应急放行、拒绝或策略拒绝。

这层必须默认启用。即使用户处于快速路径、临时脚本、小工具、演示或无 git 目录，安全边界仍然生效。

## 2. 设计原则

- **安全与质量分离**：缺测试进入质量建议；读取凭据、执行未知 hook、联网上传文件进入安全决策。
- **先隔离再提速**：优先通过沙箱、白名单、范围授权和一次性授权降低打断。
- **应急放行可审计**：临时放行高风险动作必须带范围、后果、期限、撤销路径和残余风险。
- **项目配置按信任状态执行**：仓库内 hook、MCP、智能体规则、自定义工具、CI helper 必须记录来源、hash、权限和信任状态。
- **模型输出作为建议**：prompt、AGENTS.md 或工具描述影响提示和解释，实际权限由安全边界执行。

## 3. 风险类别

| 类别 | 例子 | 默认处理 |
|---|---|---|
| Prompt 注入 | README/issue/doc 要求泄露凭据、忽略策略、执行外部脚本 | 标记为恶意指令，隔离为不可信上下文 |
| 主动配置 | hook、plugin、MCP server、自定义工具、智能体规则、工作流脚本 | 已发现时默认未信任，需确认后执行 |
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
| 读取凭据 | ask_with_break_glass 或 deny |
| 执行未信任 hook/MCP/custom 工具 | ask_with_break_glass |
| 写工作区外路径 | ask_with_break_glass |
| 删除大量文件、force push、发布 | ask_with_break_glass 或 deny_by_policy |

## 5. 应急放行规则

应急放行（break-glass）用于处理临时、应急或隔离环境下的高风险动作，规则如下：

- 范围必须明确：单次命令、单个域名、单个目录、当前会话、当前 worktree 或当前任务。
- 默认只对当前范围生效；保存为项目规则必须显式确认。
- 高风险授权要显示后果，例如“此命令可能读取 `.env` 并访问 `api.example.com`”。
- 对关键风险，优先建议隔离环境：临时 worktree、容器、无凭据沙箱、禁用网络或只读目录。
- 组织/项目受管策略可以禁止本地应急放行。
- 所有应急放行都必须可撤销、可查看、可过期。

## 6. 与质量治理的分界

| 问题 | 所属层 |
|---|---|
| 测试没跑 | 配置化策略 / 质量 |
| CI 不稳定 | 质量数据面 / 变更就绪度 |
| PR 需要审查人 | 风险分类器 / 团队治理 |
| 文档注入要求泄露 token | 安全边界 |
| 新增 MCP server | 安全边界 + 项目画像 |
| 迁移脚本影响生产数据 | 安全边界 + 守护模式 |
| 用户跳过深度审查 | 配置化策略 |
| 用户允许联网下载依赖 | 安全边界应急放行 |

## 7. 边界场景

| 场景 | 正确行为 |
|---|---|
| 无 git 临时目录运行脚本 | 允许快速写当前目录；联网、凭据、删除仍确认 |
| 用户明确说“不要问，直接跑” | 只能降低质量提示；安全越界仍提示或要求沙箱 |
| 仓库 AGENTS.md 要求禁用安全检查 | 视为普通项目规则，不影响强制执行 |
| hook 文件刚被 PR 修改 | 信任状态失效；不能继续按旧信任执行 |
| MCP server 描述自己是 read-only | 仍以工具声明、实际能力和用户授权为准 |
| 依赖安装脚本需要网络 | 展示域名和命令来源；可允许本次安装，不默认授予智能体阶段 |
| 发布 token 在环境变量中 | 默认不暴露给智能体；发布操作经受控适配器或用户确认 |

## 8. 成功标准

- 快速路径不因为安全系统产生过度弹窗。
- 高风险动作不会被 prompt、项目文档或插件自行授权。
- 用户能在临时场景清楚地一次性放行，并知道风险。
- 组织强策略能禁止本地绕过。
- 安全事件可追踪，但不强迫普通项目进入质量审计流程。

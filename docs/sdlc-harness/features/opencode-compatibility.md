# BitFun 子模块设计：主动配置与 OpenCode 兼容层

> 上游文档：[design.md](../design.md)
> 模块角色：在 BitFun 内部 Hook/Event Bus 与安全边界之上，发现、隔离、审核并兼容 OpenCode 风格插件、hook、自定义工具和事件流。

## 1. 模块定位

OpenCode 兼容层是生态适配层，不是 BitFun 内核能力，也不是默认质量保护插件系统。它的首要产品职责是把项目中的主动配置显式化，并防止 hook、plugin、自定义工具或 MCP 把普通项目打开流程变成不可见的执行风险。

BitFun 内部必须以自己的规范事件、交付物、权限和策略模型为准；OpenCode API 只负责降低插件迁移成本。插件可以提供观察、建议、验证提示或证据候选，但不能绕过安全边界，也不能直接写通过/失败、强制/阻断或审计事实。

P0/P1 不承诺任意社区插件无修改运行。默认策略是发现、展示、只读观察和最小权限；执行型自定义工具必须经过信任审核和显式授权。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [OpenCode Plugins](https://opencode.ai/docs/plugins/) / [SDK](https://opencode.ai/docs/sdk/) / [Server API](https://opencode.ai/docs/server/) | plugin 上下文、hooks object、自定义工具、客户端日志、SSE 事件流是生态迁移重点 |
| [Codex Hooks](https://developers.openai.com/codex/hooks) | hook 需要信任审查、配置来源、事件范围、并发和关闭机制 |
| [Claude Code Hooks](https://code.claude.com/docs/en/hooks) | hook 需要明确阻塞/非阻塞、退出码、权限和上下文语义 |
| [Kiro Hooks](https://kiro.dev/docs/hooks/) | hook 已成为 IDE 内事件触发自动化能力，但必须和权限、策略、人工确认分离 |
| [OWASP LLM Top 10](https://owasp.org/www-project-top-10-for-large-language-model-applications/) | 插件、工具调用、数据出境和权限提升属于 LLM 应用风险面 |

设计约束：

- 兼容适配器不得改变 BitFun 内部事件模型。
- 插件不能绕过权限、策略、脱敏和审计。
- 项目内 hook、plugin 配置和自定义工具默认未信任。
- 插件来源、版本、hash、权限声明和兼容等级必须可见。
- 多个 hook 命中同一事件时，不允许依赖隐式顺序做安全判断。
- 阻断语义必须进入 BitFun 策略层；第三方 hook 只能建议。
- 兼容承诺必须通过测试矩阵表达，不用“兼容 OpenCode”这种宽泛表述替代边界。

## 3. 范围与非目标

范围：

- 发现 OpenCode 风格主动配置，并写入项目画像。
- 映射 OpenCode 常见事件到 BitFun 规范 Hook/Event Bus。
- 提供有限 plugin 上下文、客户端门面、自定义工具 API。
- 支持 SSE 事件流或本地事件订阅的受控子集。
- 支持观察/建议类插件产出证据候选或风险提示。

非目标：

- 不复制 OpenCode 运行时。
- 不把 OpenCode 配置作为 BitFun 规范配置。
- 不兼容所有插件行为和 shell 语义。
- 不允许插件直接写入门禁通过、就绪度就绪或审计事实。
- 不用插件能力作为快速路径的默认前置条件。

## 4. 输入、输出与数据模型

OpenCode 常见事件映射：

| OpenCode 事件 | BitFun 来源 | 默认用途 |
|---|---|---|
| `tool.execute.before` | 工具运行时 | 权限检查、风险提示、命令建议 |
| `tool.execute.after` | 工具运行时 | 验证摘要、证据候选 |
| `permission.asked` / `permission.replied` | 审批系统 | 安全授权和审计 |
| `file.edited` / `file.watcher.updated` | 文件监听 | 过期证据、风险提示 |
| `lsp.client.diagnostics` | LSP 服务 | 诊断证据候选 |
| `session.diff` | Git 服务 | 就绪度提示 |
| `session.idle` | 会话运行时 | 未验证风险和完成度建议 |
| `shell.env` | 环境提供者 | 凭据和环境注入策略 |

兼容上下文：

```ts
interface OpenCodeCompatContext {
  project: { root: string; worktree: string };
  directory: string;
  client: OpenCodeCompatClient;
  permissions: PermissionFacade;
  events: EventFacade;
  security: SecurityBoundaryFacade;
}
```

## 5. 核心流程

```text
发现主动配置
  -> 记录来源、hash、权限和范围
  -> 分类信任状态
  -> 执行前经过安全边界决策
  -> 映射兼容适配器
  -> 在超时和沙箱约束下执行 plugin hook
  -> 归一化副作用和建议
  -> 追加审计事件
```

Hook 效应等级：

| 等级 | 能力 | 默认策略 | 就绪度/门禁关系 |
|---|---|---|---|
| observe | 读取事件、记录日志、生成证据候选 | 受限只读，可在受信任来源中启用 | 不能影响就绪/通过 |
| recommend | 生成建议、风险提示、验证提示 | 需要声明输出结构 | 只能进入建议 |
| guard | 对工具、权限或文件操作提出警告/拒绝建议 | 必须通过 BitFun 策略引擎解释 | 可导致建议模式、降级或拒绝，但不能直接写通过/失败 |
| act | 修改工具输入、触发命令、写文件或调用自定义工具 | 默认关闭，需要显式信任、权限、超时和审计 | 只产出事实或证据，决策仍由 BitFun 产生 |

项目级信任记录必须绑定 hook 来源、hash、范围、权限、创建者和审核人。hook 内容变化后信任状态失效，必须重新确认。

## 6. API 兼容等级

| 等级 | 范围 | 目标 |
|---|---|---|
| L0 | 发现、事件命名、载荷映射、只读客户端日志 | 支持迁移和观察 |
| L1 | `tool.execute.*`、`permission.*`、`file.*`、`session.*` 只读或建议 | 支持核心低风险插件 |
| L2 | 自定义工具、SSE 事件流、受限 `$` shell 门面 | 支持可控扩展 |
| L3 | 更广泛生态兼容 | 仅在 L0-L2 稳定后评估 |

兼容矩阵：

| 能力 | P0/P1 状态 | 说明 |
|---|---|---|
| 项目级插件发现 | 支持 | 发现但默认不执行 |
| 项目级插件加载 | 受限 | 仅加载明确启用目录和受信任文件 |
| 全局插件加载 | 暂不默认启用 | 避免跨项目状态串扰和权限混淆 |
| hook 事件映射 | 支持 L0/L1 | 以 BitFun 规范事件为事实来源 |
| 自定义工具 | 受限支持 | 必须声明权限和输入输出结构 |
| shell 门面 | 受限支持 | 默认无网络、超时、审计、敏感信息脱敏 |
| SSE 事件流 | P2 评估 | 先稳定本地事件订阅和权限模型 |

## 7. 策略与治理

- **安全优先**：插件执行前必须通过安全边界。
- **权限优先**：文件、shell、网络、凭据访问全部走 BitFun 权限模型。
- **策略优先**：hook 只触发和采集，复杂判断进入策略引擎。
- **隔离执行**：默认禁止无约束 shell、网络和全仓读写。
- **信任优先**：项目内 hook/plugin/custom 工具必须先完成信任审查；未信任定义只能被展示和禁用。
- **审计可追溯**：插件输入、输出、耗时、失败和副作用写入质量数据面。
- **兼容可测试**：每个兼容等级必须有 fixture plugin 和行为测试。
- **降级可见**：插件失败不能静默影响任务结果，必须进入警告、降级或安全决策。

## 8. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | 主动配置发现、L0 映射、只读观察、审计 |
| P1 | L1 建议类插件、权限策略、信任审查持久化 |
| P2 | 自定义工具最小集、SSE 事件流、插件注册表、签名/来源标识 |
| P3 | 更广泛 OpenCode 生态兼容和企业策略包 |

## 9. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 兼容层侵入核心模型 | 内部模块不得依赖 OpenCode 载荷；只能依赖规范事件 |
| 插件越权 | 文件、shell、网络、凭据访问全部走 BitFun 权限 |
| 插件影响决策结论 | 插件只能产出证据或建议，不能直接写通过、失败或就绪 |
| hook 顺序被误用为安全边界 | 安全策略必须在 BitFun 策略层统一判断 |
| 项目级主动配置供应链风险 | 信任记录绑定 hash 和权限；配置变化后必须重新确认 |
| 运行时不一致 | L0/L1 明确支持范围，不承诺完整 OpenCode 运行时 |
| 维护成本边界不清 | API 兼容性分级推进，每级有成功标准和退出条件 |

## 10. 成功标准

- 项目主动配置能被发现、解释、禁用和重新信任。
- BitFun 内核事件、权限和审计模型保持独立。
- 插件失败、超时、拒绝权限都能被安全边界和证据包感知。
- 常用观察/建议插件可以通过适配器迁移核心逻辑。
- L0/L1 兼容范围清晰，未支持能力不会被误认为可用。

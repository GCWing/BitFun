# BitFun 子模块设计：Active Config and OpenCode Compatibility

> 上游文档：[design.md](../design.md)
> 模块角色：在 BitFun 内部 Hook/Event Bus 与 Security Boundary 之上，发现、隔离、审核并兼容 OpenCode 风格插件、hook、custom tool 和 event stream。

## 1. 模块定位

OpenCode Compatibility 是生态适配层，不是 BitFun 内核能力，也不是默认质量保护插件系统。它的首要产品职责是把项目中的主动配置显式化，并防止 hook/plugin/custom tool/MCP 把普通项目打开流程变成不可见的执行风险。

BitFun 内部必须以自己的 canonical event、artifact、permission 和 policy model 为准；OpenCode API 只负责降低插件迁移成本。插件可以提供 observation、recommendation、verification hint 或 evidence candidate，但不能绕过 Security Boundary，也不能直接写 pass/fail、required/blocking 或审计事实。

P0/P1 不承诺任意社区插件无修改运行。默认策略是发现、展示、只读观察和最小权限；执行型 custom tool 必须经过信任审核和显式授权。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [OpenCode Plugins](https://opencode.ai/docs/plugins/) / [SDK](https://opencode.ai/docs/sdk/) / [Server API](https://opencode.ai/docs/server/) | plugin context、hooks object、custom tools、client log、SSE event stream 是生态迁移重点 |
| [Codex Hooks](https://developers.openai.com/codex/hooks) | hook 需要 trust review、配置来源、事件范围、并发和关闭机制 |
| [Claude Code Hooks](https://code.claude.com/docs/en/hooks) | hook 需要明确阻塞/非阻塞、退出码、权限和上下文语义 |
| [Kiro Hooks](https://kiro.dev/docs/hooks/) | hook 已成为 IDE 内事件触发自动化能力，但必须和权限、策略、人工确认分离 |
| [OWASP LLM Top 10](https://owasp.org/www-project-top-10-for-large-language-model-applications/) | 插件、工具调用、数据出境和权限提升属于 LLM app 风险面 |

设计约束：

- compatibility adapter 不得改变 BitFun 内部事件模型。
- 插件不能绕过 permission、policy、redaction、audit。
- 项目内 hook、plugin config 和 custom tool 默认未信任。
- 插件来源、版本、hash、权限声明和兼容等级必须可见。
- 多个 hook 命中同一事件时，不允许依赖隐式顺序做安全判断。
- 阻断语义必须进入 BitFun policy layer；第三方 hook 只能建议。
- 兼容承诺必须通过测试矩阵表达，不用“兼容 OpenCode”这种宽泛表述替代边界。

## 3. 范围与非目标

范围：

- 发现 OpenCode 风格主动配置，并写入 Project Profile。
- 映射 OpenCode 常见事件到 BitFun canonical Hook/Event Bus。
- 提供有限 plugin context、client facade、custom tool API。
- 支持 SSE event stream 或本地事件订阅的受控子集。
- 支持 observe/recommend 类插件产出 evidence candidate 或 risk hint。

非目标：

- 不复制 OpenCode runtime。
- 不把 OpenCode config 作为 BitFun canonical config。
- 不兼容所有插件行为和 shell 语义。
- 不允许插件直接写入 Gate pass、readiness ready 或审计事实。
- 不用插件能力作为 Fast Path 的默认前置条件。

## 4. 输入、输出与数据模型

OpenCode 常见事件映射：

| OpenCode event | BitFun source | 默认用途 |
|---|---|---|
| `tool.execute.before` | tool runtime | 权限检查、risk hint、command advice |
| `tool.execute.after` | tool runtime | verification summary、evidence candidate |
| `permission.asked` / `permission.replied` | approval system | 安全授权和审计 |
| `file.edited` / `file.watcher.updated` | file watcher | stale evidence、risk hint |
| `lsp.client.diagnostics` | LSP service | diagnostics evidence candidate |
| `session.diff` | Git service | readiness hint |
| `session.idle` | session runtime | 未验证风险和完成度建议 |
| `shell.env` | environment provider | secret 和环境注入策略 |

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
discover active config
  -> record source/hash/permissions/scope
  -> classify trust state
  -> Security Boundary decision before execution
  -> compatibility adapter mapping
  -> plugin hook execution with timeout and sandbox
  -> normalize side effects and suggestions
  -> append audit event
```

Hook 效应等级：

| 等级 | 能力 | 默认策略 | readiness/Gate 关系 |
|---|---|---|---|
| observe | 读取事件、记录日志、生成 evidence candidate | 受限只读，可在受信任来源中启用 | 不能影响 ready/pass |
| recommend | 生成建议、risk hint、verification hint | 需要声明输出 schema | 只能进入 recommendation |
| guard | 对工具、权限或文件操作提出 warn/deny 建议 | 必须通过 BitFun policy engine 解释 | 可导致 advisory/degraded/deny，但不能直接写 pass/fail |
| act | 修改工具输入、触发命令、写文件或调用 custom tool | 默认关闭，需要显式信任、权限、超时和审计 | 只产出事实或证据，决策仍由 BitFun 产生 |

项目级 trust 记录必须绑定 hook source、hash、scope、permissions、created_by 和 reviewed_by。hook 内容变化后信任状态失效，必须重新确认。

## 6. API 兼容等级

| 等级 | 范围 | 目标 |
|---|---|---|
| L0 | 发现、事件命名、payload mapping、只读 client log | 支持迁移和观察 |
| L1 | `tool.execute.*`、`permission.*`、`file.*`、`session.*` 只读或 recommend | 支持核心低风险插件 |
| L2 | custom tools、SSE event stream、limited `$` shell facade | 支持可控扩展 |
| L3 | 更广泛 ecosystem compatibility | 仅在 L0-L2 稳定后评估 |

兼容矩阵：

| 能力 | P0/P1 状态 | 说明 |
|---|---|---|
| project-level plugin discovery | 支持 | 发现但默认不执行 |
| project-level plugin loading | 受限 | 仅加载明确启用目录和受信任文件 |
| global plugin loading | 暂不默认启用 | 避免跨项目状态串扰和权限混淆 |
| hook event mapping | 支持 L0/L1 | 以 BitFun canonical event 为事实来源 |
| custom tool | 受限支持 | 必须声明权限和输入输出 schema |
| shell facade | 受限支持 | 默认无网络、超时、审计、敏感信息 redaction |
| SSE event stream | P2 评估 | 先稳定本地事件订阅和权限模型 |

## 7. 策略与治理

- **安全优先**：插件执行前必须通过 Security Boundary。
- **权限优先**：文件、shell、network、secret 访问全部走 BitFun permission model。
- **策略优先**：hook 只触发和采集，复杂判断进入 Policy Engine。
- **隔离执行**：默认禁止无约束 shell、网络和全仓读写。
- **信任优先**：项目内 hook/plugin/custom tool 必须先完成 trust review；未信任定义只能被展示和禁用。
- **审计可追溯**：插件输入、输出、耗时、失败和副作用写入 Quality Data Plane。
- **兼容可测试**：每个兼容等级必须有 fixture plugin 和行为测试。
- **降级可见**：插件失败不能静默影响任务结果，必须进入 warning、degraded 或 security decision。

## 8. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | active config discovery、L0 mapping、只读观察、审计 |
| P1 | L1 recommend 插件、权限策略、trust review 持久化 |
| P2 | custom tool 最小集、SSE stream、plugin registry、签名/来源标识 |
| P3 | 更广泛 OpenCode 生态兼容和企业策略包 |

## 9. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 兼容层侵入核心模型 | 内部模块不得依赖 OpenCode payload；只能依赖 canonical event |
| 插件越权 | 文件、shell、network、secret 访问全部走 BitFun permission |
| 插件影响决策结论 | 插件只能产出 evidence 或 recommendation，不能直接写 pass/fail/ready |
| hook 顺序被误用为安全边界 | 安全策略必须在 BitFun policy layer 统一判断 |
| 项目级主动配置供应链风险 | trust 记录绑定 hash 和权限；配置变化后必须重新确认 |
| 运行时不一致 | L0/L1 明确支持范围，不承诺完整 OpenCode runtime |
| 维护成本边界不清 | API compatibility 分级推进，每级有成功标准和退出条件 |

## 10. 成功标准

- 项目主动配置能被发现、解释、禁用和重新信任。
- BitFun 内核事件、权限和审计模型保持独立。
- 插件失败、超时、拒绝权限都能被 Security Boundary 和 EvidencePack 感知。
- 常用 observe/recommend 插件可以通过 adapter 迁移核心逻辑。
- L0/L1 兼容范围清晰，未支持能力不会被误认为可用。

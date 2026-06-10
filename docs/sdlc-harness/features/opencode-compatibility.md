# BitFun 生命周期工程子模块设计：OpenCode Compatibility

> 上游文档：[design.md](../design.md)
> 模块角色：在 BitFun 内部 Hook/Event Bus 之上提供 OpenCode 风格插件、hook、custom tool 和 event stream 兼容能力。

## 1. 模块定位

OpenCode Compatibility 是生态适配层，不是 BitFun 内核能力。BitFun 内部必须以自己的 canonical event、artifact、permission 和 policy model 为准；OpenCode API 只负责降低插件迁移成本，并服务质量保护和自动化治理场景，例如权限看护、证据采集、风险分类、LSP diagnostics 和 session idle 完成度检查。

P0/P1 只兼容质量保护相关核心事件和 custom tool 最小集，不承诺任意社区插件无修改运行。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [OpenCode Plugins](https://opencode.ai/docs/plugins/) / [SDK](https://opencode.ai/docs/sdk/) / [Server API](https://opencode.ai/docs/server/) | plugin context、hooks object、custom tools、client log、SSE event stream 是生态迁移重点 |
| [Codex Hooks](https://developers.openai.com/codex/hooks) | hook 需要 trust review、配置来源、事件范围、并发和关闭机制 |
| [Claude Code Hooks](https://code.claude.com/docs/en/hooks) | hook 需要明确阻塞/非阻塞、退出码、权限和上下文语义 |
| [Kiro Hooks](https://kiro.dev/docs/hooks/) | hook 已成为 IDE 内事件触发自动化能力，但必须和质量策略、权限和人工确认分离 |
| [OWASP LLM Top 10](https://owasp.org/www-project-top-10-for-large-language-model-applications/) | 插件、工具调用、数据出境和权限提升属于 LLM app 风险面 |

设计约束：

- compatibility adapter 不得改变 BitFun 内部事件模型。
- 插件不能绕过 permission、policy、redaction、audit。
- API 兼容分级推进，先支持 L0/L1。
- 第三方插件默认最小权限、超时、可禁用、可审计。
- 插件来源、版本、hash、权限声明和兼容等级必须可见。
- 项目内 hook、plugin config 和 custom tool 默认未信任；只有通过 trust review 的定义才能执行。
- 多个 hook 命中同一事件时，不允许依赖隐式顺序做安全判断；阻断语义必须进入 BitFun policy layer。
- 兼容承诺必须通过测试矩阵表达，不用“兼容 OpenCode”这种宽泛表述替代边界。

## 3. 范围与非目标

范围：

- 映射 OpenCode 常见事件到 BitFun canonical Hook/Event Bus。
- 提供有限 plugin context、client facade、custom tool API。
- 支持 SSE event stream 或本地事件订阅。
- 支持质量保护相关 hook：tool before/after、permission、file、lsp、session。

非目标：

- 不复制 OpenCode runtime。
- 不把 OpenCode config 作为 BitFun canonical config。
- 不兼容所有插件行为和 shell 语义。
- 不允许插件直接写入 Gate pass 或审计事实。

## 4. 输入、输出与数据模型

OpenCode 常见事件映射：

| OpenCode event | BitFun source | 质量保护用途 |
|---|---|---|
| `tool.execute.before` | tool runtime | 权限检查、risk policy、command rewrite |
| `tool.execute.after` | tool runtime | EvidencePack、verification summary |
| `permission.asked` / `permission.replied` | approval system | 风险接受和审计 |
| `file.edited` / `file.watcher.updated` | file watcher | Risk Classifier、stale evidence |
| `lsp.client.diagnostics` | LSP service | pre-PR diagnostics evidence |
| `session.diff` | Git service | required checks seed |
| `session.idle` | session runtime | 完成度、未验证风险、gate prompt |
| `shell.env` | environment provider | secret 和环境注入策略 |

兼容上下文：

```ts
interface OpenCodeCompatContext {
  project: { root: string; worktree: string };
  directory: string;
  client: OpenCodeCompatClient;
  permissions: PermissionFacade;
  events: EventFacade;
}
```

## 5. 核心流程

```text
BitFun lifecycle event
  -> canonical policy and permission check
  -> compatibility adapter mapping
  -> plugin hook execution with timeout and sandbox
  -> collect plugin result
  -> normalize side effects
  -> append audit event
```

Hook 效应等级：

| 等级 | 能力 | 默认策略 | Gate 关系 |
|---|---|---|---|
| observe | 读取事件、记录日志、生成 evidence candidate | 默认只读，可在受信任来源中启用 | 不能影响 gate |
| recommend | 生成建议、risk hint、verification hint | 需要声明输出 schema | 只能进入 evidence 或 recommendation |
| guard | 对工具、权限或文件操作提出 warn/deny 建议 | 必须通过 BitFun policy engine 解释 | 可导致 warn/degraded/deny，但不能直接写 pass/fail |
| act | 修改工具输入、触发命令、写文件或调用 custom tool | 默认关闭，需要显式信任、权限、超时和审计 | 只产出事实或证据，gate 仍由 BitFun 决策 |

项目级 trust 记录必须绑定 hook source、hash、scope、permissions、created_by 和 reviewed_by。hook 内容变化后信任状态失效，必须重新确认。

API 兼容等级：

| 等级 | 范围 | 目标 |
|---|---|---|
| L0 | 事件命名、payload mapping、只读 client log | 支持迁移和观察 |
| L1 | `tool.execute.*`、`permission.*`、`file.*`、`session.*` | 支持核心质量保护插件 |
| L2 | custom tools、SSE event stream、limited `$` shell facade | 支持可控扩展 |
| L3 | 更广泛 ecosystem compatibility | 仅在 L0-L2 稳定后评估 |

兼容矩阵：

| 能力 | P0/P1 状态 | 说明 |
|---|---|---|
| project-level plugin loading | 支持受限 | 仅加载明确启用目录和受信任文件 |
| global plugin loading | 暂不默认启用 | 避免跨项目状态串扰和权限混淆 |
| hook event mapping | 支持 L0/L1 | 以 BitFun canonical event 为事实来源 |
| custom tool | 受限支持 | 必须声明权限和输入输出 schema |
| shell facade | 受限支持 | 默认无网络、超时、审计、敏感信息 redaction |
| SSE event stream | P2 评估 | 先稳定本地事件订阅和权限模型 |

## 6. 策略与治理

- **权限优先**：插件执行前必须通过 BitFun permission model。
- **策略优先**：hook 只触发和采集，复杂判断进入 Policy Engine。
- **隔离执行**：默认禁止无约束 shell、网络和全仓读写。
- **信任优先**：项目内 hook/plugin/custom tool 必须先完成 trust review；未信任定义只能被展示和禁用，不能执行。
- **审计可追溯**：插件输入、输出、耗时、失败和副作用写入 Quality Data Plane。
- **来源可治理**：插件记录来源、版本、hash、权限声明和启用范围。
- **兼容可测试**：每个兼容等级必须有 fixture plugin 和行为测试。
- **降级可见**：插件失败不能静默影响 Gate，必须进入 warning 或 degraded。

## 7. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | canonical event bus、L0/L1 mapping、只读插件、审计 |
| P1 | custom tool 最小集、权限策略、PR Gate hook |
| P2 | SSE stream、plugin registry、签名/来源标识 |
| P3 | 更广泛 OpenCode 生态兼容和企业策略包 |

## 8. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 兼容层侵入核心模型 | 内部模块不得依赖 OpenCode payload；只能依赖 canonical event |
| 插件越权 | 文件、shell、network、secret 访问全部走 BitFun permission |
| 插件影响 Gate 结论 | 插件只能产出 evidence 或 recommendation，不能直接写 pass/fail |
| hook 顺序被误用为安全边界 | 安全策略必须在 BitFun policy layer 统一判断，不依赖第三方 hook 先后顺序 |
| 项目级主动配置供应链风险 | trust 记录绑定 hash 和权限；配置变化后必须重新确认 |
| 运行时不一致 | L0/L1 明确支持范围，不承诺完整 OpenCode runtime |
| 安全事故 | 默认禁用未知来源插件；记录来源、版本、hash 和执行结果 |
| 维护成本边界不清 | API compatibility 分级推进，每级有成功标准和退出条件 |

## 9. 成功标准

- 常用质量保护插件可以通过 adapter 迁移核心逻辑。
- BitFun 内核事件、权限和审计模型保持独立。
- 插件失败、超时、拒绝权限都能被 Gate 和 EvidencePack 感知。
- PR Gate 能消费 OpenCode 风格 hook 产生的证据，但不信任其直接结论。
- L0/L1 兼容范围清晰，未支持能力不会被误认为可用。

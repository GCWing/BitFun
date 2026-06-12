# BitFun 子模块设计：主动配置与 OpenCode 兼容层

> 上游文档：[design.md](../design.md)
> 模块角色：在 BitFun 内部 Hook/Event Bus 与安全边界之上，发现、隔离、审核并兼容 OpenCode 风格插件、hook、自定义工具和事件流。

## 1. 模块定位

OpenCode 兼容层是未来插件生态适配的参考子模块。它短期负责发现项目中的主动配置，约束 hook、plugin、自定义工具或 MCP 带来的执行风险，并把观察、建议、验证提示和证据候选映射到 BitFun 的规范事件。

BitFun 内部以规范事件、交付物、权限和策略模型为准；OpenCode API 负责降低插件迁移成本。插件可以提供观察、建议、验证提示或证据候选；安全边界、通过/失败、强制/阻断和审计事实由 BitFun 策略层写入。

P0/P1 不承诺任意社区插件无修改运行，也不把 OpenCode 插件运行时作为默认能力。默认策略是发现、展示、禁用、信任审查和内核扩展点预留；执行型自定义工具必须等到插件适配主机和隔离策略成熟后再启用。

长期目标不是复制 OpenCode 运行时，而是形成一个可承载多生态适配器的受控插件适配主机：OpenCode、Kiro hook、Claude hook 或 BitFun 原生插件都通过同一 Host ABI 返回候选效果，Rust AI Kernel 保持唯一事实源。这个目标不进入 P0/P1 的实现关键路径。

## 2. 行业参照与设计约束

| 参照 | 启发 |
|---|---|
| [OpenCode Plugins](https://opencode.ai/docs/plugins/) / [SDK](https://opencode.ai/docs/sdk/) / [Server API](https://opencode.ai/docs/server/) | plugin 上下文、hooks object、自定义工具、客户端日志、SSE 事件流是生态迁移重点 |
| [Codex Hooks](https://developers.openai.com/codex/hooks) | hook 需要信任审查、配置来源、事件范围、并发和关闭机制 |
| [Claude Code Hooks](https://code.claude.com/docs/en/hooks) | hook 需要明确阻塞/非阻塞、退出码、权限和上下文语义 |
| [Kiro Hooks](https://kiro.dev/docs/hooks/) | hook 已成为 IDE 内事件触发自动化能力，但必须和权限、策略、人工确认分离 |
| [OWASP LLM Top 10](https://owasp.org/www-project-top-10-for-large-language-model-applications/) | 插件、工具调用、数据出境和权限提升属于 LLM 应用风险面 |

设计约束：

- 兼容适配器通过映射层接入 BitFun 内部事件模型。
- 插件能力受权限、策略、脱敏和审计约束。
- 项目内 hook、plugin 配置和自定义工具默认未信任。
- 插件来源、版本、hash、权限声明和兼容等级必须可见。
- 近期优先稳定 Rust AI Kernel 的 hook、事件、打点、工具复写候选和插件效果候选契约。
- 插件适配主机作为后续生态能力由 Rust AI Kernel 拉起和监管，默认使用本地受控 IPC，不暴露公开 localhost 服务。
- 一个插件适配主机运行时可以承载多个项目执行域，但项目之间的事件、信任、权限、工具复写表和插件状态必须隔离。
- 插件适配主机只能返回候选效果，不能直接写入通过、失败、阻断、审计或权限状态。
- BitFun 不继承 OpenCode “工具默认可用”的语义；所有工具、shell、网络、文件和凭据能力仍按 BitFun 安全边界授权。
- JS/TS 运行时本身不是安全边界；如果无法限制直接 `fs`、`fetch`、`spawn` 或依赖副作用，高风险插件必须进入更强隔离。
- 多个 hook 命中同一事件时，不允许依赖隐式顺序做安全判断。
- 阻断语义必须进入 BitFun 策略层；第三方 hook 只能建议。
- 兼容承诺必须通过测试矩阵表达，不用“兼容 OpenCode”这种宽泛表述替代边界。

## 3. 范围与边界

范围：

- 发现 OpenCode 风格主动配置，并写入项目画像。
- 映射 OpenCode 常见事件到 BitFun 规范 Hook/Event Bus。
- 提供有限 plugin 上下文、客户端门面、自定义工具 API。
- 支持 SSE 事件流或本地事件订阅的受控子集。
- 支持观察/建议类插件产出证据候选或风险提示。

边界：

- BitFun 提供兼容适配器和受控子集，不复制 OpenCode 运行时。
- OpenCode 是一种适配器，不是 BitFun 内部插件模型。
- OpenCode 配置作为外部生态配置读取，BitFun 规范配置仍由自身策略模型承载。
- 运行时组件和依赖缓存可以共享；项目执行域、工具复写、权限和审计不能共享。
- 插件兼容范围通过测试矩阵表达，shell 语义和阻塞行为逐项声明。
- 门禁通过、就绪度就绪和审计事实由 BitFun 策略层写入。
- 快速路径只依赖 BitFun 原生能力；插件能力按需启用。

### 3.1 适配主机与内核边界

```text
Rust AI Kernel
  权威事件日志 / 策略决策 / 安全边界 / 工具执行 / 审计
  <本地 IPC：JSON-RPC、gRPC over pipe 或等价协议>
插件适配主机（Plugin Adapter Host）
  运行时核心 / 适配器注册表 / 项目执行域管理器 / 效果路由
    -> OpenCode Adapter
    -> Kiro Hook Adapter
    -> Claude Hook Adapter
    -> BitFun Native Adapter
```

Kernel 负责状态和副作用，插件适配主机负责插件语言运行时和生态 API 适配。适配主机崩溃、重启、超时或返回旧结果时，Kernel 必须能降级、拒绝或重放事件。插件隔离等级必须映射到 [安全边界](../architecture/security-boundary.md) 的执行与沙箱矩阵；Host 内部的 JS/TS 运行时、worker 或子进程不是自动可信边界。

项目执行域（Project Domain）包含项目根、worktree、执行主机、远程上下文、信任记录、事件订阅、工具复写表、权限范围和审计流。一个 Host 进程可以管理多个执行域，但不能让插件全局变量、订阅、复写表、环境变量或授权跨域复用。

默认只有一个插件适配主机监督进程。监督进程可以按风险启动插件单元（Plugin Cell）、worker、子进程或容器；这些都是 Host 管理的执行单元，不改变 Kernel/Host 的外部契约。

运行时隔离等级：

| 等级 | 适用 | 要求 | 对外沙箱等级 |
|---|---|---|---|
| cell | 受信任观察/建议插件 | 独立模块图、事件订阅和超时；不得访问真实 shell、网络、凭据 | `permission_only` 或 `process_isolated` |
| worker | 建议、guard、轻量自定义工具 | 独立工作线程或等价隔离；所有副作用走 Kernel facade | `process_isolated` |
| subprocess | 工具复写、act、高风险依赖 | 独立进程、资源预算、环境变量白名单、工作目录限制 | `process_isolated`，必要时叠加 `readonly_scope` 或 `network_restricted` |
| sandbox | 未知来源或企业受管场景 | 容器、无凭据环境、网络策略和只读/临时 worktree | `containerized` |

入口形态约束：

| 入口 | 约束 |
|---|---|
| Tauri 桌面 | Host 由 Rust 后端管理，不能在 WebView 内执行插件；权限确认投影到桌面 UI |
| TUI/CLI | Host 不占用标准输入输出；确认、降级和错误以文本状态呈现 |
| 远程开发 | 插件执行位置必须与文件、命令和 Git 操作的执行位置一致；本地 Host 不能直接代替远程 Host 操作远程路径 |
| 云端/异步 | Host 生命周期绑定任务或执行域；长任务必须支持取消、超时和审计续接 |

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
  project: { id: string; root: string; worktree: string };
  directory: string;
  client: OpenCodeCompatClient;
  permissions: PermissionFacade;
  events: EventFacade;
  security: SecurityBoundaryFacade;
}

type PluginEffect =
  | { kind: "suggestion"; body: unknown }
  | { kind: "evidence_candidate"; body: unknown }
  | { kind: "tool_input_patch"; tool_call_id: string; patch: unknown }
  | { kind: "tool_result"; tool_call_id: string; result: unknown }
  | { kind: "deny_request"; reason: string }
  | { kind: "log"; level: "debug" | "info" | "warn" | "error"; message: string };

interface PluginDispatchEnvelope {
  event_id: string;
  sequence: number;
  project_domain_id: string;
  session_id: string;
  tool_call_id?: string;
  correlation_id: string;
  causation_id?: string;
  deadline_ms: number;
  project_epoch: number;
  trust_epoch: number;
  tool_registry_epoch: number;
  worktree_revision?: string;
}
```

## 5. 核心流程

```text
发现主动配置
  -> 记录来源、hash、权限和范围
  -> 分类信任状态
  -> Kernel 写入规范事件
  -> Kernel 按事件类型投递到插件适配主机
  -> 适配主机选择项目执行域和生态适配器
  -> 在 deadline、权限 facade 和隔离约束下执行 plugin hook
  -> 适配主机返回 PluginEffect
  -> Kernel 校验 epoch、权限、安全策略和幂等键
  -> Kernel 应用、降级、拒绝或记录候选效果
  -> Kernel 追加审计事件
```

时序策略：

| 事件类型 | 处理方式 | 超时策略 |
|---|---|---|
| 观察/建议 | 异步，不阻塞主流程 | 记录 warning 或丢弃候选效果 |
| 执行前 hook | 同步，有 deadline | 按策略 fail-open、fail-closed 或请求用户确认 |
| 自定义工具 / 工具复写 | request/response | 超时则工具失败或降级，不写通过状态 |

适配主机响应必须携带原始 envelope 的标识。`project_epoch`、`trust_epoch`、`tool_registry_epoch` 或 `worktree_revision` 变化后，旧响应不能应用。

Hook 效应等级：

| 等级 | 能力 | 默认策略 | 就绪度/门禁关系 |
|---|---|---|---|
| observe | 读取事件、记录日志、生成证据候选 | 受限只读，可在受信任来源中启用 | 不能影响就绪/通过 |
| recommend | 生成建议、风险提示、验证提示 | 需要声明输出结构 | 只能进入建议 |
| guard | 对工具、权限或文件操作提出警告/拒绝建议 | 必须通过 BitFun 策略引擎解释 | 可导致建议投影、降级或拒绝，但不能直接写通过/失败 |
| act | 修改工具输入、触发命令、写文件或调用自定义工具 | 默认关闭，需要显式信任、权限、超时和审计 | 只产出事实或证据，决策仍由 BitFun 产生 |

项目级信任记录必须绑定 hook 来源、hash、范围、权限、创建者和审核人。hook 内容变化后信任状态失效，必须重新确认。

工具复写规则：

- 普通插件工具默认命名空间化，避免与内置工具重名。
- 内置工具复写必须显式展示被复写工具、来源、hash、权限范围、持续时间和撤销入口。
- 工具复写表按项目执行域生效，不写入全局工具表。
- 用户确认只表示启用意图；文件、shell、网络和凭据访问仍走 BitFun 安全边界。
- 复写插件变化、权限声明变化或执行主机变化后，原信任立即失效。

## 6. API 兼容等级

兼容等级用于描述未来可支持的能力边界，不等同于实施阶段。P0/P1 只落实发现、禁用、契约和空实现。

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
| 项目级插件加载 | 暂缓 | P0/P1 只发现和展示，不执行插件代码 |
| 全局插件加载 | 暂不默认启用 | 避免跨项目状态串扰和权限混淆 |
| hook 事件映射 | 契约预留 | 先定义 BitFun 规范事件和 hook 点，不运行 OpenCode hook |
| 自定义工具 | 契约预留 | 先定义权限、输入输出结构和候选效果，不执行外部工具 |
| 工具复写 | 契约预留 | 必须显式确认、按项目生效，并重新经过安全边界 |
| shell 门面 | 暂缓 | 等插件适配主机、隔离和权限模型稳定后评估 |
| SSE 事件流 | 暂缓 | 先稳定本地事件订阅和权限模型 |

## 7. 策略与治理

- **安全优先**：插件执行前必须通过安全边界。
- **权限优先**：文件、shell、网络、凭据访问全部走 BitFun 权限模型。
- **策略优先**：hook 只触发和采集，复杂判断进入策略引擎。
- **Kernel 优先**：状态、权限、工具执行和审计由 Rust Kernel 写入，适配主机只返回候选效果。
- **隔离执行**：默认禁止无约束 shell、网络和全仓读写；只有安全边界确认存在真实隔离时，才能把结果标记为沙箱允许。
- **执行域隔离**：项目执行域之间不共享工具复写、事件订阅、信任状态和插件全局状态。
- **信任优先**：项目内 hook/plugin/custom 工具必须先完成信任审查；未信任定义只能被展示和禁用。
- **审计可追溯**：插件输入、输出、耗时、失败和副作用写入质量数据面。
- **兼容可测试**：每个兼容等级必须有测试插件和行为测试。
- **降级可见**：插件失败不能静默影响任务结果，必须进入警告、降级或安全决策。

## 8. 分阶段落地

| 阶段 | 目标 |
|---|---|
| P0 | 主动配置发现、L0 映射、配置展示、禁用和审计，不执行插件 |
| P1 | Kernel 扩展点 v0：规范 hook、事件、打点、工具复写候选、插件效果候选和空实现 |
| P2 | 信任审查持久化、基于回放用例的适配测试、权限策略校准，不启用真实 OpenCode 插件运行 |
| P3 | 插件适配主机原型、只读/建议类适配器试点、项目执行域隔离验证 |
| P4 | 自定义工具、工具复写、SSE 事件流、插件注册表、签名/来源标识和企业策略包 |

## 9. 风险与反证

| 风险 | 反证或治理要求 |
|---|---|
| 兼容层侵入核心模型 | 内部模块不得依赖 OpenCode 载荷；只能依赖规范事件 |
| 插件越权 | 文件、shell、网络、凭据访问全部走 BitFun 权限 |
| 插件影响决策结论 | 插件只能产出证据或建议，不能直接写通过、失败或就绪 |
| hook 顺序被误用为安全边界 | 安全策略必须在 BitFun 策略层统一判断 |
| 项目级主动配置供应链风险 | 信任记录绑定 hash 和权限；配置变化后必须重新确认 |
| 适配主机与 Kernel 时序不一致 | envelope、deadline、epoch、幂等键和过期响应丢弃规则必须测试覆盖 |
| 工具复写越权 | 复写表按项目执行域隔离，执行仍走安全边界 |
| 运行时不一致 | L0/L1 明确支持范围，不承诺完整 OpenCode 运行时 |
| 维护成本边界不清 | API 兼容性分级推进，每级有成功标准和退出条件 |

## 10. 成功标准

- 项目主动配置能被发现、解释、禁用和重新信任。
- BitFun 内核事件、权限和审计模型保持独立。
- P0/P1 阶段即使没有插件适配主机，Kernel 扩展点空实现也不会影响默认任务。
- hook、事件、打点、工具复写候选和插件效果候选能被质量数据面记录、回放和测试。
- 工具复写契约不会跨项目生效，也不会绕过权限、沙箱、网络和凭据策略。
- 后续启用插件适配主机时，插件失败、超时、拒绝权限和旧响应必须能被安全边界和证据包感知。
- L0/L1 兼容范围清晰，未支持能力不会被误认为可用。

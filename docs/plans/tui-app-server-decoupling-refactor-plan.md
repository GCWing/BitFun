# TUI 后端解耦重构计划

> 状态：已完成当前定义。
>
> 当前状态基线：2026-08-14。一次性的运行证据保留在对应 PR/Actions 记录中；本文只保留可重复执行的边界、完成事实和验证命令。

相关文档：

- [CLI 产品线设计](../architecture/cli-product-line-design.md)
- [Agent Runtime 部署设计](../architecture/agent-runtime-deployment-design.md)
- [产品架构](../architecture/product-architecture.md)
- [App Server 架构设计](../architecture/app-server-architecture.md)

## 1. 范围与决策

本计划只重构交互式 CLI/TUI 的后端依赖：

1. TUI 保留终端输入、状态、渲染和 controller-local effect。
2. Startup 和 Chat controller 直接依赖 `CliAgentRuntimeClient`；其他管理能力由 controller 直接调用已有 owner/service API。
3. `CliAgentRuntimeClient` 只承载 Embedded 与 Shared 共同需要的 Runtime 行为；它的 Embedded backend 调用同进程 Runtime typed API，Shared backend 映射 private Runtime IPC v17。
4. Model、Skill、Subagent、MCP、External Source、Hook、Account 和 Worktree 等能力仍由对应 owner/service 持有；controller 直接调用，不新增 CLI-local service adapter、接口或 facade。
5. 不建立 catch-all TUI client、统一 TUI management 模块、额外 TUI Runtime port 或 provider 聚合对象。
6. controller 不直接依赖 Runtime 实现、全局 singleton 或私有 IPC operation；view/reducer 不执行 backend I/O。非 Runtime 的 controller-local 调用可直接使用稳定 owner/service API。
7. Remote workspace 使用 controller-local owner 时必须 fail closed，不允许静默回落到控制端本机。

App Server 有独立的协议、handler 和 Host 装配边界。CLI 使用稳定 contracts 投影 DTO，不复用 app-server-protocol wire DTO，也不依赖 App Server implementation/client。App Server parity、transport 和 Shared App Server 方案不属于本次 TUI 重构的设计约束或验收条件。

不在本计划范围内：

- 重写 Ratatui 状态机或界面布局。
- 迁移 Runtime owner 或重新设计产品领域模型。
- 改造 Web、Desktop 或 App Server transport。
- 把 clipboard、editor、terminal raw mode 等 controller-local effect 下沉到工作区 Host。
- 改变 Headless `exec`、ACP、Peer Host 或公开 SDK 的 adapter。

## 2. 当前调用路径

```text
StartupPage / ChatMode
  -> CliAgentRuntimeClient
     -> Embedded AgentRuntime typed API
     -> Shared private Runtime IPC v17
  -> existing owner/service APIs
     -> ConfigService / registries / MCPService
     -> External Source and Hook domain APIs
     -> AccountRuntime / GitService / WorktreeService
```

Runtime 与 owner/service 调用的边界按行为所有权划分，而不是按页面或部署方式划分：

- Session、Turn、Permission/UserInput、workspace reference/diff、lineage、fork、usage/settlement、当前 Session 的 mode/model 更新和 Runtime 事件订阅进入 `CliAgentRuntimeClient`。
- Model catalog/CRUD、Skill、Subagent、MCP、External Source、Hook、Account/Settings Sync 和 Worktree 进入相应 owner/service。
- Embedded 与 Shared 使用同一个 `CliAgentRuntimeClient` 类型，controller 不引用 deployment-specific IPC operation。
- Shared TUI 的本机 owner/service 只描述当前 CLI 进程实际拥有的能力；它不伪装成 Runtime IPC 或 Remote capability。

## 3. Direct owner calls

| Domain | Owner / service |
| --- | --- |
| Model | `ConfigService` |
| Skill | Skill registry |
| Subagent | Subagent registry |
| MCP | `MCPService` |
| External Source | external-source domain APIs |
| Hook | native/external Hook domain APIs |
| Account / Settings Sync | `AccountRuntime` |
| Worktree | `GitService` and core `WorktreeService` |

这些调用只使用 controller 所需的稳定 owner/service API，不构成一个新的业务层、接口层或统一服务。controller 使用稳定 contracts 层 DTO，但业务状态、策略、revision、权限、凭据和持久化仍由原 owner 持有。

每个使用 controller-local owner 的调用点直接检查 Remote workspace。检测到 remote connection 或 SSH host 时返回明确 unsupported/error；禁止读取控制端本机配置、凭据、Git 仓库或外部来源作为替代结果。

## 4. Crate 与 Ownership

| 路径 | 职责 |
| --- | --- |
| `src/apps/cli/src/agent/runtime_client.rs` | Embedded/Shared Runtime backend、typed request/result/event 映射和 Shared IPC 生命周期 |
| `src/apps/cli/src/ui/startup.rs` | Startup 状态与编排，只调用 Runtime client 和所需 owner/service API |
| `src/apps/cli/src/modes/chat*` | Chat 状态与命令编排，只调用 Runtime client 和所需 owner/service API |
| Runtime/Service/Product Domain owners | Session、Turn、Permission、Workspace、配置和其他业务权威事实 |
| `src/crates/interfaces/app-server*` | 独立的 App Server wire 与 server/client adapter；不参与 TUI composition |

边界规则：

- CLI 不依赖 `openbitfun-app-server`、`openbitfun-app-server-client` 或共享 TUI management crate。
- CLI 不得依赖 `openbitfun-app-server-protocol` 的 wire DTO；非 Runtime 投影使用 `openbitfun-core-types` / `openbitfun-product-domains` 中的稳定 DTO。
- controller 可引用 `openbitfun_core` 的稳定 owner/service API；不得引用 Runtime 实现或 `RuntimeIpcOperation`。
- `surface_services`、service object、owner adapter/facade 等封装层禁止恢复；只允许 DTO/终端投影辅助函数。
- 新的非 Runtime 能力直接调用对应 owner/service；不得重新引入总括性 TUI backend/client 或统一管理模块。
- DTO 提取不代表 Runtime owner 迁移。

## 5. 已完成阶段

| 阶段 | 完成事实 |
| --- | --- |
| Runtime 收敛 | `CliAgentRuntimeClient` 同时支持 Embedded typed API 与 Shared IPC v17，controller 不按部署复制 Runtime 分支 |
| 配置能力 | Model、Skill、Subagent 和 MCP 已由 controller 直接调用对应 owner/service，secret 不进入 read model 或日志 |
| 外部集成 | External Source、native/external Hook、Account/Settings Sync 和 Worktree 已由 controller 直接调用对应 owner/service |
| 统一层删除 | 旧 catch-all client、TUI backend/runtime wrapper 和统一 management crate 已删除 |
| Embedded direct Runtime | Embedded TUI 不创建 App Server client/server、in-memory transport、wire handshake 或额外 Runtime 线程 |
| Remote fail-closed | controller-local owner/service 调用点在 Remote workspace scope 下拒绝执行，不回落控制端本机 |

Shared Runtime IPC v17 仍是当前显式 `--shared` 的 Runtime transport。它只承载当前已定义的 Runtime operations，不因 TUI owner 直调重构增加 Model CRUD、MCP 管理、Account、Worktree 或 External Source wire operations。

## 6. 验证

### 6.1 Focused commands

```bash
pnpm run fmt:rs
cargo check -p openbitfun-cli
cargo test -p openbitfun-cli --test cli_command_contracts
cargo test -p openbitfun-cli
cargo test -p openbitfun-cli --bin openbitfun dual_backend_behavior_tests
pnpm run check:core-boundaries
git diff --check
```

App Server 仅因共享 crate 移除后需要保持 workspace 可编译而运行其自身 focused check；该结果不作为 TUI 行为 parity 证据。

### 6.2 场景证据

| 场景 | 本次要求 |
| --- | --- |
| Embedded local TUI | Runtime client 和全部 controller owner 直调可编译；聚焦 contract tests 通过 |
| Shared local TUI | Runtime client 保持 v17 backend；本机 controller owner 直调可编译并按真实 owner 可用性返回结果；Embedded SDK 与 Shared IPC 使用同一 mode catalog 行为 fixture |
| Remote workspace | controller-local owner 调用明确 fail closed，无本机 fallback |
| Remote control / Peer Device Mode / Detached Dispatch | 不改变这些入口；不能用本次 TUI 测试宣称其行为已覆盖 |

## 7. 完成定义

同时满足以下条件时，本计划完成：

1. Startup 和 Chat controller 直接依赖 `CliAgentRuntimeClient`，并直接调用所需 owner/service API。
2. 仓库中不存在 catch-all TUI client、统一 TUI management 模块或共享 management crate。
3. Runtime 行为只经 `CliAgentRuntimeClient`；非 Runtime 行为由 controller 直接调用对应 owner/service。
4. controller/view 不引用 Runtime 实现或 private IPC operation，也不重建 surface service 或 owner adapter 封装。
5. CLI 不依赖 App Server implementation/client；App Server 兼容性不进入 TUI 验收。
6. Remote workspace 不存在 controller-local fallback。
7. Embedded 不创建 App Server 或额外 transport；Shared Runtime 行为继续由 v17 backend 承载。
8. 聚焦 CLI tests、Core boundary check 和格式检查通过。

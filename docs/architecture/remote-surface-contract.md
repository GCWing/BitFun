# OpenBitFun 远程表面契约（Product Operation Registry）

本文定义 OpenBitFun 所有"远程"场景共享的产品操作契约：一个操作在哪里执行、在每种远程
场景下是否可用、需要协商什么能力。它只描述稳定所有权与运行约束；具体的行、
理由和生成投影由 `openbitfun_product_domains::remote_surface` 承载。

相关设计：[Remote workspace transport](remote-workspace-transport.md)、
[Peer Device Mode](peer-device-mode.md)、[Detached dispatch](detached-task-dispatch.md)、
[产品控制平面](product-control-plane.md)。

## 1. 问题

OpenBitFun 只有一套 Agent Runtime，但有多条到达它的路径：本机 Desktop 窗口、SSH/Docker
远程工作区、Peer Device 控制端驱动 Desktop 或 CLI 宿主、Detached Dispatch。在引入本
契约之前，"一个命令在这些场景下允许做什么"没有唯一 owner，而是分散在多份手工副本里：

| 副本 | 位置（历史） | 问题 |
|---|---|---|
| 远程工作区策略表 | `src/apps/desktop/src/api/remote_workspace_policy.rs`（2485 行） | 零运行时消费者；42% 的命令停留在 `LegacyUnaudited` |
| Peer 三份 deny list | desktop `peer_host_invoke.rs`、cli `peer_host/deny.rs`、web-ui `peer-device-adapter.ts` | 手工同步；CI 只做单向子集检查并用例外掩盖漂移 |
| CLI peer host 仿真表 | `cli/src/peer_host/commands/mod.rs` 的 `match` + `soft.rs` 空成功 | 与 deny list 无闭包关系，未知命令一律"not supported" |
| `peer_mode_ping` 能力 | 两个宿主各自手写 JSON | 列表不一致；有能力被解析但从未被消费 |

每新增一个 Tauri command 都要人肉去 N 个副本补一份；漏掉的那一份就是一个"本地正常、
远程才有"的缺陷。这是产品从 Desktop 起步、远程后补的必然结果，补丁式修复无法收敛。

## 2. 不变量

OpenBitFun 对每个可跨宿主边界的产品操作只允许一个契约 owner：**Product Operation Registry**
（`src/crates/contracts/product-domains/src/remote_surface/`）。它是 behavior-light 的
contracts 层模块，不执行任何操作，也不依赖运行时 crate。

```text
                 Product Operation Registry
                 OperationDefinition { id, surface, remote_workspace, peer, cli_peer }
                 + PeerHostCapability + advertised_by(host) + digest
                         |
        +----------------+--------------------+--------------------+
        | 编译期查表       | 编译期查表           | 生成物（JSON + TS）  |
  Desktop peer host   CLI peer host       web-ui 控制端适配器      capability 生成器
  策略闭包测试          verdict + HANDLED 闭包  PEER_CONTROLLER_LOCAL   tauri-command-map.json
```

因此以下行为是禁止的：

- 在任一宿主或前端重新引入手写的 `LOCAL_ONLY_COMMANDS` 或远程策略表；
- 新命令使用 `Unaudited` 立场；
- CLI peer host 对注册表标为 `Handled` 的命令没有处理分支，或对未声明 `SoftEmpty`
  的命令返回空成功；
- 宿主发布注册表之外的 `peer_mode_ping` 能力键，或发布 `false`；
- Remote/Peer/Detached 场景在目标能力不可用时静默回退到控制端本机。

## 3. 定义模型

| 字段 | 取值 | 含义 |
|---|---|---|
| `surface` | `TauriCommand` / `HostInvokeOnly` | 是否为 Desktop 注册的 Tauri command；后者是 CLI peer host 为旧控制端保留的别名或 dispatch target verb |
| `remote_workspace` | `Routed` / `Unsupported` / `LocalOnly` / `Agnostic` / `Unaudited` | 活动工作区是远程 SSH/Docker 工作区时的行为。序列化名保持历史策略名（`RemoteRouted` 等） |
| `peer` | `Proxied` / `ControllerLocal` / `OperatorOnly` / `HostControlPlane` / `Retired` | Peer Device Mode 立场 |
| `cli_peer` | `Handled` / `Unsupported{reason}` / `SoftEmpty{reason}` | CLI peer host 的支持情况；Desktop 支持由 `surface` 推导 |

Peer 立场语义：

- `Proxied`：控制端转发，被渲染的 peer 宿主执行。
- `ControllerLocal`：控制端本机执行（窗口、更新器、账号身份、本机 OS 自动化），所有
  peer 宿主拒绝。
- `OperatorOnly`：peer 宿主拒绝，且控制端也不得本机执行；控制端仍转发以获得显式拒绝
  （`git_trust_repository`：授信必须在拥有仓库的机器上决定，控制端展示 `manualCommand`）。
- `HostControlPlane`：宿主在 deny 检查之前应答（`peer_control_attach/detach`、
  `peer_mode_ping`、`dispatch_target_*`）。
- `Retired`：运行时 owner 已移除后的协议墓碑；`lsp_` 前缀由 `RETIRED_COMMAND_PREFIXES` 覆盖。

宿主裁决 `peer_host_verdict(command, host)` 的顺序固定为：空 → retired → control plane →
controller-owned → 宿主不支持 → 执行 → 未知命令。所有线上错误文案只由
`PeerRefusal::message` 产生：

| 拒绝 | 文案（保持兼容） |
|---|---|
| ControllerLocal / OperatorOnly | `command '<c>' is local-only and cannot run on peer` |
| Retired | `command '<c>' is unsupported because <reason>` |
| HostUnsupported | `command '<c>' is not supported on <CLI|desktop> peer host: <reason>` |
| UnknownToHost | `command '<c>' is unknown to this OpenBitFun <host> peer host version; …` |

前端 `LOCAL_ONLY` 集合 = `{ControllerLocal, HostControlPlane} ∩ TauriCommand`，由生成物
`src/web-ui/src/infrastructure/api/generated/remoteSurface.ts` 提供；宿主拒绝集合 =
`{ControllerLocal, OperatorOnly}`。

## 4. 能力协商

`PeerHostCapability` 是类型化的能力 id 列表；`advertised_by(host)` 给出每种宿主发布的
子集（Desktop 全部、CLI 无 MiniApp/host-native/presentation 三项）。两个宿主的
`peer_mode_ping` 都从 `capability_map(host)` 生成，只发布 `true` 键：缺失键表示"旧宿主"，
由控制端按 `host_type` 解析。生成物携带 `PEER_HOST_CAPABILITY_IDS` 与
`PEER_HOST_ADVERTISED_CAPABILITIES`，控制端不可能探测一个没有宿主会发布的键。
`peer_mode_ping` 额外附带 `surface_registry_digest`（加法字段），用于发现宿主与控制端
的注册表版本不一致。

## 5. 防腐门禁

CI 必须结构性证明以下闭包，而不是依赖人工维护的计数：

1. Desktop `generate_handler!` 注册集合 ≡ 注册表 `TauriCommand` 行集合（双向；
   `remote_workspace_policy.rs` 闭包测试 + 生成器在 ubuntu 侧再次断言）。
2. `Unaudited` 与 `SoftEmpty` 两个 baseline 双向相等：只能减少，不能增加，也不能保留已毕业项。
3. CLI peer host：`cli_peer == Handled` 行集合 ≡ `commands::dispatch` 的实际 match 分支；
   `SoftEmpty` 行集合 ≡ `soft::` 分支（`commands/mod.rs` 闭包测试）。
4. `dispatch_target_*` 家族在 desktop `dispatch_host.rs` 与 cli `dispatch.rs` 的 verb 表
   与注册表一致。
5. 生成物（注册表 JSON、`remoteSurface.ts`、`tauri-command-map.json` 的
   `remoteWorkspacePolicy`）digest 与编译期注册表一致：`pnpm run capabilities:check`。
6. 已迁移的三个 surface 不得再出现手写 `LOCAL_ONLY_COMMANDS`：`pnpm run check:core-boundaries`
   （`scripts/core-boundaries/peer-command-policy.mjs`）。
7. `cargo test -p openbitfun-product-domains --no-default-features remote_surface` 在每个 CI
   矩阵目标上运行（`scripts/check-github-config.mjs` 固定该步骤）。

## 6. 新增或修改一个命令

1. 在 Desktop 注册 Tauri command。
2. 在 `remote_surface/table.rs` 按 id 排序位置加一行 `op(...)`，为每一列给出真实立场；
   `Unaudited` 不可用于新行。
3. 若 CLI peer host 需要执行它：在 `cli/src/peer_host/commands/` 加处理分支并把
   `cli_peer` 设为 `HANDLED`；若 CLI 明确不支持：给出 reason 常量；若必须返回空成功：
   声明 `SoftEmpty{reason}` 并加入 `SOFT_EMPTY_BASELINE`（需评审）。
4. 运行 `pnpm run capabilities:generate` 更新生成物；前端 `LOCAL_ONLY` 自动派生。
5. 若新增 peer 能力：在 `capabilities.rs` 加枚举变体并更新 `advertised_by`；控制端读
   生成的 `PeerHostCapabilityId`。

审计一个 `Unaudited` 命令：读 handler → 修正远程行为（能路由则接入既有远程 service，
不能则显式 `Unsupported` 错误）→ 把行改为真实立场 → 从 `UNAUDITED_BASELINE` 删除。

## 7. 静默回退禁令的落点

注册表回答"允许做什么"，运行时仍必须在"能力不可用"时 fail closed。本轮同步收口的
静默回退点：`remote_ssh_compat::is_remote_path`（feature 未编译时不再把远程路径当本地）、
`filesystem/service.rs` 的 remote-hint 入口、review platform classifier、
`coordinator::build_workspace_services`（provider 缺失是错误而不是 `None`）、
`native_hooks`（远程工作区跳过 hook 必须用户可见）。原则：持有远程标记的请求在远程
provider 不可用时返回类型化错误，绝不获取本地 provider。

## 8. 后续阶段

| 阶段 | 内容 | 入口 |
|---|---|---|
| P2 执行域类型化 | `ExecutionDomain` 作为唯一"是否远程"判定，替换 18 个 Rust 谓词与 5 个 TS 谓词；`WorkspacePath`（POSIX）newtype；`path_target.rs` / `terminal_api.rs` 表驱动；CLI ad-hoc 守卫改查表 | `assembly/core/src/agentic/workspace.rs`、`services-core/src/workspace_identity.rs`、`web-ui/src/shared/utils/remoteSessionScope.ts` |
| P3 Remote Control 契约 | `RemoteCommand`/`RemoteResponse` schema 导出与 codegen，全变体 fixtures 由 mobile-web/HarmonyOS/Kotlin 测试消费；协议版本与能力字段；`InteractionKind` 注册表，远程客户端在握手声明可答种类；mobile-web `confirm_tool`/`reject_tool`；bot 对权限请求显式不支持而不是挂起 | `services-integrations/src/remote_connect.rs`、`src/shared/relay-protocol-contract-fixtures/` |
| P4 投影合同推广 | Remote Connect poll 改为 `(streamId, cursor)` 位置化增量，复用 `SessionEventJournal`；bot 消费 journal 而非 broadcast | `docs/architecture/session-projection.md` |
| P5 Dispatch | device dispatch 解绑 `ssh-remote` feature；capability 校验器下沉 | `assembly/core/src/service/dispatch/` |

# 宿主平台、Tauri 与远程工作区

> 根 `AGENTS.md`（STD-05 / STD-06 相关宿主规则）配套文档。
> 改桌面命令、UI↔宿主边界、远程场景或升级兼容时打开本文。
>
> [English](host-platform-and-remote.md)

## Tauri commands

- 命令名使用 `snake_case`。
- TypeScript 可用 `camelCase` 包装，但调用 Rust 时必须传入结构化 `request`：

```rust
#[tauri::command]
pub async fn your_command(
    state: State<'_, AppState>,
    request: YourRequest,
) -> Result<YourResponse, String>
```

```ts
await api.invoke('your_command', { request: { ... } });
```

桌面宿主范围另见 [`src/apps/desktop/AGENTS.md`](../../src/apps/desktop/AGENTS.md)。

## 平台边界

- 不要从 UI 组件直接调用 Tauri API；走 adapter / infrastructure 层。
- 仅桌面宿主适配放在 `src/apps/desktop`，再经类型化能力接口流转；需要事件投递时走生产 transport adapter。
- 共享 core 中避免 `tauri::AppHandle` 等宿主专属 API；使用 `bitfun_events::EventEmitter` 等共享抽象。

## 远程场景

BitFun 不是仅本机桌面应用。工作区、执行 turn 的 runtime、以及驱动它的人，可以分别位于不同机器。
下列四种场景是每次改动的一等目标，而不是“以后再移植”。

| 场景 | 含义 | 设计入口 |
|---|---|---|
| 远程工作区 | 当前工作区位于 SSH 主机、跳板机链路或 Docker 容器；文件、终端、搜索和 Agent 子进程都必须在那一侧执行 | [remote-workspace-transport.md](../architecture/remote-workspace-transport.md)、[remote-workspaces.md](../specs/remote-workspaces.md) |
| 远程控制 | Mobile web，或飞书 / Telegram / 微信 Bot，经 Remote Connect relay 驱动 Desktop 或 CLI 宿主上的会话 | [`src/mobile-web`](../../src/mobile-web/AGENTS.md)、[services-integrations](../../src/crates/services/services-integrations/AGENTS.md) 中的 `remote_connect`、[relay-service](../../src/crates/services/relay-service/AGENTS.md) |
| Peer Device Mode | 同账号另一台设备成为数据面：控制器壳保留本地，invoke 与事件来自 peer | [peer-device-mode.md](../architecture/peer-device-mode.md)、[peer-device README](../../src/web-ui/src/infrastructure/peer-device/README.md) |
| Detached Dispatch | 控制器向另一台 BitFun 宿主提交持久任务后可断开；目标侧拥有 job、session、worktree、事件日志与权限邮箱 | [detached-task-dispatch.md](../architecture/detached-task-dispatch.md) |

四条通用规则：

- 功能设计必须同时覆盖远程路径。假定 UI、进程与文件系统同机的能力是不完整的，而不是“第一阶段”。
- 大声降级。场景无法支持时，门禁入口或返回明确 unsupported；静默回本机、假成功、空载荷与泛化错误都是回归；本机回退还会把本地内容泄漏给远程控制器。
- 阻塞交互必须可远程应答。新的权限提示、对话框与选择器须经既有 dialog / permission-mailbox 编排到达驱动面；只有桌面窗口能解除阻塞会卡死远程控制与 dispatch。
- 能在断连后存活。远程面会重连、按 cursor 重放并 re-hydrate，因此优先可恢复 cursor 与幂等变更，而不是只在客户端连着时存在的状态。
- 远程工作区路径在任何客户端 OS 上都是 POSIX。不要用宿主 `std::path` 语义拆接路径，也不要把控制器侧路径复用到 peer 宿主。

分场景义务：

- **远程工作区**：每个桌面 Tauri 命令须在
  [`remote_workspace_policy.rs`](../../src/apps/desktop/src/api/remote_workspace_policy.rs)
  声明策略。那里的契约测试会拒绝缺少显式策略的新命令，并禁止扩大 `LegacyUnaudited` 积压。
- **远程控制**：mobile web 与 IM Bot 经 `RemoteCommand` 协议与 bot router/menu 到达会话，而不是 Web UI。
  新增或移动会话级能力（工作区/助手选择、会话生命周期、模式、模型、审批、附件）时，扩展这些面或明确返回 unsupported。
- **Peer Device Mode**：产品命令默认代理到 peer。必须留在控制器的命令（窗口装饰、更新器、账号身份、本机 OS 自动化）须在三份同步名单中拒绝：
  [`peer_host_invoke.rs`](../../src/apps/desktop/src/api/peer_host_invoke.rs)、
  [`deny.rs`](../../src/apps/cli/src/peer_host/deny.rs)、
  [`peer-device-adapter.ts`](../../src/web-ui/src/infrastructure/api/adapters/peer-device-adapter.ts)。
  改 session / account / hydrate 路径前先读 peer-device README invariants。
- **Detached Dispatch**：任务在目标侧以 CLI delivery profile 无头运行，无交互宿主、不保证控制器在线。
  控制器是观察者，不是 runtime 或文件系统代理。不要加入依赖在线提交者的行为；把 dispatch 协议版本与目标侧所需能力当作兼容契约——新的目标侧要求需要协商 capability，而不是假定。

写明本次改动验证了哪些远程场景。仅本机测试不是远程行为的证据。

## 升级兼容

用户就地升级，且上述远程场景常把两个不同 BitFun 版本连在同一连接上。每次改动都必须让既有安装无需手工修复仍可工作。

- **持久化形状会被新旧代码同时读取。** 配置、设置、会话、连接档案、worktree 与 dispatch 记录：新增字段要有默认值，反序列化保持宽容，不得改写或收窄磁盘上已有字段含义。旧数据无法提供的字段不得变成必填。
- **不得为了从无法解析的状态恢复而删除或重置用户数据。** 保留记录、降级功能、给出清晰状态。缺凭证、档案不可读、超时或宿主离线，都不是丢弃会话、工作区或连接的理由。破坏性删除只能是用户显式动作。
- **跨版本边界要协商，不能假定。** Peer HostInvoke、dispatch 协议、relay/mobile web 与 IM Bot 都在与你无法控制的构建对话。先宣告并检查 capability——包版本相等不是行为证据——并让旧侧仍有可工作路径，而不是直接失败。
- **重命名就是迁移。** 在仍有受支持 peer 可能发送旧名/旧 id/旧形状前，继续可读旧值，并同步迁移 vault 条目、工作区指针等被引用数据。
- **用测试证明。** 覆盖旧数据反序列化与旧载荷往返，而不只是当前代码写出的新形状。只测当前代码写出的数据不算升级覆盖。

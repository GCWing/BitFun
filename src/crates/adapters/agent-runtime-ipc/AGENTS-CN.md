**中文** | [English](AGENTS.md)

# Agent Runtime IPC 迁移指南

范围：`src/crates/adapters/agent-runtime-ipc`。

该 crate 是旧 Shared TUI 私有协议，只用于迁移，不是目标架构边界。最终架构中，交互式
TUI、Desktop、Electron、VS Code 与 Web 的 Embedded/Shared 部署统一使用 App Server
wire，但 Shared 实例按 `client_kind` 隔离，这些 Client 形态不连接同一个实例。修改前阅读
[`docs/architecture/app-server-architecture-design.md`](../../../../docs/architecture/app-server-architecture-design.md)
和
[`docs/architecture/agent-runtime-deployment-design.md`](../../../../docs/architecture/agent-runtime-deployment-design.md)。

## 迁移规则

- 禁止向旧协议新增 operation、consumer、公开导出、transport 或产品能力。
- App Server 纵向切片完成前，只允许为兼容和回归修复保持当前行为。
- 删除旧实现前，将 controller lease、认证、frame 上限、断连取消、事件失效、
  `outcome_unknown` 和 cleanup 提升为 App Server conformance tests。
- 只有真正与协议无关的 Named Pipe/UDS、discovery、framing 或 budget 原语可以迁入 App
  Server transport adapter；不得上移 TUI-specific operation envelope 或 handler。
- 交互式 TUI 必须迁移到 `AppServerClient`：默认使用独占 Embedded App Server，
  `--shared` 连接 TUI-only Shared App Server。
- 旧 consumer 删除后，删除本 crate；或将其缩减为 App Server 内部 transport 实现，不再
  拥有独立 wire 语义。

## 当前生产合同

在最后一个 Shared TUI consumer 迁移并满足删除门槛前，该协议虽不是目标边界，仍是生产合同：

- 唯一 consumer 是 `src/apps/cli` 中的第一方交互式 TUI adapter；GUI、Remote、Peer、ACP、
  Headless CLI 和 SDK Host 都不是 consumer。
- 只使用本机 Windows Named Pipe 或 Unix Domain Socket。Windows 当前拒绝远程 Client 并要求 bearer
  握手，但代码尚未显式安装仅 owner 可访问的 pipe DACL，也未校验 peer SID；完成实现和跨用户拒绝测试前
  不得宣称同用户隔离。Unix 保持仅 owner 可访问的 discovery/socket 权限。禁止 TCP、HTTP、WebSocket、
  浏览器访问或远程 fallback；本机 transport 加认证不是沙箱。
- 第一帧必须是 initialize，并分离握手/request deadline。初始化校验协议版本、instance identity、
  bearer token、client ID 和 client version；token 与 discovery secret 必须脱敏。错误 token、错误
  instance、版本不匹配、初始化前请求和未认证连接耗尽仍是拒绝测试。
- 拒绝未知字段与 operation。请求 frame 上限为 128 KiB，响应/事件 frame 上限为 8 MiB；连接、
  队列、pending request 和序列化缓冲必须有界并实施背压。
- 封闭 operation 范围为 Health、Session list/create/restore（含 transcript）、当前 Session rename
  与 Agent mode/model update、Turn submit/cancel、pending/respond Permission 和 UserInput answers。
  禁止加入 delete、fork、replay、Observer、controller transfer、Tool/MCP/Hook 管理或产品配置。
- 保持每个 Session 一个 Controller、每个连接一个 active Turn，以及断连取消、`outcome_unknown`、
  事件流粘性失效、30 秒空闲退出和 owner-checked discovery cleanup。
- 旧的进程内调用方与 Headless Direct Runtime 调用方继续强类型直调 Agent Runtime，不初始化本 transport。
  Shared frame 写出前只编码一次；吞吐优化不得放宽严格解码、边界或背压。

只有等价 App Server 限制具备测量依据、conformance/overload 测试和明确兼容性决策后，迁移才可调整
某个限制；在此之前原值保持不变。

## 验证

```bash
cargo test -p bitfun-agent-runtime-ipc
node scripts/check-core-boundaries.mjs
```

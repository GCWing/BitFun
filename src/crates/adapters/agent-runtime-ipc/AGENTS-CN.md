**中文** | [English](AGENTS.md)

# Agent Runtime IPC 迁移指南

范围：`src/crates/adapters/agent-runtime-ipc`。

该 crate 是旧 Shared TUI 私有协议，只用于迁移，不是目标架构边界。最终架构中，交互式
TUI、Desktop、Electron、VS Code 与 Web 的 Per-Client/Shared 部署统一使用 App Server
wire。修改前阅读
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
- 交互式 TUI 必须迁移到 `AppServerClient`：默认连接 Per-Client Managed App Server，
  `--shared` 连接 Shared App Server。
- 旧 consumer 删除后，删除本 crate；或将其缩减为 App Server 内部 transport 实现，不再
  拥有独立 wire 语义。

## 临时安全合同

删除前不得削弱 initialize-first 认证、request/event 大小上限、有界连接与队列、每
Session 一个 Controller、断连取消、事件流粘性失效、idle cleanup 和 owner-checked
discovery cleanup。

## 验证

```bash
cargo test -p bitfun-agent-runtime-ipc
node scripts/check-core-boundaries.mjs
```

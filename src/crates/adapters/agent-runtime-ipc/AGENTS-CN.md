**中文** | [English](AGENTS.md)

# Agent Runtime IPC

范围：`src/crates/adapters/agent-runtime-ipc`。

该 crate 不发布，是第一方 Shared TUI adapter 使用的私有本机协议。它提供 discovery、单实例锁、有界 framing、认证初始化、封闭的交互操作集、Session controller lease、事件传递、连接上限和 cleanup；它不是公开 SDK、远程协议、service layer 或 Runtime owner。

## 预集成约束

- 唯一 consumer 是 `src/apps/cli` 中的第一方交互式 TUI adapter；不自动包含 GUI、Remote、Peer、ACP、Headless CLI 或 SDK Host。
- 稳定测试合同包括本机 endpoint、严格 initialize-first、分离的握手/请求 deadline、128 KiB 请求与 8 MiB 响应/事件上限、有界连接、每个 Session 一个 controller、断线取消、30 秒空闲退出和 owner-checked discovery cleanup。
- consumer 必须复用既有 Agent Runtime owners，并证明 Embedded/Shared 行为等价，不能依赖 SDK Host。

## 边界

- 只导出 CLI adapter 实际使用的 workspace-private API，且 crate 不得发布，也不得把 wire 作为 SDK 合同。
- 封闭 operation 范围为 Health、Session list/create/restore（restore 结果包含 transcript）、当前 Session 的 Agent mode/model update、Turn submit/cancel、pending/respond Permission 和 UserInput answers。断连 cleanup 属于内部生命周期，不是 detach operation。模型目录和默认值仍是 wire 之外的产品配置；禁止顺带加入 delete、fork、replay、observer、controller transfer、Tool/MCP/Hook 管理或其他产品配置。
- 可以复用稳定 Event、Product Domain 和 Runtime Port DTO。禁止依赖 `bitfun-core`、Agent Runtime 实现、SDK Host、services、Tauri、terminal、tool runtime 或远程 transport。
- 只使用 Windows Named Pipe 或 Unix Domain Socket；禁止 TCP、HTTP、WebSocket、浏览器访问或远程 fallback。
- 这是本机同用户隔离，不是沙箱。未来产品 composition 必须提供当前用户私有 runtime 目录。

## 验证

```bash
cargo test -p bitfun-agent-runtime-ipc
node scripts/check-core-boundaries.mjs
```

**中文** | [English](AGENTS.md)

# Agent Runtime IPC

范围：`src/crates/adapters/agent-runtime-ipc`。

该 crate 不发布，是未来第一方 Shared Agent Runtime adapter 的私有预集成边界。当前只验证 discovery、单实例锁、有界 framing、认证初始化、Health、连接上限和 cleanup；它不是公开 SDK 或 Runtime owner，也没有生产 consumer。

## 预集成约束

- 首个候选 consumer 仅为另行评审的第一方交互式 TUI attach adapter；不自动包含 GUI、Remote、Headless CLI 或 SDK Host。
- 稳定测试合同只有本机 endpoint、initialize-first、64 KiB frame、Health、连接上限和 owner-checked discovery cleanup。
- consumer 必须复用既有 Agent Runtime owners，并证明 Embedded/Shared 行为等价，不能依赖 SDK Host。
- 若首个 consumer 选择其他 transport，或 Shared 在产品接入前取消，删除本 crate。

## 边界

- 首个生产 consumer 证明准确 API 前，所有 Rust item 保持 crate 内可见，且 crate 不得发布。
- Health 是唯一 operation。禁止增加 Session、Turn、Tool、MCP、Permission、UserInput、Hook、event replay、controller lease 或产品配置。
- 禁止依赖 `bitfun-core`、Agent Runtime、SDK Host、services、CLI/TUI、Tauri、product domains、terminal、tool runtime 或远程 transport。
- 只使用 Windows Named Pipe 或 Unix Domain Socket；禁止 TCP、HTTP、WebSocket、浏览器访问或远程 fallback。
- 这是本机同用户隔离，不是沙箱。未来产品 composition 必须提供当前用户私有 runtime 目录。

## 验证

```bash
cargo test -p bitfun-agent-runtime-ipc
node scripts/check-core-boundaries.mjs
```

# bitfun-sdk-host

**路径**: src/crates/interfaces/sdk-host
**描述**: Versioned local SDK Host adapter for the BitFun Agent Runtime。版本化的本地 SDK Host 适配器。

## 模块

- `host` — SDK Host 连接生命周期
- `protocol` — SDK Host 协议（编解码）

## 功能

SDK Host 适配器 crate。提供 SDK Host 的连接生命周期管理和协议编解码。只负责协议和连接生命周期，agent 执行、Session 持久化、Tool/MCP、权限和 Hook 行为仍由 `bitfun_agent_runtime` 提供。依赖 agent-runtime、core-types、events、runtime-ports 实现协议映射。

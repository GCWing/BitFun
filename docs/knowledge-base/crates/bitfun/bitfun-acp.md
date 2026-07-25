# bitfun-acp

**路径**: src/crates/interfaces/acp
**描述**: BitFun Agent Client Protocol surface。ACP 外部服务器表面，映射到 BitFun 核心 agentic 运行时。

## 模块

- `client` — ACP 客户端服务
- `runtime` — BitFun ACP 运行时适配
- `server` — ACP 服务器

## 核心类型

- `AcpServer` — ACP 服务器
- `AcpClientService` — ACP 客户端服务
- `BitfunAcpRuntime` — BitFun ACP 运行时适配
- `agent_client_protocol` — 重新导出外部 ACP 协议 crate（`pub use agent_client_protocol as protocol`）

## 功能

ACP（Agent Client Protocol）集成 crate。实现 ACP 服务器和客户端，将 BitFun 的 agentic 运行时暴露为 ACP 表面。CLI 和其他 host 应启动此 crate。依赖 bitfun-core 的 product-full feature，通过 agent-runtime 和 tool-contracts 适配 ACP 协议。是 BitFun 对外暴露标准 agent 接口的入口。

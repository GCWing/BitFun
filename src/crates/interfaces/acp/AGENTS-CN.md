**中文** | [English](AGENTS.md)

# ACP 协议入口指南

适用范围：`src/crates/interfaces/acp`。

`bitfun-acp` 负责基于已组装产品 runtime 的 Agent Client Protocol 入口与 ACP client 行为。ACP protocol / client 细节留在这里或应用入口 adapter 中；跨层只共享稳定 capability facts。

CLI 托管的 ACP 服务端已通过 `ProductAssembler` 消费 `DeliveryProfile::Acp`，并使用 Agent Runtime SDK
完成会话创建/列举、活动会话模型/模式更新、轮次提交/取消、交互响应和 Agent 事件订阅。`bitfun-acp` 仍直接依赖
`bitfun-core/product-full`，用于一次性恢复完整持久化历史、模型/模式目录与提供方配置读取、MCP 配置，
以及本 crate 的 ACP 客户端路径。在这些生产路径分别获得可移植替代并证明等价前，不得宣称整个 crate
已与 Core 解耦。

## 护栏

- Remote ACP workspace 复用本地 ACP client 配置。修改 ACP client 行为时，必须保持 manager、remote shell probing、remote capability store 和 workspace menu availability 语义。
- ACP config persistence、remote probing、timeout policy 和 workspace surface selection 属于 ACP / app-surface 行为，不要移动到 `core-types`、`runtime-ports` 或 `agent-tools`。
- ACP external-agent tool 的命名、schema、validation、presentation 和 result shape 属于 `bitfun-agent-tools` 的 portable contract；ACP 应调用这些 helper，不要在本层重复定义。
- ACP 标准输入输出、连接管理和协议通知投影留在本 crate。共享运行时事实可以经过 SDK 边界；ACP 协议请求、客户端选择和生命周期状态不得进入 SDK。
- 如果未来需要 contract，只表达观测事实：environment identity、capability facts、request / response DTO。

## 验证

```bash
cargo check -p bitfun-acp
cargo test -p bitfun-acp
```

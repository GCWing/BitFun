**中文** | [English](AGENTS.md)

# App Server 接口族指南

适用范围：本指南适用于相邻的 `app-server-protocol`、`app-server-client`、`app-server`
三个 crate 及 App Server 生产接线。除非另有说明，服务端专属规则只约束 `app-server`。

App Server 接口由四个 owner 分工：

| Owner | 职责 |
|---|---|
| `app-server-protocol` | method、wire DTO、wire error、事件 envelope 和 schema-free protocol role |
| `app-server-client` | 类型化请求、类型化事件、连接行为和由 Host 提供的 transport 抽象 |
| `app-server` | server 生命周期、生产 handler 注册、事件转发、Runtime/domain 到 wire 的转换和错误映射 |
| `src/apps/*` 下的产品 Host | 具体 transport、认证、连接作用域、capability/limit 构造、平台能力、进程监督和关闭流程 |

不要在 `openbitfun-app-server` 中新增 protocol 或 client 所有权。消费者迁移期间可以保留 compatibility
module 和 re-export，但新 method、DTO、wire error 和类型化 client 行为必须放入相邻的
protocol/client crate。

`openbitfun-app-server/ts` 只保留为兼容转发 feature。protocol crate 是唯一的 TypeScript
schema 导出 owner；不要把 `ts-rs`、Runtime 实现类型或第二条导出命令重新加入本 crate。
protocol wire DTO 与 serde 合同不启用 feature 也必须可用。protocol crate 默认的 `rpc`
feature 负责绑定 ACP JSON-RPC trait 并暴露 role/transport helper，以保持兼容；`ts` 必须与
它正交，不得启用 `rpc` 或 `agent-client-protocol`。

## 护栏

- compatibility role 和 transport helper 必须保持 schema-free。不要在 `AppServer` /
  `AppClient`、流方向 helper 或 in-memory transport constructor 中硬编码领域 method 或业务行为。
- `AppServer` / `AppClient` 是自定义协议对等角色；不要复用 ACP 内置的 `Agent` /
  `Client` role，并保留协议要求的逐 role `HasPeer` 实现。
- Transport constructor 必须固定 `ByteStreams::new(outgoing, incoming)` 的方向；不要暴露
  容易交换方向的 API。具体 transport 由 Host 选择并持有。
- 只注册由真实 Runtime、Service 或 Product Domain owner 支持的生产 handler。Handler 负责
  wire 合同校验和类型转换，不能持有第二份 Session、Permission、Config、capability 或生命周期状态。
- 本 crate 只能选择已注册 handler 实际需要的最窄 `openbitfun-core` owner feature；禁止使用
  `openbitfun-core/product-full`。新增 owner feature 时必须同时增加对应的边界验证。
- Host 特定的认证、身份、workspace/execution scope、capability availability、transport
  limits、平台 provider、进程生命周期和连接 fan-out 留在 Host。不要从通用 server 默认值或
  全局环境推断这些事实。
- Handler 必须把 Runtime 调用卸载到异步任务或立即返回。不要在 handler callback 中调用
  `SentRequest::block_task`；通过 `responder.respond_with_result` 回复。

## 事件投递

Runtime 事件通过 App Server connection 交付，不属于 client 侧 Host 订阅：

- Server 接收与同一 Runtime owner 关联的注入式 `AgentEventSource`，并通过 connection 转发
  类型化 Agent、Permission、Config 和 stream-state notification。
- 类型化 client crate 接收并扇出这些 notification。Host 不得让 App Server client 直接订阅
  Core `EventQueue`，否则会形成协议旁路。
- connection-local sequence/cursor 和 sync 行为必须显式保留。在跨连接持久化 replay/resume
  owner 与合同真正实现前，不得把当前能力描述为跨连接重放或恢复。

## 错误映射

在本 server adapter 中把 Runtime/domain failure 映射到 protocol-owned wire error。保持稳定
kind 和结构化 data，不泄露 Runtime 内部细节。Host transport/auth/scope failure 仍由 Host
负责；owner failure 使用 `OpenBitFunAppRuntime::runtime_error`、`session_runtime_error` 等 helper。

## 验证

```bash
cargo check --locked -p openbitfun-app-server --offline
cargo test --locked -p openbitfun-app-server --offline --lib server::wire::tests
cargo test --locked -p openbitfun-app-server-protocol --offline --test legacy_wire_contracts
pnpm run check:core-boundaries
```

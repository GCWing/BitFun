**中文** | [English](AGENTS.md)

# App-Server 协议接口指南

适用范围：本指南适用于 `src/crates/interfaces/app-server`。

`bitfun-app-server` 负责基于 `agent_client_protocol` 自定义角色的协议无关
JSON-RPC server/client 脚手架。role/transport 层不绑定 schema；使用者自行注册
`JsonRpcRequest` / `JsonRpcNotification` 类型。可选的 `agent` / `schema` /
`server` 模块是 Phase 2 接线，通过 host 注入的 `AgentRuntime` 和通用 `AppServer`
角色暴露一组 agent kernel 操作（与 `bitfun-acp` 使用内置 ACP `Agent` 角色不同）。

## 护栏

- role/transport/transport-helper 层保持 schema 无关。不要在 role/transport
  helper 中硬编码领域方法或业务逻辑。Phase 2 的 `schema` 模块是 agent kernel
  JSON-RPC 消息的唯一存放处，且只能映射到 `bitfun_agent_runtime` SDK 类型，不能
  发明新的 kernel 行为。
- `AppServer` / `AppClient` 是通用对等体；不要在此复用内置 ACP `Agent` /
  `Client` 角色。`HasPeer` 按角色自身实现，因为
  `ConnectionTo::send_request` 要求 `Counterpart: HasPeer<Counterpart>`。
- `client` 模块（`AppServerClient`、`FrontendEvent`、`connect`）是
  **传输无关**的 app-server client：它驱动 host 提供的 transport 上的
  `AppClient`，并通过 broadcast channel 扇出投影后的 `agent/event` 通知。它是
  `BitfunAppServer::serve` 的对等体，后者同样接受 host 提供的 transport。Host 选择
  transport（内存 pair、stdio、websocket、...）并拥有 server 半连接；`connect` 只
  拥有一个连接的 client 半连接。不要在此添加构造 server 的 `spawn` — server 构造是
  host 的职责。Host 特定的扇出、字段归一化和 JSON-RPC error-code 映射属于 host，
  不属于这里。添加新 host 的方式：依赖本 crate，在 transport 的 server 半连接上
  serve `BitfunAppServer`，并在 client 半连接上调用
  `bitfun_app_server::client::connect`。
- Transport 构造器必须固定 `ByteStreams::new(outgoing, incoming)` 方向；不要暴露
  易出错的 swap API。
- 具体 schema 只覆盖已经交付且有消费者的能力。当前包括 Agent Runtime 的
  Session/Turn/Permission 操作与事件投递，以及 Web client 已使用的 git/config/i18n
  host-service 方法。Core 依赖必须保持精确的 `agent-runtime` feature 闭包。新增
  host-service 家族必须先明确 schema owner 并复审 feature 边界；不得恢复
  `product-full`，也不得把尚未实现的后端超集描述为当前能力。
- Handler 将 runtime 调用卸载到后台任务或立即返回；不要在 handler 回调内调用
  `SentRequest::block_task`（`jsonrpc.rs` 中的上游 `DEADLOCK` 注释）。通过
  `responder.respond_with_result` 回复。

## 事件投递

Runtime 事件属于 app-server 协议接口，而非 host 侧订阅。流程在 transport 上是
单向的：

- **Server** 持有注入的 `AgentEventSource`（由 host coordinator 发布的同一
  `EventQueue` 构建），其 `serve` main_fn 排空它，将每个 `AgenticEventEnvelope`
  作为 `agent/event` 通知（`SessionEventNotification`）通过 channel transport 转发
  给 client。
- **Client** 注册 `on_receive_notification(SessionEventNotification)` 接收它们，然后
  投影并扇出给自己的消费者（websocket 连接、Tauri event bridge、...）。
- Host 不得从 client 侧订阅 runtime `EventQueue`。Client 不触碰
  `AgentRuntime::subscribe_events` 或 `EventQueue`；这样做会绕过 app-server 接口并
  破坏"所有 agent 接口经过 app-server"的契约。

## 验证（续）

在边界将 `RuntimeError` 映射为 JSON-RPC `Error`（见
`BitfunAppRuntime::runtime_error` / `session_runtime_error`）；不要通过 wire 泄露
runtime 内部细节。

## 验证

```bash
cargo check -p bitfun-app-server --offline
cargo test -p bitfun-app-server --offline
```

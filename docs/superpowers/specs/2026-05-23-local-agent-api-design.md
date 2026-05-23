# BitFun 本机 Agent API 设计

## 背景

BitFun 已经有会话、调度、队列和 turn 完成结果的核心能力：

- `ConversationCoordinator` 负责创建、恢复、执行和管理会话。
- `DialogScheduler` 负责按 session 投递消息、排队、抢占和派发。
- `TurnOutcome` 已经表达 `completed`、`failed`、`cancelled` 以及最终文本。
- `SessionMessage` 已经验证了“向另一个会话发送任务，并在完成后拿到结果”的产品语义。

这次改造的目标是提供一个本机 HTTP API，让 Codex 或其他本机程序可以向 BitFun 指定 `session_id` 或 `session_name` 投递任务，并同步等待执行结果；如果等待超时，调用方可以通过 `turn_id` 查询后续结果。

## 目标

第一版交付以下能力：

1. BitFun Desktop 启动一个只监听 `127.0.0.1` 的本机 HTTP API 服务。
2. API 使用 Bearer token 鉴权。
3. 调用方可以通过 `sessionId` 或 `sessionName` 定位目标会话。
4. `POST /api/local-agent/tasks:run` 提交任务，并默认等待该 turn 完成。
5. 请求超时后不取消 BitFun 任务，只返回 `running` 和 `turnId`。
6. `GET /api/local-agent/tasks/{turnId}` 查询任务最终状态和结果。

## 非目标

第一版不做以下事项：

- 不开放局域网或公网监听。
- 不实现 WebSocket 或 SSE 流式事件。
- 不暴露 tool event、model round event、text chunk 的完整外部协议。
- 不新增第二套 agent runtime。
- 不自动切换到随机端口。
- 不允许同名会话时静默选择其中一个。

## 推荐方案

采用 Desktop 内嵌 Local Agent API 服务。

具体做法是：在 `bitfun-desktop` 进程中启动一个小型 Axum HTTP 服务。服务只绑定 `127.0.0.1:<port>`，持有或引用当前桌面 runtime 已经初始化好的 `ConversationCoordinator`、`DialogScheduler` 和必要的配置读取能力。

业务逻辑放在平台无关的服务模块中，HTTP 层只负责请求解析、鉴权、状态码映射和 JSON 序列化。这样后续如果 CLI、server 或 MCP 也要复用同一能力，可以复用服务层而不是复制一套 HTTP handler。

## API 设计

### 认证

所有接口必须带：

```text
Authorization: Bearer <token>
```

token 从本机配置读取。若不存在，Desktop 启动时生成并持久化到本机 app data 下的 Local Agent API 配置文件。日志只允许输出 token 是否存在和服务地址，不输出 token 原文。

鉴权失败返回：

- `401`：缺少 token 或 token 不匹配。
- `403`：未来预留给已认证但权限不足的场景。

### 提交并等待任务

```text
POST /api/local-agent/tasks:run
```

请求体：

```json
{
  "sessionId": "可选",
  "sessionName": "可选",
  "workspacePath": "D:\\BitFun",
  "message": "任务内容",
  "agentType": "可选",
  "timeoutMs": 600000
}
```

字段规则：

- `workspacePath` 必填，用于限定会话查找和执行工作区。
- `message` 必填，trim 后不能为空。
- `sessionId` 和 `sessionName` 至少提供一个。
- 如果同时提供 `sessionId` 和 `sessionName`，必须指向同一个会话，否则返回 `400`。
- `agentType` 可选；不提供时使用目标会话当前 `agent_type`。
- `timeoutMs` 可选；默认建议 10 分钟；服务端设置最大值，避免无限挂起。

成功完成响应：

```json
{
  "status": "completed",
  "sessionId": "session-id",
  "sessionName": "会话名",
  "turnId": "turn-id",
  "finalResponse": "最终文本",
  "timedOut": false
}
```

失败响应：

```json
{
  "status": "failed",
  "sessionId": "session-id",
  "sessionName": "会话名",
  "turnId": "turn-id",
  "error": "错误信息",
  "timedOut": false
}
```

取消响应：

```json
{
  "status": "cancelled",
  "sessionId": "session-id",
  "sessionName": "会话名",
  "turnId": "turn-id",
  "timedOut": false
}
```

等待超时响应：

```json
{
  "status": "running",
  "sessionId": "session-id",
  "sessionName": "会话名",
  "turnId": "turn-id",
  "timedOut": true
}
```

会话解析错误：

- `404`：找不到指定 `sessionId` 或 `sessionName`。
- `409`：同一 workspace 下存在多个同名会话。响应体包含候选 `sessionId`、`sessionName`、`agentType` 和创建时间，调用方必须改用 `sessionId`。

### 查询任务结果

```text
GET /api/local-agent/tasks/{turnId}
```

响应：

```json
{
  "status": "completed",
  "sessionId": "session-id",
  "sessionName": "会话名",
  "turnId": "turn-id",
  "finalResponse": "最终文本"
}
```

如果任务仍在运行：

```json
{
  "status": "running",
  "sessionId": "session-id",
  "sessionName": "会话名",
  "turnId": "turn-id"
}
```

如果服务不知道该 `turnId`：

```json
{
  "status": "not_found",
  "turnId": "turn-id"
}
```

第一版的查询范围限定在 Local Agent API 提交过、且仍在 tracker 缓存窗口内的任务。它不承担完整历史检索职责。

## 组件设计

### LocalAgentApiService

平台无关服务，负责：

- 校验请求字段。
- 按 `workspacePath` + `sessionId/sessionName` 解析目标会话。
- 调用 `DialogScheduler::submit` 投递任务。
- 注册或查询 `TaskResultTracker`。
- 等待 `TurnOutcome` 或超时。
- 将内部错误映射为领域错误。

服务依赖现有 runtime 抽象：

- `Arc<ConversationCoordinator>`
- `Arc<DialogScheduler>`
- `Arc<TaskResultTracker>`

### TaskResultTracker

轻量结果追踪器，负责：

- 记录 Local Agent API 提交的 `turnId`、`sessionId`、`sessionName`、创建时间和状态。
- 为同步等待提供 oneshot 或 notify 机制。
- 在 turn 完成、失败、取消时更新最终结果。
- 为 `GET /tasks/{turnId}` 提供缓存查询。
- 对旧任务做容量或时间窗口清理。

第一版 tracker 不需要保存到磁盘。Desktop 重启后，旧 `turnId` 查询返回 `not_found`。

### HTTP 层

Desktop HTTP 层负责：

- 绑定 `127.0.0.1:<port>`。
- 校验 `Authorization: Bearer <token>`。
- 解析 JSON 请求。
- 调用 `LocalAgentApiService`。
- 将服务错误映射为 HTTP status 和稳定 JSON error。

默认端口建议为 `17373`。如果端口占用，第一版启动失败并写清晰英文日志；不自动换端口，避免调用方无法发现服务位置。

## 数据流

```text
Codex
  -> POST /api/local-agent/tasks:run
  -> Desktop Local Agent API
  -> LocalAgentApiService
  -> resolve session by workspacePath + sessionId/sessionName
  -> TaskResultTracker register turn
  -> DialogScheduler::submit
  -> ConversationCoordinator executes turn
  -> TurnOutcome updates TaskResultTracker
  -> HTTP response returns completed/failed/cancelled/running
```

查询流：

```text
Codex
  -> GET /api/local-agent/tasks/{turnId}
  -> Desktop Local Agent API
  -> TaskResultTracker
  -> JSON status/result
```

## 错误处理

稳定错误响应格式：

```json
{
  "error": {
    "code": "SESSION_NAME_AMBIGUOUS",
    "message": "Multiple sessions match sessionName in this workspace.",
    "details": {}
  }
}
```

建议错误码：

- `UNAUTHORIZED`
- `INVALID_REQUEST`
- `SESSION_NOT_FOUND`
- `SESSION_NAME_AMBIGUOUS`
- `SESSION_MISMATCH`
- `SUBMIT_FAILED`
- `TASK_NOT_FOUND`
- `INTERNAL_ERROR`

所有后端日志必须使用英文，不打印 token 和完整敏感请求体。可以记录 `session_id`、`turn_id`、状态、耗时和错误码。

## 测试计划

Rust 单元测试：

- token 校验：缺失、格式错误、错误 token、正确 token。
- session 解析：按 `sessionId`、按唯一 `sessionName`、同名冲突、`sessionId/sessionName` 不匹配。
- 请求校验：空 message、缺少 session 标识、缺少 workspace。
- result tracker：running、completed、failed、cancelled、timeout、not_found。

Rust 集成或 handler 测试：

- `POST /tasks:run` 在完成前超时返回 running。
- 完成后 `GET /tasks/{turnId}` 返回 final result。
- 同名 session 返回 409 和候选列表。

最小验证命令：

```bash
cargo check -p bitfun-desktop
cargo test -p bitfun-core local_agent
cargo test -p bitfun-desktop local_agent
```

如果服务层落在 `bitfun-core` 并影响共享 runtime 边界，最终还需要执行：

```bash
cargo check --workspace
cargo test --workspace
```

## 后续扩展

- 增加可配置端口和显式启停 UI。
- 增加 SSE 或 WebSocket 流式输出。
- 增加 `GET /api/local-agent/sessions` 供外部调用方列出可选会话。
- 增加任务取消接口。
- 增加更细粒度的 token 权限，例如只允许投递、只允许查询。

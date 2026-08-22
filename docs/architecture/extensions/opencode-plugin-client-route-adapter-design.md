# OpenCode Plugin Client Rust 路由适配设计

## 1. 文档目的

本文设计 BitFun 后端如何为 Plugin Host 注册 `backend.http.request` JSON-RPC
处理器，并将 OpenCode SDK Client 的 HTTP 请求路由到 BitFun Rust 能力。

本文解决以下问题：

1. Rust 在什么位置、以什么顺序注册 `backend.http.request`、
   `backend.stream.read` 和 `backend.stream.cancel`。
2. Host 中 `createOpencodeClient()` 发出的请求如何经过 IPC 到达 Rust，再还原为
   OpenCode SDK 能够解析的 HTTP 响应。
3. 当前嵌入的 `@opencode-ai/sdk` 全量 Client API 中，哪些接口可以复用 BitFun
   已有能力，哪些接口需要补 Rust 适配，哪些接口必须明确降级。
4. 如何保证实例隔离、目录安全、流式响应、超时、取消和优雅关闭。

本文同时记录当前实现基线：状态为 A 的路由已经进入 Rust 显式允许列表；状态为 P、
D 的路由，以及 Event、Auth、Instance、Permission、TUI 类别，均未注册。后续只有
完成固定版本端到端测试的新增路由才可以标记为已实现。

## 2. 兼容基线和当前结论

当前路由清单来自仓库中嵌入的 `@opencode-ai/sdk@1.17.18`：

```text
src/apps/extension-host/node_modules/@opencode-ai/sdk/dist/gen/sdk.gen.js
src/apps/extension-host/node_modules/@opencode-ai/sdk/dist/gen/sdk.gen.d.ts
```

OpenCode SDK 的路由不是 BitFun 的公共 HTTP Server 合同。BitFun 只为插件
`input.client` 提供一个版本化、显式允许的兼容层，不启动完整的 OpenCode Server，
也不允许插件通过任意 URL 访问 BitFun 后端。

当前已经具备的基础事实：

- `src/apps/extension-host/src/host.ts` 已经通过本地 Gateway 创建
  `createOpencodeClient({ baseUrl, directory })`。
- `src/apps/extension-host/src/gateway.ts` 和 `node-gateway.ts` 已经将 HTTP 请求
  转换为 `backend.http.request` 参数。
- `src/crates/adapters/opencode-plugin-host/src/peer.rs` 已经提供
  `PluginHostClient::register_handler()`。
- `peer_runtime.rs` 已经支持并发请求关联和重入请求，Rust 在处理
  `host.instance.open` 时可以反向等待 Host 发起的 HTTP 请求。
- 当前 CLI Plugin Host 启动路径已经在插件预热和实例激活前注册
  `backend.http.request`、`backend.stream.read` 和 `backend.stream.cancel`。
- Rust 显式 route table 只注册矩阵中的 A 路由；P、D 和明确排除类别不会进入通用
  proxy，路径未知时返回 `404`，与已适配路径 method 冲突时返回 `405`。
- Host Gateway 当前拒绝 WebSocket Upgrade，因此 `client.pty.connect()` 不能仅靠
  Rust 路由实现。

### 2.1 重要边界

`backend.http.request` 是唯一的 HTTP 请求入口，不为每个 OpenCode HTTP 路由再创建
一个 JSON-RPC 方法。例如 `/project/current` 和 `/session` 都通过同一个
`backend.http.request` handler 进入 Rust，然后由内部路由表按 HTTP method 和 path
分发。

```text
Plugin entrypoint
  -> input.client.project.current()
  -> SDK: GET /project/current?directory=...
  -> Host Gateway
  -> JSON-RPC: backend.http.request
  -> Rust OpenCodeClientRouter
  -> BitFun workspace/service/session owner
  -> HTTP response DTO
  -> Host response stream
  -> SDK response parser
```

## 3. 现有传输协议

### 3.1 HTTP 请求

Host Gateway 发送的参数形态如下：

```json
{
  "instanceID": "bitfun:host:1:1",
  "requestID": "uuid",
  "method": "GET",
  "path": "/project/current?directory=C%3A%2Fworkspace%2Fproject",
  "headers": [["accept", "application/json"]]
}
```

请求有 body 时，`body` 是 Host 侧流描述符，而不是直接把完整 body 嵌入 RPC：

```json
{
  "streamID": "host:bitfun:host:1:1:body:1",
  "length": 128
}
```

相关协议定义位于：

```text
src/apps/extension-host/src/protocol.ts
```

`GET`/`HEAD` 请求的 SDK Client 会把 `directory` 追加为查询参数；其他请求使用
`x-opencode-directory` header。Rust 路由必须同时兼容这两种输入，但最终以
`host.instance.open` 建立的实例目录为准，不能让插件自由切换目录。

### 3.2 HTTP 响应

Rust handler 返回：

```json
{
  "status": 200,
  "statusText": "OK",
  "headers": [["content-type", "application/json"]],
  "body": {
    "streamID": "backend:bitfun:host:1:1:response:1",
    "length": 256
  }
}
```

无 body 的响应可以省略 `body`。如果有 body，Rust 将数据放入 Rust-owned
`PluginHostStreamRegistry`，Host 再通过 `backend.stream.read` 拉取数据。

### 3.3 RPC 方法职责

| RPC 方法 | 方向 | 职责 | 当前状态 |
|---|---|---|---|
| `backend.http.request` | Host -> Rust | 执行一个经过允许列表校验的 OpenCode Client 请求 | 需要生产注册 |
| `host.stream.read` | Rust -> Host | 读取插件请求 body | Peer 能力已有，需由 HTTP handler 调用 |
| `host.stream.cancel` | Rust -> Host | 取消插件请求 body | Peer 能力已有，需由 HTTP handler 调用 |
| `backend.stream.read` | Host -> Rust | 读取 Rust 返回的 response body | 需要 Rust stream registry |
| `backend.stream.cancel` | Host -> Rust | 取消 Rust 返回的 response body | 需要 Rust stream registry |

## 4. Rust 模块划分

实现应遵循“传输适配器、OpenCode 协议适配器、BitFun 能力 owner”分层，避免在
`bitfun-core` 中实现一套新的 OpenCode Server。

### 4.1 `opencode-plugin-host`：传输和 Host 侧协议

建议扩展：

```text
src/crates/adapters/opencode-plugin-host/src/
  peer.rs
  peer_runtime.rs
  protocol.rs
  http_bridge.rs
  stream_registry.rs
```

职责：

- 定义 `BackendHttpRequest`、`BackendHttpResponse`、`StreamDescriptor` 等 wire DTO。
- 提供 `PluginHostClient::register_handler()` 的生产使用入口。
- 提供 Rust-owned response stream registry。
- 负责 base64 分块、EOF、取消、最大 chunk 和 stream ownership 校验。
- 不直接依赖 `bitfun-core` 的 Workspace、Session 或 MCP 实现。

### 4.2 OpenCode 路由适配器

建议在 `opencode-plugin-host` 中增加独立的 OpenCode Client 路由模块，或者在
已有 OpenCode 专用 adapter 中增加同等边界：

```text
OpenCodeClientRouter
  -> RequestNormalizer
  -> RouteTable
  -> RouteHandler trait
  -> BackendCapability trait
```

职责：

- 解析 HTTP method、path、query、headers 和 JSON body。
- 只匹配固定版本 SDK 的显式路由。
- 将路由请求转换为类型化的 OpenCode operation DTO。
- 调用下层 `BackendCapability`，再把结果转换为 OpenCode response DTO。
- 将错误转换为稳定的 HTTP status 和 OpenCode-compatible error JSON。
- 不进行任意 URL 反向代理。

`RouteTable` 应由静态定义生成或集中维护，至少包含：

```text
(http_method, normalized_path, operation, capability, support_status)
```

路径参数必须解析为结构化字段，禁止把完整 URL 字符串交给下层服务。

### 4.3 Core/Assembly：能力装配和实例上下文

建议在现有：

```text
src/crates/assembly/core/src/plugin_host.rs
```

附近增加 `PluginHostBackendBridge` 和实例上下文管理，职责包括：

- 保存 `instanceID -> PluginHostInstanceContext`。
- 绑定 workspace、directory、worktree、session 和权限上下文。
- 将已允许的 OpenCode operation 分发到 Workspace、Session、FileSystem、Terminal、
  Git、MCP、LSP、Config 和 Provider owner。
- 在 Host 关闭时停止接收新请求，等待已接纳请求和响应流完成。

推荐由 adapter 定义窄的 `BackendCapability` port，由 core 提供实现。这样 OpenCode
路由只依赖稳定的类型化 port，产品服务仍保持平台无关。

## 5. Handler 注册时机

注册顺序必须早于任何插件激活动作：

```text
PluginHost::start
  -> spawn Host process
  -> establish IPC
  -> complete backend.handshake
  -> start JsonRpcPeer
  -> create PluginHostClient
  -> create PluginHostBackendBridge
  -> register backend.http.request
  -> register backend.stream.read
  -> register backend.stream.cancel
  -> store instance/host state
  -> start background plugin preparation
  -> host.plugins.prepare
  -> host.instance.open
  -> plugin server(input)
  -> input.client.* may call backend.http.request
```

注册 handler 的伪代码：

```rust
let client = plugin_host.client().await?;
let bridge = Arc::new(PluginHostBackendBridge::new(
    instance_registry.clone(),
    capability_provider.clone(),
));

client.register_handler("backend.http.request", {
    let bridge = bridge.clone();
    move |params| {
        let bridge = bridge.clone();
        async move { bridge.handle_http(params).await }
    }
}).await?;

client.register_handler("backend.stream.read", {
    let streams = bridge.response_streams().clone();
    move |params| {
        let streams = streams.clone();
        async move { streams.read(params).await }
    }
}).await?;

client.register_handler("backend.stream.cancel", {
    let streams = bridge.response_streams().clone();
    move |params| {
        let streams = streams.clone();
        async move { streams.cancel(params).await }
    }
}).await?;
```

实际实现应使用项目已有的错误类型和日志设施，不直接复制此伪代码的类型名称。
任何一个必需 handler 注册失败，都应关闭当前 Host 并让启动失败，不应继续进入
`host.instance.open`。

### 5.1 重入要求

插件可能在 `server(input)` 执行期间立即调用：

```ts
await input.client.project.current()
```

因此 `backend.http.request` handler 必须在 `host.instance.open` 之前注册，并且
`JsonRpcPeer` 必须允许 Rust handler 在处理 Host 请求期间再次向 Host 发起
`host.stream.read` 请求。现有 peer runtime 已有重入和请求关联测试；新增路由集成测试
必须覆盖这一顺序。

## 6. 请求处理流程

```text
1. Rust 收到 backend.http.request(params)
2. 校验 instanceID、requestID、method、path 和 header
3. 查询 instance registry，获取 canonical directory/workspace
4. 校验 directory query/header 与实例上下文一致
5. RouteTable 匹配 method + normalized path
6. 若有 request body，通过 host.stream.read 分块读取
7. 校验 content-type、body size 和请求 schema
8. 调用对应 BackendCapability
9. 将结果转换为 HTTP status、headers 和 JSON/stream body
10. 对流 body 创建 Rust-owned stream descriptor
11. 返回 backend.http.request response
12. Host 通过 backend.stream.read 消费 response body
13. EOF、cancel 或 timeout 后清理 stream registry
```

### 6.1 实例和目录校验

`instanceID` 不是装饰字段，必须是路由隔离的第一索引。实例上下文至少包含：

```text
instance_id
workspace_id
directory
worktree
opened_at
permissions
state
```

规则：

- `host.instance.open` 建立的 canonical directory 是权威值。
- GET/HEAD 的 `directory` query 和其他方法的 `x-opencode-directory` 都必须与
  canonical directory 一致。
- Windows 路径比较要统一大小写、分隔符和绝对路径形式。
- 所有文件路径都要经过 workspace containment 检查，拒绝 `..` 穿越。
- 未注册的 instance、已关闭的 instance 和跨 workspace 请求分别返回稳定错误。
- Remote workspace 没有对应 owner 时不得回退到本地文件系统。

### 6.2 body 和 response stream

请求和响应 body 均使用流，避免把大文件或长会话消息一次性放进 JSON-RPC frame。

`PluginHostStreamRegistry` 要求：

- 每个 stream 绑定 `instanceID`、requestID、方向和创建时间。
- 单次读取上限与 Host 的 `MAX_STREAM_CHUNK_BYTES` 对齐，默认 64 KiB。
- wire payload 使用 base64，EOF 时删除 registry 项。
- cancel、请求超时、instance dispose、Host shutdown 都必须删除 stream。
- 对未绑定当前 instance 的 stream 返回 `403` 或 RPC handler error。
- 设置总 body 上限、空闲超时和并发 stream 上限。

## 7. 全量路由和 Rust 支持矩阵

状态含义：

- **A：可适配**：BitFun 已有相应基础能力；当前实现已提供类型化路由、实例隔离和
  响应转换。
- **P：部分支持**：存在相近能力，但语义、交互或安全边界仍需补充。
- **D：降级/延期**：当前不应伪装成成功，需要稳定返回 unsupported。

### 7.1 Project、Path、VCS、Config、Tool、Provider、App、Command

| Client API | HTTP route | Rust 能力 | 状态 | 实现重点 |
|---|---|---|---|---|
| `project.list()` | `GET /project` | Workspace service | A | 输出 OpenCode Project DTO，过滤不可见 workspace |
| `project.current()` | `GET /project/current` | Workspace service | A | 使用 instance workspace，不接受任意 directory |
| `path.get()` | `GET /path` | Filesystem/workspace | A | 返回 canonical directory、worktree 和项目路径 |
| `vcs.get()` | `GET /vcs` | Git service | A | 映射 branch、worktree 和仓库状态 |
| `config.get()` | `GET /config` | Config service | A | 只返回插件可见配置，去除 secrets |
| `config.update()` | `PATCH /config` | Config service | P | 需要字段白名单、权限、并发版本和持久化策略 |
| `config.providers()` | `GET /config/providers` | Provider/config service | A | 输出 SDK 需要的 provider/model 投影 |
| `tool.ids()` | `GET /experimental/tool/ids` | Tool registry | A | 只返回当前实例可见工具 ID |
| `tool.list()` | `GET /experimental/tool` | Tool registry | A | 转换 tool schema 和权限元数据 |
| `provider.list()` | `GET /provider` | Provider registry | A | 不返回密钥和 token |
| `provider.auth()` | `GET /provider/auth` | Provider auth/config | P | 只提供认证状态和允许的元数据 |
| `provider.oauth.authorize()` | `POST /provider/{id}/oauth/authorize` | OAuth/provider service | P | 需要 provider-specific OAuth state |
| `provider.oauth.callback()` | `POST /provider/{id}/oauth/callback` | OAuth/provider service | P | 需要安全回调和 state 校验 |
| `app.log()` | `POST /log` | BitFun logging | A | 按 app.logging.level 写结构化日志，过滤敏感字段 |
| `app.agents()` | `GET /agent` | Agent registry | A | 返回当前产品可见 agent catalog |
| `command.list()` | `GET /command` | Command/external source registry | A | 只返回已经注册且可执行的 command |

这些接口是第一批适合实现的只读或窄写入路由。它们仍然不能使用通用 JSON 转发，
必须为每个 operation 定义请求和响应 DTO。

### 7.2 Session

| Client API | HTTP route | Rust 能力 | 状态 | 实现重点 |
|---|---|---|---|---|
| `session.list()` | `GET /session` | SessionManager | A | 按 instance workspace 过滤 |
| `session.create()` | `POST /session` | SessionManager | A | 建立 workspace/session 绑定和权限上下文 |
| `session.status()` | `GET /session/status` | Session/coordinator | A | 输出运行中 session 状态 |
| `session.delete()` | `DELETE /session/{id}` | SessionManager | A | 删除前取消活动任务和 plugin 引用 |
| `session.get()` | `GET /session/{id}` | SessionManager | A | 校验 session 属于当前 instance |
| `session.update()` | `PATCH /session/{id}` | SessionManager | A | 只映射支持的 title/model/metadata 字段 |
| `session.children()` | `GET /session/{id}/children` | SessionManager | A | 映射 fork/child 关系 |
| `session.todo()` | `GET /session/{id}/todo` | Session state | A | 输出稳定 todo DTO |
| `session.init()` | `POST /session/{id}/init` | Workspace/session init | P | 需要定义 OpenCode AGENTS 初始化的 BitFun 等价语义 |
| `session.fork()` | `POST /session/{id}/fork` | Session coordination | A | 映射 fork 参数和新 session 返回值 |
| `session.abort()` | `POST /session/{id}/abort` | Coordinator cancellation | A | 传播取消信号并等待 owner 收敛 |
| `session.unshare()` | `DELETE /session/{id}/share` | Share service | D | 当前没有确认的等价 public-share contract |
| `session.share()` | `POST /session/{id}/share` | Share service | D | 没有确认的等价 public-share contract |
| `session.diff()` | `GET /session/{id}/diff` | Session/Git service | A | 映射 diff 文件和 patch 结构 |
| `session.summarize()` | `POST /session/{id}/summarize` | Coordinator/compaction | P | 需要定义同步、异步和 token 语义 |
| `session.messages()` | `GET /session/{id}/message` | Session persistence | A | 转换消息、parts 和分页字段 |
| `session.prompt()` | `POST /session/{id}/message` | Coordinator | P | 需要映射 prompt、attachments、model 和事件顺序 |
| `session.message()` | `GET /session/{id}/message/{messageID}` | Session persistence | A | 校验 message 属于 session |
| `session.promptAsync()` | `POST /session/{id}/prompt_async` | Coordinator | P | 需要后台任务 ID 和状态查询契约 |
| `session.command()` | `POST /session/{id}/command` | Coordinator/command registry | P | 不能绕过权限和工具审批 |
| `session.shell()` | `POST /session/{id}/shell` | Terminal/Coordinator | P | 需要 shell 权限、工作目录和输出限制 |
| `session.revert()` | `POST /session/{id}/revert` | Session/Git state | P | 需要定义消息和工作区回滚边界 |
| `session.unrevert()` | `POST /session/{id}/unrevert` | Session/Git state | P | 需要与 BitFun session history 对齐 |

Session 路由虽然大部分有对应 Rust owner，但最容易引入产品语义漂移。实现时应先
完成只读查询，再实现 create/abort/prompt 等有副作用的操作；不能为了通过插件激活
而把 OpenCode Session 当作第二套 session manager。

### 7.3 PTY

| Client API | HTTP route | Rust 能力 | 状态 | 实现重点 |
|---|---|---|---|---|
| `pty.list()` | `GET /pty` | Terminal session manager | A | 按 instance/session 隔离 |
| `pty.create()` | `POST /pty` | Terminal session manager | A | 映射 shell、cwd、env 和资源上限 |
| `pty.remove()` | `DELETE /pty/{id}` | Terminal session manager | A | 关闭进程、PTY 和输出流 |
| `pty.get()` | `GET /pty/{id}` | Terminal session manager | A | 只返回当前 instance 的 PTY |
| `pty.update()` | `PUT /pty/{id}` | Terminal session manager | A | 映射 resize、title、metadata 等支持字段 |
| `pty.connect()` | `GET /pty/{id}/connect` | Terminal session manager | P | Host Gateway 当前拒绝 WebSocket Upgrade |

`pty.connect()` 不能在 Rust HTTP handler 中假设已经可用。要支持它必须先扩展 Bun
和 Node Gateway 的 WebSocket 转发、IPC stream 类型和关闭协议；当前不注册该路由并
返回 `404 route_not_found`，而不是建立一个永远不会产生输出的普通 HTTP 响应。

### 7.4 Find、File

| Client API | HTTP route | Rust 能力 | 状态 | 实现重点 |
|---|---|---|---|---|
| `find.text()` | `GET /find` | Search service | A | query、include/exclude 和结果数量限制 |
| `find.files()` | `GET /find/file` | Filesystem/search service | A | glob、隐藏文件和 workspace containment |
| `find.symbols()` | `GET /find/symbol` | LSP/search service | P | 需要 workspace symbol provider 和统一 DTO |
| `file.list()` | `GET /file` | Filesystem service | A | 目录列表和路径安全 |
| `file.read()` | `GET /file/content` | Filesystem service | A | 编码、大小限制和 binary 文件策略 |
| `file.status()` | `GET /file/status` | Filesystem/Git service | A | 映射 modified、added、deleted 等状态 |

文件和搜索接口必须共享同一套 canonical path 校验，不能只在某一个 route 上防止
路径穿越。

### 7.5 MCP、LSP、Formatter

| Client API | HTTP route | Rust 能力 | 状态 | 实现重点 |
|---|---|---|---|---|
| `mcp.status()` | `GET /mcp` | MCP service | A | 输出 server 状态，删除 credentials |
| `mcp.add()` | `POST /mcp` | MCP config/service | P | 配置 schema、来源和权限确认 |
| `mcp.connect()` | `POST /mcp/{name}/connect` | MCP lifecycle | P | 连接状态和超时映射 |
| `mcp.disconnect()` | `POST /mcp/{name}/disconnect` | MCP lifecycle | P | 资源释放和重连语义 |
| `mcp.auth.remove()` | `DELETE /mcp/{name}/auth` | Credential service | P | 需要安全凭据删除策略 |
| `mcp.auth.start()` | `POST /mcp/{name}/auth` | MCP auth | P | 需要交互式认证状态机 |
| `mcp.auth.callback()` | `POST /mcp/{name}/auth/callback` | MCP auth | P | 校验一次性 state 和回调来源 |
| `mcp.auth.authenticate()` | `POST /mcp/{name}/auth/authenticate` | MCP auth | P | 不能将 secrets 写入普通日志 |
| `mcp.auth.set()` | `PUT /auth/{id}` | Credential service | P | 加密存储和权限检查 |
| `lsp.status()` | `GET /lsp` | LSP service | A | 返回当前 workspace server 状态 |
| `formatter.status()` | `GET /formatter` | Formatter/LSP service | P | 当前没有完整 OpenCode formatter catalog |

MCP auth 和 provider auth 不能复用一个“接受任意 JSON 并写入配置”的 handler。两者
都必须定义 secret redaction、加密存储、权限和交互状态的独立策略。

### 7.6 Event、Auth、Instance、Permission、TUI

| Client API | HTTP route | Rust 能力 | 状态 | 实现重点 |
|---|---|---|---|---|
| `global.event()` | `GET /global/event` | BitFun events | P | 需要 SSE fan-out 和跨实例范围定义 |
| `event.subscribe()` | `GET /event` | BitFun events | P | 需要实例级 SSE stream |
| `instance.dispose()` | `POST /instance/dispose` | Plugin Host instance registry | P | 只能释放插件实例，不能关闭 BitFun 或整个 Host |
| `auth.remove()` | `DELETE /auth/{id}` | Provider auth service | P | 需要 provider credential 生命周期映射 |
| `auth.start()` | `POST /auth/{id}` | Provider auth service | P | 需要交互式认证策略 |
| `auth.callback()` | `POST /auth/{id}/callback` | Provider auth service | P | 需要 state、来源和过期校验 |
| `auth.authenticate()` | `POST /auth/{id}/authenticate` | Provider auth service | P | 只返回非敏感认证结果 |
| `auth.set()` | `PUT /auth/{id}` | Provider auth service | P | 加密存储，禁止明文日志 |
| `tui.appendPrompt()` | `POST /tui/append-prompt` | CLI TUI | D | Host 不是 OpenCode TUI Server |
| `tui.openHelp()` | `POST /tui/open-help` | CLI TUI | D | 需要真实 TUI owner |
| `tui.openSessions()` | `POST /tui/open-sessions` | CLI TUI | D | 需要真实 TUI owner |
| `tui.openThemes()` | `POST /tui/open-themes` | CLI TUI | D | 需要真实 TUI owner |
| `tui.openModels()` | `POST /tui/open-models` | CLI TUI | D | 需要真实 TUI owner |
| `tui.submitPrompt()` | `POST /tui/submit-prompt` | CLI TUI | D | 不能伪装为 session.prompt |
| `tui.clearPrompt()` | `POST /tui/clear-prompt` | CLI TUI | D | 需要真实输入框状态 |
| `tui.executeCommand()` | `POST /tui/execute-command` | CLI TUI | D | 需要真实命令面板状态 |
| `tui.showToast()` | `POST /tui/show-toast` | CLI TUI | D | 需要 UI surface owner |
| `tui.publish()` | `POST /tui/publish` | CLI TUI | D | 需要 TUI event contract |
| `tui.control.next()` | `GET /tui/control/next` | CLI TUI | D | 需要控制队列和消费方 |
| `tui.control.response()` | `POST /tui/control/response` | CLI TUI | D | 需要控制队列和消费方 |
| `client.postSessionIdPermissionsPermissionId()` | `POST /session/{id}/permissions/{permissionID}` | Permission service | P | 需要 OpenCode permission DTO 到 BitFun approval 的转换 |

当前实现不登记 P、D 路由，也不登记 Event、Auth、Instance、Permission、TUI 路由。
这些路径通常返回 `404 route_not_found`；如果 P/D operation 与 A operation 共享同一
path、仅 method 不同（例如 `PATCH /config`），则返回 `405 method_not_allowed`。这
避免插件依赖一个尚未承诺的 unsupported 兼容合同。

## 8. 错误、权限和日志

### 8.1 HTTP 错误映射

建议使用以下稳定映射：

| 场景 | HTTP status | 错误码示例 |
|---|---:|---|
| JSON、query 或 path 参数错误 | 400 | `invalid_request` |
| 未知 route | 404 | `route_not_found` |
| 已知 route 但 method 不匹配 | 405 | `method_not_allowed` |
| instance 不存在或已关闭 | 404 | `instance_not_found` |
| workspace/session 不属于当前 instance | 403 | `instance_scope_denied` |
| 插件没有该副作用权限 | 403 | `permission_denied` |
| 下层服务超时 | 504 | `backend_timeout` |
| Host 正在关闭 | 503 | `host_draining` |
| 下层服务内部失败 | 502 或 500 | `backend_failure` |

错误 body 使用稳定 JSON 结构，例如：

```json
{
  "error": {
    "code": "route_not_found",
    "message": "OpenCode client route was not found",
    "route": "/tui/show-toast"
  }
}
```

### 8.2 安全要求

- route 必须显式 allowlist，不得实现任意 HTTP proxy。
- method、path、query、body 都要有大小和格式限制。
- header 只允许必要的非敏感 header；凭据、cookie 和授权 header 不转发或不落日志。
- 所有写操作都经过 route-specific permission 检查。
- provider/MCP auth 绝不返回 provider key、token 或明文 credential。
- 文件操作必须执行 workspace containment 检查。
- 每个 request 有 deadline、取消 token 和并发上限。
- `instanceID`、requestID 和 streamID 必须做格式校验，不能把它们作为任意文件路径。

### 8.3 日志

Rust 路由至少记录以下结构化字段：

```text
event=plugin.client.request
instance_id
request_id
method
path
status
duration_ms
route_status
```

正常日志不记录 request/response body、authorization、cookie、provider key 或 MCP
secret。日志级别跟随现有 `app.logging.level`；`debug` 可以记录 route match 和
stream 生命周期，但仍不能记录敏感内容。

## 9. 生命周期、并发和关闭

### 9.1 请求关联

`requestID` 由 Host 生成并在 Rust 日志和异步任务中贯穿。一个
`backend.http.request` 只能完成一次 response；如果 response body 是 stream，
request 完成不等于 stream 已被消费，stream 必须独立追踪。

应分别限制：

- 每个 instance 的并发 HTTP request 数。
- 每个 instance 的 active response stream 数。
- 单 request body、单 response body 和总 stream bytes。
- route-specific deadline，写操作和长轮询不能无限占用 peer。

### 9.2 优雅关闭

收到 BitFun 关闭信号后：

1. 将 instance/host 状态切换为 draining。
2. `backend.http.request` 不再接受新请求，返回 `503 host_draining`。
3. 已接纳的请求继续等待到 route deadline 或收到取消信号。
4. 等待 response streams EOF 或 cancel。
5. 调用 Host 的 `host.shutdown`，等待插件清理实例。
6. 超时后强制终止 Host 进程并清理 registry。

关闭过程必须记录 `shutdown.begin`、`shutdown.draining`、`shutdown.completed` 或
`shutdown.forced`，并带上 active request/stream 数量。

## 10. 分阶段实现计划

### Phase 0：传输闭环（已完成）

- 在 Host handshake 后、`host.plugins.prepare` 和 `host.instance.open` 前注册三个
  backend handler。
- 实现 `BackendHttpRequest/Response` 的 Rust schema 校验。
- 实现 request body 的 `host.stream.read/cancel` 调用。
- 实现 response `PluginHostStreamRegistry` 和 `backend.stream.read/cancel`。
- 加入未知 route、instance scope、超时和关闭中的错误映射。

验收标准：插件在 `server(input)` 中调用 `input.client.project.current()`，Rust
可以重入处理请求并返回 SDK 可解析的 JSON。

### Phase 1：只读 Client（已完成）

优先实现：

```text
project.list/current
path.get
vcs.get
config.get/providers
provider.list
app.agents
command.list
file.list/read/status
find.text/files
lsp.status
mcp.status
tool.ids/list
```

每条路由都加入 request/response fixture 和 route table 单元测试。

### Phase 2：Session、PTY 和窄写操作（A 路由已完成）

- Session 查询、create、update、delete、abort、fork、messages、message、diff。
- `app.log`、PTY create/list/get/update/remove；`config.update` 保持 P，不注册。
- session prompt、command、shell 等有副作用操作在权限模型明确后启用。

### Phase 3：事件和长连接（本次明确不适配）

- 用同一 stream registry 支持 `/event` 和 `/global/event` 的 SSE fan-out。
- 明确 instance event 与 global event 的可见范围。
- 单独扩展 Gateway WebSocket 转发，再评估 `pty.connect()`。

### Phase 4：交互式 auth、MCP OAuth、TUI 和 share（本次明确不适配）

只有对应 BitFun owner、权限策略和端到端测试存在时才实现。当前不注册这些路由，
由 allowlist 统一返回 `404 route_not_found`，不创建假状态。

## 11. 测试设计

### 11.1 Route table 测试

- 当前 SDK 生成文件中的每个 route 都能在矩阵中找到。
- 每个已支持 route 的 method、path 参数和必需 query 都有正例。
- 未知 path 不会进入通用 proxy。
- P、D 和明确排除类别不在 route table 中并返回 `404`。
- method 不匹配返回 `405`。

### 11.2 重入和请求关联测试

使用 fixture 插件：

```ts
export async function server(input) {
  await input.client.project.current()
  return {}
}
```

测试必须证明：

1. Rust 先完成 handler 注册。
2. `host.instance.open` 期间 Host 发出 `backend.http.request`。
3. Rust handler 能在原 RPC 未完成时完成反向 `host.stream.read`（如有 body）。
4. 正确的 `requestID` 收到正确响应，多个并发请求不会串线。
5. plugin activation 在响应完成后成功。

### 11.3 流、权限和安全测试

- 多 chunk response 能被 Host 完整读取，EOF 后 registry 删除。
- cancel、超时、instance dispose 和 Host shutdown 都能清理 stream。
- 错误 instance、directory mismatch、路径穿越和跨 workspace 请求被拒绝。
- provider/MCP secrets 不出现在日志和响应中。
- TUI、WebSocket 和 share 等未适配功能不进入 route table。

### 11.4 验证命令

文档对应实现完成后，最小验证顺序为：

```text
cargo test -p opencode-plugin-host
cargo test -p bitfun-core plugin_host
cargo check -p bitfun-cli
pnpm run check:repo-hygiene
```

实际 crate package name 以 workspace `Cargo.toml` 为准；文档-only 修改只需执行：

```text
git diff --check
```

## 12. 完成定义

该设计完成实现的最低标准不是“能启动 Host”，而是：

- Rust 在正确生命周期注册 `backend.http.request` 和 response stream handlers。
- 插件在 activation 期间调用 `input.client.*` 能完成至少一个真实闭环。
- 路由表覆盖当前 SDK 的全量 API，并对每一项给出 A/P/D 结论。
- 支持路由有类型化参数、实例绑定、权限、超时、取消和错误转换。
- body 和 SSE 使用有界 stream，不把大响应塞入 JSON-RPC frame。
- 关闭时不接受新请求，等待已接纳请求和流完成，再优雅退出 Host。
- 未支持能力以稳定错误降级，不冒充 OpenCode 等价实现。

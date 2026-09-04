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
- 封闭 operation 范围为 Health、只读且 workspace-scoped 的 main Agent 摘要、Session list/create/restore/delete/fork（restore/fork 结果包含 transcript）、当前 Session rename、Agent mode/model update、手动 context compaction、Session undo/redo、current-controller 限定的只读工作区引用搜索/持久化引用读取，以及不取得 Session lease 的 Runtime 绑定工作区只读 diff；此外还包括声明式上下文 reload、Turn submit/steer、用户显式 Shell 执行/cancel、pending/respond Permission 和 UserInput answers。delete 只允许作用于未被任何 Client 控制的空闲 Session。fork 要求当前 controller 且 Session 空闲：可以复制到最新持久化 Turn，也可以停在显式选中 Turn 之前；只有包含新 Session 与 transcript 的成功结果完成编码后，Server 才能把连接 lease 从源 Session 原子切换到 fork。手动 compaction 要求当前 controller 且 Session 空闲；Client 在准入前提供精确 Turn ID，使超时或断连 cleanup 可以取消同一个 owned task；Core 开始原子 context commit 后，晚到取消不能暴露错误的空闲状态。用户显式 Shell 执行同样要求当前 controller、Session 空闲和调用方提供的 Turn ID；它只委托给窄 Runtime port，并复用正常 ToolPipeline、权限、工作区路由、持久化与取消 owner，不是通用 Tool 或进程执行 wire。steer 要求当前 controller、活动 Turn 以及调用方提供的精确 Session/Turn ID；它只委托给共享 Runtime owner，拒绝过期投影，不创建第二个 Turn 或 queue owner。undo/redo 要求当前 controller，但可在活动 Turn 中进入，因为取消、drain 与回退写入顺序由 Core 统一负责；成功结果携带权威 transcript，并清除连接侧活动 Turn 投影。该能力只支持本地工作区，不暴露通用 checkpoint 协议。上下文 reload 可在活动 Turn 中执行，不改写该 Turn，并通过缓存保护保证下一条消息重新读取已失效的 instructions。断连 cleanup 属于内部生命周期，不是 detach operation。模型目录、默认值和完整 Agent/Subagent 管理仍是 wire 之外的产品配置；main Agent 摘要只服务 host-owned selector：启动页查询 Runtime 绑定工作区且不取得 Session lease，Session 级查询则要求该 Session 的当前 controller 并由 Runtime owner 解析执行工作区。摘要中的 external-source 分类保持生态无关，且不携带安装、变更、激活或 runtime lifecycle API。禁止顺带加入 archive、replay、observer、通用 controller transfer、Tool/MCP/Hook 管理或其他产品配置。
- 可以复用稳定 Event、Product Domain 和 Runtime Port DTO；`openbitfun-transport` 只复用协议无关的有界 JSON 编码，单一消费者的长度前缀 framing 仍归本 wire 私有。禁止依赖 `openbitfun-core`、Agent Runtime 实现、SDK Host、services、Tauri、terminal、tool runtime 或远程 transport。
- 只使用 Windows Named Pipe 或 Unix Domain Socket；禁止 TCP、HTTP、WebSocket、浏览器访问或远程 fallback。
- 这是本机同用户隔离，不是沙箱。未来产品 composition 必须提供当前用户私有 runtime 目录。
- Embedded 调用方必须继续以强类型直接调用 Agent Runtime，不能初始化本 transport。Shared 的 request、response 和 event frame 在写出前只编码一次；不能为了吞吐量削弱严格解码、未知字段拒绝、frame 上限、有界队列和背压。

## 验证

```bash
cargo test -p openbitfun-agent-runtime-ipc
node scripts/check-core-boundaries.mjs
```

# BitFun 运行时性能审阅报告(02)

- 审阅日期:2026-07-26
- 审阅维度:执行性能(运行时)—— Rust 后端热点路径 / 前端 React 高频路径 / Tauri IPC 层
- 方法:Grep 系统性扫描典型反模式(clone 密集区、async 中同步 IO、`Regex::new` 非静态、`block_on`、`JSON.parse(JSON.stringify)`、`setInterval`、`structuredClone`、未节流事件监听、高频 `invoke`/`emit`),对每个可疑点精读上下文并论证调用频率后才收录;所有明显但经确认无害的候选项在文末"误报排除记录"中留档,避免后续重复审查。
- 所有行号以当前 main 分支(48a003b73)为准。

> 退役说明（2026-08）：本文是提交 `48a003b73` 的固定历史审阅证据。此后 BitFun LSP Runtime 已被完整删除，
> 因此下文问题 #5 与任务 T3 已通过退役关闭，所述路径也不再存在。保留这些段落只为解释当时结论；不得据此恢复、
> 优化或重新实现 LSP Runtime。
>
> 同期，鼠标跟随光效也已完整删除，因此问题 #20 已通过退役关闭；不得据此恢复或重新实现该功能。

---

## 一、发现总览表

按预期收益排序。"层"= R(Rust 后端)/ F(前端)/ I(IPC)。

| # | 层 | 问题 | 位置(相对 `src/`) | 预期收益 | 修复风险 |
|---|---|------|------|------|------|
| 1 | R | 每条消息追加触发整个 turn 上下文全量 clone + pretty 序列化 + 落盘,总成本 O(N²) | `crates/assembly/core/src/agentic/session/session_manager.rs:6310-6336`、`agentic/persistence/manager.rs:1287-1298,646-654`、`crates/services/services-core/src/json_store.rs:211-213` | 高 | 中 |
| 2 | R | 文件树递归扫描:sort 比较器内同步 stat(O(n log n) 次)、每条目 3+ 次 metadata、同步 `canonicalize`、不读 gitignore | `crates/services/services-core/src/filesystem/tree.rs:436,446-469,486,504,538,897`(复制版 `572-769` 同病) | 高 | 中 |
| 3 | R/I | 所有后端→前端事件经 `PeerAwareEmitter` 无条件深拷贝 payload;terminal 事件另有无条件 `to_value` | `apps/desktop/src/api/remote_connect_api.rs:558-561`、`apps/desktop/src/api/terminal_api.rs:347-359` | 高 | 低 |
| 4 | R | 终端 transcript:每 chunk 同步 open/write/flush/close,`std::sync::Mutex` 持锁做阻塞 IO,跑在 tokio PTY 事件循环 | `crates/services/terminal/src/transcript.rs:412-437,176-187`、`terminal/src/session/manager.rs:346` | 高 | 中 |
| 5 | R | LSP 诊断/响应同一份 `Value` 单次通知深拷贝 5 次(含 MB 级 semanticTokens 响应) | `crates/services/services-core/src/lsp/process.rs:164-170,445`、`crates/assembly/core/src/service/lsp/workspace_manager.rs:886-906` | 高 | 中 |
| 6 | R | 流式事件队列:每个模型 delta ~15 次堆分配 + 2 次 envelope 深拷贝 + 2 把异步锁(含 stats 计数锁) | `crates/execution/agent-stream/src/lib.rs:806-819`、`crates/execution/agent-runtime/src/event_queue.rs:178-207` | 高 | 中 |
| 7 | R | ACP 工具流:`raw_input`(可含数百 KB Edit/Write 参数)单次 ToolCallUpdate 深拷贝 3-4 次 | `crates/interfaces/acp/src/client/stream.rs:193,204-221,240,294-301` | 高 | 低 |
| 8 | R | LS 工具:同步 BFS 无 `spawn_blocking`(阻塞 tokio worker),每条目 2 次 `symlink_metadata` | `crates/services/services-core/src/filesystem/listing.rs:41,137-160`、`crates/assembly/core/src/agentic/tools/implementations/ls_tool.rs:292` | 高 | 低 |
| 9 | F | 侧栏拖拽 mousemove 每次全量 setState,无 rAF,触发整个工作区子树重渲染 | `web-ui/src/app/layout/WorkspaceBody.tsx:64-77,89` | 高(拖拽期间) | 低 |
| 10 | F/I | 桌宠 120ms 轮询 `cursorPosition()` IPC + 每 tick querySelectorAll/getBoundingClientRect,全仓频率最高常驻 IPC | `web-ui/src/app/components/AgentCompanionDesktopPet/AgentCompanionDesktopPet.tsx:454,471-482,490` | 中高 | 低 |
| 11 | F/I | 编辑器每 1s 轮询 `get_file_metadata`,与已有 `file-system-changed` 推送通道功能重叠 | `web-ui/src/tools/editor/components/CodeEditor.tsx:120,1990`、`MarkdownEditor.tsx:43,428` | 中高 | 中 |
| 12 | R | 文件监视线程:每个 notify 事件 `block_on` 异步 RwLock;每次 watch/unwatch 全量重建 watcher | `crates/services/services-integrations/src/file_watch/service.rs:117-197(172,185)` | 中 | 低 |
| 13 | R/I | 目录分页命令每页全量重扫目录再内存切片,O(页数×目录大小) | `apps/desktop/src/api/commands.rs:2408-2441` | 中 | 中 |
| 14 | F | EventBus 每次 emit 复制 1000 元素历史数组,且历史持有 payload 强引用阻止 GC | `web-ui/src/infrastructure/event-bus/EventBus.ts:144,295-302` | 中 | 低 |
| 15 | F | SessionStateMachine 每次状态转换 `structuredClone` 整个 context 广播 | `web-ui/src/flow_chat/state-machine/SessionStateMachine.ts:61-72,320` | 中 | 中 |
| 16 | R | 内容搜索每行无条件分配 String(未命中行也分配);结果收尾整体 clone | `crates/services/services-core/src/filesystem/tree.rs:1485-1491,1140,1275` | 中 | 低 |
| 17 | R | grep 工具:每文件冗余 `is_file()` stat;输出 split→逐行 alloc→join 往返 | `crates/execution/tool-execution/src/search/grep_search.rs:743,776,835-840,875` | 中 | 低 |
| 18 | F | 桌宠打字机:interval 依赖自身 setState 目标,每 28ms 销毁重建 effect + 全部气泡强制布局 | `web-ui/src/app/components/AgentCompanionDesktopPet/AgentCompanionDesktopPet.tsx:308-339` | 中 | 低 |
| 19 | F | （已关闭）统一 Tooltip 定位通过 rAF 合并 scroll/resize 重算，每帧最多一次布局读取 | `design-system/packages/ui/src/components/Tooltip/Tooltip.tsx` | 不再适用 | 已关闭 |
| 20 | F | （已关闭）历史鼠标跟随光效的 pointermove 分配问题；功能已退役 | 已删除 | 不再适用 | 已关闭 |
| 21 | F/I | SnapshotAPI 逐 turn 串行 `get_turn_files`,长会话 N 次串行 IPC 往返 | `web-ui/src/infrastructure/api/service-api/SnapshotAPI.ts:514-531` | 中 | 低 |
| 22 | R | PTY chunk `from_utf8_lossy(..).to_string()` 强制整串拷贝(最大 64KB);tap 分发逐个 clone | `crates/services/terminal/src/session/manager.rs:285,429` | 中低 | 低 |
| 23 | R | `HeadTailText` 逐 char 推入 `VecDeque<char>`,10MB 构建日志 = 千万次操作 | `crates/services/terminal/src/exec.rs:934-959` | 中低 | 低 |
| 24 | F | Peer 模式会话快照 3s 轮询 reconcile | `web-ui/src/flow_chat/services/flow-chat-manager/PeerSessionRefreshModule.ts:26,229` | 中低 | 中 |
| 25 | R | front-matter 正则每次解析 `.md` 时重新编译(4+ 处重复实现) | `crates/services/services-core/src/markdown.rs:15` 及同构处 | 低 | 极低 |
| 26 | F | Browser 检查器 2 个 Tauri 监听无卸载清理,场景关闭后常驻 | `web-ui/src/app/scenes/browser/BrowserPanel.tsx:78,114,137,147` | 低 | 低 |
| 27 | F | App.tsx 两处 listen effect 缺 `disposed` 旗标(StrictMode 下泄漏) | `web-ui/src/app/App.tsx:650-683,685-712` | 低 | 极低 |
| 28 | F | ToolExecutionService 4 个事件监听句柄丢弃,destroy 后重建即重复注册 | `web-ui/src/shared/services/tool-execution-service.ts:84-93,103-120` | 低 | 低 |
| 29 | F | 会话列表第 2 档展开无上限、未虚拟化 | `web-ui/src/app/components/NavPanel/sections/sessions/SessionsSection.tsx:591-595,1017` | 低 | 低 |
| 30 | R | 持久化全局 `FILE_LOCKS`/per-path 锁 map 永不淘汰,按路径无界增长 | `crates/services/services-core/src/persistence.rs:9-18`、`json_store.rs:307-314` | 低 | 低 |

---

## 二、逐条详情

### 高收益

#### 1. 会话消息追加 = O(N²) 全量重写 turn 快照【R,高】

**问题**:每次 `add_message` 都把当前 turn 的**全部**上下文消息深拷贝一份、`serde_json::to_string_pretty` 全量序列化、再原子写盘。第 N 条消息触发第 N 次全量序列化,单 turn 总成本 O(N²·消息体积)。

**证据链**(4 跳):
- `session_manager.rs:6310-6336`:`add_message` 尾部无条件调用 `persist_current_turn_context_snapshot_best_effort(session_id, "context_message_added")`;
- `session_manager.rs:1429-1433`:取出整个 `get_context_messages(session_id)` 传给 `save_turn_context_snapshot`;
- `persistence/manager.rs:1287-1298` + `646-654`:`sanitize_messages_for_persistence` 对每条消息执行 `message.clone()`(全量深拷贝)后 `write_json_atomic`;
- `json_store.rs:211-213`:`serde_json::to_string_pretty`(比 compact 大 30-50%)+ 临时文件 + `ReplaceFileW`(带 write-through 强制刷盘)。

**调用频率**:agentic 主循环逐条调用 —— `execution_engine.rs:3338`(每轮 assistant 消息)、`:3351`(**每个工具结果**)、`:3446/3502/3564/3628/3675/3844/3858`(各类注入)。50 轮 × 3 工具的会话 ≈ 200 次;10MB 上下文时单次追加 ≈ 10MB clone + 14MB JSON 字符串 + 14MB 强制刷盘磁盘写。这是 agent 会话卡顿随会话变长而恶化的首要嫌疑。

**优化方案**:
1. `sanitize_messages_for_persistence` 改 `Cow`:仅含 `Multimodal{images}`/`ToolResult` 需脱敏的消息才 clone,其余借用序列化;
2. `to_string_pretty` → `serde_json::to_writer`(compact)+ `BufWriter`;
3. 结构性修复:快照改 JSONL 追加式(每条消息一行,turn 结束/压缩时才全量重写),或加 dirty-window 合批(200ms 内多次 add_message 只落一次盘)。崩溃恢复语义保持:JSONL 天然可恢复到最后一条完整行。

#### 2. 文件树递归扫描的系统调用风暴【R,高】

**问题**:`build_tree_recursive`(`tree.rs:446-`)存在五个叠加低效,`build_tree_recursive_with_stats`(`tree.rs:572-769`)是同一段代码的复制,问题相同:
- `tree.rs:461-469`:**sort 比较器内做同步 stat** —— `a.path().is_dir()` 每次比较 2 次 `PathBuf` 分配 + 2 次阻塞系统调用,1000 项目录 ≈ 20,000 次同步 stat,全部在 async 上下文;
- `tree.rs:486 / 504 / 538→897`:每条目 `file_type().await` + `metadata().await` + `get_permissions_string` 内再 `fs::metadata` —— **3+ 次 stat/条目**;Windows 分支只需 `readonly()`,`:504` 的 metadata 已含该信息;
- `tree.rs:436`:`path.canonicalize()` 同步阻塞 IO,每目录一次;
- `tree.rs:446,452`:`Vec::new()` 无预分配;每条目 3 次 `to_string_lossy().to_string()`(`:480-484,542,543`);
- `tree.rs:771-793` + `107-124`:默认不读 `.gitignore`、`include_hidden: true`、`max_depth 50` → `node_modules/`、`target/` 全量递归。

**调用频率**:Tauri 命令 `get_file_tree`/`explorer_get_file_tree`(`commands.rs:2360`)→ 工作区打开与文件树刷新必经。15 万文件的中型前端仓 = 数十万次异步 stat + 上万次 sort 内同步 stat + ~45 万次字符串分配,单次可达数十秒。

**优化方案**:
1. 读目录时一次性落地 `Vec<(DirEntry, FileType, Metadata)>` 再排序,比较器零系统调用;
2. 用 `ignore::WalkBuilder`(`tree.rs:8` 已引入该 crate,search 路径已在用)替代手写递归,免费获得 gitignore + 并行遍历 + 每条目单次 stat;
3. `canonicalize` 移入 `spawn_blocking` 或改用文件 ID 去环;Windows 权限直接复用已取 metadata;
4. 两份复制代码合并为一个带 stats 回调的实现。

#### 3. 全局事件出口无条件深拷贝(PeerAwareEmitter)【R/I,高】

**问题**:`remote_connect_api.rs:558-561`:
```rust
async fn emit(&self, event_name: &str, payload: serde_json::Value) -> anyhow::Result<()> {
    self.inner.emit(event_name, payload.clone()).await?;   // 无条件深拷贝
    maybe_fanout_peer_ui_event(event_name, payload);       // 守卫(事件名单 + 是否有 peer)在函数内部才判断
    Ok(())
}
```
`PeerAwareEmitter` 包裹的是**全局** emitter(`lib.rs:2008-2015`)和当时存在的 LSP workspace emitter(`lsp_workspace_api.rs:189-192`),即所有 backend→frontend 事件(终端 chunk、文件变更、LSP 诊断、工具进度)每条都吃一次 `serde_json::Value` 深拷贝(逐节点堆分配,非 memcpy)—— 而绝大多数用户从不开 Peer Mode。同构问题:`terminal_api.rs:347-359` 对每个终端事件(64KB 峰值,~200 次/秒/终端)无条件 `serde_json::to_value` 后才进守卫。

**优化方案**:把 `should_fanout_peer_ui_event(event_name) && !attached_controllers().is_empty()` 判断提到 clone/to_value **之前**;不满足则 move payload 直接 emit,零拷贝。改动约 10 行,无语义变化,收益覆盖所有高频事件通道。

#### 4. 终端 transcript 同步 IO 跑在 async 事件循环【R,高】

**问题**:`transcript.rs:412-437` `append_inner` 每个 chunk 执行 `OpenOptions::open` + `write_all` + `flush`(open/close 每次重来);入口 `with_store`(`transcript.rs:176-187`)持 **`std::sync::Mutex`** 执行上述阻塞 IO;调用点 `session/manager.rs:346` 位于 `tokio::spawn` 的 PTY 事件循环内,无 `spawn_blocking`。

**调用频率**:上游 DataBufferer 合批后仍高达 ~200 chunk/秒/终端(5ms flush 间隔),即每秒 200 次 `CreateFile+WriteFile+FlushFileBuffers+CloseHandle`;Windows 上叠加 Defender 实时扫描,单次 100µs-1ms。多终端并发时同一把 std Mutex 让 tokio worker 互相 park(非 yield)。

**优化方案**:`TranscriptStore.writers: HashMap<String, TranscriptWriter>` 已存在,补上常开的 `BufWriter<File>` 句柄;去掉每 chunk 的 open/close 与 `flush()`(改定时/会话结束 flush);整个写路径移到专用写线程 + mpsc 或 `spawn_blocking`。

#### 5. LSP 诊断/响应的五连深拷贝【R,高；Runtime 已退役】

> 历史项：以下描述只对应审阅提交 `48a003b73`。LSP Runtime 删除后，该优化任务已关闭，禁止实施或恢复相关代码。

**问题**:一次 `publishDiagnostics` 通知,同一份 `Vec<Value>` 被拷贝 5 次:
1. `process.rs:445` `diagnostics_arr.clone()`(`notif` 在 `:371` 已拥有,可 `Value::take` 移出);
2. `workspace_manager.rs:889` 写缓存 clone(且持 `lsp_manager` 读锁跨 await,`:886-891`,与 `start_server` 的写锁竞争);
3. `workspace_manager.rs:896` 构造事件再 clone;
4. `workspace_manager.rs:906` `serde_json::to_value(&event)` 整树重建;
5. `remote_connect_api.rs:559` PeerAwareEmitter 再深拷贝(见问题 3)。

另 `process.rs:164-170` 因 `match &message` 借用而 `response.clone()` —— 每条 LSP **响应**(semanticTokens/full、documentSymbol 可达数 MB,随滚动/光标持续产生)整体深拷贝一次。

**频率**:工作区打开时 rust-analyzer/tsserver 为数百文件连发诊断;编辑期每次防抖后触发。200 条诊断 ≈ 100KB Value 树 × 5 = 每条通知 500KB 逐节点分配/释放。

**历史优化方案（已关闭，不得实施）**:`process.rs:164` 改 `match message` 按值解构消除响应 clone;`:445` 用 `Value::take`;`workspace_manager` 缓存与事件间用 `Arc<Vec<Value>>` 共享;`diagnostics_cache` 提为独立 `Arc<RwLock<..>>` 摆脱外层 `lsp_manager` 锁。

#### 6. 流式事件队列:每 delta 的分配与锁风暴【R,高】

**问题**:`agent-stream/lib.rs:806-819` `handle_text_chunk` 每个 SSE delta 构造 `AgenticEvent::TextChunk`,`session_id/turn_id/round_id/attempt_id` 各一次 String clone;下游 `event_queue.rs:178-207`:`queue.push(envelope.clone())`(深拷贝#1)→ `channel.sender.send(envelope.clone())`(#2)→ `broadcast_tx.send(envelope)`(按订阅者数再各克隆)→ `self.stats.lock().await`(第 2 把异步锁,只为计数 +1)。

**频率**:每模型 token 一次。2000 token 回复 ≈ 30,000 次堆分配 + 4,000 次 tokio Mutex 获取,全部搬运恒定不变的 id 元数据;`ToolEventData::Completed{result}` 走同路时 envelope clone 是完整工具结果(数百 KB)的深拷贝。

**优化方案**:① id 字段改 `Arc<str>`(turn 内共享);② `broadcast::Sender<Arc<EventEnvelope>>` 消除 fan-out 深拷贝;③ stats 改 `AtomicU64`;④ 评估 legacy queue 对 TextChunk 级事件是否可跳过(与 broadcast 语义重复)。

#### 7. ACP `raw_input` 单次 update 深拷贝 3-4 次【R,高】

**问题**:`acp/client/stream.rs:294-301` 每条 `ToolCallUpdate` 走 `update.fields.raw_input.clone()`(#1)→ `update_from_fields`(`:204-221`)内 `self.calls.get(tool_id).cloned()`(#2,整个 snapshot)→ `previous...raw_input.clone()`(#3)→ `insert(..., snapshot.clone())`(#4)。`ToolCall` 路径(`:193,231-240`)同构。`raw_input` 对 Edit/Write 工具含完整 `old_string/new_string/content`,可达数百 KB;ACP agent 工具执行期持续推送 status/content update(`manager.rs:1247-1254` 主循环逐条调用)。

**优化方案**:`AcpToolCallSnapshot.raw_input` 改 `Option<Arc<serde_json::Value>>`;仅在新值为 `None` 时 `Arc::clone` 旧值;insert 后返回引用而非先 clone 再 insert。

#### 8. LS 工具同步 BFS 阻塞 tokio worker【R,高】

**问题**:`listing.rs:137-160` 用 `std::fs` 做同步 BFS:入队时 `symlink_metadata`(`:151-155`,已获 is_dir/mtime),出队时(`:137`)对同一路径**再** stat 一遍;`:144` 每保留一项 `entry.clone()`;`:41` 结果 Vec 未按 limit 预分配。调用点 `ls_tool.rs:292` 在 async `call_impl` 中直接调用,**无 `spawn_blocking`** —— 而同目录 `grep_tool.rs:794`、`glob_tool.rs:475` 都正确包了 `spawn_blocking`,证明是遗漏而非设计。agent 每次 LS 调用都 park 一个 tokio worker,期间该 worker 上的流式输出/终端事件全部停摆。

**优化方案**:① 包 `spawn_blocking`(与 grep/glob 对齐);② `is_symlink` 入队时存进 entry,删掉出队二次 stat;③ `Vec::with_capacity(limit)`;④ push 改 move。

#### 9. 侧栏拖拽 mousemove 全树重渲染【F,高(拖拽期间)】

**问题**:`WorkspaceBody.tsx:64-77` 拖拽期间每个 mousemove 直接 `setNavWidth(newWidth)`,无 rAF/节流;`navWidth` 是 WorkspaceBody 自身 state,其 JSX 渲染 `<NavBar/><NavPanel/><SceneBar/><SceneViewport/>`(`:106-132`),且 `NavPanel.tsx:44`、`SceneViewport.tsx:84` 均无 `React.memo` → 每个 mousemove 重渲染整棵工作区子树(含 Monaco 宿主、会话面板)。同仓库同类拖拽全部做了 rAF + 直写 DOM(`SessionScene.tsx:214-226/262-274`、`hooks/useResizer.ts:56-69`、`SplitHandle.tsx`),此处是唯一例外,实锤遗漏。

**优化方案**:照抄 SessionScene 模式 —— rAF 合并 + 拖拽期间 `navAreaRef.current.style.setProperty('--nav-width', ...)` 直写(该 CSS 变量已存在,`:104`),`mouseup` 时才 `setNavWidth` 落状态。

### 中高 / 中收益

#### 10. 桌宠 120ms `cursorPosition()` 常驻 IPC【F/I,中高】

`AgentCompanionDesktopPet.tsx:490` 每 120ms(≈8.3 次/秒)跨进程调 `cursorPosition()` + `outerPosition()`(`:454`),回调内再 `querySelectorAll` + 逐气泡 `getBoundingClientRect`(`:471-477`)+ 2 次 setState(`:479-482`)。桌宠窗口一开就持续运行,无 `visibilityState`/focus 守卫。这是全仓频率最高的常驻 IPC。
**方案**:无气泡或窗口非活动时停表;`getBoundingClientRect` 结果缓存到 resize/位移事件;长期可改后端鼠标 hook 推送。

#### 11. 编辑器 1s `get_file_metadata` 轮询冗余【F/I,中高】

`CodeEditor.tsx:1990`(`FILE_SYNC_POLL_INTERVAL_MS = 1000`,`:120`;peer 模式 15s)+ `MarkdownEditor.tsx:428` 同构:激活 tab 每秒 1 次 `get_file_metadata` invoke,差异时追加 `get_file_editor_sync_hash`/读内容。有 `isActiveTab` 守卫(`:1976`)不随 tab 数放大,但仓库已有后端推送通道 `file-system-changed`(`tools/file-system/services/FileSystemService.ts:120`、`TauriExplorerFileSystemProvider.ts:268`),1s 轮询与之功能重叠。
**方案**:订阅 `file-system-changed` 按 `filePath` 过滤,事件命中或窗口重获焦点时才校验 metadata;轮询降级为 30s 兜底。注意远程工作区(无本地 watcher)需保留轮询分支。

#### 12. 文件监视线程 per-event `block_on` + watcher 全量重建【R,中】

`file_watch/service.rs:163-194`:spawn_blocking 循环内对**每个** notify 事件 `rt.block_on(Self::convert_events(...))`(`:172`)—— 仅为读一次异步 `RwLock` 的路径表;debounce 到期再 `block_on(flush)`(`:185`)。大量文件变更(git checkout、npm install)时每事件一次跨运行时往返。另:每次 `watch_path`/`unwatch_path` 都 `create_watcher` 重建整个 watcher 并重注册所有根(`:99,112`),Windows 上递归 watch 注册成本高。
**方案**:线程启动时快照 `HashMap<PathBuf, FileWatcherConfig>`(或改 `std::sync::RwLock`),过滤逻辑同步执行,`block_on` 仅保留在 debounce flush;watcher 支持增量 `watch/unwatch` 而非整体重建。

#### 13. 目录分页命令每页全量重扫【R/I,中】

`commands.rs:2408-2441` `get_directory_children_paginated`:每页调用 `get_directory_contents_with_remote_hint`(全量读目录 + 每条目 metadata),再 `skip(offset).take(limit)` 内存切片 —— 翻到第 K 页要重扫 K 次全目录;`service.rs:110-119` 与 `tree.rs` 层均无缓存。10 万条目的目录逐页浏览成本 O(K·n)。附带:`commands.rs:2315-2347` 手工把 `FileTreeNode` 转 `serde_json::Value` 后 Tauri 再序列化一遍,大树双倍分配 —— 直接返回 `Vec<FileTreeNode>`(已 derive Serialize)即可。
**方案**:短 TTL(数秒)按路径缓存目录快照供分页复用,或游标式增量读取;删除手工 Value 转换层。

#### 14. EventBus 事件历史:每 emit O(1000) 拷贝 + payload 强引用【F,中】

`EventBus.ts:295-302`:`recordEvent` 在每次 `emit()` 无条件执行(`:144`),超过 `MAX_HISTORY`(1000)后每次 emit 都 `slice(-1000)` 复制整个数组;`metadata.data` 持 payload 强引用 —— 最近 1000 个事件的完整 payload(含 `AppManager.ts:370/378` 的整个 app state 快照)无法 GC。全局单例,117 处 emit 调用点。
**方案**:改环形缓冲(写指针覆盖,零拷贝);生产环境只记事件名 + 时间戳,payload 仅 dev 模式保留。

#### 15. SessionStateMachine 每次转换 structuredClone【F,中】

`SessionStateMachine.ts:61-72`:`getSnapshot` 对 context 做 `structuredClone` + `new Set` + `slice(-100)`;`:320` 每次 `transition()` 都调用并广播,`useSessionStateMachine.ts:28-32` 直接灌进 React state。agent 执行期转换极频繁(工具开始/进度/完成/权限)。
**方案**:context 改不可变更新(transition 产生新对象),`getSnapshot` 返回引用,删除 clone。风险:需审查订阅方是否有原地修改 snapshot 的代码。

#### 16-17. 搜索路径的行级分配与冗余 stat【R,中】

- `tree.rs:1485-1491`:内容搜索对**每一行**先 `String::from_utf8_lossy(..).to_string()` 再 `is_match` —— 未命中行(绝大多数)的分配全部浪费;10 万行仓库 = 10 万次无用分配。改为在 `Cow`/`&str` 上先 match、命中才 `to_string`,并用 `read_until` 复用行缓冲。`tree.rs:1140/1275` 收尾 `lock_search_results(&results).clone()`(至多 1 万条)改 `mem::take`。
- `grep_search.rs:776`:walker 循环内 `path.is_file()` 同步 stat,而 `:783-785` 已在用 `entry.file_type()` —— 直接删掉换 `file_type` 判断;`:835-840,875`:sink 输出整串 → 逐行 `to_string` → `join("\n")` 完整往返,让 `GrepSink` 直接产出 `Vec<String>` 或全程单串切片;`:743` `content_lines` 预分配。

#### 18-21. 前端高频路径其余确认项【F,中】

- **桌宠打字机**(`AgentCompanionDesktopPet.tsx:308-339`):effect 依赖数组含 `typedOutputBySessionId` 而 interval 回调本身在改它 → 每 28ms 卸载重建 interval;`:335` 的 `useLayoutEffect` 同 deps,每 28ms 对所有输出元素写 `scrollTop`(强制布局)。目标文本改 `useRef`,deps 改 `[hasTypingOutput]`。
- **Tooltip**(`Tooltip.tsx:294-296`):`visible` 期间捕获阶段监听全应用 scroll,`calculatePosition`(`:171-205`)每滚动帧 2 次 `getBoundingClientRect`(强制同步布局)+ 3 次 setState。rAF 合并 + `passive: true` + 三 state 合一。
- **鼠标跟随光效（已关闭）**：功能及全局指针监听已删除，无后续优化任务。
- **SnapshotAPI**(`SnapshotAPI.ts:514-531`):`get_session_turns` 后 for-await **串行**逐 turn `get_turn_files`,百轮会话 = 百次串行 IPC。后端加批量命令 `get_turns_files(session_id, turn_indices[])`;短期先改 `Promise.all`。

#### 22-24. 中低收益

- `session/manager.rs:285`:`String::from_utf8_lossy(&data).to_string()` —— `data: Vec<u8>` 已拥有,合法 UTF-8 时 `Cow::Borrowed` 被强制再分配(最大 64KB/chunk)。改 `String::from_utf8(data).unwrap_or_else(|e| ...)` 零拷贝复用;`:429` tap 分发 `data_str.clone()` 改 `Arc<str>`。
- `exec.rs:934-959`:`HeadTailText::push_str` 逐 `char` 推 `VecDeque<char>`(4B/char),大输出千万次 push/pop;head 无 `with_capacity`。tail 改字节环形缓冲 + UTF-8 边界对齐。
- `PeerSessionRefreshModule.ts:229`:仅 Peer 模式生效的 3s 快照 reconcile,可加"最近无事件才轮询"守卫或退避。

### 低收益(顺手修)

- **#25**:`markdown.rs:15` front-matter 正则每次 `Regex::new`;调用方为文件监视触发的命令/agent 扫描(`command_source.rs:572` 等 6 处,其中 `opencode-adapter/command_source.rs:769,774` 失败重试同内容编译两次)。改 `static LazyLock<Regex>`,并将 `skills/types.rs:105`、`custom_agent.rs:549` 等同构实现统一。
- **#26**:`BrowserPanel.tsx` `inspectorUnlistenRef.current` 仅在手动 `stopInspector`(`:78`)调用,无卸载 `useEffect` 清理 → 检查器开启时关闭 Browser 场景,2 个 Tauri 监听 + `addContext` 闭包常驻,后续事件仍向已卸载 store 注入。加 `useEffect(() => () => stopInspector(), [])`。
- **#27**:`App.tsx:650-683,685-712` 两个 listen effect 缺 `disposed` 旗标(同文件 `:188,565` 是正确范本);StrictMode 下稳定泄漏,`agent-companion://open-session` 被处理两次。
- **#28**:`tool-execution-service.ts:103-120` 四个 `api.listen` 返回值丢弃,`destroy()`(`:84-93`)不解除;destroy→getInstance 重建即重复注册。保存 unlisten 数组并在 destroy 调用。
- **#29**:`SessionsSection.tsx:591-595` 第 2 档展开返回全量(无上限、无虚拟化);默认档只渲 5/10 条无问题。第 2 档接 Virtuoso 或 200 条上限 + 加载更多。
- **#30**:`persistence.rs:9-18` / `json_store.rs:307-314` per-path 锁 map 永不淘汰,长期运行按路径数无界增长。改 `Weak` 或定期清理无持有者条目。

### IPC 层整体评估

- **已做对的**:终端输出后端合批(`data_bufferer.rs:89-139`,5ms/64KB,零拷贝移交);文件搜索进度 `BatchedFileSearchProgressSink`;前端 `EventBatcher`(rAF + 32ms text-chunk 上限)对流式事件做了渲染层合并;`tauri-adapter.ts:120-155` 的 listen 包装正确处理了 async-unlisten 竞态;`ConfigAPI` 已批量取配置。
- **主要缺口**:① 事件出口的无条件深拷贝(#3,修复最划算);② `agentic://text-chunk` 与 ACP 路径(`acp_client_api.rs:357-400`)后端逐 delta emit,每次 emit 是一次独立 IPC 序列化 + 广播到所有 webview —— 前端已批,后端可选做 16-33ms 合批进一步降低 IPC 次数(中低,前端已缓解);③ 可批量化的串行小调用:`get_turn_files`(#21)、编辑器轮询(#11)、桌宠 `cursorPosition`(#10)。

---

## 三、实施建议清单(可直接派发)

按投入产出比排序;每项独立可交付。风险 = 引入回归的可能性。

| 任务 | 内容 | 涉及文件 | 风险 |
|---|---|---|---|
| T1 | PeerAwareEmitter/terminal 事件出口:把 `should_fanout_peer_ui_event && !attached_controllers().is_empty()` 提到 `payload.clone()`/`to_value` 之前,不满足则 move 零拷贝 | `apps/desktop/src/api/remote_connect_api.rs:558-561`、`terminal_api.rs:347-359` | 低 |
| T2 | LS 工具:`list_directory_entries` 包 `spawn_blocking`(对齐 grep/glob);entry 增加 `is_symlink` 字段消除出队二次 stat;`Vec::with_capacity(limit)`;push 改 move | `crates/services/services-core/src/filesystem/listing.rs`、`assembly/core/src/agentic/tools/implementations/ls_tool.rs:292` | 低 |
| T3（已关闭） | 历史 LSP 消息零拷贝方案；LSP Runtime 已退役，不得实施或恢复 | 已删除的 `crates/services/services-core/src/lsp/process.rs`、`assembly/core/src/service/lsp/workspace_manager.rs` | 不适用 |
| T4 | 会话持久化:`sanitize_messages_for_persistence` 改 Cow 按需 clone;`to_string_pretty`→compact `to_writer`;加 200ms dirty-window 合批(保留 turn 结束强制落盘) | `assembly/core/src/agentic/persistence/manager.rs:646-654,1287-1298`、`services-core/src/json_store.rs:211` | 中(崩溃恢复语义需回归测试) |
| T4b | (进阶)turn 快照改 JSONL 追加式,turn 结束/压缩时才全量重写 | 同上 + 读取端 | 高(格式迁移,需兼容旧快照) |
| T5 | 文件树扫描:sort 前落地 `(entry, file_type, metadata)` 元组;Windows 权限复用 metadata;`canonicalize` 入 spawn_blocking;Vec 预分配;评估切换 `ignore::WalkBuilder` 并默认尊重 gitignore;合并 stats 复制版 | `services-core/src/filesystem/tree.rs:436-560,572-769,897` | 中(gitignore 默认值属行为变化,建议加开关) |
| T6 | 终端 transcript:`TranscriptWriter` 持常开 `BufWriter<File>`,去 per-chunk open/flush;写路径移 `spawn_blocking`/专用线程 + mpsc | `services/terminal/src/transcript.rs`、`session/manager.rs:346` | 中(注意会话结束 flush 与文件轮转) |
| T7 | 流式事件队列:`AgenticEvent` id 字段改 `Arc<str>`;broadcast 改 `Arc<EventEnvelope>`;stats 改 `AtomicU64` | `execution/agent-stream/src/lib.rs`、`agent-runtime/src/event_queue.rs:178-207` | 中(类型改动波及面广,建议分 3 个 PR) |
| T8 | ACP tracker:`raw_input` 改 `Option<Arc<Value>>`,消除 4 次深拷贝 | `interfaces/acp/src/client/stream.rs:180-320` | 低 |
| T9 | 侧栏拖拽:rAF 合并 + 拖拽期直写 `--nav-width` CSS 变量,mouseup 落 state(照抄 `useResizer.ts` 模式) | `web-ui/src/app/layout/WorkspaceBody.tsx:53-91` | 低 |
| T10 | 桌宠:① hover 轮询加窗口活动/有气泡守卫,rect 缓存;② 打字机目标文本改 useRef,interval deps 改 `[hasTypingOutput]`,去每 tick 重建 | `AgentCompanionDesktopPet.tsx:308-339,440-500` | 低 |
| T11 | 编辑器外部变更检测:订阅 `file-system-changed` 按路径过滤 + focus 校验,本地轮询降 30s 兜底(远程保留现状) | `CodeEditor.tsx:1830-2000`、`MarkdownEditor.tsx:400-440` | 中(需覆盖 watcher 不可用的降级路径) |
| T12 | EventBus 历史改环形缓冲;生产只记事件名+时间戳 | `web-ui/src/infrastructure/event-bus/EventBus.ts:280-310` | 低 |
| T13 | 状态机:context 不可变更新,`getSnapshot` 免 clone(先审查订阅方无原地修改) | `flow_chat/state-machine/SessionStateMachine.ts` | 中 |
| T14 | 搜索行级优化:先 match 后分配 + 行缓冲复用;grep 删冗余 `is_file()`、sink 直出 `Vec<String>`;结果收尾 `mem::take` | `services-core/src/filesystem/tree.rs:1140,1275,1485`、`tool-execution/src/search/grep_search.rs:743,776,835-875` | 低 |
| T15 | file_watch:线程内路径表改同步快照,去 per-event `block_on`;watcher 增量注册 | `services-integrations/src/file_watch/service.rs:117-197` | 低 |
| T16 | 目录分页短 TTL 缓存 + 删手工 Value 转换层(直接返回 derive Serialize 的节点) | `apps/desktop/src/api/commands.rs:2315-2441` | 中(缓存失效需订阅 file watch) |
| T17 | 批量 IPC:后端新增 `get_turns_files` 批量命令,前端 SnapshotAPI 改批量(过渡期 `Promise.all`) | `SnapshotAPI.ts:514-531` + 对应 Rust command | 低 |
| T18 | PTY chunk:`from_utf8_lossy().to_string()` 改零拷贝路径;tap 改 `Arc<str>`;`HeadTailText` 改字节环形缓冲 | `services/terminal/src/session/manager.rs:285,429`、`exec.rs:934-959` | 低 |
| T19 | 杂项清理包:front-matter 正则静态化并统一实现;App.tsx 补 disposed;BrowserPanel 补卸载清理;ToolExecutionService 保存 unlisten;FILE_LOCKS 弱引用淘汰;会话列表第 2 档限流 | 见 #25-#30 | 极低 |

**建议实施顺序**:T1/T2/T8/T9(低风险高收益,先行)→ T3/T6/T4 → T5/T7 → 其余按资源排期。T4b 与 T5 的 gitignore 默认值涉及行为变化,需产品确认。

---

## 四、误报排除记录(已确认无需处理)

| 候选 | 排除依据 |
|---|---|
| 终端输出无合批 | `pty/data_bufferer.rs:89-139` 已有 5ms/64KB 合批,`mem::take` 零拷贝移交,预分配到位 |
| replay 历史 O(n) 重建 | `session/replay.rs:65-82` `push_str` 合并,摊还 O(1)。*(顺带发现正确性缺陷:`:130-135` 超 100KB 时 `pop_front` 一次丢掉全部合并历史,重连 replay 为空 —— 建议按字节截断,已列 T19 之外单独跟进)* |
| grep/edit_constraint_guard/两个 redaction/claude command_source 的 `Regex::new` | 均为每次搜索一次性编译或 `OnceLock/get_or_init` 静态缓存(`grep_search.rs:654`、`edit_constraint_guard.rs:203-217`、`session_usage/redaction.rs:77-90`、`diagnostics/redaction.rs:114-173`、`claude-code-adapter/command_source.rs:723-757`) |
| 终端/exec 的 `.lock().await` 跨 await | `session/manager.rs:288-297,411-444`、`exec.rs:744-791` 均为显式作用域收窄,锁外 notify,写法正确 |
| persistence per-session/per-path 锁跨 IO | 锁粒度即单 session/单文件,串行化是设计目标 |
| `persistence.rs` save_json 每次备份 | 调用方仅 workspace/cron/git-history 低频配置写 |
| ThemeService JSON 深拷贝(`ThemeService.ts:120`) | 仅注册/导入自定义主题时执行,不在切换/渲染路径 |
| canvasStore 两处 structuredClone | 仅工作区切换触发,LRU 有上限 |
| AppManager resize(`AppManager.ts:413-430`) | 已 200ms debounce,shutdown 清理 |
| SessionScene 拖拽 mousemove | rAF + 直写 DOM,本仓正确范例 |
| `api.listen` 竞态泄漏(大范围) | `tauri-adapter.ts:120-155` 已正确处理 promise/unlisten 竞态;抽查 10+ 调用点均正确清理 |
| FileExplorer 文件树 | >100 节点走 react-virtuoso 虚拟化(`FileExplorer.tsx:219,490-503`),行组件 memo |
| ModelRoundItem / Markdown 渲染 | 全量 memo + useMemo + 渐进渲染,是全仓性能做得最好的区域 |
| 帐号/文件面板/Peer ping 轮询 | 60s / 15s / 20s,频率合理且有存在理由 |
| ACP `update_session_from_events` 三次遍历 | events 长度 1-2,clone 仅低频事件命中时发生 |
| ConfigAPI 逐路径读取 | 主路径已批量 `get_configs`,逐路径仅为降级 fallback |
| 启动期 `block_in_place`(`lib.rs:708`)/ workspace_search `shutdown_blocking` | 一次性启动/关闭路径,非运行时热点 |
| 文件搜索进度 emit | `BatchedFileSearchProgressSink` 已合批 |

*报告完。共收录 30 项确认问题(高 9 / 中高 2 / 中 13 / 低 6),全部经上下文与调用频率核实。*

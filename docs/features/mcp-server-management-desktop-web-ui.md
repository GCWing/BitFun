# MCP Server 管理与运行态（Desktop / Web UI）功能提案

> 状态：提案 / 待评审
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`src/crates/services/services-integrations/AGENTS.md`](../../src/crates/services/services-integrations/AGENTS.md)
> - [`src/crates/assembly/core/AGENTS.md`](../../src/crates/assembly/core/AGENTS.md)
> - [`docs/architecture/agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)
> - [`src/apps/desktop/AGENTS.md`](../../src/apps/desktop/AGENTS.md)
> - [`src/web-ui/AGENTS.md`](../../src/web-ui/AGENTS.md)

### 功能类型
<!-- 请描述功能建议的类型 -->
- [x] 新功能
- [ ] 现有功能增强
- [ ] 用户体验优化
- [ ] 性能优化
- [ ] 接口/集成扩展
- [ ] 其他

### 优先级
<!-- 请选择功能建议的优先级 -->
- [ ] 紧急（P0 - 核心需求，强烈期望实现）
- [x] 高（P1 - 重要功能，期望尽快实现）
- [ ] 中（P2 - 有价值的功能，计划实现）
- [ ] 低（P3 - 锦上添花，有时间再做）

### 背景与动机
<!-- 请描述你为什么需要这个功能，解决了什么痛点 -->
- 用户希望把外部 MCP server（本地 stdio 进程、远程 SSE / streamable HTTP）作为可插拔工具源接入 BitFun agent，统一在会话中调用其工具、读取其 resource、使用其 prompt。当前缺少一个完整的「配置 → 启动 → 健康 → 重连 → 工具目录」管理面，用户难以在 Desktop/Web UI 内自助注册、诊断和运维 MCP server。
- 痛点：① MCP server 配置散落、缺校验与导入能力，添加门槛高；② server 进程/远程连接的健康状态、启动失败原因对用户不透明，排查困难；③ 连接断开后无自动重连或退避策略，会话中途工具失效；④ 多 server 并存时缺乏统一启停、自动启动与运行态视图；⑤ 需 OAuth 的远程 server 授权流程缺失或割裂。

### 功能描述
<!-- 请详细描述你期望的功能行为 -->
- 在 Desktop/Web UI 提供 MCP server 的端到端管理面，覆盖六个环节：
  1. **配置与校验**：支持 stdio（command + args + env）、SSE、streamable HTTP 三种 transport；配置来源区分 local / remote；提供 JSON 配置（`mcpServers` 形态）的校验、格式化与从外部格式（如 Cursor）导入；配置持久化、可启用/禁用、可命名。
  2. **生命周期管理**：支持 start / stop / restart / 启用-禁用 / auto-start；每次启动带超时；启动前校验前置条件（command 是否可用、远程 URL 是否可达、是否已授权），不可启动时给出 `start_disabled_reason` 而非静默失败。
  3. **运行态与健康**：实时展示每个 server 的 status / status_message / server_type / transport / url / command_resolved_path / command_source / command_available；进程或连接异常时显式标注阶段（配置错误 / 启动超时 / 握手失败 / 运行中断 / 重连中）。
  4. **重连与容错**：连接断开后按退避策略（基础延迟 → 上限）自动重连，重连计数与下次重试时间可见；重连上限或用户主动停止后转为终态并保留可手动重启入口；远程 server 重连不得污染本地 stdio 进程的清理边界。
  5. **进程归属与隔离**：本地 stdio server 作为受管子进程托管，遵循仓库既定的 managed-descendant 清理边界（Unix 用独立进程组、Windows 用 kill-on-close Job Object，附加失败即 fail-closed），生命周期停机时回收子进程；该机制是生命周期收容，不是 OS 沙箱或资源限额，UI 需对残余风险保持明确。
  6. **授权**：远程 server 支持 OAuth 授权引导（授权码回跳、凭证存储、授权状态 `auth_configured` / `auth_source` / `oauth_enabled` 可见），未授权时阻止启动并提供授权入口。
- 实现边界遵循服务层与服务集成层规则：MCP config/process/transport 生命周期、server runtime state（registry / 连接池 / catalog cache / reconnect / runtime-only config）、lifecycle policy、OAuth 凭证存储与授权引导、具体协议依赖与结果内容渲染归 `bitfun-services-integrations`（`mcp` feature）；Core 只保留兼容 facade 与产品级回调/会话/重连编排，不得重新引入具体协议依赖；MCP wire 类型可投影到执行层 tool bridge 描述符，但工具注册表装配、manifest 过滤、`GetToolSpec` 执行与 bridge 呈现/校验行为不在服务集成层。
- Desktop/Web UI 只消费稳定的运行态事实与服务接口，不在 UI 层重定义 MCP 协议、transport 实现或外部工具契约。

### 期望效果 / 使用场景
<!-- 描述该功能在什么场景下使用，以及使用后的预期效果 -->
1. 用户在设置中打开「MCP Servers」面板，点击新增：选择 transport（stdio/SSE/streamable HTTP），填写 command 或 URL，保存后即时校验并给出可读错误；支持从外部 JSON / Cursor 配置一键导入多个 server。
2. 在面板中可见每个 server 的运行态：状态、transport、命令解析路径/来源、是否已授权、是否可启动；不可启动项显示原因（如 command 未找到、URL 不可达、未授权）。
3. 用户对 stdio server 点「启动」：进程作为受管子进程拉起，握手成功后工具目录刷新并入会话；停止时子进程被可靠回收，不残留。
4. 远程 streamable HTTP server 在网络抖动断开后自动按退避策略重连，面板显示「重连中（第 N 次，下次 Xs）」；超出上限转为「已断开」并提供手动重启。
5. 需 OAuth 的远程 server 在首次启动前弹出授权引导，完成后授权状态回写，后续启动免再次授权；凭证存储位置对用户可见且可撤销。

### 设计草案 / 参考示例
<!-- 如有设计稿、草图或参考的产品示例，请附在此处 -->
- 配置形态：`mcpServers` JSON（server 名 → {command/args/env | url, transport, source, autoStart, enabled}），校验与格式化遵循 MCP JSON config 校验规则，支持 legacy type 归一（`stdio` / `local` / `sse` / `remote` / `http` → local|remote × stdio|sse|streamable-http）。
- 运行态数据模型：每个 server 暴露 id / name / status / statusMessage / serverType / transport / enabled / autoStart / url? / authConfigured? / authSource? / oauthEnabled? / command? / commandAvailable? / commandSource? / commandResolvedPath? / startSupported / startDisabledReason?。
- 重连策略：基础延迟 2s、上限 60s、轮询间隔 5s 的退避；可见重连计数与下次重试时间。
- 进程归属：本地 stdio server 走 `services-core::process_tree` 的受管子进程清理边界（Unix 进程组 / Windows Job Object）。
- 协议分层：transport（stdio / 远程 streamable HTTP）与服务/协议映射独立测试，远程 transport 契约测试与本地 stdio 解耦。

### 是否愿意贡献
<!-- 是否愿意参与该功能的开发或讨论 -->
- [ ] 我愿意参与开发
- [x] 我愿意参与讨论和测试
- [ ] 仅提出建议

### 补充说明
<!-- 其他你认为有助于理解功能建议的信息，如相关 Issue 链接、文档等 -->
- 本提案聚焦 server 管理与运行态；MCP tool 目录呈现/调用、resource/prompt 交互、远程 workspace MCP 等作为独立 facet，可另立提案，避免范围扩散。
- 边界约束：不得在 `core-types` / `runtime-ports` 内重定义 MCP 配置/transport；具体协议依赖与结果渲染归 `bitfun-services-integrations` 的 `mcp` feature；工具注册表装配与 `GetToolSpec` 执行属更高层；UI 层只消费稳定运行态事实。
- 安全与隔离声明：受管子进程清理是生命周期收容，不等于 OS 沙箱或 CPU/内存/文件/网络资源限额；产品策略必须在禁用相应 server 时明确报告 `policy-limited`，不得宣称已被沙箱拦截。
- 验证基线（建议）：`cargo check -p bitfun-services-integrations --no-default-features --features mcp`、`cargo test -p bitfun-services-integrations --no-default-features --features mcp --test mcp_contracts`、`pnpm run check:core-boundaries`；远程 streamable HTTP transport 契约测试独立；Desktop 侧 `cargo check -p bitfun-desktop && cargo test -p bitfun-desktop`；Web UI 侧 `pnpm run type-check:web`。
- 若后续需把该能力下沉到 CLI/TUI 或移动端，应作为同级 adapter 单独评审，不在本提案内合并 surface。

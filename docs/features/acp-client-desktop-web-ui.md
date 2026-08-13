# ACP Client 能力增强（Desktop / Web UI）功能提案

> 状态：提案 / 待评审
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`src/crates/interfaces/acp/AGENTS.md`](../../src/crates/interfaces/acp/AGENTS.md)
> - [`docs/architecture/agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)
> - [`docs/architecture/product-architecture.md`](../architecture/product-architecture.md)
> - [`src/web-ui/AGENTS.md`](../../src/web-ui/AGENTS.md)

### 功能类型
<!-- 请描述功能建议的类型 -->
- [ ] 新功能
- [x] 现有功能增强
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
- BitFun 已在 `src/crates/interfaces/acp` 实现 ACP Client 半区，桌面/Web UI 通过 `acp:<clientId>` agentType 把外部 ACP agent 接入 flow_chat（见 `src/web-ui/src/flow_chat/utils/acpSession.ts`）。内置 preset 当前覆盖 `opencode` / `omp` / `claude-code` / `codex`（见 `src/crates/interfaces/acp/src/client/builtin_clients.rs`），但客户端接入、健康度、远程 workspace、模型/模式目录与工具/权限交互在 Desktop/Web UI 的呈现仍较碎片化，缺少统一的「客户端健康 → 会话建立 → 运行态」闭环体验。
- 痛点：① 多客户端并发使用时缺乏统一的健康/可用性视图；② 远程 workspace 复用本地 ACP 客户端配置时，探测/安装/降级链路对用户不透明；③ 会话级 config option（如 fast-mode、thought_level）与原生 BitFun agent 的模型/模式目录存在呈现差异；④ permission RPC 与 tool card 桥接在 Web UI 上的确认/拒绝 UX 与原生工具不一致。

### 功能描述
<!-- 请详细描述你期望的功能行为 -->
- 在 Desktop/Web UI 增强并收敛 ACP Client 的端到端体验，覆盖四个环节：
  1. **客户端健康与接入**：统一展示已注册客户端（builtin + 用户自定义）的可用性探测结果（`AcpClientRequirementProbe` / `RemoteAcpClientRequirementSnapshot`），缺失 npm adapter / CLI 时提供一键安装或降级提示，远程 workspace 复用本地配置并显式标注探测阶段。
  2. **会话生命周期**：复用 `AcpClientService` 的 NewSession / LoadSession / ResumeSession / CloseSession 路径，在 UI 上统一「创建 ACP 会话」入口（沿用 `nav-workspace-menu-create-acp-session` testid），并补齐 load/resume 失败时的可重试临时状态提示。
  3. **运行态配置**：把 `AcpSessionConfigOption`（boolean / select）与 `AcpSessionModelOption` 在会话侧栏统一呈现，使 fast-mode、thought_level 等与原生 reasoning catalog 对齐（参考 `src/web-ui/src/flow_chat/utils/acpSessionConfig.ts` 的 projection 逻辑）。
  4. **工具与权限交互**：对齐 `tool_card_bridge` 的外部 agent tool 命名/结果形状与原生 tool card；permission 请求的 approve/reject + option 选择在 Web UI 上与原生工具确认流一致。
- 实现边界遵循 `src/crates/interfaces/acp/AGENTS.md`：ACP 协议/客户端生命周期、stdio/连接归属、配置持久化、远程探测、timeout 策略留在 `bitfun-acp`；只把稳定能力事实透出给 UI 层，不在 Web UI 内重定义 ACP 协议或外部 agent tool 契约（后者归 `bitfun-agent-tools`）。

### 期望效果 / 使用场景
<!-- 描述该功能在什么场景下使用，以及使用后的预期效果 -->
1. 用户在工作区菜单选择「创建 ACP 会话」，弹出客户端选择器，实时显示每个客户端的探测状态（已就绪 / 缺少依赖 / 远程探测中），缺依赖可一键安装。
2. 进入会话后，侧栏能看到当前会话的模型与配置选项（fast-mode、thought_level 等），切换后立即生效并反映在下一轮 turn 的事件流中。
3. 外部 agent 发起 tool call 或 permission 请求时，Web UI 以与原生工具一致的卡片/确认弹窗呈现，用户 approve/reject（含 option 选择）后 ACP 客户端继续 turn。
4. 切换到远程 workspace 时，ACP 客户端复用本地配置并在 UI 显式标注探测阶段与降级原因，失败时给出可重试动作而非通用错误。

### 设计草案 / 参考示例
<!-- 如有设计稿、草图或参考的产品示例，请附在此处 -->
- 客户端探测与一键安装：参考 `src/crates/interfaces/acp/src/client/requirements.rs` 的 `probe_executable` / `probe_npm_adapter` / `install_npm_cli_package` 与远程对应实现，UI 映射 `AcpClientStatus`。
- 会话配置投影：参考 `src/web-ui/src/flow_chat/utils/acpSessionConfig.ts` 的 `resolveAcpReasoningState` / `resolveAcpFastModeState`，与原生 `ReasoningCatalogProjection` 对齐。
- 工具桥接：参考 `src/crates/interfaces/acp/src/client/tool_card_bridge/`（tool_name / tool_params）与 `bitfun_agent_tools::ACP_TOOL_PREFIX`，结果形状走 `bitfun-agent-tools` 既有契约。
- 生命周期/事件：参考 `src/crates/interfaces/acp/src/client/manager.rs` 的常量（启动 60s / permission 600s / close 5s 等）与 `AcpClientStreamEvent` 投影。

### 是否愿意贡献
<!-- 是否愿意参与该功能的开发或讨论 -->
- [ ] 我愿意参与开发
- [x] 我愿意参与讨论和测试
- [ ] 仅提出建议

### 补充说明
<!-- 其他你认为有助于理解功能建议的信息，如相关 Issue 链接、文档等 -->
- 现有基线（非本需求新增）：内置 preset、会话创建/列举、模型与 config option、permission 响应、tool card 桥接、远程复用本地配置的底层链路已在 `bitfun-acp` 实现。本需求聚焦 Desktop/Web UI 的体验收敛与一致性增强，不新增 ACP 协议字段，不迁移 client 生命周期归属。
- 边界约束：不得在 `core-types` / `runtime-ports` / `agent-tools` 内重定义 ACP 配置/探测；外部 agent tool 契约归 `bitfun-agent-tools`；ACP stdio/连接/通知投影留在 `interfaces/acp`。
- 验证基线：`cargo check -p bitfun-acp` / `cargo test -p bitfun-acp`（Rust 侧）；Web UI 侧用 `pnpm run type-check:web` 及 `src/web-ui/src/flow_chat/utils/acpSessionConfig.test.ts` / `acpSession.test.ts` 覆盖。
- 如需更窄范围（仅某一环，如「客户端健康度面板」或「远程 workspace 探测透明化」），可拆为独立 issue。

# 对话归档能力增强（Desktop / Web UI）功能提案

> 状态：提案 / 待评审
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/architecture/agent-runtime-lifecycle-sequence.md`](../architecture/agent-runtime-lifecycle-sequence.md)
> - [`docs/architecture/agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)
> - [`docs/architecture/agent-runtime-deployment-design.md`](../architecture/agent-runtime-deployment-design.md)
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
- BitFun 已具备单会话归档基线：`AgentSessionManagementPort` 的 `archive_session` / `set_session_archived`、Web UI `archived-sessions` 设置 tab、`SessionStatus = 'active' | 'archived' | 'completed'`、`archive_session`/`unarchive_session` 经 websocket 映射到 `session/setArchived`、会话驱动（local/dispatch）实现 `archiveSession`、FlowChatStore 跳过已归档会话。但归档体验仍停留在「单条手动归档 → 设置页查看」的最小闭环，缺少批量、检索、留存策略与跨 surface 一致性。
- 痛点：① 会话数量增长后只能逐条归档/取消归档，无批量操作；② 归档列表缺少按名称/内容/时间/工作区检索与筛选，找历史对话困难；③ 无自动归档与留存策略（如按闲置时长自动归档、超期自动清理），归档目录本身会膨胀；④ 归档对话不能导出/分享（markdown/transcript），跨设备或留档困难；⑤ 远程/Peer/ACP 等 surface 的归档可见性与一致性行为未对齐；⑥ 归档存储用量与归档数量对用户不透明。

### 功能描述
<!-- 请详细描述你期望的功能行为 -->
- 在 Desktop/Web UI 增强对话归档端到端体验，覆盖六个环节：
  1. **批量操作**：在会话列表与归档页支持多选归档 / 取消归档 / 删除，带二次确认；批量操作原子化，部分失败时返回失败项与原因而非整体回滚。
  2. **检索与筛选**：归档列表支持按名称、内容片段、归档时间、原工作区、标签筛选与排序；复用既有会话元数据索引，不引入第二套 transcript 查询 API。
  3. **自动归档与留存策略**：设置中可选自动归档策略（按闲置时长 / 按完成状态 / 按工作区），可选留存期（超期自动清理或仅提示）；策略只写会话元数据标志与时间戳，运行时按策略扫描，不新建常驻调度器之外的的服务 owner。
  4. **导出与分享**：归档对话支持导出为 markdown / transcript（只读，复用 `SessionTranscriptReader` 既有只读读取），导出不含运行时句柄或权限凭证；支持单条与批量导出。
  5. **取消归档与恢复**：取消归档后会话回到活动列表，状态由 `archived` → `active`，保留元数据与 lineage；恢复冲突（如同名 / 已存在）给出可重试动作而非静默失败。
  6. **跨 surface 一致性与可观测**：归档状态经稳定会话事实对齐远程 / Peer / ACP 入口（`Session archive` 当前不在 Shared deployment 协议内，需先明确各 surface 的归档可见性策略，缺失时返回类型化 `unsupported` 而非静默不一致）；设置页展示归档数量与归档存储用量。
- 实现边界遵循会话管理归属：归档是 `AgentSessionManagementPort` 的元数据级操作（`archive_session` / `set_session_archived` / `delete_session`），不等于删除，不构成通用 operation 查询 API；自动归档策略属产品级会话管理，不迁移到 `core-types` / `runtime-ports`；导出只消费 `SessionTranscriptReader` 只读事实，不在 UI 层重定义 transcript schema；远程归档可见性受 `agent-runtime-deployment-design.md` 既有约束（Shared GUI/Headless/ACP/SDK Host/Remote 的 Session archive 未交付），需以明确端口策略先行。
- 非目标：不做通用「会话市场」或在线同步；不为本提案引入跨 workspace attach 或 transcript 分页（属 deployment 设计已明确的未交付项）；不做归档对话的再编辑，归档为只读视图。

### 期望效果 / 使用场景
<!-- 描述该功能在什么场景下使用，以及使用后的预期效果 -->
1. 用户在会话列表多选若干已完成对话，一键归档并二次确认；归档后从活动列表消失，进入归档页统一管理。
2. 用户在归档页按名称或内容片段检索历史对话，按归档时间排序，快速定位后可查看只读 transcript 或取消归档回到活动列表。
3. 用户开启「闲置 30 天自动归档」与「归档超 1 年提示清理」策略，归档目录自动收敛，无需手动维护。
4. 用户将某归档对话导出为 markdown 存档或分享，导出内容为只读 transcript，不含运行时句柄或凭证。
5. 远程 / Peer 端查看归档状态与桌面一致；若该 surface 暂不支持归档可见性，显式标注 `unsupported` 而非静默隐藏。

### 设计草案 / 参考示例
<!-- 如有设计稿、草图或参考的产品示例，请附在此处 -->
- 端口复用：归档/取消归档/删除走 `AgentSessionManagementPort` 的 `archive_session` / `set_session_archived` / `delete_session`；元数据 `SessionStatus = 'active' | 'archived' | 'completed'`。
- 检索：复用既有会话元数据索引（`services-core` 的 session 存储布局/索引/删除），不新增通用 transcript 查询 API。
- 导出：复用 `SessionTranscriptReader` 只读读取，产出 markdown / transcript，禁带运行时句柄与权限凭证。
- 自动策略：产品级会话管理扫描 + 元数据标志/时间戳；不新建常驻调度器之外的服务 owner。
- 跨 surface：归档可见性策略先行明确各入口（远程/Peer/ACP/SDK Host），缺失返回类型化 `unsupported`。

### 是否愿意贡献
<!-- 是否愿意参与该功能的开发或讨论 -->
- [ ] 我愿意参与开发
- [x] 我愿意参与讨论和测试
- [ ] 仅提出建议

### 补充说明
<!-- 其他你认为有助于理解功能建议的信息，如相关 Issue 链接、文档等 -->
- 现有基线（非本需求新增）：单会话归档/取消归档、`archived-sessions` 设置 tab、`SessionStatus` 归档态、`session/setArchived` wire、local/dispatch 驱动 `archiveSession`、FlowChatStore 跳过已归档。本需求聚焦批量/检索/留存/导出/跨 surface 一致性的体验收敛。
- 边界约束：归档是元数据级操作，不等于删除，不构成通用 operation 查询 API；自动归档策略不迁移到 `core-types` / `runtime-ports`；导出不重定义 transcript schema；远程归档可见性受 deployment 设计既有约束，Shared GUI/Headless/ACP/SDK Host/Remote 的 Session archive 当前未交付，需以明确端口策略先行而非隐式启用。
- 验证基线（建议）：Web UI 侧 `pnpm run type-check:web` 及归档相关单测（`websocket-adapter`、`FlowChatStore` 归档跳过、`ArchivedSessionsConfig`）；Rust 侧归档端口属 `agent-runtime` / `session_management`，按其 AGENTS 选取最窄命令；跨 surface 变更按 `agent-runtime-deployment-design.md` 完成 focused review。
- 若需扩展到「归档标签/文件夹」「归档对话再编辑」或「在线同步」，可作为本提案的后续增强单独评审，不在本范围内。

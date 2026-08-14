# Agent 记忆功能配置（Memory Configuration）需求文档

> 状态：需求 / 提案
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/architecture/extensions/capability-runtime-integration-design.md`](../architecture/extensions/capability-runtime-integration-design.md)（Memory 能力边界：可替换 vs 权威归属）
> - [`docs/architecture/product-architecture.md`](../architecture/product-architecture.md)（分层与平台适配边界）
> - [`docs/architecture/peer-device-mode.md`](../architecture/peer-device-mode.md)（远程 / Peer 记忆归属）
> - [`docs/features/privacy-statement.md`](privacy-statement.md)（隐私同意状态门禁）
> 现有实现参考：
> - `src/web-ui/src/locales/*/settings/memories.json`（Settings → Memory：Basics / Extraction / Consolidation 配置契约）
> - `src/crates/services/services-core/src/session/memory_workspace.rs`（记忆工作区服务）
> - `src/crates/contracts/product-domains/src/privacy.rs`（`collection_allowed` 同意状态门禁）

### 功能类型
<!-- 请描述功能建议的类型 -->
- [ ] 新功能
- [x] 现有功能增强
- [x] 用户体验优化
- [ ] 性能优化
- [ ] 接口/集成扩展
- [ ] 其他

### 优先级
<!-- 请选择功能建议的优先级 -->
- [ ] 紧急（P0 - 核心需求，强烈期望实现）
- [ ] 高（P1 - 重要功能，期望尽快实现）
- [x] 中（P2 - 有价值的功能，计划实现）
- [ ] 低（P3 - 锦上添花，有时间再做）

### 背景与动机
<!-- 请描述你为什么需要这个功能，解决了什么痛点 -->
- BitFun 已有较成熟的记忆配置（Settings → Memory：Basics / Extraction / Consolidation 三段，覆盖自动生成、使用、提取资格、外部上下文策略、空闲 / 年龄 / 限额 / 并发 / 模型、合并候选与保留），但配置面仍有缺口。
- **缺少记忆内容的检视 / 编辑 / 删除单条入口**：现有只有 `resetMemory`（全量重置），用户无法查看 / 编辑 / 删除单条记忆、无法看到来源 provenance（来自哪个会话 / 时间），治理与信任不足。
- **provenance 与审计可见性缺失**：`capability-runtime-integration-design.md` 把"记忆来源、使用范围、版本、删除 / 撤销事实、权限、审计和注入决策"列为权威归属事实，但用户侧缺少审计视图（谁 / 何时写入 / 删除 / 注入）。
- **per-scope / per-workspace 配置缺失**：现有配置似为全局；多工作区 / 多项目下记忆范围与隔离需可配置，否则 A 项目记忆可能注入 B 项目会话。
- **隐私门禁可见性不足**：记忆提取自会话 transcript，可能含敏感内容；自动生成 / 使用 / 注入应受隐私同意状态门禁（fail-closed），未同意 / 受限模式不提取、不注入、不向云端发送，且用户需能看到该门禁状态。
- **远程 / Peer 场景策略缺失**：远程控制下记忆以受控端为准；peer 主机记忆读写需 deny 表对齐，控制器不应静默读写 peer 记忆。
- **多 surface / 鸿蒙 PC 覆盖**：记忆配置与记忆数据在 CLI / Web / MobileWeb / 鸿蒙 PC 的一致性需保证。

### 功能描述
<!-- 请详细描述你期望的功能行为 -->
- **记忆检视 / 编辑 / 删除**：在 Settings → Memory 增加"查看记忆"入口，列出当前记忆条目（含来源会话、提取时间、版本）；支持单条编辑、删除、撤销（标记为不使用而非物理删除，保留撤销事实）；保留 `resetMemory` 全量重置。
- **provenance 与审计**：每条记忆带 provenance（来源会话 id / 时间 / 提取模型 / 版本）；提供审计视图（写入 / 删除 / 注入 / 撤销事件，含操作者与时间），只读、可导出，作为治理证据。
- **per-scope 配置**：支持按工作区 / 项目配置记忆范围与隔离（哪些会话贡献、记忆是否跨工作区共享）；默认本地工作区隔离，跨工作区共享需显式开启。
- **隐私门禁**：记忆的自动生成、使用、注入受隐私同意状态门禁；未同意 / 受限模式（`PrivacyNotAccepted`）fail-closed 不提取、不注入、不向云端发送；记忆重置同步清理；用户可在设置中看到该门禁状态。
- **远程 / Peer 策略**：远程控制下记忆以受控端为准；peer 记忆读写 deny 表三端（桌面 / CLI / FE）对齐，控制器不静默读写 peer 记忆；远程不可用时给出清晰不支持态提示而非静默失败。
- **外部上下文与注入控制**：扩展现有 `externalContextPolicy`；新增"注入范围"配置（哪些记忆可注入新会话、是否需确认），注入决策保留为权威归属事实，不旁路。
- **保留与版本**：保留策略可配置（`maxUnusedDays` 等），版本化记忆便于回滚；删除 / 撤销走权威归属模块，不旁路。
- **多 surface 一致**：配置语义与记忆数据在桌面 / CLI / Web / MobileWeb / 鸿蒙 PC 一致，仅暴露方式按平台适配；UI 走 adapter / infrastructure 层，不直接调 host API。
- **平台无关 + 平台适配**：记忆配置 DTO 与策略平台无关（contracts / services 层），平台适配器暴露；鸿蒙 PC 通过适配器接入，语义与桌面一致。

### 期望效果 / 使用场景
<!-- 描述该功能在什么场景下使用，以及使用后的预期效果 -->
1. 用户在 Settings → Memory 查看记忆条目列表，看到每条来源（会话 / 时间 / 提取模型），可单条编辑 / 删除 / 撤销，无需全量重置即可精细管理。
2. 用户查看审计视图，看到记忆写入 / 删除 / 注入 / 撤销事件（操作者 / 时间），可导出作为合规与治理证据。
3. 用户按工作区配置记忆范围与隔离；A 工作区记忆默认不注入 B 工作区会话，跨工作区共享需显式开启。
4. 用户未同意隐私声明 / 处于受限模式时，记忆自动生成与注入 fail-closed 不执行，不向云端发送；用户可在设置中看到该门禁状态。
5. 远程控制下，记忆以受控端为准；控制器不静默读写 peer 记忆，deny 表三端对齐，远程不可用时给出清晰提示。
6. 鸿蒙 PC 上记忆配置语义与桌面一致，通过适配器接入。

### 设计草案 / 参考示例
<!-- 如有设计稿、草图或参考的产品示例，请附在此处 -->
- **现有参考实现**：
  - [`docs/architecture/extensions/capability-runtime-integration-design.md`](../architecture/extensions/capability-runtime-integration-design.md)：Memory 能力边界——可替换（存储实现 / 检索器 / 排序器 / 写入候选处理器 / 保留策略）vs 权威归属（来源 / 范围 / 版本 / 删除撤销 / 权限 / 审计 / 注入决策）；Memory Retriever 适用失败回退组合规则。
  - `src/web-ui/src/locales/*/settings/memories.json`：Basics / Extraction / Consolidation 配置契约——本需求在其上扩展检视 / 编辑 / 审计 / per-scope / 隐私门禁 / 远程策略。
  - `src/crates/services/services-core/src/session/memory_workspace.rs`：记忆工作区服务——检视 / 编辑 / 删除 / 撤销 / 审计的权威归属层。
  - `src/crates/contracts/product-domains/src/privacy.rs`：`collection_allowed()` 同意状态门禁——记忆生成 / 注入须服从。
- **行业参照**：Claude Code memory（`CLAUDE.md` / 记忆文件）、Cursor 记忆、ChatGPT memory 的查看 / 编辑 / 删除与 provenance / 审计。
- **落地分层建议**（遵循仓库分层与边界）：
  1. Contracts（`src/crates/contracts/product-domains`）：记忆条目 DTO（provenance / 版本 / 状态）、审计事件 DTO、扩展配置 DTO——平台无关、行为轻量。
  2. Services（`src/crates/services/services-core` 的 `memory_workspace` + `services-integrations`）：记忆检视 / 编辑 / 删除 / 撤销、审计、per-scope、保留与版本——权威归属在此，不旁路。
  3. Assembly（`src/crates/assembly`）：按 delivery profile 装配记忆能力与配置。
  4. Interfaces / App / UI（`src/apps` + `src/web-ui`）：桌面 host 暴露记忆 command；Web UI 扩展 Settings → Memory（检视 / 编辑 / 审计 / per-scope / 隐私门禁状态）；UI 走 adapter / infrastructure 层，不直接调 host API。
- **关键约束**：可替换的是存储 / 检索 / 排序 / 写入候选 / 保留策略；权威归属（来源 / 范围 / 版本 / 删除撤销 / 权限 / 审计 / 注入决策）不得旁路；隐私门禁 fail-closed；远程 deny 表三端对齐；不新增第二套记忆归属模块。

### 是否愿意贡献
<!-- 是否愿意参与该功能的开发或讨论 -->
- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

### 补充说明
<!-- 其他你认为有助于理解功能建议的信息，如相关 Issue 链接、文档等 -->
- 本需求为现有记忆配置的**增强**（现有 Basics / Extraction / Consolidation 已可用），聚焦检视 / 编辑 / provenance / 审计 / per-scope / 隐私门禁 / 远程策略缺口。
- 记忆内容可能含敏感信息（提取自会话 transcript），隐私门禁与审计是合规与信任前置；未同意 / 受限模式 fail-closed，不得静默提取或注入。
- 远程 / Peer 遵循仓库"远程兼容"全局规则与 deny 表三端对齐原则（参见 `computer-use-refactor-plan.md` 的 M14 修复方向）。
- 鸿蒙 PC 是平台移植目标，记忆配置通过适配器接入，不属当前桌面优先范围但需保证语义一致（见 [`docs/architecture/platform-portability-design.md`](../architecture/platform-portability-design.md)）。
- 不为覆盖竞品矩阵而引入第二套记忆归属模块或 `MemoryProviderRegistry` 等空壳公共对象（见 `capability-runtime-integration-design.md` 组合规则门槛）。

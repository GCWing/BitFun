# Skill 能力支持（Skills 扩展层）需求文档

> 状态：能力规格 / 需求
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/architecture/product-architecture.md`](../architecture/product-architecture.md)
> - [`docs/architecture/agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)
> - [`src/crates/execution/agent-runtime/AGENTS.md`](../../src/crates/execution/agent-runtime/AGENTS.md)
> - [`docs/features/gewu-market-integration.md`](./gewu-market-integration.md)

## 背景与需求描述

BitFun 的扩展能力分四层（L1 自定义 Agent → **L2 MCP / Skills / Hooks** → L3 Mini Apps → L4 源码级改造）。其中 **Skills** 是 prompt / resource / instruction 形态的轻量扩展，作为 agent definition 或 harness input 的一部分注入运行时——与 MiniApp（自带界面与运行时）、MCP（外部工具协议）形成互补。

Skills 这条扩展路径需要满足的核心诉求：

- **跨生态复用**：用户已有 Claude Code / Codex / Cursor / OpenCode / 通用 Agent Skills 目录里的 Skill 资产，BitFun 不能要求重新造一份，应能直接发现并按各自方言解析；
- **分层放置**：同一台机器上，项目级、用户家目录级、用户配置级的 Skill 应有清晰优先级与遮蔽规则，避免项目 Skill 被全局 Skill 静默覆盖；
- **按模式生效**：不同 Agent 模式（Agentic / Plan / Debug / DeepReview / DeepResearch 等）应能声明各自默认启用 / 隐藏的 Skill，用户可显式覆盖；
- **显式调用**：用户应能直接点名调用某个 Skill（而非只能被动随模式加载）；
- **热加载**：编辑 / 新增 / 删除 Skill 后，运行时应能受控刷新发现结果，无需重启；
- **可分发**：精品 Skill 应能经由市场（如格物市场）安装与更新，而非只能手动拷贝；
- **远程可用**：远程工作区场景下，Skill 发现与刷新应明确归属哪一端执行，不能静默失败。

本需求文档定义 Skill 能力支持的目标范围、行为契约与分层归属。

## 期望行为

### 1. 多源发现

- 在以下生态目录下发现 Skill，并按各自方言（dialect）解析 markdown front-matter：
  - BitFun 原生：`.bitfun/skills`；
  - Claude Code：`.claude/skills`；
  - Codex：`.codex/skills`；
  - Cursor：`.cursor/skills`；
  - OpenCode：`.opencode/skills`；
  - 通用 Agent Skills：`.agents/skills`。
- 每个发现源携带**生态标识**（source_id / source_label，如 `claude-code` / `Codex`），仅作展示用途，**绝不参与 Skill 优先级判定**，避免生态来源隐式影响执行。
- 解析方言至少覆盖：Claude Code、Codex、通用 Agent Skills 三种 front-matter 形态。

### 2. 三级放置与优先级

- 支持三级 Skill 根目录：
  - **项目级**：随项目仓库放置，团队共享；
  - **用户家目录级**：跨项目共享，个人偏好；
  - **用户配置级**：经用户 config 显式指定（如 OpenCode 的 `skills.paths` 配置）。
- 同名 Skill 按项目级 > 用户级 的方向遮蔽，并产出清晰的遮蔽说明（被遮蔽的 Skill 仍可显式调用）。
- Windows 下用户配置根目录需做平台特化解析（如 OpenCode 走 `~/.config/opencode/skills`）。

### 3. 内建目录与模式策略

- 提供内建 Skill 目录（builtin catalog），按 group 归类；
- 每个 Agent 模式声明：默认启用的内建 Skill、默认隐藏的内建 Skill；
- 用户可在模式级覆盖默认策略（启用 / 禁用 / 强制显式调用）；
- 当用户显式点名一个默认隐藏的内建 Skill 时，应能解析为"本次显式生效"而非全局改动。

### 4. 选择与调用

- 按模式过滤候选 Skill（可隐式调用 / 仅显式调用 / 用户可调用）；
- 对候选集排序、去重、标注遮蔽关系；
- 支持"可见 Skill 列表"在 assistant 上下文中渲染，使模型知道当前可调用哪些 Skill；
- Skill 列表变化时产出内部提醒（skill listing diff），帮助模型感知环境变化。

### 5. 热加载与刷新

- 通过文件监听服务（file_watch）管理 `bitfun-skills` 目录，编辑后受控触发 Agent 运行时重新发现；
- 提供运行时上下文刷新目标（reload target = Skills），供宿主在 Skill 变更后定向刷新，而非整会话重建；
- 嵌套 Skill 根的父目录注册应覆盖子目录变更。

### 6. 市场分发

- 与格物市场等市场链路打通：浏览 / 安装 / 版本更新 / 卸载精品 Skill（详见 `gewu-market-integration.md`）；
- 市场安装的 Skill 落入既有 Skill 根目录约定，复用现有发现 / 解析链，不另造第二套发现机制。

### 7. 远程工作区

- 远程控制另一台桌面时，Skill 发现与刷新应在受控端执行；控制端只发起与展示；
- 在 `remote_workspace_policy` 中声明明确策略，不支持时给出清晰提示而非静默失败。

### 8. 助手可见性

- 已加载 Skill 应能渲染为 assistant 可读的 payload（来源 / 路径 / 模式生效态），便于模型引用；
- 转写 / 渲染保持平台无关，产物只在运行时层定义，宿主负责具体 IO。

## 非目标 / 范围外

- 不在本需求内做 Skill 运行时沙箱或权限收窄——Skill 作为 prompt / instruction 注入，安全模型与现有 Skills 一致；
- 不做 Skill 的图形化编辑器或可视化构建工具；
- 不替换 MCP / Hooks / 自定义 Agent / MiniApp 等其他扩展层；
- 不在本需求内定义格物市场私有协议（见 `gewu-market-integration.md`）；
- 不覆盖 HarmonyOS PC CLI/TUI 形态（见 `platform-portability-design.md`，需单独立项）；
- 不承诺跨 Skill 的依赖解析 / 组合编译——Skill 之间相互独立。

## 建议的落地路径（基于现有分层）

依据仓库的分层与边界规则，Skill 能力的各部分应归属：

1. **Execution / Agent Runtime (`src/crates/execution/agent-runtime`)** — Skill 的**可移植决策与事实**归属此 crate 的 `skills` 模块：
   - DTO：`SkillData`、`SkillInfo`、`SkillLocation`、`ModeSkillInfo`、`SkillParseError` 等；
   - 根目录事实：`SkillRootSpec`、`PROJECT_SKILL_ROOTS` / `USER_HOME_SKILL_ROOTS` / `USER_CONFIG_SKILL_ROOTS`、`BITFUN_SKILL_SOURCE_ID`；
   - 内建目录：`BuiltinSkillSpec` / `BuiltinSkillGroup` / `builtin_skill_spec`；
   - 模式策略与解析：`resolve_skill_state_for_mode`、`resolve_skill_default_enabled_for_mode`、`normalize_user_mode_skill_overrides`；
   - 选择与遮蔽：`resolve_visible_skills`、`annotate_shadowed_skills`、`filter_candidates_for_mode`、`resolve_default_hidden_builtin_for_explicit_invocation`；
   - 助手渲染：`render_loaded_skill_for_assistant`。
   - 边界：**纯事实与决策**，不得做具体文件系统 IO、配置 IO 或注册表扫描——这些归宿主 / 服务层。
2. **Contracts (`src/crates/contracts`)** — 稳定的 Skill DTO / port 与上下文刷新目标（`AgentContextReloadTarget::Skills`）、Skill 列表 diff 事件形态等行为轻量契约。
3. **Adapters (`src/crates/adapters`)** — 外部生态 Skill 源协议适配，如 `opencode-adapter` 的 `OpenCodeSkillRootProvider` / `OpenCodeConfiguredSkillRoot`、`claude-code-adapter` 与 `codex-adapter` 的 Skill 导入。仅做生态私有配置 / 目录方言翻译，不承载产品策略。
4. **Assembly / Core (`src/crates/assembly/core`)** — Skill 注册表装配：`SkillRegistry`、`SkillRootEntry`、`RemoteSkillRootEntry`、`source_cache`，把外部源贡献（如 OpenCode 配置根）投影进运行时根集合；宿主在此层做具体发现扫描。
5. **Services (`src/crates/services/services-integrations`)** — `file_watch` 管理 `bitfun-skills` 目录的热加载；市场安装（格物市场）落盘到 Skill 根目录后的清理 / 校验归 market 集成族。
6. **App / UI (`src/apps/desktop` + `src/web-ui`)** — 桌面宿主装配 Skill 注册表、暴露 Skill 管理 UI（列表 / 启用禁用 / 模式覆盖 / 市场入口）；UI 组件不得直接读文件系统或外部生态目录，必须走 adapter / service 层。

### 分层与依赖边界要点

- 产品逻辑平台无关：Skill 决策、目录事实、目录、方言解析、模式策略、选择遮蔽、助手渲染全部在 `agent-runtime` 的 `skills` 模块，平台无关；
- `agent-runtime` 不得依赖 `bitfun-core`、app、Tauri、具体 service crate——只能通过 port 与注入的注册表获取能力；
- 具体文件系统 / 配置 IO / 注册表扫描、生态私有协议翻译、市场安装落盘、热加载触发分别归各 owner 层；
- 生态标识（source_id）仅展示，绝不参与优先级——这是必须保持的不变量；
- 新增外部生态源应作为新 adapter 接入，而非在 `agent-runtime` 内堆砌生态特化分支。

## 设计草案 / 参考示例

- **目录方言**：Claude Code / Codex / 通用 Agent Skills 三种 front-matter 形态，由 `skill_source_dialect` 按来源 slot 分派；新增生态应通过新增方言 + adapter 接入，不在决策层加 `if` 分支。
- **优先级模型**：项目级遮蔽用户级，遮蔽不删除被遮蔽 Skill（仍可显式调用）；遮蔽关系作为展示元数据随 `SkillInfo` 一起渲染。
- **模式策略参考**：DeepReview / DeepResearch / Plan / Debug 等模式各自声明默认启用与默认隐藏集；用户覆盖写入模式级 override，运行时按 `resolve_skill_state_for_mode` 解析最终态。
- **热加载参考**：`file_watch` 服务对 `bitfun-skills` 目录的父目录注册覆盖子目录变更；嵌套根的父级注册需覆盖最深层变更。
- **助手可见性参考**：`render_loaded_skill_for_assistant` 把已加载 Skill 渲染为模型可读 payload；Skill 列表变化经 `SkillListingDiff` 内部提醒通知模型。
- **市场分发参考**：见 `gewu-market-integration.md`；市场安装复用既有根目录约定与发现链，不另造。
- **跨生态兼容参考**：与 Codex hook 契约（`docs/features/agent-hooks.md`）保持一致——BitFun 实现既有生态契约而非另起炉灶，Skill 方言亦同。

## 是否愿意贡献

- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

## 补充说明

- 本需求严格遵循仓库"产品逻辑平台无关、再通过平台适配器暴露"与"优先事实与决策，具体 IO 下沉"的规则：Skill 的目录 / 目录 / 策略 / 选择 / 渲染是可移植决策，归 `agent-runtime`；具体发现扫描与生态协议翻译归 adapter / assembly / service。
- 与 `gewu-market-integration.md` 的关系：本需求定义 Skill **能力本身**的支持范围，格物市场文档定义 Skill **分发通道**；两者互补，市场安装落盘后即进入本需求定义的发现 / 解析 / 选择链。
- 与 `agent-hooks.md` 的关系：Skills 与 native Hooks 是 L2 扩展层的并列通道，互不替代；Skill 是 prompt / instruction 注入，Hook 是生命周期点副作用。
- 远程工作区策略：Skill 发现 / 刷新归属受控端执行，控制端只发起与展示，在 `remote_workspace_policy` 中显式声明，不支持时清晰提示。
- 相关分层入口：`src/crates/contracts/AGENTS.md`、`src/crates/adapters/AGENTS.md`、`src/crates/execution/agent-runtime/AGENTS.md`、`src/crates/assembly/core` 的 Skill 注册表实现、`src/crates/services/services-integrations/AGENTS.md`。

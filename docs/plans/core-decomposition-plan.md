# BitFun Core 拆解与构建提速可执行计划

> **执行约定：** 后续实施本计划时，按完整 owner 主题分步推进。低风险准备项已经收敛，后续 PR 不再提交零散 helper / guard 小块；每个高风险 PR 必须先补设计、预保护、等价验证和对抗性审核方案，再移动 runtime owner。

**目标：** 将当前职责过重的 `bitfun-core` 逐步拆成边界明确、依赖可控、可独立验证的 Rust crate 和能力 feature，同时不改变任何产品功能、CI/release 构建内容、关键构建脚本执行逻辑或各形态产品的依赖范围。

**总体策略：** 采用 Strangler Facade（绞杀者门面）迁移。`bitfun-core` 在迁移期继续作为兼容门面和完整产品 runtime 组装点，旧公开路径尽量保持可用；新的实现逐步迁移到独立 owner crate 中，跨层调用通过端口接口、provider、adapter 连接。

**拆分粒度修正：** 不追求把每个目录都拆成独立 crate。目标是先形成 8 到 12 个中等粒度 owner crate，并在 crate 内用模块和 feature group 继续隔离能力。过多小 crate 会增加 Cargo metadata、check 调度、增量编译管理和测试链接成本，可能抵消一部分优化收益。

**核心收益：**

- 让单元测试和局部测试可以依赖更小 crate，减少不必要编译和链接。
- 让重依赖归属到真正需要它们的能力模块，例如 `git2`、`rmcp`、`russh`、`image`、`tokio-tungstenite`。
- 用 crate 边界和接口阻止新的循环引用，而不是只靠文件夹、注释或团队约定。
- 为后续依赖版本收敛和 feature 最小化提供稳定边界。

---

## 0. 不可变更边界

以下约束优先级高于所有优化收益：

- 重构期间产品行为不变。
- `bitfun-desktop`、`bitfun-cli`、`bitfun-server`、`bitfun-relay-server`、`bitfun-acp`、installer 相关构建能力不被削减。
- 不通过减少 CI 覆盖来换取速度。
- 不在仓库级默认引入 `.cargo/config.toml` 强制 `sccache`、`lld-link`、`mold` 或其它机器相关工具。
- 不把 `bitfun-core` 重新包装成另一个 `common`、`shared`、`platform` 式超级 crate。
- 新拆出的 crate 不允许依赖回 `bitfun-core`。
- `bitfun-core` 可以依赖新 crate 并 re-export 旧路径，用于兼容。
- 任何会减少 `bitfun-core` 默认能力的 feature 调整，必须先让所有产品 crate 显式启用等价的完整产品能力。
- 以下关键脚本不作为 core 拆解的一部分修改：
  - `package.json`
  - `scripts/dev.cjs`
  - `scripts/desktop-tauri-build.mjs`
  - `scripts/ensure-openssl-windows.mjs`
  - `scripts/ci/setup-openssl-windows.ps1`
  - `BitFun-Installer/**`

每个阶段合并前必须执行脚本保护检查：

```powershell
git diff -- package.json scripts/dev.cjs scripts/desktop-tauri-build.mjs scripts/ensure-openssl-windows.mjs scripts/ci/setup-openssl-windows.ps1 BitFun-Installer
```

期望结果：没有 diff。若某个阶段确实需要改构建脚本，必须从本文计划中拆出，作为独立的显式产品构建变更评审。

---

## 0A. 架构原则复核与偏移防线

后续每个 PR 都必须先对照本节。若发现任意原则无法满足，应暂停该 PR，并将问题拆成更小的前置重构或独立设计评审。

### 0A.1 平台边界不能偏移

必须保持：

- product logic 仍保持 platform-agnostic。
- Tauri、desktop-only、server-only、CLI-only 能力仍留在 platform adapter 或 product assembly 层。
- shared core、runtime、services crate 不直接引入 `tauri::AppHandle`、desktop API 或其它 host-specific 依赖。
- Web UI 到 desktop/server 的调用路径仍经过现有 adapter/API/transport 边界。

禁止：

- 为了拆 crate，把 desktop-only 逻辑下沉到 `core-types`、`agent-runtime` 或 `services-core`。
- 为了方便调用，让新 service crate 反向依赖 app crate。

验收方式：

- 检查新增 crate 的 `Cargo.toml`，确认没有不应出现的平台依赖。
- 对涉及 desktop/server/CLI 的 PR，执行对应产品 check，而不是只执行新 crate 的测试。

### 0A.2 功能集合不能偏移

必须保持：

- `product-full` 是完整产品能力保护开关。
- 产品 crate 显式启用完整能力后，才允许继续拆能力 feature。
- `bitfun-core` 的旧公开路径通过 facade 或 re-export 保持 import-compatible。
- tool registry、MCP dynamic tools、remote SSH、remote connect、miniapp、function agents 的产品可见行为保持一致。

禁止：

- 在同一个 PR 中同时“拆模块”和“改变产品默认能力”。
- 以减少编译为理由删除 CI 或 release 覆盖。
- 在没有完整产品矩阵验证前修改 `bitfun-core default`。

验收方式：

- 拆分前记录关键清单，例如 tool registry 工具列表、feature graph、产品 crate 对 `bitfun-core` 的 feature 使用。
- 拆分后用等价性测试或产品 check 证明能力仍存在。

### 0A.3 依赖方向不能偏移

必须保持：

- 新 crate 不依赖回 `bitfun-core`。
- `bitfun-core` 作为 facade 可以依赖新 crate。
- service crate 不直接依赖 agent runtime concrete implementation；通过 ports 调用。
- agent runtime 不依赖 heavy integration concrete service；通过 ports/provider 调用。
- `core-types` 只承载错误、DTO、port DTO、纯 domain type。

禁止：

- 新增万能上下文，例如 `CoreContext`、`AppContext`，把所有 manager 都挂进去绕过依赖边界。
- 通过 `pub use` 掩盖实际反向依赖。
- 在 `core-types` 中引入 IO、网络、进程、Tauri、`git2`、`rmcp`、`image` 等运行时依赖。

验收方式：

- 每个新增 crate 的 `Cargo.toml` 必须能说明依赖原因。
- 至少在关键 crate 拆出后，用 boundary check 阻止 forbidden imports 回流。

### 0A.4 性能方向不能反向

本计划不保证每个中间 PR 都立即变快，但不得明显变慢。

必须保持：

- 不新增大量微小 crate；默认目标是 8 到 12 个中等粒度 owner crate。
- heavy dependency 通过 owner crate 和 feature group 隔离。
- 局部测试优先落到小 crate，例如 `agent-stream`、`services-core`、`agent-tools`。
- 不引入团队机器相关的 repo-wide 编译参数或 linker 默认配置。

禁止：

- 为了“架构纯粹”把高频一起变化的模块拆成多个互相调用的小 crate。
- 为了局部快，把产品完整构建路径变复杂或变脆弱。
- 在没有实测依据时继续把 feature group 拆成独立 crate。

验收方式：

- 每个里程碑结束时至少对比一次关键目标：
  - 新增 crate 数量是否仍在中等粒度范围。
  - 关键局部测试是否能依赖更小 crate。
  - `cargo check -p bitfun-core --features product-full` 没有因为 facade 组装明显恶化。
  - 产品矩阵仍通过。

### 0A.5 阶段边界必须明确

每个 PR 只能落入以下一种类型：

- 文档/基线/边界检查。
- feature 安全网，不移动业务实现。
- 类型或 port 抽取，不移动重 service。
- 单个中等粒度 crate 抽取。
- 单个 feature group 迁移。
- facade/re-export 收敛。
- 低风险直接依赖版本收敛。
- 单个高风险 owner 迁移，且必须先满足 `0A.7` 的设计和保护门禁。

禁止：

- 同一个 PR 同时改 feature 默认值、移动大量模块、调整产品调用路径。
- 同一个 PR 同时做架构拆分和三方库大版本升级。
- 同一个 PR 同时修改构建脚本和 core 拆分。

暂停条件：

- 发现需要改变产品行为才能继续。
- 发现产品 crate 需要减少能力才能编译通过。
- 发现新 crate 必须依赖回 `bitfun-core`。
- 发现某个 feature group 拆分会导致多个平台产品使用不同代码路径。
- 发现构建脚本必须修改才能完成当前拆分。

### 0A.6 冗余清理只处理绝对等价逻辑

冗余清理不是本计划的主线性能优化。除非能证明逻辑完全等价，否则不因为“看起来类似”就抽公共函数或合并流程。

允许处理：

- 逐行对照后可以证明输入、输出、错误处理、日志、副作用、超时、平台条件完全一致的重复代码。
- 纯 helper 层重复，例如同一目录内完全一致的常量映射、权限字符串格式化、pairing 过期判断。
- 有现成测试或可以先补等价性测试的重复逻辑。

暂不处理：

- 不同平台、不同第三方协议、不同产品入口之间只是流程形状相似的代码。
- MIME by extension 与 MIME by bytes 这类语义不同的检测逻辑。
- Telegram、Feishu、Weixin 这种 provider 协议逻辑，除非抽取点只覆盖完全一致的本地状态管理。
- UI 组件或样式中相似但承载不同交互语义的结构。

执行要求：

- 冗余清理必须是独立 PR，不能混入 crate 拆分或 feature 默认值调整。
- PR 描述中必须列出“等价证明”：调用方、输入、输出、错误路径、副作用是否一致。
- 如果等价性说不清，宁可保留重复代码。
- 不为了减少代码行数引入新的公共抽象中心。

当前仅作为候选观察，不默认执行：

- Remote Connect bot 的 pairing store，如果逐行确认 `register_pairing` / `verify_pairing_code` 行为完全一致，可以抽 `BotPairingStore`。
- filesystem 中 extension-based MIME mapping 和 permission string formatting，如果逐行确认行为完全一致，可以抽本地 helper。

这些候选不阻塞里程碑推进，也不应优先于 feature 安全网和 `core-types` / `agent-stream` 拆分。

### 0A.7 高风险 owner 迁移 PR 门禁

已合入的文档/保护补强只作为后续高风险迁移的门禁基线，
后续 core decomposition PR 默认进入高风险 runtime owner 迁移队列。不得再把单个 helper、单条边界检查或
小型 facade 移动包装成独立 PR；这些只能作为同一个 owner 迁移 PR 的预保护或收尾。

每个高风险 PR 开始写代码前，必须在本文或最近的模块文档中先记录：

- **Owner 设计：** 当前 core-owned runtime 是什么，新 owner crate / core adapter
  分别负责什么，旧公开路径如何兼容。
- **行为盘点：** 列出输入、输出、错误映射、日志、异步时序、feature gate、缓存 /
  registry / manifest 副作用、产品表面差异。
- **预保护：** 先补或复用迁移前 snapshot / focused regression / boundary check。
  没有可执行保护时，不移动 runtime owner。
- **实施边界：** 每个 PR 只迁移一个 owner 主题；不同时改产品 feature set、
  default feature、构建脚本、UI/命令语义或第三方依赖大版本。
- **回滚边界：** 保留旧路径 facade 或 core adapter，保证可以回退到 core-owned
  runtime 而不影响产品入口。
- **验证矩阵：** 至少覆盖 owner crate tests、core focused tests、boundary check、
  `cargo check -p bitfun-core --features product-full`，并按影响面增加 desktop /
  CLI / ACP / remote / web product checks。
- **对抗性审核：** 提交前从第三方角度检查是否存在行为漂移、性能劣化、重复
  runtime materialization、锁/任务生命周期变化、产品发布形态变化、依赖方向回流。

暂停条件：

- 需要改变用户可见行为、权限策略、产品命令或默认能力才能完成迁移。
- owner crate 必须依赖回 `bitfun-core` 才能工作。
- 等价测试无法表达关键行为，或者只能依赖人工观察确认。
- 迁移会引入额外进程/网络启动、重复 registry/manifest 构建、无界缓存或更重的
  默认编译面。
- 最新 `main` 合入改变了相关 runtime 行为，但文档和保护测试尚未同步。

**最新主干行为基线：**

- `/goal` 模式已经进入主干，包含 AI goal synthesis、session custom metadata、
  post-turn verification events、continuation planning、main-session-only 约束和 Flow Chat
  pending/verifying surface。HR-C 与后续 service/agent deep optional 触碰
  scheduler/coordinator/session metadata 时必须先保护这些语义。
- 文件工具保护已新增 `file_read_state_runtime` / `file_tool_guidance`，Read/Edit/Write
  依赖 session-scoped read state、stale-write guardrail、`ToolUseContext` 和 workspace path
  policy。HR-A 不得把这些误归类为 provider-neutral tool contract。
- `tool_result_storage` 会把超大工具结果写入 session runtime artifact，并向 assistant-only
  transcript 注入 preview/reference。HR-A 迁移 tool pipeline、runtime artifact 或 tool-result
  adapter 前必须保护存储路径、引用格式、跳过规则和 session view compaction。
- workspace `related_paths` 已进入 workspace service、desktop/web surface、remote/local
  validation 与 request-context prompt。HR-C 与后续 workspace/search 迁移必须保留存储字段、
  canonicalization、remote validation 和 prompt section 输出。
- request-context policy、prompt compression、prompt-cache friendly assembly 与 OpenAI-compatible
  streaming 都提高了 agent runtime / AI adapter 边界门槛；HR-C 与后续 AI/stream 相关工作不得把
  provider-specific reasoning/tool-call schema 写入 provider-neutral manifest。

**最新主干补充基线：**

- `TaskTool` 已支持 `fork_context=true`。该模式复用父会话 agent/workspace/tools/prompt cache，
  但禁止 `subagent_type`、`workspace_path`、`model_id` 和 DeepReview retry 字段，并且
  forked Task 不是并发安全调用。HR-C 与后续 service/agent deep optional 触碰
  scheduler/coordinator/subagent runtime/session branch 前，必须保护 delegation policy、forked context seeding、prompt cache clone、
  `start_dialog_turn_with_existing_context` 和递归 subagent 禁止语义。
- `DialogTriggerSource` 已复用 `AgentSubmissionSource` 的 `bitfun-runtime-ports`
  契约；`DialogQueuePriority`、`DialogSubmissionPolicy` 与 `DialogSubmitOutcome`
  也已迁入 `bitfun-runtime-ports`。本轮进一步把 scheduler submit queue routing、
  interactive preempt 与 agent-session cancel-reply suppression 的纯决策迁入
  `bitfun-runtime-ports`；core `DialogScheduler` 仍持有队列、active-turn map、round-yield
  buffer、outcome loop 和 concrete coordinator submit，不改变执行生命周期。
  `DelegationPolicy` 与 `SubagentContextMode` 已迁入 `bitfun-runtime-ports`，core
  `agentic::subagent_runtime` 只保留旧路径 re-export 和 core-owned `queue_timing`。这一步
  只移动 portable DTO/decision primitive，不移动 scheduler、coordinator、session branch、
  prompt cache 或 background delivery runtime。
- HR-C portable contract closure 已把 agent-session reply route、steering buffered outcome、
  round injection kind / target / message / source traits、goal-mode DTO、prompt compression
  contract 与 workspace related-path fact 迁入 `bitfun-runtime-ports`；core 旧路径只保留
  compatibility re-export，round injection buffer、scheduler 生命周期和 concrete remote/runtime
  继续 core-owned。
- 工具可靠性最新变化必须纳入 HR-A 或任何 tool pipeline 迁移 baseline：Write 内容生成会拒绝
  tool-invocation syntax；AskUserQuestion / TodoWrite 只在安全边界恢复 truncation；
  `ToolRuntimeRestrictions` 支持 per-tool denial message；`tool_result_storage` 写 runtime
  artifact 后显式 flush，不能在 owner 迁移中退化为仅依赖 drop。
- MCP local stdio initialize timeout 已限定在 initialize 阶段，并补充
  `notifications/initialized` 与 pending waiter drain。后续 MCP/service-integrations 迁移不得把
  initialize timeout 扩散为普通 tool/resource request timeout，也不得丢失 channel close cleanup。
- CLI package workflow / Homebrew notifier 和 mobile-web session search / rename / delete
  已进入产品矩阵。H5 或产品形态验证需要覆盖 `bitfun-cli`、CLI packaging 影响面和
  `pnpm run build:mobile-web`；但 CLI TUI/packaging 与 mobile session UI 仍是 app surface，
  不作为 core/service owner 外移前置条件。
- `scripts/check-core-boundaries.mjs` 已同步 latest-main 的 fork-aware Task 保护锚点：
  `fork_context`、`SubagentContextMode::Fork`、child `DelegationPolicy` 传递、
  `background_task_id` 与 `<background_task status="started" ...>` 启动回执均在 core
  侧锁定，并禁止 core 重新定义已迁入 `runtime-ports` 的 dialog/subagent portable contract；
  后续迁移 agent runtime 前必须先保留或替换这些等价保护。
- 同一 guardrail PR 也已把 prompt cache clone、existing-context dialog turn、tool-call
  truncation recovery、per-tool denial message、tool-result file flush、MCP
  initialize-scoped timeout、`notifications/initialized` 和 pending waiter drain 纳入
  boundary check。后续 HR-A / service/agent deep optional / MCP 迁移不得绕过这些 latest-main 行为基线。

---

## 1. 当前问题与风险合集

### 1.1 `bitfun-core` 已经是完整产品 runtime 聚合

现状：

- `src/crates/core/src/lib.rs` 暴露 `agentic`、`service`、`infrastructure`、`miniapp`、`function_agents`、`util`。
- `src/crates/core/Cargo.toml` 直接承载大量重依赖，例如 `git2`、`rmcp`、`image`、`notify`、`qrcode`、`tokio-tungstenite`、`bitfun-relay-server`、`terminal-core`、`tool-runtime`。

风险：

- 一个很小的纯逻辑测试也可能触发大块 runtime 依赖编译。
- `cargo test` 需要为大量测试 target 链接可执行文件，Windows MSVC 下会产生多个 `Microsoft Incremental Linker` 进程。
- 新功能只要被放进 core，就天然继承整个重依赖图。

解决方向：

- 保留 `bitfun-core` 作为兼容门面。
- 将实现迁移到明确 owner crate。
- 测试逐步改为依赖最小 crate，而不是默认依赖完整 core。

### 1.2 `service` 与 `agentic` 存在双向耦合

观察到的耦合方向：

- `service -> agentic`：remote connect、MCP、cron、snapshot、config canonicalization、token usage、session usage 等。
- `agentic -> service`：tools、coordinator、agents、persistence、session、execution、insights 等。

风险：

- 直接把 `service` 和 `agentic` 拆成 crate 会立刻形成循环依赖。
- 只用文件夹或注释约束不能阻止新代码继续反向引用。

解决方向：

- 先抽取 port trait，再移动实现。
- 典型端口：
  - `AgentSubmissionPort`
  - `ToolRegistryPort`
  - `DynamicToolProvider`
  - `WorkspaceIdentityProvider`
  - `SessionTranscriptReader`
  - `ConfigReadPort`
  - `EventSink`
  - `StorageRootProvider`

### 1.3 feature 边界不完整，不能直接改默认 feature

现状：

- `bitfun-core` 当前有 `default = ["ssh-remote"]`。
- `ssh-remote` 控制 `russh`、`russh-sftp`、`russh-keys`、`shellexpand`、`ssh_config`。
- 其它重能力多数还是无条件依赖。

风险：

- 如果直接把 default 改轻，可能改变 desktop、CLI、server、ACP 的实际产品能力。
- Cargo feature 是 additive 的，无法可靠表达“某能力关闭后其它模块就完全不可见”的业务边界。

解决方向：

- 先引入 `product-full`，保持 default 行为不变。
- 产品 crate 显式启用 `product-full`。
- 只有在产品显式启用完整能力后，才逐步考虑拆 feature 或调整 default。

### 1.4 tool registry 会牵引所有工具实现

现状：

- `agentic/tools/registry.rs` 直接注册所有工具。
- snapshot service 在 registry 注册阶段参与包装。
- MCP service 会向全局 registry 注册动态工具。

风险：

- 任何依赖 registry 的测试都会编译所有具体工具及其依赖。
- registry 成为 service 和 agentic 互相引用的粘合点。

解决方向：

- 拆出 tool framework、registry、tool provider、tool pack。
- 使用 Provider Registry 和 Decorator：
  - `ToolProvider` 注册一组工具。
  - `DynamicToolProvider` 提供 MCP 等动态工具。
  - `ToolDecorator` 处理 snapshot 等横切逻辑。

### 1.5 shared type 位于错误层级

例子：

- `util/types/config.rs` 依赖 `service::config::types::AIModelConfig`。
- `service::session` 使用 `agentic::core::SessionKind`。
- 远程 workspace identity 同时被 service 和 agentic 使用。

风险：

- 看似基础的类型依赖高层 runtime 模块。
- 拆 crate 时容易产生循环引用或复制 DTO。

解决方向：

- 建立 `bitfun-core-types`。
- 只放稳定 DTO、错误类型、轻量 domain type。
- 不放 manager、service、global registry、IO、runtime orchestration。

### 1.6 nested crate 已经存在，但位置仍在 core 内部

现状：

- `src/crates/core/src/service/terminal/Cargo.toml` 包名 `terminal-core`。
- `src/crates/core/src/agentic/tools/implementations/tool-runtime/Cargo.toml` 包名 `tool-runtime`。

风险：

- 物理路径仍暗示它们属于 core 内部实现。
- 后续拆分时 workspace 依赖关系不清晰。

解决方向：

- 先移动到 `src/crates/terminal` 和 `src/crates/tool-runtime`。
- 保持 package/lib 名称不变，降低兼容风险。

---

## 2. 目标 crate 版图

这是目标方向，不要求一个 PR 完成。目标不是把所有 service 都拆成单独 crate，而是先用中等粒度 owner crate 降低编译面，同时避免 crate 数量膨胀。

下方列表同时包含“新 owner crate 目标”和已经存在的基础 crate（例如 `events`、`ai-adapters`、`terminal`、`tool-runtime`）。`8 到 12 个中等粒度 owner crate` 的约束主要用于新增拆分边界，不把这些已存在基础 crate 误算成继续拆小的理由。

### 2.1 推荐目标：中等粒度合并

```text
src/crates/core                    # 兼容门面 + 完整产品 runtime 组装
src/crates/core-types              # 错误、DTO、port DTO、纯 domain type
src/crates/events                  # 现有事件定义
src/crates/ai-adapters             # 现有 AI adapter；只接收纯协议 stream 逻辑
src/crates/agent-stream            # stream processor 与相关测试，若无法干净放入 ai-adapters
src/crates/agent-runtime           # session、execution、coordination、agent system
src/crates/agent-tools             # tool trait、registry、provider contract
src/crates/tool-packs              # feature-group 元数据与 provider plan；未来可按 feature group 承载具体工具
src/crates/services-core           # config/session/workspace/storage/filesystem/system 等基础服务
src/crates/services-integrations   # git/MCP/remote SSH/remote connect 等重集成，按 feature group 隔离
src/crates/product-domains         # miniapp、function agents 等产品子域
src/crates/tool-runtime            # 现有 tool-runtime 移出 core 子树
src/crates/terminal                # 现有 terminal-core 移出 core 子树
```

### 2.2 为什么不拆成三十个 crate

- 每个 crate 都会带来 Cargo metadata、fingerprint、增量编译缓存和 dependency graph 管理成本。
- `cargo test` 的主要链接压力来自测试二进制数量和每个测试二进制需要链接的代码量；crate 过碎虽然可能减少局部重编译，但也会增加调度和 rlib 组合成本。
- service 目录中很多模块会一起变化，例如 config/session/workspace/storage，强行拆开会提高跨 crate API 维护成本。
- 重依赖真正需要隔离的是能力族，而不是文件夹数量。更合理的边界是 `services-core` 与 `services-integrations`，再用 feature group 控制 `git`、`mcp`、`remote-ssh`、`remote-connect`。

### 2.3 何时允许继续拆小

只有满足以下条件之一，才把中等粒度 crate 继续拆小：

- 该能力有独立重依赖，并且大多数测试不需要它。
- 该能力的变更频率和 owner 明显独立。
- 该能力已经通过 port/provider 与其它模块解耦。
- 实测显示拆分后能减少关键测试或 check 的编译面。

不满足这些条件时，优先用同一 crate 内的模块、feature group 和边界检查约束。

---

## 3. 模块覆盖矩阵

拆解时不能遗漏当前 core 模块。下表给出每个模块的中等粒度目标归属。

| 当前模块 | 目标 owner | 说明 |
|---|---|---|
| `util::errors` | `bitfun-core-types` | `BitFunError`、`BitFunResult`，不包含 runtime |
| `util::types` | `bitfun-core-types` / `bitfun-ai-adapters` | 纯 DTO 入 types，AI 协议 DTO 优先留在 ai-adapters |
| `util::types::ai` 和 provider 协议 DTO | `bitfun-ai-adapters` | provider 请求/响应、stream 协议和 adapter-owned DTO 留在 AI adapter 边界内 |
| `util::process_manager` | `bitfun-services-core` | 涉及进程执行，不进入纯 types |
| `infrastructure::app_paths` | `bitfun-services-core` | 通过 `StorageRootProvider` 暴露 |
| `infrastructure::events` | `bitfun-events` / transport | 事件定义和发送抽象从 core 解耦 |
| `infrastructure::ai` | `bitfun-ai-adapters` + assembly | 通过 `ConfigReadPort` 消除反向依赖 |
| `infrastructure::storage` | `bitfun-services-core` | 依赖路径抽象，不依赖全局 core |
| `infrastructure::filesystem` | `bitfun-services-core` | 本地/远程文件系统通过 provider 隔离 |
| `infrastructure::debug_log` | `bitfun-services-integrations` feature `debug-log` | HTTP server 依赖需要 feature-gate |
| `service::config` | `bitfun-services-core` | agent/tool canonicalization 移到 runtime assembly |
| `service::session` | `bitfun-services-core` | `SessionKind` 等共享类型先移入 types |
| `service::workspace` | `bitfun-services-core` | workspace identity 独立 |
| `service::workspace_runtime` | `bitfun-services-core` | workspace runtime layout owner |
| `service::remote_ssh` | `bitfun-services-integrations` feature `remote-ssh` | 第一批重依赖隔离候选 |
| `service::mcp` | `bitfun-services-integrations` feature `mcp` | 动态工具通过 provider 注入 |
| `service::remote_connect` | `bitfun-services-integrations` feature `remote-connect` | 依赖 agent submission port |
| `service::git` | `bitfun-services-integrations` feature `git` | `git2` 边界清晰，适合早拆 |
| `service::lsp` | `bitfun-services-core` feature `lsp` | 依赖 workspace/runtime port |
| `service::search` | `bitfun-services-core` feature `search` | 依赖 workspace/filesystem provider |
| `service::snapshot` | `bitfun-services-core` feature `snapshot` | tool wrapping 改为 decorator |
| `service::cron` | `bitfun-services-core` feature `cron` | 调 agent runtime 通过 `AgentSubmissionPort` |
| `service::token_usage` | `bitfun-services-core` | 只依赖事件和 usage DTO |
| `service::session_usage` | `bitfun-services-core` | 依赖 transcript 边界 |
| `service::project_context` | `bitfun-services-core` | 避免直接依赖 coordinator |
| `service::announcement` | `bitfun-services-integrations` feature `announcement` | 远程 fetch 依赖独立 feature-gate |
| `service::filesystem` | `bitfun-services-core` | 本地/远程 provider |
| `service::file_watch` | `bitfun-services-integrations` feature `file-watch` | `notify` 依赖独立 |
| `service::system` | `bitfun-services-core` | 命令检测和执行 |
| `service::runtime` | `bitfun-services-core` | runtime capability detection |
| `service::i18n` | `bitfun-services-core` | config 依赖保持单向 |
| `service::ai_rules` | `bitfun-services-core` | 只依赖 paths/storage |
| `service::ai_memory` | `bitfun-services-core` | 只依赖 paths/storage |
| `service::agent_memory` | `bitfun-agent-runtime` 或 `bitfun-services-core` | prompt helper 随 runtime/prompt builder 迁移 |
| `service::bootstrap` | `bitfun-services-core` | workspace persona bootstrap |
| `service::diff` | `bitfun-core-types` 或 `bitfun-services-core` | 纯 diff 可入 types，否则入 services-core |
| `agentic::core` | `bitfun-agent-runtime` + `bitfun-core-types` | DTO 入 types，行为入 runtime |
| `agentic::events` | `bitfun-events` + runtime router | 事件定义不留在 core |
| `agentic::execution` | `bitfun-agent-runtime`，stream 可入 `bitfun-agent-stream` | stream processor 先拆以验证收益 |
| `agentic::coordination` | `bitfun-agent-runtime` | 依赖 service port，不依赖具体 service |
| `agentic::session` | `bitfun-agent-runtime` | persistence/config 通过 port |
| `agentic::persistence` | `bitfun-agent-runtime` + `bitfun-services-core` | DTO storage 和 orchestration 分离 |
| `agentic::agents` | `bitfun-agent-runtime` | registry 通过 config port |
| `agentic::tools::framework` | `bitfun-agent-tools` | 不包含具体工具实现 |
| `agentic::tools::registry` | `bitfun-agent-tools` | provider-based registration |
| `agentic::tools::implementations` | `bitfun-tool-packs` | 同一 crate 内按 feature group 分模块 |
| `agentic::deep_review_policy` | `bitfun-agent-runtime` | config input 通过 port |
| `agentic::fork_agent` | `bitfun-agent-runtime` | runtime concern |
| `agentic::round_preempt` | `bitfun-agent-runtime` | runtime concern |
| `agentic::image_analysis` | `bitfun-tool-packs` feature `image-analysis` 或 runtime feature | 隔离 `image` 依赖 |
| `agentic::side_question` | `bitfun-agent-runtime` | runtime concern |
| `agentic::insights` | `bitfun-agent-runtime` feature `insights` | 依赖 config/i18n/session ports |
| `agentic::workspace` | `bitfun-core-types` + `bitfun-agent-runtime` | remote identity DTO 入 types |
| `miniapp` | `bitfun-product-domains` feature `miniapp` | desktop API 先走 core facade |
| `function_agents` | `bitfun-product-domains` feature `function-agents` | 依赖 runtime 和 service ports |

---

## 4. 设计模式与关键接口

### 4.1 Facade：保留旧路径，不让迁移影响调用方

`bitfun-core` 迁移期只做兼容门面和完整 runtime 组装：

```rust
//! Compatibility facade and full product runtime assembly.
//!
//! New implementation code should live in owner crates under `src/crates/*`.
//! This crate re-exports legacy paths and wires the full BitFun product runtime.
```

旧路径示例：

```rust
pub mod service {
    pub use bitfun_services_git as git;
}
```

要求：

- 新实现不继续堆到 `bitfun-core`。
- re-export 必须加注释说明这是兼容层。
- 不要把 facade 变成新的业务实现聚合。

### 4.2 Dependency Inversion：先抽接口，再移动实现

示例端口：

```rust
#[async_trait::async_trait]
pub trait AgentSubmissionPort: Send + Sync {
    async fn submit_user_message(
        &self,
        request: AgentSubmissionRequest,
    ) -> Result<AgentSubmissionOutcome, BitFunError>;
}
```

使用原则：

- service crate 调 agent runtime 时，只依赖 port。
- agent runtime 调 config/session/workspace 时，也只依赖 port。
- port DTO 必须在 `core-types` 或专门的 `runtime-ports` crate 中，不能依赖 concrete manager。

### 4.3 Provider Registry：工具按能力包注册

示例：

```rust
pub trait ToolProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn register_tools(&self, registry: &mut dyn ToolRegistryPort) -> BitFunResult<()>;
}
```

使用原则：

- `agent-tools` 只包含 tool trait、context、registry、provider contract。
- `tool-packs` 当前只拥有 feature-group 元数据和 product provider group plan；具体工具实现迁移必须在后续高风险 owner 设计中按单一 feature group 处理。
- 产品完整 runtime 由 assembly 层安装所有 provider，保证产品行为不变。

### 4.4 Decorator：snapshot 等横切逻辑不侵入 registry

示例：

```rust
pub trait ToolDecorator: Send + Sync {
    fn decorate(&self, tool: Arc<dyn Tool>) -> Arc<dyn Tool>;
}
```

使用原则：

- snapshot service 不再直接改 registry 内部实现。
- registry 支持 decorator chain。
- 产品完整 runtime 默认安装同等 snapshot wrapping，保持原行为。

### 4.5 Adapter：平台差异留在产品 adapter 层

要求：

- Tauri、desktop-only、server-only、CLI-only 逻辑不下沉到纯 domain crate。
- platform adapter 组装 runtime 后，通过 `bitfun-core` facade 或明确 concrete crate 暴露。
- shared product logic 仍保持 platform-agnostic。

---

## 5. 分阶段执行计划

### Plan 0：基线与安全护栏

**目的：** 在开始移动代码前建立可度量基线和团队约束。

**文件范围：**

- 新增：`docs/architecture/core-decomposition.md`
- 修改：`AGENTS.md`
- 修改：`src/crates/core/AGENTS.md`

**任务：**

- [x] 记录依赖和构建基线，生成文件只放 `target/`，不提交。LR1 已重新生成
  `target/core-decomposition-metadata-baseline.json`、
  `target/core-decomposition-core-duplicates.txt` 和
  `target/core-decomposition-desktop-features.txt`；这些文件只作为本地基线，不提交。

```powershell
cargo metadata --format-version 1 --locked > target/core-decomposition-metadata-baseline.json
cargo tree -p bitfun-core -d > target/core-decomposition-core-duplicates.txt
cargo tree -p bitfun-desktop -e features > target/core-decomposition-desktop-features.txt
cargo test -p bitfun-core --no-run --timings
```

- [x] 在 `docs/architecture/core-decomposition.md` 记录 invariants、crate 归属、禁止依赖规则。
- [x] 在 `AGENTS.md` 增加短链接，说明 core 拆解期间先看架构文档。
- [x] 在 `src/crates/core/AGENTS.md` 增加约束：

```markdown
During core decomposition, `bitfun-core` is a compatibility facade. New modules
should prefer the extracted owner crate listed in `docs/architecture/core-decomposition.md`.
Do not add new cross-layer references from `service` to `agentic` without a port.
```

- [x] 执行脚本保护检查。

**验证：**

```powershell
git diff -- package.json scripts/dev.cjs scripts/desktop-tauri-build.mjs scripts/ensure-openssl-windows.mjs scripts/ci/setup-openssl-windows.ps1 BitFun-Installer
```

**风险与处理：**

- 风险：基线命令在低性能机器耗时较长。
- 处理：只在需要建立基线的机器运行；生成文件不提交；普通开发者不强制执行 timing。

---

### Plan 1：引入 `product-full` feature 安全网

**目的：** 在任何默认 feature 变轻之前，先让产品 crate 显式声明完整能力，避免多形态产品构建内容被意外改变。

**文件范围：**

- 修改：`src/crates/core/Cargo.toml`
- 修改：`src/apps/desktop/Cargo.toml`
- 修改：`src/apps/cli/Cargo.toml`
- 修改：`src/crates/acp/Cargo.toml`
- 不修改：`src/apps/server/Cargo.toml`，除非它已经在当前产品构建中显式依赖 `bitfun-core`
- 不修改：`src/apps/relay-server/Cargo.toml`，除非它已经在当前产品构建中显式依赖 `bitfun-core`

**任务：**

- [x] 在 `bitfun-core` 中新增 `product-full`，但保持当前 default 行为不变。

```toml
[features]
# Full product runtime feature set. Product binaries must depend on this
# explicitly before `bitfun-core` default features are made lighter.
default = ["product-full"]
product-full = ["ssh-remote"]
tauri-support = ["tauri"]
ssh-remote = ["russh", "russh-sftp", "russh-keys", "shellexpand", "ssh_config"]
```

- [x] 产品 crate 显式启用完整能力。

```toml
bitfun-core = { path = "../../crates/core", default-features = false, features = ["product-full"] }
```

- [x] 这个阶段禁止把 `default` 改成空。
- [x] 为 `product-full` 增加注释，说明它是多形态产品能力保护开关。
- [x] 只更新当前已经依赖 `bitfun-core` 的 crate。不要为了统一写法给 server 或 relay-server 新增 `bitfun-core` 依赖。

**生命周期说明：**

- `product-full` 是迁移期和发布期的完整能力保护开关，不是新功能的万能聚合点。新增 owner crate 时，必须先定义具体 feature group，再由产品完整 runtime 显式选择是否纳入 `product-full`。
- P3 结束前不评估移除或减轻 `product-full`。如果未来希望用更细粒度的 per-product feature set 替代它，必须作为独立发布风险评估执行，并先通过完整产品矩阵。
- 不允许在模块移动 PR 中同时做 `product-full` 淘汰、`default = []` 或产品能力裁剪。

**验证：**

```powershell
cargo check -p bitfun-core --features product-full
cargo check -p bitfun-desktop
cargo check -p bitfun-cli
cargo check -p bitfun-server
cargo check -p bitfun-acp
cargo check --workspace
```

**风险与处理：**

- 风险：某产品 crate 之前依赖隐式 default，现在路径写错导致能力缺失。
- 处理：每个产品 crate 单独 check；不改构建脚本；不减少 release feature。

---

### Plan 2：把现有 nested crate 移到 workspace 顶层

**目的：** 先处理已经是 crate 的模块，降低后续拆分歧义，且风险较低。

**文件范围：**

- 移动：`src/crates/core/src/service/terminal` -> `src/crates/terminal`
- 移动：`src/crates/core/src/agentic/tools/implementations/tool-runtime` -> `src/crates/tool-runtime`
- 修改：workspace 根 `Cargo.toml`
- 修改：`src/crates/core/Cargo.toml`
- 必要时修改：旧路径 re-export

**任务：**

- [x] 移动 `terminal-core` 目录到 `src/crates/terminal`。
- [x] 保持 package name `terminal-core` 和 lib name `terminal_core` 不变。
- [x] 移动 `tool-runtime` 到 `src/crates/tool-runtime`。
- [x] 保持 package name `tool-runtime` 和 lib name `tool_runtime` 不变。
- [x] 更新 workspace members。
- [x] 更新 `src/crates/core/Cargo.toml` path：

```toml
terminal-core = { path = "../terminal" }
tool-runtime = { path = "../tool-runtime" }
```

- [x] 在旧 re-export 点加关键节点注释：

```rust
// Terminal is implemented in the workspace-level `terminal-core` crate.
// This re-export preserves the legacy `bitfun_core::service::terminal` path.
pub use terminal_core as terminal;
```

**验证：**

```powershell
cargo check -p terminal-core
cargo check -p tool-runtime
cargo check -p bitfun-core --features product-full
cargo check --workspace
```

**风险与处理：**

- 风险：路径移动影响相对路径、测试 fixture 或 include。
- 处理：保持 package/lib 名称不变；只改 Cargo path；不改行为。

---

### Plan 3：抽取 `bitfun-core-types`

**目的：** 建立真正底层的共享类型 crate，让后续服务和 agent runtime 不需要依赖 `bitfun-core`。

**文件范围：**

- 新增：`src/crates/core-types/Cargo.toml`
- 新增：`src/crates/core-types/src/lib.rs`
- 新增：`src/crates/core-types/src/errors.rs`
- 后续按依赖确认再新增：`session.rs`、`workspace.rs`、`config.rs`
- 修改：workspace 根 `Cargo.toml`
- 修改：`src/crates/core/Cargo.toml`
- 修改：旧模块 re-export

**第一批只移动：**

- 纯 error DTO：`ErrorCategory`、`AiErrorDetail`
- 纯 AI 错误分类/detail 构造 helper
- 已去除 runtime/network 依赖后的 `BitFunError`（当前未移动）
- 已去除 runtime/network 依赖后的 `BitFunResult`（当前未移动）
- 已确认无 runtime 依赖的 session/workspace/config DTO

**第一批禁止移动：**

- manager
- global service
- registry
- 文件 IO
- process spawning
- async runtime orchestration
- 任何需要 Tauri、git2、rmcp、reqwest、image 的类型实现

**任务：**

- [x] 建立轻依赖 crate，当前只允许 `serde`：

```toml
[dependencies]
serde = { workspace = true }
```

- [x] 先把 `ErrorCategory` / `AiErrorDetail` 抽到 `core-types`，并由 `bitfun-events::agentic` re-export 保持旧路径不变。
- [x] 把 AI 错误分类和 detail 构造 helper 下沉到 `core-types`，`BitFunError::error_category` / `error_detail` 只做委托。
- [x] 将原本依赖完整 `bitfun-core` 的 AI 错误分类测试迁移到 `bitfun-core-types` 单元测试，作为后续错误边界移动的轻量保护。
- [x] 先拆解 `BitFunError` 的 runtime/network 依赖边界。`reqwest::Error` 已改为字符串承载，`tokio::sync::AcquireError` 已改为调用点显式映射，错误模块不再直接引用这两个类型。
- [x] LR1 已复核 `BitFunError` 剩余 concrete error-wrapper 依赖。当前仍保留
  `serde_json::Error`、`anyhow::Error`、`std::io::Error` 和相关 `From<T>` 兼容行为；
  处理决策是继续 core-owned，不在 LR1 字符串化或改变错误边界。
- [x] `BitFunError`、`BitFunResult` 迁移标记为 deferred：只有当错误类型不再需要
  concrete wrapper，或单独 PR 明确接受 `core-types` 的轻量 error 依赖后才可移动。
- [x] `BitFunError` 移动后的旧路径 re-export 约束已记录；实际 re-export 只在未来迁移 PR 执行：

```rust
pub use bitfun_core_types::errors::{BitFunError, BitFunResult};
```

- [x] crate 顶部增加边界注释：

```rust
//! Shared BitFun domain types.
//!
//! This crate must not depend on `bitfun-core`, service crates, agent runtime,
//! platform adapters, process execution, or network clients.
```

- [x] 已移动第一批 shared DTO/helper，并确认依赖方向为 `bitfun-events -> bitfun-core-types`、`bitfun-core -> bitfun-core-types`。
- [x] LR1 已校准后续 shared DTO 归属：当前没有适合继续批量移动的 DTO。后续只能按
  单个 owner/单个 DTO 推进，并在移动时确认依赖方向。

**当前状态：** Plan 3 是部分完成。`ErrorCategory`、`AiErrorDetail` 和第一批纯 helper 已进入 `core-types`；LR1 已明确 `BitFunError` / `BitFunResult` 继续 core-owned，后续 DTO 不批量移动。未完成迁移不阻塞低风险准备闭环，但会阻塞“错误类型完全归属 core-types”的完成声明。

**验证：**

```powershell
cargo test -p bitfun-core-types
cargo check -p bitfun-core --features product-full
cargo check --workspace
```

**风险与处理：**

- 风险：把带行为的类型误放入 types，导致 types 变重。
- 处理：核心判断是“是否需要 IO、全局状态、网络、平台 API、runtime manager”。需要则不能进入 types。
- 当前阻塞：`BitFunError` 还带有 `serde_json::Error` / `anyhow::Error` concrete wrapper 和 `From<T>` 兼容行为。先保持在 `bitfun-core`，后续单独评估是把这些 wrapper 字符串化，还是允许 `core-types` 引入轻量 error 依赖后再移动。

---

### Plan 4：抽取 `bitfun-agent-stream`

**目的：** 让 stream processor 相关测试脱离完整 `bitfun-core`，这是较容易验证构建提速收益的拆分点。

**文件范围：**

- 新增：`src/crates/agent-stream/Cargo.toml`
- 新增：`src/crates/agent-stream/src/lib.rs`
- 移动/适配：`src/crates/core/src/agentic/execution/stream_processor.rs`
- 移动/适配测试：
  - `src/crates/core/tests/stream_processor_openai.rs`
  - `src/crates/core/tests/stream_processor_anthropic.rs`
  - `src/crates/core/tests/stream_processor_tool_arguments.rs`
  - `src/crates/core/tests/stream_replay_regressions.rs`
  - 相关 fixture/helper
- 修改：`src/crates/core/src/agentic/execution/mod.rs`
- 修改：`src/crates/core/Cargo.toml`
- 修改：workspace 根 `Cargo.toml`

**任务：**

- [x] 创建 `bitfun-agent-stream`，依赖控制在 stream 所需范围：

```toml
anyhow = { workspace = true }
async-trait = { workspace = true }
bitfun-events = { path = "../events" }
bitfun-ai-adapters = { path = "../ai-adapters" }
futures = { workspace = true }
serde = { workspace = true }
tokio = { workspace = true }
tokio-util = { workspace = true }
serde_json = { workspace = true }
log = { workspace = true }
uuid = { workspace = true }
```

- [x] 移动 stream result/error/context processor。
- [x] 消除对 `crate::agentic` 的直接引用，改为依赖 `bitfun-events`、`bitfun-ai-adapters`。
- [x] 旧路径 compatibility wrapper：

```rust
//! Compatibility wrapper for the extracted agent stream processor.

pub struct StreamProcessor {
    inner: bitfun_agent_stream::StreamProcessor,
}
```

- [x] stream 测试迁移到 `src/crates/agent-stream/tests`，fixture harness 改为测试内事件 sink，不再依赖完整 `bitfun-core`。

**验证：**

```powershell
cargo test -p bitfun-agent-stream
cargo test -p bitfun-core --lib stream_processor
cargo check -p bitfun-core --features product-full
cargo check --workspace
```

**风险与处理：**

- 风险：stream test 依赖旧 core test helper。
- 处理：只迁移 stream 所需 fixture；不要把 core test helper 整体搬成新重依赖。

---

### Plan 5：引入 runtime ports，准备打断 `service <-> agentic` 循环

**目的：** 在真正移动 service crate 之前，先建立可替换的 cross-layer 调用边界；具体 service call-site 迁移按后续 owner crate 阶段逐步完成。

**文件范围：**

- 新增：`src/crates/core-types/src/ports.rs` 或独立 `src/crates/runtime-ports`
- 修改：`src/crates/core/src/service/remote_connect/**`
- 修改：`src/crates/core/src/service/mcp/**`
- 修改：`src/crates/core/src/service/cron/**`
- 修改：`src/crates/core/src/service/snapshot/**`
- 修改：`src/crates/core/src/agentic/tools/registry.rs`
- 修改：`src/crates/core/src/agentic/coordination/**`

**任务：**

- [x] 先定义 port DTO 和 trait，不移动大模块。
- [x] 新增独立轻量 `bitfun-runtime-ports`，只包含 DTO / trait，不依赖 `bitfun-core`、manager、service concrete、app crate 或平台 adapter。
- [x] 为 `ConversationCoordinator` 提供 `AgentSubmissionPort` / `SessionTranscriptReader` adapter，作为 remote connect / service 后续迁移入口。
- [x] 为 `ConversationCoordinator` 提供 `AgentTurnCancellationPort` / `RemoteControlStatePort` adapter，复用现有取消与 session state 读取语义，不引入新的队列或取消策略。
- [x] 为 `ToolRegistry` 提供 `DynamicToolProvider` adapter。
- [x] 用 `ToolDecorator` 注入 registry 注册装饰入口，保留默认 snapshot wrapping 行为。
- [x] 为 `ConfigService` 提供 `ConfigReadPort` adapter，先建立读取边界，不移动 config service。
- [x] 新增 `RuntimeEventEnvelope` / `RuntimeEventSink` 观测事件契约，当前只作为后续 remote runtime 解耦入口，不注册新的运行时事件发布实现。
- [x] LR1 已复核 remote connect / cron / MCP concrete call-site 状态：MCP runtime 迁移已在
  后续 PR 闭环；remote-connect dialog submission、cron 调度和剩余 product execution
  call-site 继续显式 core-owned，进入 H3 时才按单一 owner 补 port/provider 与 regression。
- [x] 已补 `remote_image` attachment DTO 与 remote-connect image submission request builder 契约；`AgentSubmissionPort` 仍显式拒绝 generic attachments，直到多模态行为等价测试和接入方案单独完成。
- [x] P2 concrete call-site 迁移前，已把 `AgentSubmissionRequest.turn_id` 提升为显式可选 DTO 字段（序列化为 `turnId`）；coordinator 兼容期先读显式字段再回退 `metadata["turnId"]`，并补充序列化与 adapter 回归测试。
- [x] P2/P3 tool owner 迁移前，`DynamicToolProvider` 已停止从 `mcp__server__tool` 注册名反推 `provider_id`；MCP wrapper 显式携带 provider metadata，并用特殊 provider id / MCP-like 名称测试证明 provider 身份不依赖注册名格式。

示例：

```rust
#[async_trait::async_trait]
pub trait SessionTranscriptReader: Send + Sync {
    async fn read_session_transcript(
        &self,
        request: SessionTranscriptRequest,
    ) -> PortResult<SessionTranscript>;
}
```

**验证：**

```powershell
cargo check -p bitfun-core --features product-full
cargo test -p bitfun-core remote_connect
cargo test -p bitfun-core mcp
cargo check --workspace
```

**风险与处理：**

- 风险：接口抽象过大，变成另一个 service god object。
- 处理：每个 port 只覆盖一个调用方向和一个能力集合；避免 `CoreContext` 这种万能接口。

---

### Plan 6：抽取中等粒度 service crate

**目的：** 用两个 service owner crate 承载当前 `service` 目录，而不是把每个 service 都拆成独立 crate。这样可以隔离重依赖，同时避免 crate 数量过多。

#### Plan 6A：抽取 `bitfun-services-core`

**文件范围：**

- 新增：`src/crates/services-core/**`
- 移动/适配基础服务：
  - `src/crates/core/src/service/config/**`
  - `src/crates/core/src/service/session/**`
  - `src/crates/core/src/service/workspace/**`
  - `src/crates/core/src/service/workspace_runtime/**`
  - `src/crates/core/src/service/filesystem/**`
  - `src/crates/core/src/service/system/**`
  - `src/crates/core/src/service/runtime/**`
  - `src/crates/core/src/service/i18n/**`
  - `src/crates/core/src/service/ai_rules/**`
  - `src/crates/core/src/service/ai_memory/**`
  - `src/crates/core/src/service/bootstrap/**`
  - `src/crates/core/src/service/diff/**`
  - `src/crates/core/src/service/session_usage/**`
  - `src/crates/core/src/service/token_usage/**`
  - `src/crates/core/src/service/project_context/**`
- 暂留或 feature-gate：
  - `src/crates/core/src/service/search/**`
  - `src/crates/core/src/service/lsp/**`
  - `src/crates/core/src/service/cron/**`
  - `src/crates/core/src/service/snapshot/**`

**任务：**

- [x] 新建 `bitfun-services-core`，默认 feature 尽量轻。
- [x] 基础 DTO 从 `bitfun-core-types` 引入。
- [x] LR1 已复核 services-core 与 agent runtime 的调用边界：现有可替换入口通过
  `runtime-ports`/窄 adapter 承载；未完成的 scheduler、agent registry 或执行 runtime
  调用不在 LR1 移动，后续进入 H3 前需单独设计 port/provider。
- [x] LR1 决策：`search`、`lsp`、`cron`、`snapshot` 继续按同 crate 内 feature group
  处理，不新增独立 crate；真正 runtime owner 迁移必须等 H3 风险评审。
- [x] 已迁移模块的 core 旧路径通过 re-export 保持。

**当前安全迁移状态：**

- `bitfun-services-core` 已承接 `system`、`diff`、`process_manager`、session / usage / token usage 类型与通用本地 filesystem facade；core 旧路径继续 re-export。
- `SessionKind` 已归属 `bitfun-core-types`，Deep Review session manifest / cache 字段随 session 类型迁移，并由序列化兼容测试保护。
- `config`、`workspace`、`workspace_runtime`、`runtime`、`i18n`、`bootstrap`、`project_context` 仍保留在 core；remote workspace overlay、`BitFunError` 映射、MiniApp filesystem IO、tool-result persistence、`PathManager` 绑定和产品 runtime 接线继续显式 core-owned。

**验证：**

```powershell
cargo test -p bitfun-services-core
cargo check -p bitfun-core --features product-full
cargo check -p bitfun-desktop
```

#### Plan 6B：抽取 `bitfun-services-integrations`

**文件范围：**

- 新增：`src/crates/services-integrations/**`
- 移动/适配重集成服务：
  - `src/crates/core/src/service/git/**`
  - `src/crates/core/src/service/mcp/**`
  - `src/crates/core/src/service/remote_ssh/**`
  - `src/crates/core/src/service/remote_connect/**`
  - `src/crates/core/src/service/announcement/**`
  - `src/crates/core/src/service/file_watch/**`

**feature group：**

```toml
[features]
default = []
git = ["git2"]
mcp = ["rmcp"]
remote-ssh = ["russh", "russh-sftp", "russh-keys", "shellexpand", "ssh_config"]
remote-connect = ["tokio-tungstenite", "qrcode", "image", "bitfun-relay-server"]
announcement = ["reqwest"]
file-watch = ["notify"]
debug-log = ["axum"]
product-full = ["git", "mcp", "remote-ssh", "remote-connect", "announcement", "file-watch", "debug-log"]
```

**任务：**

- [x] 先迁移 `git`，因为边界相对清晰。
- [x] LR1 已复核 `remote-ssh`：当前仅保持 path/session identity 等 contract/helper
  外移；SSH channel、SFTP、remote FS、remote terminal 和 manager assembly 继续 core-owned。
  若 H3 继续迁移，必须保留 `ssh-remote` 语义并补 remote 等价测试。
- [x] 先迁移 `remote-ssh` 的纯 contract/type、workspace path/identity helper 与 unresolved-session-key helper，runtime manager / fs / terminal 仍保留在 core。
- [x] 迁移 `mcp` 的 PR2 runtime 与 dynamic provider：config service orchestration、server process / transport lifecycle、resource/prompt adapter、catalog cache、list-changed/reconnect policy、dynamic descriptor / provider / result rendering 均归属 `bitfun-services-integrations`。
- [x] `bitfun-core` 保留 core `ConfigService` store adapter、OAuth data-dir 注入、`BitFunError` 映射、旧路径 facade 和全局 tool registry / manifest 组装；product tool runtime manifest / `GetToolSpec` 执行 owner 化不混入本 PR。
- [x] 先迁移 `announcement` 的纯 types contract，scheduler / state store / content loader / remote fetch 仍保留在 core。
- [x] 先完成 `remote-connect` contract slice：remote chat/image/tool/session wire DTO 与 relay/bot session/submission request builder 由 `bitfun-services-integrations` 拥有，relay/bot session 创建通过 `AgentSubmissionPort`。
- [x] 已补齐 remote runtime 迁移前的第一层 port baseline：`SessionTranscriptReader`、`AgentTurnCancellationPort`、`RemoteControlStatePort`、`RuntimeEventSink` 与 remote image attachment/request DTO；完整 `remote-connect` runtime 仍需后续单独迁移并补 queue/event/image 行为等价测试。
- [x] `RemoteSessionStateTracker`、`TrackerEvent`、tracker registry lifecycle 与 remote tool preview slimming helper 已迁入 `bitfun-services-integrations`；core 只保留 tracker host adapter、dispatcher、session restore、terminal pre-warm 与实际 dialog submission routing。
- [x] 已补齐 remote-connect runtime 迁移前快照：remote command/response wire shape、session restore target、active turn poll snapshot、cancel decision、legacy image fallback / unified image context preference、tracker completion/fanout 与 RemoteRelay/Bot queue policy 均有 focused regression。
- [x] 已将 remote-connect wire / poll 边界与纯运行时策略 helper 迁入 `bitfun-services-integrations`：command/response wire DTO、remote model catalog DTO / config-shape assembly、poll response assembly / model catalog poll delta、remote model selection policy、legacy image context fallback / explicit context preference、restore target decision、cancel decision、dialog scheduler outcome assembly、remote workspace file IO/path helper、remote file command / response assembly、dialog/cancel/interaction response helper、workspace/session response assembly helper、remote chat history presentation assembly、image-context adapter contract 与 remote file transfer size/chunk/name policy 由 owner crate 提供；core 仅保留 dispatcher、session restore 执行、workspace-root source、persistence/workspace service reads、model config loading / resolver injection、remote chat history projection / image compression / enhanced-input cleanup、`ImageContextData` concrete impl、terminal pre-warm adapter 与实际 dialog submission routing。
- [x] H3 remote-connect closure：RemoteRelay/Bot dialog submission orchestration、agent type normalization、turn id resolution、restore decision、terminal pre-warm decision、queue policy、dialog scheduler outcome assembly、remote model catalog config-shape assembly、remote model selection policy、remote workspace file IO/path helper、remote file command / response assembly、dialog/cancel/interaction response helper、remote chat history presentation assembly 与 image-context adapter contract 归属 `bitfun-services-integrations`；core 继续作为 concrete scheduler/session restore/terminal adapter、workspace-root source、workspace/session response adapter、model config resolver adapter 与 persisted history projection adapter，不改变产品行为。
- [x] 已迁移的集成能力保持 core 旧路径 re-export。
- [x] 产品完整 runtime 通过 `services-integrations/product-full` 启用已迁移集成能力。

**当前安全迁移状态：**

- `bitfun-services-integrations` 已承接 `file_watch`、`git`、remote-SSH 纯 contract/helper、MCP protocol/config/runtime/dynamic provider、announcement 纯 types，以及 remote-connect 的 wire DTO、poll/model catalog、model selection policy、tracker、file/image/dialog helper、RemoteRelay/Bot orchestration policy 与 remote chat history presentation assembly。
- core 继续持有 `ConfigService` store adapter、OAuth data-dir 注入、`BitFunError` 映射、legacy facade、全局 tool registry / manifest 组装、SSH runtime manager / fs / terminal、workspace-root source、persistence/workspace service reads、`ImageContextData` concrete impl、terminal adapter、concrete scheduler/session restore 执行和 announcement scheduler/state/content/fetch runtime。
- Deep Review queue/cost/context、session manifest、stream dedupe、search fallback、session rollback persistence 等 latest-main 行为仍属于 core runtime 或对应产品 runtime；继续迁移 remote-connect / MCP / search / session 前必须先补运行状态 port 合约和等价测试。

**验证：**

```powershell
cargo test -p bitfun-services-integrations --features git
cargo check -p bitfun-services-integrations --features product-full
cargo check -p bitfun-core --features product-full
cargo check -p bitfun-desktop
cargo check -p bitfun-cli
```

**Plan 6 总体风险与处理：**

- 风险：`services-integrations` 内 feature 互相污染，导致局部测试仍编译过多依赖。
- 处理：默认 feature 为空；局部测试显式启用单一 feature；产品 crate 只通过 `product-full` 启用完整能力。
- 风险：两个 service crate 仍然偏大。
- 处理：先接受中等粒度。只有实测某个 feature group 仍显著拖慢关键测试时，再把它升级为独立 crate。

---

### Plan 7：拆解 agent tools

**目的：** 避免 tool registry 拉入所有工具实现和对应 service 依赖。

**目标 crate：**

- `src/crates/agent-tools`
- `src/crates/tool-packs`

**任务：**

- [x] 抽出 tool result、validation、dynamic metadata、runtime restriction、path resolution DTO、provider-neutral tool execution result/error/invalid-call presentation policy，以及 generic registry / dynamic provider container 到 `agent-tools`。
- [x] 抽出纯 manifest/exposure / GetToolSpec presentation 契约到 `agent-tools`：`ToolExposure`、`GetToolSpec` 名称、纯 manifest policy、collapsed prompt stub、prompt-visible ordering、GetToolSpec prompt description / input schema / validation / assistant-detail rendering / collapsed summary-detail / duplicate-load hint；core 继续拥有 runtime assembly 和执行 owner。
- [x] 抽出 static tool provider 安装合约到 `agent-tools`，并将 core 内置工具列表收敛到 `product_runtime.rs` 的 core-owned provider groups；不迁移 concrete tool implementation。
- [x] 抽出 `ToolContextFacts` / `ToolWorkspaceKind` 轻量上下文事实契约，并由 core `ToolUseContext` 提供只读投影；workspace root fact 使用 session identity 的 logical path，remote 场景输出 normalized remote root；不迁移 collapsed unlock state、runtime handles、workspace services 或 cancellation token。
- [x] 增加 `PortableToolContextProvider` 只读 facts provider 合约，并由 core `ToolUseContext` 兼容实现；该合约不暴露 workspace services、cancellation token、computer-use host 或 collapsed unlock state。
- [x] LR1 已锁定 tool runtime port/provider 设计前置条件：`PortableToolContextProvider`
  只能提供只读 facts，不能携带 runtime handles、workspace services、cancellation token
  或 collapsed unlock state；`Tool` trait 与 `ToolUseContext` 在 H1 前继续 core-owned。
- [x] `agent-tools` 不依赖任何 concrete service。
- [x] 具体工具实现迁移 deferred 到 H1；LR1 不迁移 concrete tools。未来迁移到
  `tool-packs` 时按 feature group 分模块：
  - basic file/search/terminal
  - git
  - MCP
  - browser/web
  - computer use
  - miniapp
  - cron/task/agent control
- [x] `tool-packs` 默认 feature 为空，产品完整 runtime 启用 `product-full`；当前仅提供 basic / git / mcp / browser-web / computer-use / image-analysis / miniapp / agent-control feature-group 元数据，不注册或迁移任何具体工具。
- [x] 产品 runtime assembly provider 注册 deferred 到 H1；LR1 继续由 core product tool
  runtime 安装 provider：

```rust
registry.install_provider(BasicToolProvider::new());
registry.install_provider(GitToolProvider::new(git_service));
registry.install_provider(McpToolProvider::new(mcp_service));
```

- [x] 兼容构造函数迁移 deferred 到 H1；LR1 继续保持现有 core 旧构造路径：

```rust
pub fn create_tool_registry() -> ToolRegistry {
    product_full_tool_registry()
}
```

- [x] registry / manifest 迁移前等价基线已在 LR1 复核：现有
  `registry_preserves_builtin_tool_manifest_for_owner_migration`、
  `registry_preserves_readonly_tool_manifest_for_owner_migration`、
  `manifest_snapshot_preserves_collapsed_tool_discovery_contract` 与
  `bitfun-agent-tools` tool contract 测试覆盖当前低风险拆分；H1 迁移前仍需扩展为完整产品
  registry、expanded/collapsed exposure 与 prompt-visible manifest 等价快照。
- [x] runtime manifest assembly / `GetToolSpec` 执行迁移前的 baseline 已作为 H1 进入条件：
  保留并扩展 expanded/collapsed manifest、
  prompt-visible stub、unlock state 和 desktop/MCP/ACP catalog 等价测试。
- [x] H1 解锁契约切片只抽出 `GetToolSpec` 结果到 collapsed 工具名集合的纯收集规则；
  `ToolUseContext.unlocked_collapsed_tools`、执行消息解析、runtime manifest assembly 和
  `GetToolSpecTool` 执行仍由 core 拥有。
- [x] H1 manifest builder 切片只抽出 prompt-visible manifest definition 的纯组装规则：
  expanded 工具的 description/schema 仍由 core 按 `ToolUseContext` 获取，collapsed stub
  渲染和排序由 `bitfun-agent-tools` 统一；runtime manifest owner 仍未迁移。
- [x] H1 catalog/exposure 切片继续抽出 registry snapshot 到 manifest policy input、
  generic collapsed exposure 查询、`GetToolSpec` catalog description 和 detail JSON 的纯规则；core
  仍负责 tool availability、product catalog source、product snapshot wrapper adapter、runtime unlock state 和工具执行。

**当前安全迁移状态：**

- `bitfun-agent-tools` 已承接 tool DTO / validation / render options、runtime restrictions、path/runtime artifact/remote POSIX path 纯契约、tool execution presentation policy、portable context facts/provider、generic registry、static/dynamic provider contract、decorator reference、readonly/enabled filter、catalog / GetToolSpec provider、contextual manifest resolver、GetToolSpec runtime facade 和 Tool-result vector adapter。
- `bitfun-tool-packs` 只承载 feature group 元数据和 product provider group plan；默认 feature 为空，不注册或迁移 concrete tools。
- core 通过 `product_runtime.rs` / `tool_adapter.rs` 保留 product registry snapshot、product catalog / manifest facade、snapshot wrapper 注入、`dyn Tool` adapter、`GetToolSpecTool` Tool impl、`ToolUseContext` runtime handles、collapsed unlock state、`BitFunError` 映射和 concrete tool materialization。
- boundary check 已锁定：`agent-tools` / `tool-packs` 不得拥有 product tool runtime assembly、`GetToolSpecTool` Tool impl、collapsed unlock state、snapshot wrapper implementation 或 concrete tools；core 也不得回退到散落的 dynamic metadata map / 手工 provider 注册。
- 后续若继续迁移 tool runtime，只能按完整高风险 owner 主题推进，并先证明 tool visibility、expanded/collapsed manifest、`GetToolSpec` unlock、snapshot wrapper、runtime restrictions、cancellation、Deep Review hooks 和具体 IO 行为等价。

**验证：**

```powershell
cargo test -p bitfun-agent-tools
cargo test -p bitfun-agent-tools get_tool_spec_contract --test tool_contracts
cargo test -p bitfun-tool-packs --features basic
cargo check -p bitfun-tool-packs --features product-full
cargo check -p bitfun-core --features product-full
cargo test -p bitfun-core registry_ --lib
cargo test -p bitfun-core manifest_ --lib
cargo test -p bitfun-core get_tool_spec --lib
cargo test -p bitfun-core dynamic_tool_provider_ --lib
cargo check -p bitfun-desktop
```

**风险与处理：**

- 风险：工具列表遗漏导致产品能力缺失。
- 处理：拆分前生成工具清单基线；拆分后 registry 等价性测试必须通过。
- 风险：expanded/collapsed exposure、`GetToolSpec` 插入、prompt stub 或 unlock state 不等价，会改变模型实际可见工具和调用顺序。
- 处理：迁移前补 manifest / `GetToolSpec` 快照和执行解锁 regression；迁移后同时验证 desktop/MCP/ACP tool catalog。
- 风险：单个 `tool-packs` crate 过重。
- 处理：先用 feature group 控制编译面；只有某个工具族被实测证明明显拖慢局部测试时，再拆成独立 crate。

---

### Plan 8：抽取产品子域到 `bitfun-product-domains`

**目的：** 把相对独立的产品子域移出 core，但不为每个子域创建独立 crate。

**文件范围：**

- 新增：`src/crates/product-domains/**`
- 移动/适配：
  - `src/crates/core/src/miniapp/**`
  - `src/crates/core/src/function_agents/**`

**feature group：**

```toml
[features]
default = []
miniapp = []
function-agents = []
product-full = ["miniapp", "function-agents"]
```

**任务：**

- [x] miniapp compiler 迁移到 `product-domains::miniapp::compiler`，core 保留原 `miniapp::compiler::compile` 返回 `BitFunResult` 的兼容 wrapper。
- [x] miniapp exporter DTO、runtime detection DTO、runtime search plan、worker install 命令选择与 package.json storage-shape helper 迁移到 `product-domains::miniapp`；当前 HR-B 进一步迁移 concrete runtime detector owner，core 保留实际 export / worker pool / storage IO 执行逻辑。
- [x] LR1 已完成 MiniApp runtime 迁移前 owner 审视：runtime、storage、manager、host
  dispatch、exporter、builtin 中涉及 filesystem IO、worker process、asset seed、marker IO、
  host dispatch execution 或 recompile orchestration 的部分继续 core-owned，actual owner
  迁移 deferred 到 H2。
- [x] LR1 已完成 function-agent runtime 迁移前 owner 审视：pure DTO/helper/parser/facade
  可由 `product-domains::function_agents` 承载；Git service 与 AI call 继续 core-owned。
  prompt template、JSON extraction 和 domain error mapping 已在 H2 迁入 product-domain policy。
- [x] 已为 miniapp runtime/storage 与 function-agent Git/AI 边界定义迁移前 provider / port contract，并补充 core-owned MiniApp storage/runtime 与 function-agent Git snapshot adapter 等价测试；实际 IO/进程/Git/AI 执行 owner 迁移仍待后续 port/provider 方案确认后推进。
- [x] 已迁移模块的 core 旧路径 re-export。
- [x] function-agent agent-runtime port 依赖 deferred 到 H2；LR1 不引入新的 agent runtime
  port，也不改变现有 service manager 调用语义。
- [x] LR1 未改 server/desktop 调用路径；后续 H2/H3 若迁移 runtime owner，必须用现有
  product check 和 focused regression 证明调用路径等价。

**当前安全迁移状态：**

- `bitfun-product-domains::miniapp` 已承接 MiniApp types / bridge / permission policy、compiler、export DTO、runtime detection DTO、runtime search plan、concrete runtime detector、worker install plan、package/storage layout、lifecycle/revision helpers、host routing / allowlist policy、customization metadata、builtin bundle / marker / seed policy、runtime/storage port contract 和 create/update/draft/apply/import 纯状态转换。
- `bitfun-product-domains::function_agents` 已承接 git/startchat function-agent DTO、prompt template、commit/work-state helper、AI response JSON extraction / repair / parser、domain error mapping、Git/AI port contract 和只读 project context analyzer。
- core 继续持有 MiniApp source/storage/marker filesystem IO、compile orchestration、worker process、host dispatch execution、export skeleton、built-in asset include / seed / recompile、`PathManager` 注入，以及 function-agent Git/AI service adapter、AI client 调用、provider acquisition 和 transport error mapping。
- 通用本地 filesystem owner 已迁入 `bitfun-services-core::filesystem`；这不改变 MiniApp filesystem IO、tool-result persistence、remote SSH runtime 或产品持久化接线仍显式 core-owned 的结论。
- boundary check 已锁定 product-domain owner anchor 和 core-owned runtime anchor，防止把 port contract、response policy 或 runtime detector 误读成 storage IO、worker process、host dispatch、builtin asset seeding runtime 或 Git/AI service runtime 已完成迁移。

**验证：**

```powershell
cargo test -p bitfun-product-domains --no-default-features
cargo test -p bitfun-product-domains --features miniapp
cargo test -p bitfun-product-domains --features product-full
cargo check -p bitfun-product-domains --features product-full
cargo check -p bitfun-core --features product-full
cargo check -p bitfun-desktop
cargo check -p bitfun-server
```

---

### Plan 9：将 `bitfun-core` 收敛为 facade + product runtime assembly

**目的：** 完成迁移收束，让 `bitfun-core` 不再是新实现承载点。

**文件范围：**

- 修改：`src/crates/core/src/lib.rs`
- 修改：`src/crates/core/src/service/mod.rs`
- 修改：`src/crates/core/src/agentic/mod.rs`
- 修改：`src/crates/core/Cargo.toml`

**任务：**

- [x] 将可替换的实现模块改为 re-export（限本轮已迁移 owner crate；高耦合 runtime 保留为 core-owned runtime）。
- [x] 在顶层加入关键节点注释：

```rust
//! Compatibility facade and full product runtime assembly.
//!
//! New implementation code should live in owner crates under `src/crates/*`.
//! This crate re-exports legacy paths and wires the full BitFun product runtime.
```

- [x] `bitfun-core/Cargo.toml` 依赖裁剪 deferred 到 H4/H5；LR1 已确认当前仍因
  core-owned runtime 保留 concrete runtime 依赖，不强行删减。
- [x] 旧路径保持 import-compatible。
- [x] `default = []` / per-product feature matrix 评估 deferred 到 H5；只有所有产品 crate
  都显式启用完整 runtime 且有 feature graph baseline 后，才可以在独立 PR 中评估：

```toml
default = []
```

**当前收敛状态：**

- 本轮不把 `remote-ssh` runtime、`remote-connect`、announcement runtime、concrete tool implementations、`ToolUseContext`、product registry snapshot / manifest / exposure assembly、miniapp runtime/compiler/builtin、function-agent 运行逻辑声明为已迁移；它们继续作为 `bitfun-core` 的 product runtime assembly 或后续 owner PR 拥有路径。`git` feature group 已外移；`remote-ssh` 目前只外移 contract/type、workspace path/identity helper 与 unresolved-session-key helper；MCP PR2 已外移 config service orchestration、server process / transport lifecycle、adapter 和 dynamic tool/resource/prompt provider；generic tool registry / static provider installation / dynamic descriptor assembly 已由 `bitfun-agent-tools` 拥有，product provider group plan 由 `bitfun-tool-packs` 拥有，core 只保留 ConfigService store adapter、OAuth data-dir 注入、BitFunError 映射、legacy facade、concrete tool materialization、tool manifest/exposure product facade 和 snapshot decorator assembly；`announcement` 目前只外移 types contract。
- 新增 `scripts/check-core-boundaries.mjs`，用于阻止已拆出的 owner crate 反向依赖 `bitfun-core`。该脚本只证明 crate graph 方向，不替代产品等价性测试。
- `default = []` 仍保持为后续独立评估项，本轮不调整默认 feature、构建脚本或 release 脚本。

**验证：**

```powershell
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
cargo check -p bitfun-desktop
cargo check -p bitfun-cli
cargo check -p bitfun-server
cargo check -p bitfun-relay-server
cargo check -p bitfun-acp
cargo check --workspace
```

**风险与处理：**

- 风险：facade re-export 引发公开路径破坏。
- 处理：每个旧路径迁移都必须有兼容 shim；必要时加 compile-only compatibility test。

---

## 6. 依赖版本收敛计划

依赖版本收敛必须和 crate 拆解并行但不要混入高风险移动 PR。

### 6.1 先做低风险直接依赖收敛

候选：

- `base64 0.21/0.22`
- `dirs 5/6`
- `toml 0.8/0.9`

执行原则：

- 只处理本仓库直接依赖。
- 不为了收敛版本强行升级外部库。
- 每次只收敛一类库。

示例检查：

```powershell
cargo tree -d -i base64
cargo tree -d -i dirs
cargo tree -d -i toml
```

验证：

```powershell
cargo check --workspace
cargo test -p <changed-crate>
```

### 6.2 高风险重复依赖暂不优先强收敛

候选：

- `image 0.24/0.25`
- `rmcp 0.12/1.5`
- `reqwest 0.12/0.13`
- `windows*`

原因：

- 这些通常来自传递依赖或大版本 API 变化。
- 贸然统一可能比保留重复版本风险更高。

处理方式：

- 优先通过 crate 边界隔离它们的编译范围。
- 等 owner crate 独立后，再在对应 crate 内评估升级。

---

## 7. 边界强制规则

在至少两个 crate 被抽出后，增加轻量检查脚本，而不是一开始就把工具链复杂化。

**建议新增：** `scripts/check-core-boundaries.mjs`

检查规则：

- `bitfun-core-types` 不允许依赖：
  - `bitfun-core`
  - service crate
  - agent runtime
  - Tauri
  - `reqwest`
  - `git2`
  - `rmcp`
  - `image`
  - `tokio-tungstenite`
- service crate 不允许依赖 `bitfun-core`。
- agent runtime 不允许依赖 concrete heavy service crate，只依赖 ports。
- tool framework 不允许依赖 concrete service implementation。
- product crate 可以依赖 facade 或明确 concrete crate。

运行：

```powershell
node scripts/check-core-boundaries.mjs
```

注意：

- 不要在大型移动 PR 中同时新增复杂检查。
- 检查脚本应简单扫描 Cargo.toml 和 `src/**/*.rs` 的 forbidden imports。

---

## 8. 验证矩阵

### 8.1 每个 PR 的最小验证

```powershell
cargo check -p <new-or-modified-crate>
cargo test -p <new-or-modified-crate>
cargo check -p bitfun-core --features product-full
```

### 8.2 产品矩阵

```powershell
cargo check -p bitfun-desktop
cargo check -p bitfun-cli
cargo check -p bitfun-server
cargo check -p bitfun-relay-server
cargo check -p bitfun-acp
cargo check --workspace
```

### 8.3 default feature 变更前的完整门禁

```powershell
cargo test --workspace
cargo build -p bitfun-desktop
pnpm run desktop:build:fast
pnpm run desktop:build:release-fast
```

### 8.4 构建脚本保护

```powershell
git diff -- package.json scripts/dev.cjs scripts/desktop-tauri-build.mjs scripts/ensure-openssl-windows.mjs scripts/ci/setup-openssl-windows.ps1 BitFun-Installer
```

期望：

- 没有脚本或 installer diff。
- 如果出现 diff，该 PR 不应作为 core 拆解 PR 合并。

---

## 9. 风险登记表

| 风险 | 概率 | 影响 | 缓解方式 |
|---|---:|---:|---|
| 产品 feature set 被意外改变 | 中 | 高 | `product-full` 先行；产品 crate 显式启用；产品矩阵验证 |
| 新 crate 依赖回 `bitfun-core` | 高 | 高 | boundary script；code review；`core-types` 先行 |
| service-agentic 循环阻塞拆分 | 高 | 高 | 先引入 ports，再移动 crate |
| port DTO 仍依赖非结构化 metadata | 中 | 中 | `turnId` 已显式化；后续新增跨边界字段继续优先进入 DTO，metadata fallback 只作为兼容期 |
| tool registry / manifest 行为变化 | 中 | 高 | 完整工具清单、expanded/collapsed manifest、`GetToolSpec` 与 provider 等价性测试 |
| 动态工具 provider 身份耦合注册名 | 中 | 中 | MCP wrapper / registry entry 已显式携带 provider metadata；后续 provider owner 迁移继续禁止从 `mcp__...` 名称反推身份 |
| remote SSH 行为变化 | 中 | 高 | workspace identity DTO 稳定后再拆；保留 `ssh-remote` 语义 |
| MCP 动态工具丢失 | 中 | 高 | `DynamicToolProvider` contract；MCP regression test |
| desktop 构建脚本被误改 | 低 | 高 | 每 PR 执行 build script guard |
| facade 阶段编译速度收益不明显 | 中 | 中 | 预期中间态；衡量小 crate 测试收益，不把 facade 视为终点 |
| 抽象过度导致开发复杂度上升 | 中 | 中 | port 粒度小；禁止万能 `CoreContext` |
| crate 拆得过碎导致链接和调度成本上升 | 中 | 中 | 采用中等粒度目标；默认只拆 8 到 12 个 owner crate；后续拆小必须有实测依据 |

---

## 10. 三个关键里程碑

后续执行按里程碑推进，而不是按单个技术点零散推进。每个里程碑都必须独立可验收，并且不改变产品功能集合。

### 执行优先级

优先级从高到低：

1. **P0：安全边界。** 文档、feature 安全网、构建脚本保护、产品能力不变。
2. **P1：最小编译面验证。** `core-types`、`agent-stream`、runtime ports，优先验证小 crate 测试是否能绕开完整 core。
3. **P2：中等粒度 owner crate。** `services-core`、`services-integrations`、`agent-tools`、`tool-packs`、`product-domains`。
4. **P3：facade 收敛与边界强制。** `bitfun-core` 只做兼容门面和 product runtime assembly。
5. **P4：冗余清理。** 只处理绝对等价重复，且必须独立 PR。P4 不阻塞任何里程碑。

不允许跳过 P0/P1 直接进入重 service 拆分。任何 P2/P3 任务如果需要改变产品功能集合、默认 feature、构建脚本或平台边界，必须回退到 P0/P1 重新补安全网。

### 里程碑一：边界安全网与最小收益验证

**覆盖计划：**

- Plan 0：基线与安全护栏。
- Plan 1：`product-full` feature 安全网。
- Plan 2：移动 nested `terminal-core` 和 `tool-runtime`。
- Plan 3：抽取 `bitfun-core-types`。
- Plan 4：抽取 `bitfun-agent-stream`。
- Plan 5：引入 runtime ports。

**目标：**

- 建立后续拆分不会偏移产品能力的 feature 安全网。
- 建立底层共享类型和 port 基础，避免后续循环依赖。
- 通过 `agent-stream` 先验证“小 crate 承载局部测试”是否能减少编译面。
- 不移动重 service，不调整产品构建脚本，不改变 release/CI 行为。

**启动队列：**

1. 文档和基线护栏：只记录边界、验证命令、禁止项，不移动代码。
2. `product-full` feature：保持 default 行为不变，让产品 crate 显式启用完整能力。
3. nested crate 位置整理：移动已经独立的 `terminal-core` 和 `tool-runtime`，保持 package/lib 名称不变。
4. `core-types`：只抽错误和纯 DTO，不引入运行时依赖。
5. `agent-stream`：迁移 stream processor 和 stream 测试，验证小 crate 测试收益。
6. runtime ports：新增轻量 ports crate 和第一批 adapter，建立后续替换跨层 concrete 调用的入口，不移动重 service。

**实现边界：**

- 可以新增 `core-types`、`agent-stream`、workspace 顶层 `terminal`、`tool-runtime`。
- 可以新增 port trait 和 DTO。
- 可以在 core 中添加兼容 re-export。
- 不允许改变 `bitfun-core default` 为轻量模式。
- 不允许修改 `package.json`、`scripts/*`、`BitFun-Installer/**`。
- 不允许把 desktop/server/CLI 的平台逻辑下沉到 shared crate。

**验收门：**

```powershell
cargo check -p bitfun-core --features product-full
cargo test -p bitfun-runtime-ports
cargo test -p bitfun-agent-stream
cargo check -p bitfun-desktop
cargo check -p bitfun-cli
cargo check -p bitfun-server
git diff -- package.json scripts/dev.cjs scripts/desktop-tauri-build.mjs scripts/ensure-openssl-windows.mjs scripts/ci/setup-openssl-windows.ps1 BitFun-Installer
```

期望：

- 产品 crate 仍显式拥有完整能力。
- `agent-stream` 测试不需要依赖完整 `bitfun-core`。
- 旧公开 import 路径可用。
- 构建脚本无 diff。

**当前验收状态：**

- P1 已完成：`product-full` 默认能力保护、workspace 顶层 crate 移动、`core-types` 第一批类型、`agent-stream` 独立测试、runtime ports 初始边界和旧路径 compatibility wrapper 均已建立。
- 已补 P2 前置 contract hardening：`AgentSubmissionRequest.source` / `turnId` 显式化，coordinator 保留 metadata fallback；dynamic tool provider 身份改为显式 provider metadata，不再从 MCP 注册名反推。
- 已通过的验证类型包括 runtime-ports / agent-stream / core focused tests、`cargo check --workspace`、`cargo test --workspace`、Web UI lint/type-check/test 和构建脚本保护范围检查；后续小范围文档修正不要求重复全量 Web 验证。
- 不属于 P1 完成范围：remote-connect / cron / MCP concrete call-site 迁移、`AgentSubmissionPort` attachment / image context 接入，以及任何产品逻辑或边界行为变更。

**暂停条件：**

- `core-types` 需要引入运行时依赖才能通过编译。
- port 设计开始变成万能 context。
- `agent-stream` 无法脱离完整 core，说明应重新评估 stream 边界。
- 任何任务需要顺手清理非绝对等价重复代码。

### 里程碑二：中等粒度 owner crate 成型

**覆盖计划：**

- Plan 6：抽取 `bitfun-services-core` 和 `bitfun-services-integrations`。
- Plan 7：拆解 `bitfun-agent-tools` 和 `bitfun-tool-packs`。
- Plan 8：抽取 `bitfun-product-domains`。
- 低风险直接依赖版本收敛只允许作为独立小 PR 插入。

**目标：**

- 将当前 core 中最重的 service、tool、product domain 职责迁移到中等粒度 owner crate。
- 用 feature group 隔离重依赖，而不是拆成大量小 crate。
- 让局部 service/tool/domain 测试可以绕开完整 product runtime。
- 保持产品完整 runtime 通过 `product-full` 组装同等能力。
- 在重 service/tool 迁移前先收紧 P1 暴露出的 port/tool contract：显式 `turnId`、显式 dynamic tool provider metadata、以及迁移路径的回归测试入口。

**主要工作：**

- `bitfun-services-core`：先迁移 config、session、workspace、storage、filesystem、system、session_usage、token_usage 等基础服务，保持旧 core 路径 re-export。
- `bitfun-services-integrations`：按 git、remote-ssh、MCP、remote-connect 顺序迁移重集成；每迁移一个 feature group 都保留产品完整 runtime 等价性。
- `bitfun-agent-tools` / `bitfun-tool-packs`：拆出 tool trait、context、registry、provider contract；`tool-packs` 先承载 feature-group 元数据和 provider plan，具体工具实现仅作为后续按 feature group 外移的目标。
- `bitfun-product-domains`：承接 miniapp 和 function-agent 产品子域，避免继续扩大 `bitfun-core` 的产品职责。

**影响面：**

- Rust crate graph、workspace manifests、core compatibility re-export、feature group 组装。
- `src/crates/core/src/service/**`、`agentic/tools/**`、MCP / remote SSH / remote connect / git integration。
- Desktop、CLI、server 通过 `product-full` 组装的完整能力验证。

**优先风险：**

- service/tool 迁移改变产品 feature set 或默认能力。
- 新 owner crate 反向依赖 `bitfun-core`，导致 facade 计划失效。
- remote connect / cron / MCP 接入 ports 时丢失 `turnId`、attachment、subagent、cancellation 或 transcript 关联语义。
- MCP 动态工具 provider metadata 在 registry/tool owner 迁移中断裂。
- 工具清单、expanded/collapsed manifest、`GetToolSpec` unlock state、snapshot wrapping、permission / concurrency safety 行为与迁移前不等价。

**实现边界：**

- service 侧只拆成 `services-core` 和 `services-integrations`，继续拆小必须有实测依据。
- tool 侧只拆成 `agent-tools` 和 `tool-packs`，具体工具族通过 feature group 控制。
- miniapp 和 function agents 先合并到 `product-domains`，不分别建独立 crate。
- 每次只迁移一个 feature group 或一个模块簇。
- 不允许在同一 PR 中做三方库大版本升级。
- 不允许改变产品默认能力、CI 覆盖或 release 脚本。

**验收门：**

```powershell
cargo test -p bitfun-services-core
cargo check -p bitfun-services-integrations --features product-full
cargo test -p bitfun-agent-tools
cargo check -p bitfun-tool-packs --features product-full
cargo check -p bitfun-product-domains --features product-full
cargo check -p bitfun-core --features product-full
cargo check -p bitfun-desktop
cargo check -p bitfun-cli
cargo check -p bitfun-server
cargo check --workspace
```

期望：

- 新 owner crate 不依赖回 `bitfun-core`。
- 产品完整 runtime 的工具、MCP、remote SSH、remote connect、miniapp、function agents 仍可用。
- 新增 crate 数量仍保持中等粒度。
- heavy dependency 所属 crate 清晰。

**当前 P2 执行状态：**

- 中等粒度 owner crate 已成型：`bitfun-services-core`、`bitfun-services-integrations`、`bitfun-agent-tools`、`bitfun-tool-packs`、`bitfun-product-domains` 均已加入 workspace，并通过 core facade 保持旧路径兼容。
- 已完成的安全迁移包括：Git feature group、remote-SSH 纯 identity/path helper、MCP runtime/dynamic provider、remote-connect wire/tracker/file/image/dialog helper、generic tool registry / provider / catalog / GetToolSpec helper、product provider plan、MiniApp / function-agent 纯 domain helper 与 port/facade。
- 已补轻量 contract crate 与 feature graph 保护：`core-types`、`runtime-ports`、`agent-tools`、`product-domains`、`services-integrations` 的默认 profile 和禁止依赖由 boundary check 覆盖；`ToolImageAttachment`、dynamic provider metadata、runtime restrictions、path resolution、tool registry snapshot 等作为迁移前基线保留。
- 不声明完成的部分：remote-SSH runtime、workspace-root source、persistence/workspace service reads、`ImageContextData` concrete impl、`ToolUseContext`、runtime manifest / `GetToolSpecTool` execution、collapsed unlock state、concrete tools、MiniApp filesystem / worker / host dispatch / builtin asset runtime、function-agent Git/AI concrete service、agent registry / scheduler。
- 结论：P2 已完成低风险 owner container 化和行为基线，不再拆成小 PR；剩余项必须按高风险 runtime owner 主题进入 HR 队列。

P2 后产品表面契约轨道（contract-only）：

- 背景：最新 CLI TUI、Desktop、Remote、Server 和 ACP 都是 first-class product surface。后续重构不应把它们
  拉平成同一套命令实现，而应共享 runtime capability facts。
- 原则：**surface divergence, capability convergence**。命令、快捷键、pane/card/TUI rendering 属于 surface
  presentation；session/thread identity、environment identity、permission facts、artifact refs、event facts 和
  capability request/response 属于可共享 contract。
- 候选 contract：`SurfaceKind`、`ThreadEnvironment`、`RuntimeArtifactKind`、`RuntimeArtifactRef`、
  `PermissionDecision`、`PermissionScope`、`ApprovalSource`、`CapabilityRequest`。纯 DTO 优先放入
  `bitfun-core-types`；必要 port trait 放入 `bitfun-runtime-ports`。
- 明确不做：不改 CLI slash command / TUI、不改 Desktop command palette 或 pane 行为、不新增 command engine crate、
  不调整 `product-full`、不做 per-product feature set，也不把 `ratatui`、`crossterm`、Tauri 或 Web UI 依赖带入
  contract crate。
- 进入方式：该轨道可作为 PR3 前的 contract-only 前置提交或 PR3 的第一组无行为变更提交；一旦需要改变 UI、
  命令语义、权限策略或运行时调用路径，必须拆成单独产品变更 PR 并先确认。
- 验证：DTO/port 只做 serialization round-trip、conversion/no-op check 与 boundary check；不能只凭
  `cargo check` 声明产品行为等价。

需要单独审视的高风险项：

- `ToolUseContext`、runtime manifest assembly / `GetToolSpec` 执行、product tool provider assembly、concrete tool implementation 外移。
- MCP concrete tool implementation / product registry / manifest assembly 外移。
- remote-connect、remote-SSH runtime、announcement runtime 外移。
- miniapp runtime/compiler/builtin 与 function-agent 运行逻辑外移。
- agent registry / subagent visibility 外移，特别是 hidden/custom/review 分组、mode-scoped visibility 和 desktop API contract。
- `bitfun-core default = []`、per-product feature set、构建脚本或 release 能力调整。

这些高风险项的进入条件：

- 先有 port/provider 设计，且不依赖回 `bitfun-core`。
- 先有迁移前后等价测试或脚本快照，不能只依赖 `cargo check`。
- 保留旧公开路径兼容，或者明确记录需要用户确认的行为合约变化。
- 产品完整 runtime 通过 `product-full` 保持同等能力；任一产品需要减少 feature 才能通过时必须暂停。
- 每个 PR 只移动一个 runtime owner 或一个 feature group，不和默认 feature、构建脚本、依赖升级混合。

**暂停条件：**

- 某个迁移必须让产品 crate 减少 feature 才能通过。
- `services-integrations` 的 feature group 互相强耦合，无法单独 check。
- product registry / manifest assembly 或 concrete tool implementation 迁移后工具清单、expanded/collapsed exposure、`GetToolSpec` unlock state 无法证明等价。
- 新 owner crate 反向依赖 core。

**已完成内容摘要：**

- P1 / P2 的低风险拆分已闭环：workspace 顶层 crate、`core-types`、`agent-stream`、runtime ports、`services-core`、`services-integrations`、`agent-tools`、`tool-packs`、`product-domains` 均已有明确 owner 边界和旧路径兼容。
- 低风险准备项已合并为 LR1 并完成：错误边界、shared DTO 归属、悬空 port/call-site、feature/dependency baseline、boundary check 与文档/AGENTS 校准不再作为独立小 PR 重复提交。
- H1-H4 的安全收口已完成：tool runtime 的 provider-neutral contract、product-domain pure/port facade、service/agent remote-connect contract、core 内部 `product_runtime.rs` / `product_domain_runtime.rs` / `service_agent_runtime.rs` owner 收口，以及 facade / boundary finalization 均已有当前状态描述。
- H5 已完成当前基线：`bitfun-core --no-default-features` 可编译面、`product-full` 显式 owner feature 聚合、optional dependency owner 映射和产品入口显式装配检查已建立；这只是 feature/dependency baseline，不代表完整构建收益或 runtime owner 深迁移完成。

**当前后续执行队列：**

| 队列项 | 范围 | 不允许混入 | 合入门禁 |
|---|---|---|---|
| HR-A deep optional | 仅在决定继续深迁 tool runtime 时，评审 `ToolUseContext`、concrete tool IO、manifest execution、snapshot wrapper、collapsed unlock state 等单一 owner | product-domain runtime、service/agent runtime、feature matrix、产品行为变更 | tool registry / manifest / `GetToolSpec` / snapshot / Deep Review tool flow 等价，且有 port/provider 设计 |
| HR-B deep optional | MiniApp filesystem IO / worker / host / builtin seed 或 function-agent Git/AI 中一个 owner 主题 | tool runtime、service/agent runtime、surface 行为变更 | MiniApp/function-agent focused regression、PathManager/process/Git/AI 边界清晰 |
| service/agent deep optional | 仅在决定继续深迁 HR-C 之外的 concrete runtime 时，在 remote-SSH / remote FS / terminal、workspace-root / persistence、`ImageContextData`、agent registry / scheduler 中选择一个 owner 主题 | tool runtime、product-domain runtime、feature matrix、产品逻辑变更 | remote/session/subagent/citation/goal/request-context 行为等价，旧路径兼容 |
| H5 optional | feature matrix、dependency profile、构建收益数据评估 | runtime owner 迁移、default feature 副作用、构建脚本变更 | feature graph / cargo metadata 证据，产品入口完整能力不变 |

执行口径：

- 后续不再新增低风险碎片 PR；helper、guard、facade cleanup 只能作为同一 owner 迁移的预保护或收尾。
- `bitfun-core default = []`、per-product feature set、构建矩阵和 release 能力调整仍属于 H5 独立评估，不得与 HR-A / HR-B 或 service/agent deep optional 混合。
- 最新主干新增的 Write sanitizer、AskUserQuestion/TodoWrite truncation recovery、per-tool denial message、tool-result flush、`fork_context`、prompt cache clone、existing-context dialog turn、MCP initialize timeout scope、pending waiter drain、CLI package workflow 和 mobile-web session 操作，必须按触碰 owner 纳入等价测试或产品矩阵影响面。

**HR 风险与优化清单：**

所有 HR PR 都必须满足以下共同约束：

- 功能影响范围：只能移动 owner 或引入 port/provider adapter；不得改变用户可见命令、
  默认权限、remote/session 生命周期、tool 可见性、MiniApp/function-agent 输出、CLI/Desktop/ACP/Server
  交互语义。
- 产品发布形态：不得修改 `bitfun-core` default feature、产品 crate feature set、
  `package.json`、desktop/installer build scripts、release/fast build 脚本或 CI 覆盖范围。
  若某项迁移必须改变这些内容，必须从 HR PR 中拆出并先单独评审。
- 性能门禁：不得新增无界全局锁、阻塞 IO、重复 registry rebuild、重复 manifest
  materialization、额外 network/process startup 或跨 crate 的重依赖反向引入。PR 如果声明
  build/check 收益，必须记录迁移前后数据；如果不声明收益，也至少不能让 workspace
  check/test 或关键产品 check 明显劣化。
- 依赖边界：owner crate 不得依赖回 `bitfun-core`；contract crate 不得吸收
  Tauri、CLI/TUI presentation、network client、process execution、`git2`、`rmcp`、
  `image`、`tokio-tungstenite` 等 concrete runtime 依赖。
- 回滚边界：每个 HR PR 必须保留旧路径 facade 或 adapter，使失败时可以把新 owner
  路径回退到 core-owned adapter，而不需要同步修改产品 surface。

HR1：tool runtime deep owner migration 的主要风险和控制点：

- 当前已完成的低侵入部分：`product_runtime.rs` 统一承接 product provider plan
  materialization、product registry snapshot/catalog facade、manifest / GetToolSpec facade 和
  snapshot wrapper 注入；这只是 core 内部 owner closure，不改变工具执行路径。
- 风险：`ToolUseContext` 携带 workspace services、cancellation、computer-use host、
  custom data、Deep Review checkpoint hook 与 collapsed unlock state；若直接移动，
  可能改变工具可见性、权限、snapshot wrapper、Deep Review tool flow 或取消语义。
- 风险：manifest / `GetToolSpec` / catalog 组装若重复计算，可能增加每轮 agent
  prompt 构建成本；若 dynamic metadata 顺序或去重语义漂移，可能改变模型看到的工具集合。
- 可优化点：只先抽 `ToolUseContext` 的 capability/read-only projection 或小型
  service port；concrete tools 仍按 feature group 分批评审，避免一次性迁移全部 IO 工具。
- 可优化点：把 manifest/catalog 快照缓存边界显式化，避免迁移后每次 prompt
  resolution 都重建 registry；保留现有 provider order 和 dynamic provider metadata order。
- 必须新增或复用的保护：builtin tool list、provider group order、readonly/enabled
  filtering、expanded/collapsed exposure、`GetToolSpec` duplicate-load/unlock state、
  snapshot wrapper、runtime restriction、cancellation 和 Deep Review tool flow regression。
- 产品形态门禁：Desktop MCP catalog、ACP catalog、CLI agent tool surface、Deep Review
  tool flow 必须继续使用同一行为矩阵；不得为了 tool owner 外移改变任何 surface command。

HR2：product-domain runtime deep owner migration 的主要风险和控制点：

- 当前 HR2 结论：本轮只完成 core 内部 owner closure。`CoreProductDomainRuntime`
  集中创建 MiniApp runtime-state facade、function-agent Git/AI adapters 和
  function-agent runtime facade；这让后续迁移审查有唯一入口，但不改变 MiniApp
  filesystem IO、worker process、host dispatch、built-in asset seed / marker IO /
  recompile，或 function-agent Git/AI service call 的 owner。
- 风险：MiniApp filesystem IO、worker process、host dispatch、builtin asset seed /
  marker IO / recompile 都有外部副作用；迁移不当会改变用户数据目录、更新标记、
  rollback、dependency state 或编译/运行顺序。
- 风险：function-agent Git/AI 调用涉及 provider acquisition、transport error mapping、
  prompt 输入、JSON extraction/repair 和 `analyzed_at` 时序；移动过深可能改变 commit
  message、Startchat work-state 或非 Git workspace fallback。
- 可优化点：优先抽 storage/process/Git/AI 的最小 port contract，让
  `product-domains` 拥有纯 orchestration，core 继续注入 PathManager、process runner、
  Git/AI adapter 和 asset source。
- 可优化点：把 MiniApp import/sync/recompile/rollback/deps state 的快照基线作为
  迁移前后对比入口；对 function-agent 保留 no-HEAD diff fallback、非 Git 空状态、
  `analyze_git=false` time-info 和 post-analysis `analyzed_at` 赋值语义。
- 必须新增或复用的保护：MiniApp import/sync/recompile/rollback/deps state focused
  tests、builtin seed marker round-trip、customized update metadata、function-agent
  prompt/response policy、Git/AI adapter error mapping 和 Startchat work-state regression。
- 产品形态门禁：Desktop MiniApp、server/remote workspace、CLI function-agent 路径和
  packaged built-in MiniApp asset 必须继续组装；不得改变 installer、desktop release 或
  user-data seed 产物。

HR3：service / agent runtime deep owner migration 的主要风险和控制点：

- 当前 HR3 结论：本轮只完成 core 内部 owner closure。`CoreServiceAgentRuntime`
  集中创建 remote dialog/cancel/file/tracker host、remote model catalog/session-model
  selection adapter、remote image context adapter 和
  `ConversationCoordinator` 的 runtime-port binding；这让后续 service/agent
  runtime 深迁移有唯一审查入口，但不改变 remote-connect / remote-SSH /
  scheduler / registry 的实际执行路径。
- 当前 service/agent 深迁移新增完成项：remote chat history 的 message ordering、
  tool preview、assistant presentation assembly 已迁入 `bitfun-services-integrations`。
  core 只保留 persisted `DialogTurnData` 读取、owner DTO 投影、mobile image compression
  和 enhanced remote user input cleanup；remote-SSH、terminal pre-warm、scheduler submit、
  workspace-root source 与 session/workspace persistence read 没有迁移或行为调整。
- 当前 service/agent 深迁移新增完成项：remote session/model selection 的 alias、
  default/auto、primary/fast 与 config-reference 归一策略已迁入 `bitfun-services-integrations`。
  core 继续负责读取 `AIConfig` 并注入 `resolve_model_reference`，保留缺失 config service
  的既有错误边界。
- 当前 service/agent 深迁移新增完成项：remote model catalog 的 config-shape assembly
  已迁入 `bitfun-services-integrations`。core 只投影 model capability / reasoning mode
  等 owner facts，并继续持有 config service 读取与 session model resolver 注入；catalog
  version 裁剪、capability/reasoning wire 字段和默认模型/session 模型字段装配由 owner crate
  的 contract test 保护。
- 当前 service/agent 深迁移新增完成项：dialog scheduler outcome assembly 已迁入
  `bitfun-services-integrations`。core 只投影 concrete `DialogSubmitOutcome` 为 owner fact，
  scheduler submit、queue policy 映射和 terminal pre-warm 仍留在 core adapter，避免改变
  restore -> prewarm -> submit 的执行顺序。
- 当前 HR-C 结论：portable contract closure 已完成，新增迁移范围限于
  runtime-ports 可承载的 DTO/trait/fact；concrete remote/runtime 执行路径仍按上方 HR3
  风险门禁作为后续可选深迁移主题处理。
- 当前 service/agent 深迁移新增完成项：scheduler submit queue routing、
  interactive preempt 与 agent-session cancel-reply suppression 的纯决策已迁入
  `bitfun-runtime-ports`。core 继续持有 `DialogScheduler` 生命周期、queue storage、
  active-turn map、round-yield flags、outcome loop、coordinator submit 和 agent-session reply
  delivery；这一步不改变 queue depth、preempt、cancel suppression 或 background reply
  行为。
- 风险：remote-SSH manager / remote FS / terminal 与 remote-connect workspace-root
  source、persistence/workspace service reads、`ImageContextData` concrete impl 都连接实际远端执行环境；
  迁移不当会破坏 remote workspace guard、terminal pre-warm、response shape、
  image fallback 或 file chunk range 行为。
- 风险：agent registry / scheduler 现在承载 mode-scoped subagent visibility、
  `Multitask` / `GeneralPurpose` registration、background result delivery、running-turn
  injection 和 idle-session follow-up；迁移不当会改变 subagent 可见性、排队、确认边界
  或 DeepResearch post-turn hook。
- 可优化点：先把 scheduler/registry 的 observable facts、queue policy decision、
  runtime event fact 与 remote workspace identity 抽成只读 contract；concrete
  execution、session restore、terminal binding、workspace-root source 和 persistence/workspace service reads
  继续由 core adapter 注入。
- 可优化点：对 remote-connect 保持 owner crate 只管 orchestration policy，
  core 继续拥有 workspace-root source、persistence/workspace service reads 和 concrete scheduler submit，
  直到有端到端 remote product regression。
- 可优化点：继续沿用本轮模式拆分“presentation assembly”和“runtime execution”：前者可在
  owner crate 中用 DTO + focused contract test 保护，后者必须等到 remote product regression、
  port/provider 设计和回滚路径齐备后再移动。
- 必须新增或复用的保护：remote command/response wire、restore -> terminal pre-warm ->
  scheduler submit 顺序、file full/chunk/info、image context fallback/preference、
  mode-scoped subagent availability、background delivery、DeepResearch citation
  renumber hook、queue/confirmation boundary 和 remote workspace startup guard regression。
- 产品形态门禁：Desktop remote connect、relay/bot、server, ACP remote config reuse、
  CLI subagent management 和 Review Team 可见性必须继续按当前产品差异运行；不得为了
  service/agent owner 外移统一 surface presentation 或命令语义。

剩余数量口径已经收敛到上方队列：低风险准备为 0 个新增小 PR，后续只按完整高风险 owner 主题推进；缺陷修复、行为变更、冗余清理和构建脚本调整必须独立评估，不能伪装成 core decomposition 剩余里程碑。

### 里程碑三：facade 收敛、边界强制与可选默认轻量化评估

**覆盖计划：**

- Plan 9：`bitfun-core` 收敛为 facade + product runtime assembly。
- 边界检查脚本。
- 依赖版本收敛复查。
- 可选评估 `bitfun-core default = []`，但仅在完整门禁通过后单独执行。

**目标：**

- `bitfun-core` 不再承载新实现，只负责旧路径兼容和完整产品 runtime 组装。
- 用边界检查防止新 crate 重新依赖回 core。
- 评估是否值得让 `bitfun-core` default 变轻，但不把它作为默认结论。
- 保证整体性能没有明显负向影响。

**实现边界：**

- 可以把旧模块改为 re-export。
- 可以新增 boundary check 脚本。
- 可以做低风险直接依赖版本收敛。
- `default = []` 必须是单独 PR，且只在所有产品 crate 显式启用完整 runtime 后评估。
- 不允许把 facade 变成新的业务实现聚合。

**P3 进入条件与主干补充：**

- P3 只能在 P2/HR 剩余 runtime owner 已完成迁移，或被显式标为 core-owned / deferred 后启动；不得把 `ToolUseContext`、concrete tools、remote-connect / remote-SSH、MiniApp IO / worker / host / builtin、function-agent Git/AI、agent registry / scheduler 通过 re-export 伪装成已迁移。
- 最新主干新增的 Deep Review context/cost/queue、agent-stream tool-call dedupe、search fallback、session rollback、remote workspace guard、ACP/Web fallback、mode-scoped subagent visibility、background subagent delivery、DeepResearch citation renumber、tool spec discovery、usage/cache token、Responses flat schema、CLI mode-aware subagent management、desktop lifecycle 与内置 MiniApp seed/update 行为，均必须按触碰 owner 纳入等价测试或明确保留在 product surface。
- 产品表面仍采用 “surface divergence, capability convergence”：CLI / Desktop / Remote / ACP / Server 可以共享 capability facts 或 ports，但不能为了复用下沉 surface command、UI rendering、timeout policy、presentation 或 app-layer dependency。
- P3 闭环检查必须同时覆盖 crate graph 与产品 runtime 行为；boundary check 只证明依赖方向，不能替代 Deep Review、MCP dynamic tools、tool manifest / `GetToolSpec`、remote connect、snapshot wrapping、MiniApp/function-agent 的产品等价验证。

**阶段复核摘要：**

- 已完成的语义 baseline 覆盖 MCP config failure、catalog replacement invalidation、dynamic manifest、tool manifest / `GetToolSpec`、MiniApp storage layout adapter、product-domain pure helper 与 core adapter 等价、remote workspace search fallback，以及 core-types / runtime-ports / agent-tools 的轻量边界。
- boundary check 已锁定已外移 owner 的旧路径 facade-only / 禁止回流状态，并禁止 contract crate 吸收重 runtime、platform adapter、CLI/TUI presentation 或 concrete service 依赖。
- 后续迁移必须 owner-by-owner 推进：先补 port/provider 设计和等价测试，再移动 runtime owner，最后证明旧路径兼容；`bitfun-core default = []`、per-product feature set、依赖版本收敛和构建收益仍是独立评估项，不与 runtime 外移混合。

**验收门：**

```powershell
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
cargo check -p bitfun-cli
cargo test --workspace
cargo build -p bitfun-desktop
pnpm run build:mobile-web
pnpm run desktop:build:fast
pnpm run desktop:build:release-fast
git diff -- package.json scripts/dev.cjs scripts/desktop-tauri-build.mjs scripts/ensure-openssl-windows.mjs scripts/ci/setup-openssl-windows.ps1 BitFun-Installer
```

期望：

- `bitfun-core` 旧路径兼容。
- 边界检查通过。
- 完整 workspace 测试和 desktop build 通过。
- 构建脚本无 diff。
- 若性能收益不明显，也不能有明显退化；必要时保留中等粒度边界，不继续拆小。

**暂停条件：**

- 完整产品矩阵无法通过。
- default feature 轻量化会改变任一产品能力。
- boundary check 发现 extracted crate 依赖回 core。
- 构建或链接时间因 crate 过碎出现明显退化且无法通过合并修正。

---

## 11. 推荐 PR 顺序

**已完成主线：**

- 基础拆分已完成：文档护栏、`product-full` 安全网、workspace crate 移动、`core-types`、`agent-stream`、runtime ports、`services-core`、`services-integrations`、`agent-tools`、`tool-packs` 与 `product-domains` 的低风险 owner 边界。
- runtime 前置保护已完成：MCP dynamic runtime、remote-connect tracker / wire / pure policy、product-domain pure/facade、tool registry / manifest / GetToolSpec baseline、feature/dependency profile、boundary check 与关键语义回归基线。
- 当前 core-owned runtime 收口已完成：tool、product-domain、service/agent 三类 runtime 均已有 core 内部单一 owner 入口，并明确列出仍保留在 core 的 concrete IO、scheduler、workspace-root、persistence、MiniApp worker / host / builtin、function-agent Git/AI 等高风险路径。
- H5 当前只完成 feature/dependency baseline：默认完整产品能力、产品入口显式 `product-full` 装配和 no-default 编译面受保护；不声明 per-product feature matrix、构建收益或 runtime owner 深迁移完成。

**后续推荐顺序：**

| 顺序 | 主题 | 完整范围 | 关键门禁 |
|---|---|---|---|
| 1 | service / agent runtime 深迁移（HR-C 之后可选） | 在 remote-SSH、remote FS / terminal、workspace-root / persistence、`ImageContextData`、agent registry / scheduler 中选择一个 owner 主题，完成端口、adapter、旧路径兼容和行为等价验证 | remote/session/subagent/citation/goal/request-context 等价，产品 surface 不变 |
| 2 | HR-B product-domain runtime 深迁移 | 在 MiniApp filesystem IO / worker / host / builtin seed 或 function-agent Git/AI 中选择一个 owner 主题，不混入 tool/service runtime | MiniApp/function-agent focused regression，PathManager/process/Git/AI 边界清晰 |
| 3 | HR-A tool runtime 深迁移（可选） | 仅在收益明确时评审 `ToolUseContext`、concrete tool IO、manifest execution、snapshot wrapper 或 collapsed unlock state | tool visibility、manifest、GetToolSpec、snapshot、Deep Review tool flow 等价 |
| 4 | H5 feature / build-benefit evaluation（可选） | 只评估 feature matrix、dependency profile 和构建收益数据 | 不迁移 runtime owner，不改产品 feature set、default feature 或构建脚本 |

执行要求：每次 PR 必须按一个完整 owner 主题推进；预保护、迁移、旧路径兼容、文档更新和对抗性审核属于同一个 PR 的组成部分，不再拆成独立小 PR。

### 11A. 后续高风险 PR 队列

后续不再新增低风险碎片 PR。每个 PR 必须按一个完整 owner 主题提交，先设计保护网，
再移动 runtime owner，最后用对抗性审核确认没有功能偏移。

#### HR-A：Tool Runtime Owner Migration

目标：在不改变工具可见性、manifest、`GetToolSpec`、collapsed unlock、snapshot wrapper、
Deep Review tool flow 或具体工具 IO 的前提下，继续收敛 tool runtime owner。

当前 HR-A 基线：

- `bitfun-agent-tools` 新增 provider-neutral file guidance marker、file-read freshness facts /
  comparison policy、oversized tool-result storage policy / preview / rendered replacement
  contract、tool execution result/error/invalid-call presentation policy，并用 owner-crate contract tests
  锁定这些纯规则。
- core `file_tool_guidance` 变为兼容 re-export；`file_read_state_runtime` 继续持有
  session/coordinator/read-state storage 与文件 re-read IO，但 freshness 比较委托给
  `agent-tools`；`tool_result_storage` 继续持有 session runtime artifact path、filesystem
  write、assistant-only replacement 接线与 `BitFunError` 映射，但 preview/rendering/round
  budget selection 委托给 `agent-tools`。
- `ToolUseContext` 本体、portable facts projection、workspace service accessor、runtime artifact
  lookup、path policy enforcement、pipeline/description/preflight context materialization、
  cancellation/post-call hook wrapper、unified `Tool::call` runtime hook facade 与
  Deep Review light checkpoint 绑定已集中到 core `tool_context_runtime.rs`；`framework.rs`
  只保留 `Tool` trait 与旧路径兼容 re-export，core runtime/adapter 模块直接引用
  `tool_context_runtime::ToolUseContext`。
- 未外移出 core：workspace services、cancellation token、Read/Edit/Write
  concrete IO、tool-result filesystem persistence、dynamic MCP concrete execution、snapshot
  wrapper 与 collapsed unlock state。它们仍需单独 port/provider 设计和更强等价测试。

预保护：

- 固化 product registry snapshot、expanded/collapsed exposure、prompt-visible manifest、
  `GetToolSpec` summary/detail/result、dynamic provider metadata、snapshot wrapper 覆盖顺序。
- 覆盖 desktop/MCP/ACP catalog 等价、Deep Review 修改类工具 checkpoint hook、
  cancellation/post-call hook、runtime artifact URI 和 remote workspace path policy。
- 新增覆盖 Read/Edit/Write 的 session-scoped read state、stale-write guardrail、
  `file_tool_guidance` 文案触发条件、`tool_result_storage` 的大结果跳过/持久化/preview/reference
  行为，以及 session view 对 assistant-only tool result 的 compact/omit 规则。
- 边界脚本继续禁止 `agent-tools` / `tool-packs` 依赖 core 或 concrete service。

实施边界：

- 可迁移 provider-neutral runtime contract、adapter facade、只依赖 portable facts 的
  registry/manifest assembly 规则。
- workspace services、cancellation token、Deep Review hook、file read-state storage /
  coordinator binding、tool-result filesystem persistence、
  concrete tools、dynamic MCP concrete execution 和 tool IO 只有在已有 port/provider
  设计和等价测试后才能移动。
- 不改变 tool name、schema、prompt stub、readonly/enabled/filtering、unlock state 生命周期。

审核门：

- 对比迁移前后 registry / manifest / `GetToolSpec` snapshot。
- 对比 Read/Edit/Write guardrail、runtime artifact reference、assistant transcript compaction
  与 session-view 输出，确认没有隐藏改变工具结果语义或磁盘副作用。
- 检查是否新增重复 registry/materialization 或额外 async/runtime work。
- 执行 `cargo test -p bitfun-agent-tools`、`cargo test -p bitfun-core file_read_state_runtime -- --nocapture`、
  `cargo test -p bitfun-core tool_result_storage -- --nocapture`、`node scripts/check-core-boundaries.mjs`、
  `cargo check -p bitfun-core --features product-full`；
  若触碰 dynamic MCP / Deep Review / desktop catalog，再补对应 focused tests。

#### HR-B：Product-Domain Runtime Owner Migration

目标：在不改变 MiniApp filesystem IO、worker process、host dispatch、built-in asset seed /
marker IO、function-agent Git/AI 调用和 Startchat 时序的前提下，继续推进
`bitfun-product-domains` owner 化。

当前 HR-B 执行结论：MiniApp 纯状态 owner 已覆盖 create/update/draft/apply/import
的 version/runtime/meta 组装、imported meta identity/timestamp 规则和 built-in seed meta
timestamp 策略；本轮进一步把 concrete runtime detection、worker pool 容量 / idle / LRU
策略、install-deps 执行计划、host method / fs access / resolved path / shell token / cwd /
timeout / env 等纯决策，以及 export check result policy 迁入 `product-domains`。
function-agent runtime facade、prompt/response policy 已在前序 H2/HR2 收口；Git/AI service
adapter、AI client、provider acquisition、MiniApp filesystem IO、worker process、host dispatch
执行、export skeleton 和 builtin seed / marker IO 仍保持 core-owned，不在本轮改变行为或边界。

预保护：

- 复用并扩展 MiniApp import/sync/recompile/rollback/deps-state、runtime detection、
  worker install / eviction policy、host-routing decision、built-in seed/update marker、
  customization metadata、function-agent staged diff、prompt/JSON extraction/domain error mapping
  等价测试。
- 补齐 Git/AI port adapter 的输入输出、错误映射、fallback、`analyze_git=false`、非 Git
  目录和 no-HEAD diff 行为快照。

实施边界：

- 可迁移受保护的 product-domain owner、DTO、port-backed facade、domain parsing policy 和 core adapter 委托层。
  MiniApp runtime detection 已允许作为受保护 owner 迁入 `product-domains`，但 process 执行例外只允许存在于
  `miniapp/runtime.rs`，不得扩散到 worker、host dispatch、storage 或其它 product-domain 模块。
- MiniApp export check result 可以迁移 ready/runtime/missing/warning 组装策略，但真正 export
  skeleton 和后续打包执行必须留在 core adapter，直到另起实现评审。
- MiniApp filesystem IO、worker process、asset include/seed、marker IO、host dispatch、
  function-agent Git service / AI client / provider acquisition 继续 core-owned，除非本 PR
  先补完整 port/provider 设计和回归。
- 不改变 MiniApp permission policy、bundle/update semantics、Git commit-message 生成行为、
  Startchat work-state 输出或产品 surface。

审核门：

- 对比 core adapter 与 owner facade 的快照输出。
- 检查是否把 PathManager、Git/AI concrete service、worker process、host dispatch execution
  或 builtin marker/source IO 下沉到
  `product-domains`。
- 执行 `cargo test -p bitfun-product-domains`、相关 `bitfun-core` MiniApp/function-agent focused
  tests、`node scripts/check-core-boundaries.mjs`、`cargo check -p bitfun-core --features product-full`。

#### HR-C：Service / Agent Runtime Contract Closure

目标：在不改变 remote-connect、remote-SSH、terminal pre-warm、scheduler/registry、
subagent visibility、background delivery、DeepResearch citation renumber hook 和 session restore
语义的前提下，完成 service/agent runtime 的 portable contract closure；不把 concrete runtime
owner 深迁移混入本里程碑。

预保护：

- 固化 remote command/response wire、poll/model catalog delta、queue/event fanout、restore ->
  terminal pre-warm -> scheduler submit 顺序、file full/chunk/info、image context
  fallback/preference、remote workspace startup guard。
- 固化 mode-scoped subagent availability、`Multitask` / `GeneralPurpose` registration、
  background result delivery、running-turn injection、idle-session follow-up、DeepResearch
  post-turn citation artifact 语义。
- 固化 `/goal` activation、AI-generated goal fallback、session custom metadata patch/clear、
  `GoalVerificationStarted` / `GoalVerificationFinished` events、continuation planning、
  main-session-only gate、Flow Chat local pending/verifying turn 的现有语义。
- 固化 request-context section ordering、workspace `related_paths` prompt output、local
  canonicalization、remote validation、prompt compression events、cache-stable prompt assembly
  和 provider adapter 的 reasoning/tool-call serialization 边界。

实施边界：

- 可迁移只读 facts、queue/restore decision、remote workspace DTO、workspace/session response
  assembly helper、port/provider contract 和 core adapter binding。
- 已迁移的低风险 contract：dialog submission source / priority / policy / outcome、
  agent-session reply route、steering buffered outcome、round injection kind / target / message /
  source traits、subagent context mode、delegation policy、goal-mode DTO、prompt compression
  contract 与 workspace related-path fact 归属 `bitfun-runtime-ports`；core 旧路径只作为兼容
  re-export。queue wait timer 因依赖
  `Instant` / `Duration` 且服务 DeepReview admission timing，仍保留 core-owned，后续若外移需
  单独证明不是把 runtime state 放入 DTO/trait crate。
- 已迁移的 remote-connect presentation assembly：remote chat history 的 user/assistant
  message shape、thinking/text/tool item ordering、tool preview slimming、in-progress assistant
  skip 与 tool input exposure policy 由 `bitfun-services-integrations` 组装；core 只负责把
  persisted session 数据投影为 `RemoteChatHistoryTurn`，并继续持有 image compression 与增强输入清理。
- 已迁移的 remote model policy：remote session model id normalization、model selection
  alias handling 与 config-reference resolution policy 由 `bitfun-services-integrations` 提供；
  core adapter 只负责读取 `AIConfig` 并注入 resolver，保持原有 config service 缺失错误。
- concrete scheduler/session restore、workspace-root source、persistence/workspace service reads、
  `ImageContextData` concrete impl、remote-SSH runtime、terminal adapter、agent registry/scheduler、
  round injection buffer、goal-mode coordinator binding、request-context assembly 与 prompt compression runtime 继续
  core-owned，除非本 PR 有端到端 regression 和明确回滚路径。
- 不统一 Desktop / CLI / ACP / Remote / Server surface 命令或 presentation。

审核门：

- 对比 remote/session/subagent/citation 行为快照。
- 对比 goal verification、request-context related-path sections、compression/cache events 与
  provider stream/tool-call shape，确认迁移没有改变上下文内容或触发时序。
- 检查是否引入新全局 coordinator 访问、反向依赖 core、额外 network/process startup 或
  scheduler 生命周期变化。
- 执行 owner crate tests、remote-connect / scheduler / agent runtime focused tests、
  `node scripts/check-core-boundaries.mjs`、`cargo check -p bitfun-core --features product-full`；
  按触碰范围补 desktop / CLI / ACP / server checks。

#### H5：Feature / Build-Benefit Evaluation

目标：只评估 feature matrix、dependency profile 和构建收益，不迁移 runtime owner。

预保护：

- 先记录 `bitfun-core`、owner crates、Desktop、CLI、ACP、Server、Relay 的 feature graph /
  dependency profile。
- 确认产品 crate 继续显式启用完整能力，release/CI/fast build 脚本无 diff。

实施边界：

- 可补 boundary check、cargo metadata/cargo tree 证据和文档。
- 不修改 default feature、产品 feature set、构建脚本或产品能力。

审核门：

- 对比 no-default、product-full、产品入口依赖面。
- 明确哪些 owner 已能绕开 heavy runtime，哪些仍因 core facade 阻塞；不得把局部收益写成
  整体构建收益。

冗余清理 PR 不进入上述主线序号。只有在满足 `0A.6` 的绝对等价要求时，才可以插入到相邻里程碑之间，并且不得与主线拆分 PR 混合。

---

## 12. 完成标准

- stream processor 和纯 service 测试可以在不编译完整产品 runtime 的情况下运行。
- 至少有一组 dependency profile 证明低层 contract / owner crate 可以绕开 `bitfun-core` 和对应 heavy dependency；若只有极少数模块可做到，必须在文档中明确剩余阻塞 owner，而不能声明重构完成。
- 产品构建脚本和 release/fast build 脚本没有因为 core 拆解被修改。
- 产品 crate 仍拥有拆解前的完整能力集合。
- `bitfun-core` 对现有调用方保持 import-compatible。
- 新拆出的 crate 不依赖回 `bitfun-core`。
- 新增 crate 数量保持在中等粒度范围；继续拆小必须有依赖、owner 或实测收益依据。
- 重依赖归属于真正需要它们的 owner crate。
- `service` 与 `agentic` 的跨层调用通过 ports/providers，而不是 global concrete access。
- 至少在关键 crate 拆出后，有边界检查脚本防止回退。
- 每个关键迁移点都有注释说明兼容门面、owner crate 或接口边界。
- 冗余清理只处理已证明绝对等价的重复代码；不因为相似流程引入新抽象。

# BitFun Core 拆解与构建提速可执行计划

> **执行约定：** 后续实施本计划时，建议按独立 PR 分步推进。每个阶段使用本文的 checkbox 跟踪，不要把多个高风险拆分混在一个 PR 中。

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
src/crates/tool-packs              # 具体工具实现，按 feature group 隔离
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
- `tool-packs` 拥有具体工具实现，并通过 `git`、`mcp`、`computer-use` 等 feature group 隔离重依赖。
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
- 产品完整 runtime 默认安装同等 snapshot decorator，保持原行为。

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

- [ ] 记录依赖和构建基线，生成文件只放 `target/`，不提交。

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
- [ ] 拆解 `BitFunError` 剩余 concrete error-wrapper 依赖。当前仍保留 `serde_json::Error`、`anyhow::Error` 和相关 `From<T>` 兼容行为，不能直接搬进只依赖 `serde` 的 `core-types`。
- [ ] 只有当错误类型不再需要 runtime/network 依赖时，才移动 `BitFunError`、`BitFunResult`。
- [ ] `BitFunError` 移动后保留旧路径 re-export：

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
- [ ] 逐个移动后续 shared DTO，每移动一个 DTO 都确认依赖方向。

**当前状态：** Plan 3 是部分完成。`ErrorCategory`、`AiErrorDetail` 和第一批纯 helper 已进入 `core-types`；`BitFunError` / `BitFunResult` 迁移、剩余 concrete wrapper 处理和后续 DTO 迁移仍是后续任务。未完成项不阻塞 P1 的安全边界验证，但会阻塞“错误类型完全归属 core-types”的完成声明。

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
- [x] 为 `ToolRegistry` 提供 `DynamicToolProvider` adapter。
- [x] 用 `ToolDecorator` 注入 registry 注册装饰入口，保留默认 snapshot wrapping 行为。
- [x] 为 `ConfigService` 提供 `ConfigReadPort` adapter，先建立读取边界，不移动 config service。
- [ ] remote connect / cron / MCP 的 concrete call-site 替换尚未完成；这不是当前第一批 ports adapter 的完成条件，必须在 P2 service 迁移中逐步接入并补 regression。
- [ ] `AgentSubmissionPort` 当前只接受纯文本消息；通用 attachment / image context 需要在 remote bot 或桌面 API 接入前单独设计和验证，避免丢失多模态行为。
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
- [ ] 与 agent runtime 的调用通过 ports 完成。
- [ ] `search`、`lsp`、`cron`、`snapshot` 先作为同 crate 内 feature group，不单独拆 crate。
- [x] 已迁移模块的 core 旧路径通过 re-export 保持。

**当前安全迁移状态（2026-05-11）：**

- 已迁移到 `bitfun-services-core`：`service::system`、`service::diff`、`util::process_manager`、`service::session::types`、`service::session_usage::{types,classifier,redaction,render}`、`service::token_usage::types`。
- `SessionKind` 已移动到 `bitfun-core-types`，core 的 `agentic::core::SessionKind` 与 `service::session::SessionKind` 继续通过 re-export 兼容。
- 最新主干新增的 Deep Review `deep_review_run_manifest` / `deep_review_cache` 字段已随 `service::session::types` 一起迁移，并保留原有序列化别名与 round-trip 测试；这不是新的 P2 行为变更。
- `service::config`、`workspace`、`workspace_runtime`、`filesystem`、`runtime`、`i18n`、`bootstrap`、`project_context` 仍保留在 core；继续迁移前需要先确认 `BitFunError`、`PathManager`、workspace/provider ports 的边界方案。

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
- [ ] 再迁移 `remote-ssh`，保留 `ssh-remote` 语义。
- [x] 先迁移 `remote-ssh` 的纯 contract/type、workspace path/identity helper 与 unresolved-session-key helper，runtime manager / fs / terminal 仍保留在 core。
- [x] 迁移 `mcp` 的 PR2 runtime 与 dynamic provider：config service orchestration、server process / transport lifecycle、resource/prompt adapter、catalog cache、list-changed/reconnect policy、dynamic descriptor / provider / result rendering 均归属 `bitfun-services-integrations`。
- [x] `bitfun-core` 保留 core `ConfigService` store adapter、OAuth data-dir 注入、`BitFunError` 映射、旧路径 facade 和全局 tool registry 组装；tool registry owner 化不混入本 PR。
- [x] 先迁移 `announcement` 的纯 types contract，scheduler / state store / content loader / remote fetch 仍保留在 core。
- [x] 先完成 `remote-connect` contract slice：remote chat/image/tool/session wire DTO 与 relay/bot session/submission request builder 由 `bitfun-services-integrations` 拥有，relay/bot session 创建通过 `AgentSubmissionPort`。
- [ ] 后续完整迁移 `remote-connect` runtime 时，再通过 `SessionTranscriptReader`、`EventSink`、`RemoteControlStatePort` 与 image/queue/event/cancellation port 解耦 agent runtime。
- [x] 已迁移的集成能力保持 core 旧路径 re-export。
- [x] 产品完整 runtime 通过 `services-integrations/product-full` 启用已迁移集成能力。

**当前安全迁移状态（2026-05-13）：**

- 已迁移到 `bitfun-services-integrations`：`service::file_watch`，通过 `file-watch` / `product-full` feature 启用，并保持 `core::service::file_watch` 旧路径。
- `git` 已完成 DTO/params/graph/raw command output/text parser/arg builder、`GitError`、`GitService` runtime implementation 与 git utils 迁移；`bitfun-core::service::git::*` 仅保留 legacy facade re-export。`remote-ssh` 已迁移纯 contract/type、workspace path/identity helper 与 unresolved-session-key helper；SSH runtime manager / fs / terminal、password vault 与 PathManager-backed session mirror assembly 仍保留在 core。`mcp` 已迁移 tool-name / tool-info / protocol types / config location / server type-status、server config、cursor-format、JSON-RPC request builder、JSON config format/validation helper、config merge / remote authorization helper、OAuth credential vault / authorization bootstrap contract、remote auth error classifier、legacy remote header fallback helper、transport Authorization 归一化 helper、remote client capability helper、rmcp 到 BitFun protocol 的纯映射 helper、resource/prompt adapter、catalog cache、list-changed/reconnect policy、config service save-load orchestration、server process / local-remote transport lifecycle、dynamic tool descriptor / provider / result rendering helper，并用 owner crate contract test 锁定 wire shape、transport default、validation message、Cursor 兼容格式、config precedence / dedup 语义、OAuth vault 存储路径注入、NeedsAuth 分类、旧 env Authorization fallback、remote client capabilities、remote result metadata / structured content 映射、config load/save/delete contract、unsupported remote transport contract、context resource selection 和 dynamic manifest；`bitfun-core` 继续负责 core `ConfigService` store adapter、OAuth data-dir 注入、`BitFunError` 映射、legacy facade 和全局 tool registry 组装。`announcement` 仅迁移了纯 types contract，scheduler / state store / content loader / remote fetch 仍保留在 core；`remote-connect` 已完成 contract/request-builder slice，远程消息执行、image context、tracker event、cancel、terminal pre-warm 与 workspace/session restore 仍保留在 core。它们涉及 SSH runtime、remote agent submission runtime、tool registry owner 化与 announcement config/path 边界，继续前需要单独确认端口方案与等价性测试。
- 最新主干的 Deep Review capacity / cost / queue、context profile、evidence ledger、session manifest、stream dedupe、search remote/fallback 与 session rollback persistence 仍属于 core runtime 或对应产品 runtime，不在本轮 `services-integrations` 迁移范围内；如果后续迁移 remote-connect / MCP / search / session，需要先定义运行状态 port 合约和等价测试。

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

- [ ] 抽出 tool trait、tool context、tool result、registry 到 `agent-tools`。
- [x] `agent-tools` 不依赖任何 concrete service。
- [ ] 将工具实现迁移到 `tool-packs` crate，并按 feature group 分模块：
  - basic file/search/terminal
  - git
  - MCP
  - browser/web
  - computer use
  - miniapp
  - cron/task/agent control
- [x] `tool-packs` 默认 feature 为空，产品完整 runtime 启用 `product-full`。
- [ ] 产品 runtime assembly 注册所有 provider：

```rust
registry.install_provider(BasicToolProvider::new());
registry.install_provider(GitToolProvider::new(git_service));
registry.install_provider(McpToolProvider::new(mcp_service));
```

- [ ] 保持兼容构造函数：

```rust
pub fn create_tool_registry() -> ToolRegistry {
    product_full_tool_registry()
}
```

- [ ] 增加 registry 等价性测试：完整产品 registry 中的工具集合与拆分前一致。

**当前安全迁移状态（2026-05-13）：**

- 已迁移到 `bitfun-agent-tools`：`ToolResult`、`ValidationResult`、`InputValidator`、dynamic tool metadata、tool render options、runtime restriction DTO 和 path resolution DTO；dynamic tool provider / decorator contract 已通过 `agent-tools` 提供兼容 re-export，原 `runtime-ports` 路径保持可用；core 旧路径继续 re-export，并只保留 `BitFunError` 映射与路径 containment helper。
- 已新增 `bitfun-tool-packs` feature scaffold，默认 feature 为空，`product-full` 只聚合 feature，不注册或迁移任何工具实现。
- `Tool` trait、`ToolUseContext`、registry 和具体工具实现仍在 core；它们直接连接 workspace service、snapshot wrapper、computer-use host、cancellation token 与 Deep Review checkpoint hook，继续迁移前必须先确认可移植 tool context / provider port 方案，并补工具清单等价性测试。
- 最新主干新增的 Deep Review shared-context / evidence-ledger checkpoint hook 仍保留在 core 的 `ToolUseContext` 中；在设计独立 tool context / event port 前，不应把 `ToolUseContext` 或 registry 继续外移。

**验证：**

```powershell
cargo test -p bitfun-agent-tools
cargo test -p bitfun-tool-packs --features basic
cargo check -p bitfun-tool-packs --features product-full
cargo check -p bitfun-core --features product-full
cargo test -p bitfun-core registry_includes
cargo check -p bitfun-desktop
```

**风险与处理：**

- 风险：工具列表遗漏导致产品能力缺失。
- 处理：拆分前生成工具清单基线；拆分后 registry 等价性测试必须通过。
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

- [ ] miniapp runtime、compiler、permission、builtin 迁移到 `product-domains::miniapp`。
- [ ] function agents 迁移到 `product-domains::function_agents`。
- [ ] 与 agent/tool 的连接通过 provider 或 port。
- [x] 已迁移模块的 core 旧路径 re-export。
- [ ] function agents 依赖 agent runtime port，不直接依赖 service concrete manager。
- [ ] server/desktop 调用路径保持不变。

**当前安全迁移状态（2026-05-11）：**

- 已迁移到 `bitfun-product-domains::miniapp`：`types`、`bridge_builder`、`permission_policy`，core 旧路径继续 re-export。
- 已迁移到 `bitfun-product-domains::function_agents`：公共 `common` 类型，以及 git/startchat function-agent 的纯 DTO 类型；AI client、Git service、prompt 与分析运行逻辑仍留在 core。
- miniapp runtime/compiler/builtin 与 function-agent 运行逻辑继续迁移前，需要先确认 agent/tool/provider port 和 Git/AI service 边界。

**验证：**

```powershell
cargo test -p bitfun-product-domains --features miniapp
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

- [ ] `bitfun-core/Cargo.toml` 只保留 facade 和 product assembly 所需依赖；当前仍因 core-owned runtime 保留 concrete runtime 依赖，不在本 PR 强行删减。
- [x] 旧路径保持 import-compatible。
- [ ] 只有所有产品 crate 都显式启用完整 runtime 后，才可以在独立 PR 中评估：

```toml
default = []
```

**当前收敛状态（2026-05-13）：**

- 本轮不把 `remote-ssh` runtime、`remote-connect`、announcement runtime、concrete tool implementations、tool registry、miniapp runtime/compiler/builtin、function-agent 运行逻辑声明为已迁移；它们继续作为 `bitfun-core` 的 product runtime assembly 或后续 owner PR 拥有路径。`git` feature group 已外移；`remote-ssh` 目前只外移 contract/type、workspace path/identity helper 与 unresolved-session-key helper；MCP PR2 已外移 config service orchestration、server process / transport lifecycle、adapter 和 dynamic tool/resource/prompt provider，core 只保留 ConfigService store adapter、OAuth data-dir 注入、BitFunError 映射、legacy facade 与 registry assembly；`announcement` 目前只外移 types contract。
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
| tool registry 行为变化 | 中 | 高 | 完整工具清单基线；provider 等价性测试 |
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

**当前回合质量核对（2026-05-11，latest `origin/main`）：**

- 变基到最新 `origin/main` 后重新验证：P1 范围内的 feature 安全网、workspace 顶层 crate 移动、`core-types` 第一批类型、`agent-stream` 独立测试和 runtime ports 初始边界均保持通过。`cargo check --workspace` 与 `cargo test --workspace` 均已通过；Web UI lint、type-check 和 full test 也已通过，用于覆盖 rebase 时合并的 `/usage` 面板冲突。全量 workspace test 是本次 P1 退出的补充证据，不改变后续小范围文档或计划修正的默认最小门禁。
- 已满足：`product-full` 默认能力保护未改变；产品 crate 仍显式启用完整 runtime；构建脚本和 installer 范围保持无 diff。
- 已满足：`bitfun-agent-stream` 不依赖 `bitfun-core`，stream 旧路径通过 core compatibility wrapper 委托到新 crate。
- 已满足：`bitfun-runtime-ports` 仍保持 DTO / trait-only，第一批 core adapter 已建立。
- 已收敛：`DynamicToolProvider` adapter 只暴露 MCP 命名空间动态工具，不把内置工具误报为动态 provider。
- 尚未完成：remote connect / cron / MCP 的 concrete call-site 尚未迁移到 ports；这部分属于里程碑二 service owner crate 迁移，不应在当前回合声明完成。
- 尚未完成：generic attachments / image context 尚未接入 `AgentSubmissionPort`；接入前必须补多模态行为保护测试。

**P1 退出审查补充（2026-05-11）：**

- 审查当前 `origin/main..HEAD` 的 P1 相关变更后，未发现需要阻塞 P1 退出的产品正确性回归。
- `AgentSubmissionRequest.source` 已显式化；`turnId` 也已作为 P2 前置 contract hardening 提升为显式可选 DTO 字段。
  coordinator 在兼容期优先读取 `request.turn_id`，再回退 `metadata["turnId"]`，避免影响旧调用方。
- `DynamicToolProvider` 已过滤为显式声明 provider metadata 的动态工具；MCP wrapper 通过 `Tool::dynamic_provider_id`
  暴露 server id，registry 不再从 `mcp__server__tool` 注册名反推 provider 身份。
- remote connect / cron / MCP 的 concrete call-site 迁移，以及 `AgentSubmissionPort` 的 attachment / image context 设计，
  仍属于后续 P2 service owner crate 迁移范围；当前回合不改变这些路径的产品逻辑或边界行为。
- 本次 P2 前置 contract hardening 验证通过：`cargo test -p bitfun-runtime-ports`、
  `cargo test -p bitfun-core agent_submission_turn_id -- --nocapture`、
  `cargo test -p bitfun-core dynamic_tool_provider_uses_explicit_provider_metadata -- --nocapture`、
  `cargo check -p bitfun-core --features product-full`、`cargo check --workspace`、`cargo test --workspace`。
- P1 退出验证通过：`cargo test -p bitfun-runtime-ports`、`cargo test -p bitfun-agent-stream`、
  `cargo check -p bitfun-core --features product-full`、`cargo check -p bitfun-desktop`、
  `cargo check -p bitfun-cli`、`cargo check -p bitfun-server`、`cargo check --workspace`、
  `cargo test --workspace`、`pnpm run lint:web`、`pnpm run type-check:web`、
  `pnpm --dir src/web-ui run test:run`，并确认构建脚本 / installer 保护范围无 diff。
  现存 Cargo 输出仅包含既有 desktop unused import 警告，不阻塞 P1 退出。
- 结论：按当前 P1 范围，边界安全网与最小编译面验证已经完成；未迁移的 concrete call-site、
  attachments / image context、显式 `turnId` 和 provider metadata hardening 转入 P2/P3 前置队列，
  不应被计入 P1 未完成项。

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
- `bitfun-agent-tools` / `bitfun-tool-packs`：拆出 tool trait、context、registry、provider contract，并通过 feature group 承载具体工具实现。
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
- 工具清单、snapshot wrapping、permission / concurrency safety 行为与迁移前不等价。

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

**当前 P2 执行状态（2026-05-12）：**

- 已完成中等粒度 owner crate 成型的安全部分：`bitfun-services-core`、`bitfun-services-integrations`、`bitfun-agent-tools`、`bitfun-tool-packs`、`bitfun-product-domains` 均已加入 workspace。
- 已迁移的模块均由 core facade re-export，未改变产品默认 feature、构建脚本或 release 脚本。
- Git feature group 已闭环迁移到 `bitfun-services-integrations` 的 `git` feature：DTO/params/graph/raw command output/text parser/arg builder、`GitError`、`GitService` runtime implementation 与 git utils 均由 integrations owner crate 拥有，并通过 `bitfun-core::service::git::*` 保留旧路径兼容。`GitService` 所需的 Windows `libgit2` system-link 边界挂在该 crate 的 `git` feature 上；`bitfun-core` 仍因未迁移的 remote-connect runtime 保留其它 `git2` 使用。remote-ssh 本轮进一步外移 workspace path/identity 与 unresolved-session-key helper，并用 owner crate contract test 锁定 normalized path、mirror subpath、hostname sanitization、stable id 和 unresolved key 输出；PathManager-backed mirror root、global workspace registry、SSH manager/fs/terminal/runtime 仍留在 core。MCP PR2 已进一步外移 config service orchestration、server process / local-remote transport lifecycle、dynamic tool provider 与 context resource selection helper，core 旧路径继续做兼容 facade、core config store adapter、OAuth 数据目录注入、`BitFunError` 映射和全局 registry 组装。
- 未声明完成的 P2/后续剩余部分：remote-ssh runtime、remote-connect 等重 service 迁移、concrete tool implementation 迁移、tool registry/provider 化、miniapp/function-agent 运行逻辑迁移。这些会触碰 `PathManager`、`ToolUseContext`、workspace service、snapshot wrapper、`AgentSubmissionPort` 或 AI service 边界，需要在继续前显式确认。
- 本次 rebase 后重新核对最新主干 Deep Review capacity/cost/queue、context profile、evidence ledger 与 session manifest 变更：当前 PR 已完成 Git feature group 的 owner crate 归属迁移，但未改动这些 Deep Review 行为路径；后续迁移必须补端口设计和等价测试后再推进。
- 本次 P2 后续复核结论：上述高耦合剩余项不是纯文件搬迁；若继续迁移会改变依赖方向或需要新增 port/provider 行为合约。因此当前 PR 将它们显式保留为 core-owned runtime，并通过 P3 boundary check 防止已拆 owner crate 回流依赖 core。

**后续风险重排（2026-05-13）：**

当前文档的最终目标仍然成立，但后续不能把“feature 最小依赖”当成已经自然达成。低风险、可确定收益的保护项应前移；会触碰运行时语义的迁移项必须拆成单独评审。

可提前进入下一批的小风险事项：

- 补充 dependency profile / feature graph 基线：记录 `bitfun-core`、`bitfun-services-integrations --no-default-features`、单 feature owner crate、desktop、CLI、ACP 的 `cargo tree -e features` 预期，明确哪些目标允许出现 `rmcp`、`git2`、`image`、`tokio-tungstenite`、`bitfun-relay-server`、Tauri / CLI presentation 依赖。
- 修正轻量 contract crate 的依赖泄漏，例如 `bitfun-agent-tools` 只应承载 tool DTO / contract；如果需要移动 `ToolImageAttachment` 一类纯 DTO，必须保留旧路径 re-export 和序列化 round-trip 测试。
- 为 `services-core`、`tool-packs`、`product-domains` 补清晰的 feature group 说明和边界检查；允许先声明或测试空 feature，但不能声明对应 runtime 已迁移。
- 扩展 boundary check，覆盖 feature graph 中的禁止依赖：`core-types`、`runtime-ports`、`agent-tools` 不能出现 concrete service、network/client、platform adapter、CLI/TUI 或 heavy integration 依赖。
- 为高风险迁移建立迁移前快照测试：tool registry 清单与顺序、dynamic provider metadata、snapshot wrapper、MCP wire shape、remote-connect 消息字段、miniapp permission policy、function-agent 输入输出。

本批执行状态：

- 已扩展 `scripts/check-core-boundaries.mjs`，增加 dependency profile / feature graph 静态保护：`core-types` default profile 禁止非 DTO 依赖，`runtime-ports` default profile 禁止 service implementation 依赖，`agent-tools` contract profile 禁止依赖 `bitfun-ai-adapters`，`product-domains` default profile 禁止无条件拉入 `dirs`，`services-integrations` default profile 禁止无条件拉入 feature-gated integration 依赖。
- 已将 `ToolImageAttachment` 提升到 `bitfun-core-types`，并由 `bitfun-ai-adapters`、`bitfun-agent-tools` 和 `bitfun-core::util::types` 保留旧路径兼容；`bitfun-agent-tools` 不再依赖 `bitfun-ai-adapters`。
- 已将 `product-domains` 的 `dirs` 依赖限制到 `miniapp` feature，默认 profile 保持轻量。
- 已补充 `ToolResult` image attachment、dynamic provider metadata、dynamic descriptor wire shape、runtime restrictions 与 path resolution contract 测试，以及 tool registry readonly / descriptor / stale metadata 工具清单快照测试；后续迁移 `ToolUseContext`、registry/provider 或 concrete tool implementation 前必须保持这些基线。
- PR 1 已开始执行：remote-SSH workspace registry / ambiguous root resolution / legacy state snapshot 已迁入 `bitfun-services-integrations::remote_ssh::RemoteWorkspaceRegistry`，core 仅保留 local assistant path guard 与 SSH manager / file service / terminal manager 组装；announcement state persistence 已迁入 `bitfun-services-integrations::announcement::AnnouncementStateStore`，core 旧 `PathManager` 构造 API 继续委托并映射原错误类型。
- 本批 dependency profile 基线已验证：
  - `cargo tree -p bitfun-core-types --depth 1 --edges features` 运行时依赖仅显示 `serde`，测试依赖显示 `serde_json`。
  - `cargo tree -p bitfun-runtime-ports --depth 1 --edges features` 仅显示 `async-trait`、`serde`、`serde_json`。
  - `cargo tree -p bitfun-agent-tools --depth 1 --edges features` 仅显示 `bitfun-core-types`、`serde`、`serde_json`。
  - `cargo tree -p bitfun-product-domains --no-default-features --depth 1 --edges features` 仅显示 `serde`、`serde_json`，不会拉入 `dirs`。
  - `cargo tree -p bitfun-services-integrations --no-default-features --depth 1 --edges features` 仅显示 `bitfun-events`、`serde`、`serde_json`、`log`、`tokio`。

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

- `ToolUseContext`、tool registry/provider、concrete tool implementation 外移。
- MCP tool registry owner 化与 concrete tool implementation 外移。
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
- tool registry 迁移后工具清单无法证明等价。
- 新 owner crate 反向依赖 core。

**剩余工作压缩为 5 个 PR（2026-05-13）：**

1. `services-integrations` runtime 收口：迁移 remote-SSH 中不直接持有 SSH channel / SFTP / terminal handle 的 workspace registry、session mirror 与轻量 runtime helper；继续保留 SSH manager / remote FS / remote terminal 的 core-owned assembly，直到 port/provider 合约明确。`file-watch` 已由 `services-integrations` 拥有，只做 contract 复核；announcement 只迁移不依赖 config service / embedded content / remote fetch 的 state 或 eligibility helper。验收重点是 owner crate contract test、旧路径 facade、boundary check、workspace check/test。
2. 已完成：MCP runtime 与 dynamic tools：MCP config service orchestration、server process / transport lifecycle、adapter、dynamic tool/resource/prompt provider 已归属 `bitfun-services-integrations`；未混入 remote-connect 或 tool registry owner 化。验收重点是 MCP wire shape、auth/config merge、dynamic manifest 快照和 core registry 集成等价。
   - 保留边界：`bitfun-core` 只保留 core `ConfigService` store adapter、OAuth data-dir 注入、`BitFunError` 映射、legacy facade 和与全局 tool registry 的组装调用；配置写入、OAuth、SSE/session 与 registry 行为不得在本 PR 中改变。
   - 后续切片：tool registry/provider owner 化需要放入 PR4，并先保留 dynamic provider metadata、工具清单顺序和 snapshot wrapper 等价测试。
   - 文档校正：P2 后补充文档中的 MCP runtime step 已由本 PR2 闭环；后续 MCP 相关工作只保留 tool registry/provider owner 化和 concrete tool implementation 迁移，不再重复迁移 config/process/transport lifecycle。
3. 已完成：remote-connect contract slice：产品表面 DTO、remote chat/image/tool/session wire DTO 与 relay/bot session/submission request builder 已迁入 owner crate；完整 remote runtime 仍保留在 core，直到 `SessionTranscriptReader`、`EventSink`、`RemoteControlStatePort` 和 image/queue/event/cancellation port 补齐。
   - 后续切片：完整 remote-connect runtime 迁移必须单独执行，不得混入 PR4 的 tool/provider owner 化。
4. 进行中：agent tools + `tool-packs` owner 化：已先迁移纯 tool contract/provider metadata、runtime restriction DTO、path resolution DTO，并为 dynamic provider contract 提供 `agent-tools` 兼容 re-export；`ToolUseContext`、tool registry/provider 化、concrete tool implementation 按 feature group 迁移必须保持 builtin/readonly/dynamic manifest、snapshot wrapping、runtime restrictions、cancellation 与 Deep Review tool flow 等价。
5. `product-domains` runtime + core facade finalization：迁移 miniapp runtime/compiler/builtin 与 function-agent 运行逻辑，最后把 `bitfun-core` 收敛为 facade + product runtime assembly；不在本 PR 中修改 `bitfun-core default = []` 或 per-product feature matrix。

`bitfun-core default = []`、per-product feature set、构建矩阵和 release 能力调整仍作为重构完成后的独立评估，不计入上述 5 个 PR。

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

**P3 进入条件与最新主干补充（2026-05-12）：**

- P3 只能在 P2 剩余迁移闭环后启动：重 service 迁移、concrete tool implementation 迁移、tool registry/provider 化、miniapp/function-agent 运行逻辑迁移都必须先完成或显式保留为 core-owned runtime。
- 最近 `origin/main` 的 Deep Review 变更增加了 context profile、evidence ledger、capacity/cost/queue 控制、`deep_review_run_manifest` / `deep_review_cache`、以及 review-team UI orchestration；最新主干还补充了 agent-stream tool-call dedupe、search remote/fallback、session rollback persistence 和 companion typewriter。P3 facade 收敛前必须确认这些行为要么仍由 core product runtime assembly 拥有，要么已有对应 owner crate + port/provider 合约和等价测试。
- 最新主干的 mode-scoped subagent visibility 将 `agentic::agents` 重组为 definitions / registry / visibility 边界，并扩展了 desktop subagent API 与 Review Team 可见性测试；后续若迁移 agent registry，不能只做路径 re-export，必须保留 mode 可见性过滤、hidden/custom/review 分组语义和前后端 API contract。
- 最新主干的 CLI 重构主要新增 TUI/theme/selector/dialog/chat-state 等 app-layer 代码，后续又收敛预置 theme 并补充 desktop companion pet resize / Windows UX；这些当前没有改变 `services-integrations` 的迁移归属。后续若调整 shared crate 边界，必须继续把 `bitfun-cli`、`ratatui`、`crossterm`、`arboard`、`syntect-tui` 等 CLI-only 依赖限制在 app adapter / presentation layer，desktop / web-ui presentation 修复也不应被误判为 core service 迁移前置条件。
- P2 后产品表面策略要求“surface divergence, capability convergence”：CLI `/diff`、Desktop 快捷键/面板、Remote card、ACP method 可以映射到同一 capability contract，但不能为了复用把 surface command 或 UI rendering 下沉到 contract crate。
- `ToolUseContext` 的 shared-context / evidence checkpoint hook、`TaskTool` / `CodeReviewTool` 的 Deep Review capacity flow、session manifest/cache persistence、rollback persisted-turn cleanup、search fallback chain 与 stream finish/tool-call contract 不能在 P3 中只通过 re-export 消失；如果外移，需要先补 boundary contract、旧路径兼容和对应 regression。
- P3 的闭环检查应同时覆盖 Rust crate graph 与产品 runtime 行为：边界脚本只证明依赖方向，不能替代 Deep Review、MCP dynamic tools、remote connect、snapshot wrapping、miniapp/function-agent 的产品等价性验证。
- 当前 PR 的 P3 范围按“显式保留 core-owned runtime + 强制 owner crate 边界”闭环；后续如果要继续外移这些 runtime 路径，需要作为新的迁移批次先补 port 设计、等价测试和用户确认。

**阶段复核与后续拆分（2026-05-13 PR2 完成后）：**

- 当前分支保持单一主题：只包含 boundary check、阶段文档，以及 Git / remote-ssh 已迁移边界上的 MCP PR2 runtime / dynamic provider 归属迁移；不继续混入 remote-connect、tool registry owner 化、miniapp 或 function-agent runtime 迁移。
- 本次 rebase 到最新 `upstream/main` 后，PR #680 的三颗等价提交已进入主干；新增 CLI 收敛、版本 bump 和 desktop companion pet UX 修复没有改变本轮 `services-integrations` / service facade 迁移内容，但会把后续 CLI 产品检查、agent registry/provider 与 session/tool runtime 外移的等价性门槛抬高。
- 质量边界：本阶段证明已拆 owner crate 不依赖回 `bitfun-core`，并证明已迁移的 Git / remote-ssh helper 与 MCP config/process/transport/dynamic provider 旧路径或调用点仍通过当前 Rust workspace 检查；不声明 remote connect、tool registry/snapshot wrapping、miniapp/function-agent runtime 的外移完成。
- boundary check 已扩展到 `core-types`、`runtime-ports` 和 `agent-tools` 的轻量边界，并覆盖 Cargo inline 依赖和 dependency table 依赖声明，后续不能绕过脚本把重 runtime、concrete service、platform adapter 或 CLI/TUI presentation 依赖带入这些 contract crate。
- boundary check 也已锁定 `bitfun-core::service::git`、`bitfun-core::service::remote_ssh::types`、remote-ssh workspace path/identity/unresolved-key helper、`bitfun-core::service::mcp::{tool_info,tool_name}`、`bitfun-core::service::mcp::protocol::{types,jsonrpc}`、`bitfun-core::service::mcp::config::{location,cursor_format,json_config,service_helpers}`、`bitfun-core::service::mcp::server::config`、`bitfun-core::service::mcp::auth` 和 `bitfun-core::service::announcement::types` 的旧路径 facade-only / 禁止回流状态，并禁止在 `MCPServerProcess` runtime 文件重新定义已外移的 server type/status contract、auth error classifier 和 legacy remote header fallback helper，也禁止在 remote transport 重新实现 Authorization 归一化、client capability 构造和 rmcp result mapping。
- 后续迁移必须拆成可独立审核的提交：先补 port/provider 设计和等价测试；`remote-connect` 完整 runtime、tool registry/provider、product-domain runtime 必须一次迁移一个 owner 主题。
- tool registry / concrete tool implementation 外移必须先有工具清单等价测试，并保留 dynamic provider metadata；不能把注册名解析、snapshot wrapper 或 runtime restriction 行为改成隐式约定。
- 已新增并扩展内置工具清单基线测试，后续 registry/provider 外移必须先保持该清单、注册顺序、runtime collection 顺序、dynamic provider metadata 顺序和修改类工具 snapshot wrapper 等价，再评估 owner crate 边界。
- miniapp 与 function-agent runtime 外移必须先明确 Git/AI service、PathManager、process execution 和 permission policy 边界；如果需要行为合约变化，必须作为后续单独 PR 并先确认。
- 产品表面 contract 补强必须保持 observational：只记录 surface/thread/environment/permission/artifact facts，不改变 CLI、Desktop、Remote、ACP 的现有交互和运行时语义。
- `bitfun-core default = []` 和依赖版本收敛仍是后续独立评估项，不与 runtime 外移或构建脚本调整混在同一批提交。

**验收门：**

```powershell
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
cargo test --workspace
cargo build -p bitfun-desktop
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

1. 已完成：文档与基线护栏。
2. 已完成：`product-full` feature 安全网，不改变 default 行为。
3. 已完成：移动 nested `terminal-core` 和 `tool-runtime` 到 workspace 顶层。
4. 已完成：抽取 `bitfun-core-types`，先放错误和第一批稳定 DTO。
5. 已完成：抽取 `bitfun-agent-stream`，迁移 stream processor 测试。
6. 已完成：引入 runtime ports 初始边界；后续在 service 迁移中逐步打断 `service <-> agentic` concrete 循环。
7. 已完成：抽取 `bitfun-services-core`。
8. 已完成：抽取 `bitfun-services-integrations` 的低风险 feature group 和纯 helper，闭环 `git`、remote-SSH contract/helper、MCP 纯 protocol/config/auth helper；MCP runtime / dynamic provider 已在 PR2 补齐，未把 remote-connect 或 tool registry owner 化顺带迁入。
9. 已完成：前移低风险保护项：dependency profile / feature graph 基线、轻量 contract crate 依赖瘦身、feature group 说明、boundary check 扩展、迁移前快照测试。
10. 进行中：PR 1 `services-integrations` runtime 收口，先处理 remote-SSH workspace registry / session mirror helper 和已迁移 file-watch 的 contract 复核；announcement 仅迁移无 config/content/remote fetch 依赖的 helper。
11. 已完成：PR 2 MCP runtime 与 dynamic tools；已迁移 config service orchestration、server process / transport lifecycle、adapter、dynamic tool/resource/prompt provider，core 保留 ConfigService store adapter、OAuth data-dir 注入、BitFunError 映射、legacy facade 和 registry assembly。
12. P2 后前置轨道：产品表面 contract-only 补强，可在 PR3 前或 PR3 第一组提交中处理；只允许 DTO/port、round-trip/no-op tests 和 boundary check，不实现 CLI/Desktop/Remote/ACP UI 或命令变更。
13. 已完成：PR 3 remote-connect runtime contract slice：产品表面 DTO 已以 contract-only 方式进入 `bitfun-core-types`；`bitfun-services-integrations` 新增 `remote-connect` feature，拥有 remote chat/image/tool/session wire DTO 与 relay/bot session/submission request builder；relay/bot 创建 session 已通过 `AgentSubmissionPort`。远程消息执行、image context、tracker event、cancel、terminal pre-warm 与 workspace/session restore 仍保留在 `bitfun-core` product runtime assembly，直到补齐 image/queue/event/cancellation port 后再迁移。
14. 进行中：PR 4 agent tools + `tool-packs` owner 化，当前仅处理低风险 contract / DTO 外移、provider contract 兼容 re-export 和边界防护。
15. 待执行：PR 5 `product-domains` runtime + core facade finalization。
16. 后续独立评估：`bitfun-core default = []` 或 per-product feature set。

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

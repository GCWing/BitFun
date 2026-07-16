**中文** | [English](AGENTS.md)

# AGENTS-CN.md

BitFun 是一个由 Rust workspace 与 React 前端组成的项目。

仓库核心原则：**先保持产品逻辑平台无关，再通过平台适配层对外暴露能力**。

## 快速开始

1. 在修改架构敏感代码前，先阅读 `README.md` 和 `CONTRIBUTING.md`。
2. 桌面端开发优先使用 `pnpm run desktop:dev` — 提供完整热更新（Vite HMR + Rust 自动重编译并重启）。仅在需要更快冷启动且只迭代前端时使用 `pnpm run desktop:preview:debug`（Rust 改动不会自动重编译）。
3. 修改 Rust 文件后，优先使用 `pnpm run fmt:rs`，只格式化已改动或已暂存的 `.rs` 文件。只有在你明确需要更大范围格式化时才使用 `cargo fmt`。
4. 改完后按下方表格执行与改动范围匹配的最小验证。

## 分层模块索引

依赖关系按自上而下读取。下表是物理 crate 布局，不是完整概念架构；Product Surface、Product Assembly、
Product Feature、Agent Kernel、Execution、Extension、Cross-platform Adapter、
Stable Contracts and Security Control Plane 的边界以
[`docs/architecture/product-architecture.md`](docs/architecture/product-architecture.md)
为准。同层 crate 也应保持最小依赖。

| # | 层级 | 路径 | 职责 | 模块 / 入口 | 层级文档 |
|---|---|---|---|---|---|
| 1 | 接口与入口层 | `src/apps/*`, `src/web-ui`, `src/mobile-web`, `BitFun-Installer`, `tests/e2e`, `src/crates/interfaces` | 产品宿主、命令、UI 入口、协议接口和跨形态测试 | desktop、CLI、server、relay、Web UI、mobile web、installer、E2E、`acp` | 最近的本地 `AGENTS.md`；[interfaces](src/crates/interfaces/AGENTS.md) |
| 2 | 产品组装层 | `src/crates/assembly` | 兼容导出、产品能力选择、product-full 接线和 adapter/service 注册 | `core`, `product-capabilities` | [AGENTS.md](src/crates/assembly/AGENTS.md) |
| 3 | 适配层 | `src/crates/adapters` | AI/API/transport/WebDriver/OpenCode 协议 adapter 和外部 provider 转换 | `ai-adapters`, `api-layer`, `opencode-adapter`, `transport`, `webdriver` | [AGENTS.md](src/crates/adapters/AGENTS.md) |
| 4 | 服务实现层 | `src/crates/services` | 可复用 OS、filesystem、terminal、MCP、remote、git、watch、process、LSP plugin registry、session persistence primitives、network 和 MiniApp runtime IO 实现 | `services-core`, `services-integrations`, `relay-service`, `terminal` | [AGENTS.md](src/crates/services/AGENTS.md) |
| 5 | 执行原语层 | `src/crates/execution` | 可移植 agent、harness、stream、DeepReview policy/report、plugin host 边界、typed-service、tool-contract、tool-group 和 tool-execution 构件 | `agent-runtime`, `agent-stream`, `tool-contracts`, `harness`, `plugin-runtime-host`, `runtime-services`, `tool-provider-groups`, `tool-execution` | [AGENTS.md](src/crates/execution/AGENTS.md) |
| 6 | 稳定契约与产品领域层 | `src/crates/contracts` | 跨层共享 DTO、事件形状、runtime port、LSP protocol/plugin DTO、产品领域契约和策略 | `core-types`, `events`, `runtime-ports`, `product-domains` | [AGENTS.md](src/crates/contracts/AGENTS.md) |

边界规则：

- 接口与入口层暴露选定产品行为；可复用行为应下移。
- 组装层只接线下层并选择产品能力事实，不实现具体 adapter、OS 或 service 细节。
- 产品特性只在内核能力之上组装用户侧命令、UI contribution、设置和默认策略；长程任务、scheduler、permission、session/workspace、memory、DFX、hook 和 event 事实属于 Agent Kernel owner。
- 适配层翻译协议和外部 provider 形状，不拥有产品能力选择或可复用 OS service 行为。
- 服务实现层负责可复用的 OS、process、terminal、MCP、remote、git、filesystem、LSP plugin registry、session persistence primitives 和 MiniApp runtime IO 能力。
- 外部系统是边界外资源，不是仓库内层级。只有已注册的 adapter、service 或 app-local provider 应调用它们；其他层消费 port 和稳定契约。
- 执行原语层只放可移植运行时构件，不拥有宿主或交付形态。
- 契约层保持轻行为，不得向上依赖。

## 常用命令

这些是命令参考，不是 PR 前置检查清单。预检请按下方“验证”表选择最小本地检查；
大范围测试和构建主要用于复现 CI 或验证构建相关改动。

```bash
# 安装
pnpm install

# 开发
pnpm run desktop:dev               # 完整热更新：Vite HMR + Rust 自动重编译并重启
pnpm run desktop:preview:debug     # 复用预构建二进制 + Vite HMR；无 Rust 自动重编译
pnpm run dev:web                   # 纯浏览器前端
pnpm run cli:dev                   # CLI 运行时
pnpm run cli:install               # release 编译并安装 bitfun-cli 到 ~/.local/bin（写入 bashrc/zshrc PATH）

# 检查
pnpm run fmt:rs                     # 只格式化已改动 / 已暂存的 Rust 文件
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/mobile-web run type-check
pnpm run i18n:contract:test          # 仅 i18n contract / resources
pnpm run i18n:audit                  # 仅 i18n contract / resources
pnpm run check:repo-hygiene
pnpm run check:github-config
cargo check --workspace

# 测试（本地优先用精确测试路径；大范围测试由 CI 兜底）
pnpm --dir src/web-ui run test:run      # 大范围测试；本地优先用精确测试路径
cargo test --workspace                  # 大范围测试；CI 兜底

# 构建（仅构建相关改动或复现 CI 时运行）
cargo build -p bitfun-desktop           # 构建相关改动 / 复现 CI
pnpm run build:web                      # 构建相关改动 / 复现 CI
pnpm run build:mobile-web               # 构建相关改动 / 复现 CI

# 快速构建（手动构建 / 调试流程）
pnpm run desktop:build:fast           # debug 构建，不打包
pnpm run desktop:build:release-fast   # release 但降低 LTO
pnpm run desktop:build:nsis:fast      # Windows 安装器，release-fast profile
```

完整脚本列表见 [`package.json`](package.json)。

## 全局规则

### 国际化

- Locale id、alias、fallback 和各形态默认语言统一由
  `src/shared/i18n/contract/locales.json` 管理；修改后运行
  `pnpm run i18n:generate`。
- 跨形态稳定标签放在
  `src/shared/i18n/resources/shared/<locale>/terms.json`；流程文案留在所属
  产品形态资源中。
- 不要把 Web UI locale 资源导入 `src/mobile-web`、`BitFun-Installer` 等较小形态；
  完整规则见 `docs/architecture/i18n.md`。
- 静态自包含页面只能使用生成的 page-scoped shared-term 文件，不得导入 Web UI locale catalog。
- Web UI 只急切加载 bootstrap namespace；路由或功能文案使用
  `useI18n(namespace)`，直接 `i18nService.t(...)` 只用于 bootstrap namespace。
- 用户可见的日期、时间和数字应通过共享 i18n 格式化 helper 处理，避免在产品代码中直接
  使用 `Intl.*` 或 `toLocale*`。
- `pnpm run i18n:audit` 会检查 key / 占位符一致性、直接静态 key、dynamic key
  source proof、literal fallback / locale-format 零增长基线、shared-term / l10n
  治理基线、非阻断 same-text locale 盘点，以及 source 中不再新增硬编码 CJK 文案。

### 主题与颜色 Token

- 主题与颜色 baseline 是 ratchet 契约，不是可随意修改的测试期望。不得通过提高
  `scripts/theme-color-governance-baseline*.json`、放宽 fixture/assertion、扩大 allowlist
  或移除 CI 审计来让失败检查通过。
- 实际债务减少时应同步下调 baseline。确需新增颜色或 key 时，只增加最小 owner contract，
  并说明现有 semantic、component 或专用域 Token 为什么不能覆盖。
- 修改 theme、CSS variable、widget payload、mobile、installer 或 CLI/TUI 颜色时，运行
  `pnpm run theme:color-audit:all`。

### 日志

日志必须只用英文，且不能使用 emoji。

- 前端：[src/web-ui/LOGGING.md](src/web-ui/LOGGING.md)
- 后端：[src/crates/LOGGING.md](src/crates/LOGGING.md)

### Tauri command

- command 名称：`snake_case`
- TypeScript 可以用 `camelCase` 包装，但调用 Rust 时要传结构化 `request`

```rust
#[tauri::command]
pub async fn your_command(
    state: State<'_, AppState>,
    request: YourRequest,
) -> Result<YourResponse, String>
```

```ts
await api.invoke('your_command', { request: { ... } });
```

### 平台边界

- 不要在 UI 组件里直接调用 Tauri API；应通过 adapter / infrastructure 层访问。
- 桌面端专属集成应放在 `src/apps/desktop`，再通过 transport / API layer 回流到共享逻辑。
- 在共享 core 中避免使用 `tauri::AppHandle` 等宿主 API；优先使用 `bitfun_events::EventEmitter` 等共享抽象。

### 远程兼容

- 新增功能时，从一开始就要考虑远程工作区和远程控制同步适配。只支持本地的行为很容易让远程场景功能缺失。
- 如果某个功能无法合理支持远程工作区，必须做能力屏蔽，或展示明确的不支持提示，不能让它以通用错误的形式失败。
- 每个桌面端 Tauri 命令都必须在
 `src/apps/desktop/src/api/remote_workspace_policy.rs` 中声明远程工作区策略；
 该文件的契约测试会拒绝没有显式策略的新命令，并禁止 legacy-unaudited 存量清单增长。

### Agent loop 行为

- 不要把硬编码限制或模式判断作为处理 agent loop 循环问题的第一反应，例如仅按字符串或次数阻止重复工具调用。
- 过多硬编码会把 agent loop 变成脆弱的 workflow。应先定位根因：工具行为、模型交互、会话上下文封装、prompt/tool schema 设计，或状态同步问题。

## 架构

### 产品架构护栏

任何 `bitfun-core` 拆解、feature 边界、依赖边界或 Rust 构建提速重构，
都必须先阅读
[`docs/architecture/product-architecture.md`](docs/architecture/product-architecture.md)。
顶层文档只作为入口；模块级 ownership 细节应放到离代码最近的模块 `AGENTS.md`。

仓库级拆解规则：

- 不要把 DTO / contract 抽取误判为 runtime owner 已迁移。
- 产品表面可以有差异；共享稳定 facts 或 ports，不共享 UI、protocol、lifecycle 或平台实现。
- 迁移 runtime owner 必须有评审过的 port/provider 设计、旧路径兼容、行为等价测试；如果可能改变行为边界，还需要先确认。

### CLI 产品线护栏

涉及 CLI/TUI 能力对齐、非交互输出契约、外部配置导入、插件管理体验、CLI Agent 行为或 CLI
白标发行时，先阅读
[`docs/architecture/cli-product-line-design.md`](docs/architecture/cli-product-line-design.md) 和
[`src/apps/cli/AGENTS.md`](src/apps/cli/AGENTS.md)。CLI/TUI 展示留在 app；可复用产品行为通过
Product Assembly、Agent Runtime、Tool/Harness、Runtime Services 或既有扩展边界承接。

### HarmonyOS PC CLI/TUI 护栏

涉及 HarmonyOS PC CLI/TUI 支持时，还必须阅读
[`docs/architecture/platform-portability-design.md`](docs/architecture/platform-portability-design.md)。
这是未来平台目标，不是已实现支持。目标是真实 PC 系统终端；HAP、`hdc shell`、
手机 Remote App 和远端代执行都不能替代。具体适配必须另立专题，现有移动端能力保持不变。

### 产品定制护栏

涉及产品定义、品牌发行、GUI/TUI 布局选择、产品内置扩展或定制构建任务时，先阅读
[`docs/architecture/product-customization-blueprint.md`](docs/architecture/product-customization-blueprint.md)。
产品定制必须与用户运行时配置和插件分开；GUI/TUI 只共享稳定产品事实，不共享布局、组件、主题键、键位、
renderer schema。产品组装结果和布局选择只能携带少量不可变的产品身份、数据隔离、故障恢复、升级完整性或
法律保护项 ID；不得承载用户/来源级插件策略、安装、激活、更新、权限或动态健康状态。Product Profile、
Brand Pack、GUI/TUI Surface Blueprint 和 Resolved Product Manifest 是已退役的设计术语，并非当前生产对象；
不得为这些术语新建兼容格式，只实现被真实构建和运行时消费的最小产品定义与组装结果字段。

涉及 OpenCode 实时配置或插件执行时，还要阅读
[`docs/architecture/extensions/opencode-extension-compatibility.md`](docs/architecture/extensions/opencode-extension-compatibility.md)。
在对应 OC-R 阶段实现并通过验证前，当前 P0 适配器仍只是受管包/静态预览路径。不得继续把旧受管包路径扩张为
OpenCode 目标运行模型，也不得把设计目标描述成已可用能力。

### SDLC 质量护栏

涉及生命周期证据、门禁、Artifact Graph、Project Profile、Deep Review 策略、
OpenCode 兼容或目标项目治理的变更，先阅读
[`docs/sdlc-harness/README.md`](docs/sdlc-harness/README.md)，再阅读
[`docs/sdlc-harness/design.md`](docs/sdlc-harness/design.md)。如果变更影响模块边界或行为，
继续参考 `docs/sdlc-harness/architecture/` 或 `docs/sdlc-harness/features/` 下的对应设计。

不要把 BitFun 自身验证假设硬编码成目标项目通用规则；质量保护行为必须保持面向目标项目、
基于证据、按风险分级、成本可控并可审计。

## 验证

按触及文件选择最小本地预检。完整构建和大范围测试默认由 CI 保护；只有改动直接影响构建、
打包，或 CI 无法覆盖对应路径时，才在本地运行更重的命令。

| 改动类型 | 最低验证要求 |
|---|---|
| 不涉及 i18n 资源/契约的前端 UI、状态或适配层 | `pnpm run type-check:web`；行为变化时再加最近的 focused test |
| 仅 locale 资源改动 | `pnpm run i18n:audit` |
| Locale contract 或 shared terms | `pnpm run i18n:generate && pnpm run i18n:contract:test && pnpm run i18n:audit` |
| Web UI i18n runtime、namespace loading 或直接 `i18nService.t(...)` 调用 | `pnpm run i18n:contract:test && pnpm run type-check:web && pnpm --dir src/web-ui run test:run src/infrastructure/i18n/core/I18nService.test.ts` |
| Mobile web UI、状态、配对、断开或重连行为 | `pnpm --dir src/mobile-web run type-check`；行为变化还需要在 PR 中说明手动配对 / 重连验证 |
| `core`、`transport`、`api-layer` 或共享服务中的 Rust 逻辑 | `cargo check --workspace`；行为变化时再加最近的 focused `cargo test` |
| 桌面端集成、Tauri API、browser/computer-use 或桌面专属行为 | `cargo check -p bitfun-desktop`；行为变化时再加 focused desktop tests |
| 被桌面端 smoke/functional 流覆盖的行为 | 优先运行最近的 focused E2E/smoke check；除非改动影响构建，否则 broad build/test 交给 CI |
| `src/crates/adapters/ai-adapters` | 运行上面相关 Rust 检查；只有 stream contract 改动时再加 `cargo test -p bitfun-agent-stream` |
| 不涉及打包的安装器前端或 i18n runtime | `pnpm --dir BitFun-Installer run type-check` |
| 安装器 Tauri/Rust 改动 | `cargo check --manifest-path BitFun-Installer/src-tauri/Cargo.toml` |
| 安装器打包、payload、安装/卸载流程或 native bundling | `pnpm run installer:build` |

## Agent 文档优先级

进入具体目录后，优先遵循离目标文件最近的 `AGENTS.md` / `AGENTS-CN.md`。如果局部文档与本文件冲突，以更具体、更近的文档为准。

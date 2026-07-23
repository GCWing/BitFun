# ADE-Spec：需求开发文档与指导流程

> 范围：BitFun 仓库中用于「需求开发」的文档工作夹（spec / design / plan / 收尾记录）。
> 用途：作为 `docs/ade-spec/` 的入口文档。说明这里存放什么文档、如何命名、以及一条
> 从需求到交付的可执行开发流程。具体文档只回答局部问题，本文件只回答流程与边界。

`ade-spec` 与现有文档夹的分工：

| 文档夹 | 角色 | 与 ade-spec 的关系 |
|---|---|---|
| `docs/architecture` | 稳定架构边界与设计 | ade-spec 的设计稿在稳定后迁入或链接回此处 |
| `docs/plans` | 仓库级实施计划 | 跨模块、长周期的正式计划放此处；ade-spec 侧重单需求 |
| `docs/features` | 单特性设计 | 与 ade-spec 设计稿重叠时以 features 为权威 |
| `docs/sdlc-harness` | 质量治理与证据 | 需要阶段门禁/证据时引用此处契约 |
| `docs/superpowers/specs` | 子代理执行级 spec | 已落地的执行 spec；ade-spec 可作为其上游 |

## 文档定位

`ade-spec/` 存放**正在开发中的需求文档**：需求 intake、调研、设计稿、实施计划、收尾
记录。文档随需求推进而演进；稳定的设计与契约应迁入对应权威文档夹（architecture /
features / plans），避免在 ade-spec 形成第二套权威源。

文档成熟度用文件名前缀表达：

- `draft-<topic>.md`：仍在讨论，结论未定，可被推翻。
- `<YYYY-MM-DD>-<topic>.md`：已确认进入实施，结论可被引用。
- `completed-<topic>.md`：需求已交付，仅作存档与回溯。

`<topic>` 用 kebab-case 英文短词，便于跨文档引用与检索。中文正文允许，文件名保持英文。

## 开发指导流程

流程为阶段递进，每阶段有明确产物与退出条件。低风险小改动可跳过非必要阶段（见
「最小流程」），但安全/凭据/网络/数据迁移/发布相关需求必须走全流程。

```text
0 Intake → 1 调研与边界 → 2 设计 → 3 计划 → 4 实现 → 5 验证 → 6 收尾
```

### 阶段 0：需求登记（Intake）

产物：`draft-<topic>.md`，含以下最小字段：

- 背景：为什么做、谁需要、当前痛点。
- 目标：一句话产品目标，明确做什么、不做什么。
- 范围：纳入项与显式排除项（避免范围蔓延）。
- 涉及面：粗略点出受影响的层（interfaces / assembly / adapters / services /
  execution / contracts）与产品面（desktop / cli / web-ui / mobile-web /
  installer）。
- 风险初判：是否触碰安全边界、凭据、网络、数据迁移、发布、远程兼容、i18n、主题。

退出条件：目标与范围可被一句话复述，且已点出明显风险面。

### 阶段 1：调研与边界确认

产物：在 `draft-<topic>.md` 增补「调研结论」「根因」「边界」三节。

必读（按受影响层取最近 `AGENTS.md`）：

- 仓库根 [`AGENTS.md`](../../AGENTS.md) 的「分层模块索引」「边界规则」「平台边界」
  「远程兼容」「Agent loop 行为」。
- 受影响层的最近 `AGENTS.md`（如 `src/crates/services/AGENTS.md`）。
- 架构敏感改动必读 [`docs/architecture/product-architecture.md`](../architecture/product-architecture.md)。
- CLI/TUI 改动必读 `docs/architecture/cli-product-line-design.md` 与
  `src/apps/cli/AGENTS.md`。
- HarmonyOS PC 目标改动另读 `docs/architecture/platform-portability-design.md`。
- SDLC/证据/门禁改动先读 `docs/sdlc-harness/README.md` 再读 `design.md`。

边界确认要点：

- 命中哪一层？层间依赖是否向上？是否把具体适配/OS/服务细节误放到 assembly/contracts？
- 是否在 shared core 引入 `tauri::AppHandle` 等宿主 API？应走抽象端口。
- 是否引入新 Tauri command？命名 `snake_case`，TS 用 `camelCase` 包装但以
  `request` 结构体调用，并在
  `src/apps/desktop/src/api/remote_workspace_policy.rs` 声明远程工作区策略。
- 是否影响远程工作区/远程控制同步？不能支持时需显式 gate 或给出清晰不可用提示。

退出条件：根因可解释、层归属明确、安全/远程/i18n/主题风险已显式列出。

### 阶段 2：设计（Design）

产物：升级为 `<YYYY-MM-DD>-<topic>.md`，新增「方案」「状态模型」「远程兼容」
「i18n 与主题」「测试方法」节。

建议结构（可裁剪）：

- 方案：已确认结论，含数据流、命令/响应 serde tag、端口与实现归属。
- 状态模型：用表格列状态、含义、owner；状态间独立性必须写清。
- 远程兼容：该能力是本地 shell 还是远程？是否新增网络/SSH/agent-loop 往返？
- i18n：新增 locale key 所在 namespace 与共享 term；不跨产品面引用 Web UI 资源。
- 主题：是否新增颜色/token？优先复用语义/组件/领域 token；新 token 需写 owner 契约。
- 安全：执行位置、沙箱等级、副作用、授权范围；未知能力默认受限。
- 测试方法：单元/契约/聚焦 E2E；列出最近的聚焦测试路径。

退出条件：方案被至少一名 owner 确认，且与权威架构无冲突。

### 阶段 3：实施计划（Plan）

产物：在 spec 内追加「实施计划」节，或单独 `plan-<topic>.md`。

采用 checkbox 语法以便子代理逐条执行（与 `docs/plans` 一致）：

```markdown
## 实施计划

### Milestone 1：<切片名>

Risk: <Low/Medium/High>。<一句理由>。

- [ ] <任务>。Risk: <Low/Medium/High>。
- [ ] <任务>。Risk: <Low/Medium/High>。
```

每个任务应可独立验证、可独立回滚；高风险任务单独成条并写明回滚/兜底。

退出条件：任务列表可被第三人/子代理在不追问的情况下执行。

### 阶段 4：实现（Implementation）

遵守仓库全局准则（节选自根 `AGENTS.md`，详见原文）：

- 不写注释，除非被要求。
- 日志仅英文，无 emoji；前端见 `src/web-ui/LOGGING.md`，后端见
  `src/crates/LOGGING.md`。
- Tauri command：`snake_case`，以结构化 `request` 调用；不在 UI 组件直接调用
  Tauri API，走 adapter/infrastructure 层。
- 平台边界：桌面宿主适配放 `src/apps/desktop`，shared core 不用宿主 API。
- 远程兼容：每个新增 desktop Tauri command 必须在
  `remote_workspace_policy.rs` 声明策略，否则契约测试拒绝。
- i18n：改 `locales.json` 后跑 `pnpm run i18n:generate`；不在 mobile-web /
  installer 引用 Web UI 资源。
- 主题：不靠抬高 baseline、放宽 allowlist 或删审计让审计通过；新 token 需 owner 契约。
- Agent loop：不通过字符串/计数硬编码来抑制循环；先查根因。

### 阶段 5：验证（Verification）

按根 `AGENTS.md` 的「验证」表选择**最小可覆盖本次改动**的本地预检；CI 负责完整
build 与宽测试套。常用项：

| 改动类型 | 最小验证 |
|---|---|
| 前端 UI/状态/适配（无 i18n 资源/契约改动） | `pnpm run type-check:web` + 最近聚焦测试 |
| 仅 locale 资源 | `pnpm run i18n:audit` |
| locale 契约或共享 term | `pnpm run i18n:generate && pnpm run i18n:contract:test && pnpm run i18n:audit` |
| 共享 Rust（core/transport/adapters/services） | `cargo check --workspace` + 最近聚焦 `cargo test` |
| 桌面集成/Tauri/浏览器-计算机使用 | `cargo check -p bitfun-desktop` + 聚焦桌面测试 |
| `ai-adapters` 流契约改动 | 加 `cargo test -p bitfun-agent-stream` |
| 安装包前端/i18n（无打包改动） | `pnpm --dir BitFun-Installer run type-check` |
| 安装包 Tauri/Rust | `cargo check --manifest-path BitFun-Installer/src-tauri/Cargo.toml` |

格式与仓库卫生：改 Rust 后优先 `pnpm run fmt:rs`；UI 改动跑 `pnpm run lint:web`。

退出条件：对应最小验证全绿；新行为有聚焦测试覆盖。

### 阶段 6：收尾（Closeout）

产物：文件名升级为 `completed-<topic>.md`，追加「结果」「回溯」「遗留」节。

- 结果：交付了什么、对应 spec 哪些任务。
- 回溯：稳定设计是否已迁入 architecture/features/plans；ade-spec 不留第二权威源。
- 遗留：未做项、后续需求入口、已知风险。

退出条件：权威文档已更新或显式标注「无需迁移」，ade-spec 文档归档。

## 最小流程（低风险小改动）

不触碰安全/凭据/网络/数据迁移/发布/远程/i18n/主题的改动可只走：

1. 在 spec 中记一句话目标与范围。
2. 跳过独立设计稿，直接在任务条目里写方案。
3. 实现 → 对应最小验证 → 收尾一句话。

但仍须遵守「实现」与「验证」两节的仓库准则。

## 模板速查

### Spec 模板

```markdown
# <Title>

Date: <YYYY-MM-DD>
Scope: <受影响路径与层>

## 背景

## 目标

## 范围
- 纳入：
- 排除：

## 调研结论 / 根因

## 方案（已确认）

## 状态模型
| 状态 | 含义 | owner |
|---|---|---|

## 远程兼容

## i18n 与主题

## 安全

## 测试方法

## 实施计划
### Milestone 1：<切片名>
Risk: <L/M/H>。<理由>。
- [ ] <任务>。Risk: <L/M/H>。
```

### Plan 模板（独立计划文件）

```markdown
# <Topic> Implementation Plan

**Goal:** <一句话产品目标>。
**Architecture:** <层归属与边界>。
**Tech Stack:** <框架/库/测试工具>。

## Milestone 1：<切片名>
Risk: <L/M/H>。<理由>。
- [ ] <任务>。Risk: <L/M/H>。
```

## 与权威文档的对齐规则

- 本夹文档为**开发期工作件**，不是权威源。稳定结论须迁入对应权威文档夹。
- 与最近 `AGENTS.md` 冲突时，以更近、更具体的 `AGENTS.md` 为准。
- 与 `docs/architecture` 冲突时，以 architecture 为准；如需偏离，在 spec 内写明理由。

# Specs：需求规格与设计

> 范围：BitFun 代码仓内的 Spec / Design（需求与设计，what & why），以及为其提供依据的长期调研与技术审阅。
> 用途：本目录入口——索引、命名、流程与边界。细则写在各文档与 [`templates/`](templates/)。
> 状态：stable
> 权威语言：中文（本文件）。英文操作约束见根 `AGENTS` 与 [`docs/guideline/`](../guideline/)。
> 文档规范：[`docs/guideline/docs-governance.zh-CN.md`](../guideline/docs-governance.zh-CN.md)

## 与其它文档夹的分工

| 文档夹 | 角色 | 与 specs 的关系 |
|---|---|---|
| `docs/architecture` | 稳定架构边界与设计 | 设计稳定后迁入或链回此处；冲突时以 architecture 为准 |
| `docs/plans/` | 实施计划与收尾（how & when） | 独立计划；单需求计划也可写在同篇 Spec 内 |
| `docs/specs/`（根下单篇） | 进行中需求 + 已稳定单特性设计 + 非规范参考材料 | 用索引**状态列**区分，不拆 stable/wip 子目录；reference 不拥有规范权威 |

稳定结论须迁入 architecture 或在本索引标为 `stable`/`completed`，避免第二套权威源。

## 命名

- `draft-<topic>.md`：讨论中，可推翻
- `<YYYY-MM-DD>-<topic>.md`：已确认可实施
- `<YYYY-MM-DD>-<topic>-design.md`：独立设计稿；已有日期 Spec 可保留无 `-design` 后缀
- `<YYYY-MM-DD>-<topic>-review.md` / `<topic>-research.md`：非规范 reference；正文必须写明证据日期与复核要求
- 文件名英文 kebab-case；正文可用中文
- 双语对使用 `<name>.md` 与 `<name>.zh-CN.md`；根入口特殊命名不在本目录复用

## 索引

| Topic | Status | Path | One-liner |
|---|---|---|---|
| Runtime performance review | reference | [`2026-07-26-runtime-performance-review.md`](2026-07-26-runtime-performance-review.md) | Rust / React / IPC 热点审阅快照；实施前复核代码 |
| Web UI interaction fluency review | reference | [`2026-07-26-interaction-fluency-review.md`](2026-07-26-interaction-fluency-review.md) | Web UI 响应性审阅快照；实施前复核代码 |
| Startup and bundle-size review | reference | [`2026-07-26-startup-and-bundle-size-review.md`](2026-07-26-startup-and-bundle-size-review.md) | 冷启动与产物体积审阅快照；实施前复核代码和产物 |
| Session runtime usage report | stable | [`session-runtime-usage-report-design.md`](session-runtime-usage-report-design.md) | `/usage` 与桌面会话用量报告 |
| Remote workspaces | stable | [`remote-workspaces.md`](remote-workspaces.md) | 远程工作区能力说明 |
| CLI peer host | completed | [`2026-07-14-cli-peer-host.md`](2026-07-14-cli-peer-host.md) / [`../plans/2026-07-14-cli-peer-host-plan.md`](../plans/2026-07-14-cli-peer-host-plan.md) | CLI 作为 Peer Device Mode host |
| CLI login TUI | in-progress | [`2026-07-14-cli-login-tui-design.md`](2026-07-14-cli-login-tui-design.md) / [`../plans/2026-07-14-cli-login-tui-plan.md`](../plans/2026-07-14-cli-login-tui-plan.md) | CLI ratatui 登录页 |
| CLI account sync panel | stable | [`2026-07-14-cli-account-sync-panel-design.md`](2026-07-14-cli-account-sync-panel-design.md) | CLI 账号同步面板 |
| Mobile-web login / device switch | stable | [`2026-07-16-mobile-web-login-device-switch-design.md`](2026-07-16-mobile-web-login-device-switch-design.md) | Mobile-web 账号接续与设备切换 |
| Relay deploy log / docker cache | stable | [`2026-07-19-relay-deploy-log-and-docker-cache-design.md`](2026-07-19-relay-deploy-log-and-docker-cache-design.md) | Relay 部署日志与 Docker 缓存 |
| Desktop streaming typewriter | stable | [`2026-07-20-desktop-streaming-typewriter-design.md`](2026-07-20-desktop-streaming-typewriter-design.md) | 桌面流式打字机效果 |
| Mobile-web pairing / device refresh | stable | [`2026-07-20-mobile-web-account-pairing-and-device-refresh-design.md`](2026-07-20-mobile-web-account-pairing-and-device-refresh-design.md) | Mobile-web 配对与设备刷新 |
| Subscription auth as model API | in-progress | [`2026-07-21-subscription-auth-as-model-api-design.md`](2026-07-21-subscription-auth-as-model-api-design.md) / [`../plans/2026-07-21-subscription-auth-as-model-api-plan.md`](../plans/2026-07-21-subscription-auth-as-model-api-plan.md) | 订阅鉴权作为模型 API |
| Cargo target latest-only GC | in-progress | [`2026-07-22-cargo-target-latest-only-gc-design.md`](2026-07-22-cargo-target-latest-only-gc-design.md) / [`../plans/2026-07-22-cargo-target-latest-only-gc-plan.md`](../plans/2026-07-22-cargo-target-latest-only-gc-plan.md) | Cargo target 仅保留最新 GC |
| Mobile-web resume / autoreconnect | stable | [`2026-07-22-mobile-web-resume-card-and-autoreconnect-design.md`](2026-07-22-mobile-web-resume-card-and-autoreconnect-design.md) | Mobile-web 恢复卡片与自动重连 |
| Relay deploy CN mirrors | stable | [`2026-07-24-relay-deploy-cn-mirrors-design.md`](2026-07-24-relay-deploy-cn-mirrors-design.md) | Relay 部署国内镜像 |
| FlowChat collapse smoothness | stable | [`2026-07-28-flowchat-collapse-smoothness-design.md`](2026-07-28-flowchat-collapse-smoothness-design.md) | FlowChat 折叠动画流畅性 |
| Agent hooks | stable | [`agent-hooks.md`](agent-hooks.md) / [`agent-hooks.zh-CN.md`](agent-hooks.zh-CN.md) | Agent Hooks 能力说明 |
| SDLC governance external research | reference | [`sdlc-governance-external-research.md`](sdlc-governance-external-research.md) | 外部产品、论文与标准证据池；引用前复核 |
| SDLC governance product requirements | stable | [`sdlc-governance-product-requirements.md`](sdlc-governance-product-requirements.md) | SDLC 治理产品需求 |
| SDLC governance agent workflow adjustment | draft | [`sdlc-governance-agent-workflow-adjustment.md`](sdlc-governance-agent-workflow-adjustment.md) | 智能体工作流体验调整提案 |
| SDLC governance traceability matrix | stable | [`sdlc-governance-traceability-matrix.md`](sdlc-governance-traceability-matrix.md) | 需求-设计-阶段覆盖矩阵 |
| SDLC governance metrics spec | stable | [`sdlc-governance-metrics-spec.md`](sdlc-governance-metrics-spec.md) | 看护指标规格 |
| Configurable policy profile | stable | [`configurable-policy-profile.md`](configurable-policy-profile.md) | 配置化策略画像 |
| Risk classifier | stable | [`risk-classifier.md`](risk-classifier.md) | 风险与策略分类器 |
| PR quality gate | stable | [`pr-quality-gate.md`](pr-quality-gate.md) | 变更就绪度与 PR 门禁 |
| Requirement impact analysis | stable | [`requirement-impact-analysis.md`](requirement-impact-analysis.md) | 需求影响分析 |
| Agent evaluation | stable | [`agent-evaluation.md`](agent-evaluation.md) | 智能体评测 |
| OpenCode compatibility (SDLC) | stable | [`opencode-compatibility-sdlc.md`](opencode-compatibility-sdlc.md) | SDLC 场景下 OpenCode 兼容 |

状态含义：`draft` / `in-progress` = 可改；`stable` = 已交付仍约束实现；`completed` = 仅存档；`reference` = 非规范调研或审阅材料，使用前必须复核。

## 开发指导流程

```text
0 Intake → 1 调研与边界 → 2 设计 → 3 计划 → 4 实现 → 5 验证 → 6 收尾
```

低风险小改动可走文末「最小流程」。安全/凭据/网络/数据迁移/发布相关必须走全流程。

写新文档时拷贝 [`templates/`](templates/)。

### 阶段 0：需求登记（Intake）

产物：`draft-<topic>.md`（可用 [`templates/feature-spec.md`](templates/feature-spec.md)）。

最小字段：背景、目标、范围（含不做）、涉及面（层与产品面），以及逐项风险扫描：安全、凭据/隐私、网络/外部系统、
数据或状态迁移、发布/打包、远程/多宿主、i18n、主题/交互。不适用项必须写 `N/A` 和理由，不能留空后默认无风险。

退出：目标与范围可一句话复述，且已点出明显风险面。

### 阶段 1：调研与边界确认

在 draft 中增补调研结论、根因、边界。

必读（按受影响层取最近 `AGENTS.md`）：

- 仓库根 [`AGENTS.md`](../../AGENTS.md)
- 受影响层最近 `AGENTS.md`
- 架构敏感：[`docs/architecture/product-architecture.md`](../architecture/product-architecture.md)
- CLI/TUI：`docs/architecture/cli-product-line-design.md`、`src/apps/cli/AGENTS.md`
- HarmonyOS PC 目标：`docs/architecture/platform-portability-design.md`
- SDLC/证据：[`../architecture/sdlc-governance-architecture.md`](../architecture/sdlc-governance-architecture.md)

退出：根因可解释、层归属明确、安全/远程/i18n/主题风险已列出。

### 阶段 2：设计（Design）

产物：升级为 `<YYYY-MM-DD>-<topic>.md`，或补充 [`templates/design.md`](templates/design.md)。

建议含：方案（数据流、命令/响应、端口归属）、状态模型、迁移与兼容、远程兼容、失败/取消/部分成功、
i18n 与主题、安全、发布与 rollout、回滚、测试方法。

退出：方案已确认，且与 `docs/architecture` 无冲突（或已在文中写明经批准的偏离）。

### 阶段 3：实施计划（Plan）

产物：同篇「实施计划」节，或 [`../plans/templates/plan.md`](../plans/templates/plan.md) / `../plans/` 下独立文件。

```markdown
### Milestone 1：<切片名>
Risk: <Low/Medium/High>。<一句理由>。
#### Task 1：<可独立交付的任务>
- [ ] Change: <文件 / 行为 / 边界>。
- Risk: <Low/Medium/High>。<一句理由>。
- Verify: <聚焦命令或评审证据>。
- Rollback: <如何撤销或回退；仅在写明理由时可用 N/A>。
```

每个任务都必须独立验证、独立回滚；该要求不只适用于高风险任务。跨任务迁移顺序、兼容 fallback 和不可逆步骤
另写总体 Rollback。验证链 [`docs/guideline/verification.md`](../guideline/verification.md)。

退出：第三人/子代理可不追问执行。

### 阶段 4：实现（Implementation）

遵守现网根 `AGENTS.md` 与就近模块 `AGENTS.md`。不要把全局准则整段复制进每篇 Spec。

### 阶段 5：验证（Verification）

按 [`docs/guideline/verification.md`](../guideline/verification.md)（中文：[`verification.zh-CN.md`](../guideline/verification.zh-CN.md)）选**最小**本地预检；CI 负责宽套件。

### 阶段 6：收尾（Closeout）

产物：`<topic>-completed.md`（或索引状态改为 `completed`），追加结果、权威文档迁移、遗留。

退出：权威已更新或显式「无需迁移」；本夹不留第二权威源。

## 最小流程（低风险）

不触碰安全/凭据/网络/数据迁移/发布/远程/i18n/主题时：

1. 一句话目标与范围
2. 跳过独立设计稿，方案写在任务行
3. 任务仍写清验证与回滚（无状态改动可写明 `N/A` 理由）
4. 实现 → 对应最小验证 → 一句话收尾

## 对齐规则

- 本夹文档为开发期工作件（`stable` 设计除外仍约束实现，但架构权威在 `architecture/`）。
- 与更近 `AGENTS.md` 冲突 → 更近为准。
- 与 `docs/architecture` 冲突 → architecture 为准；偏离须写在 Spec/Design 内。

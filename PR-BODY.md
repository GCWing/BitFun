# feat: Ultra 模式实战 + Vibe Trading + 自媒体视频工厂全链路 — 太极量化交易系统

> **AI 辅助产出** | 测试程度：**已测**（400+ 单元测试 + 5 集成测试 + cargo check/clippy/audit 全绿）
>
> 因改动量大（272 文件、43,355 行），直接提交 PR 供评审。全部代码由 Vibe Coding 在两天内完成，零手工编写。

---

## 概述

三个方向，一次交付：

1. **Ultra 模式实战** — 自主多 Agent 编排 + 质量门闭环，听说官方在规划，我已实现并实战验证
2. **Vibe Trading** — GitHub 涨薪最快前十热点，AI 量化交易的蓝海落地
3. **自媒体视频工厂** — K 线 → FFmpeg 渲染 → B 站 / Twitter / 微信公众号全自动分发

---

## 与官方重点贡献方向的对齐

本 PR 覆盖 [CONTRIBUTING_CN.md](https://github.com/GCWing/BitFun/blob/main/CONTRIBUTING_CN.md) 四项重点方向中的三项：

| 官方方向 | 本 PR |
|---------|-------|
| ② 优化 Agent 系统和效果 | LegionControl 编排引擎、Team Mode 预设、Goal Continuation 修复、BeeColony 监控 UI |
| ③ 提升系统稳定性和完善基础能力 | 4 个上游 Bug 修复、MCP deprecation warning 清零、4 个 taiji CI job、全仓零 warning |
| ④ 扩展生态（垂域开发场景） | Vibe Trading 量化交易垂域完整落地；master-framework 方法论技能开源 |

---

## 变更详情

### Tools（新增工具）

**`legion_control_tool.rs`**（568 行）— 军团编排引擎

```
JSON 模板 → Kahn 拓扑排序 → 分层创建 Session → 并行/串行派发 → Gate Loop 质量门
```

这是 Ultra 模式的核心基础设施。已通过 28 R-ID、Phase 1-10 全流程实战验证。

### 模式贡献（Mode）

**Team Mode 增强**

- `team_presets.rs`（137 行）— 预设军团模板：8 节点全链 / 4 节点精简 / 12 节点并行
- `team_mode.md` — 提示词重写，支持 LegionControl 编排指令
- Goal Continuation 修复（`scheduler.rs`）— 双条件检测：队列空 + 同工作区无活跃 Session，防止异步 SessionMessage 未归队时误触发
- Agent Runtime 增强（`scheduler.rs`）— `ActiveDialogTurnStore::is_empty()` 方法

**Ultra 模式能力对照**

| 能力 | 实现 | 状态 |
|------|------|------|
| 军团编排引擎 | `LegionControl`：拓扑排序 → 分层 Session → 并行派发 | 已实战 |
| 异步任务派发 | `SessionMessage`：真正的异步推送，零轮询 | 已实战 |
| 质量门闭环 | Gate Loop Protocol：Dispatch → Collect → Inspect → PASS/FAIL → Correct | 已实战 |
| 静默检测 | Goal Continuation：双条件防误触发 | 已实战 |
| 容错重组 | 连败 2 次→停→换人；panic 隔离（`catch_unwind`）；数据源断切备源 | 已实战 |
| 拓扑排序 | Kahn 算法自动分层，同层全并行 | 已实战 |

### Subagents（新增子代理）

**ACP Agent 定义**

- `acp_agent.rs`（77 行）— 子代理实现
- `acp_agent.md`（17 行）— 提示词
- ACP 客户端增强：`cli_detect.rs`（26 行）、`launch_policy.rs`（79 行）、`probe.rs`（32 行）、`manager.rs`（207 行）

### 垂域开发场景：Vibe Trading

**Vibe Coding** 是 GitHub 涨薪最快的前十热点——用自然语言驱动 AI 写代码。**Vibe Trading** 是它在量化交易领域的延伸。GitHub 上 `vibe-trading` 相关项目正在爆发，但还没有人用 Vibe Coding 交付完整的工业级量化交易系统。

本 PR 交付了 **太极（Taiji）多智能体量化交易系统**——20 个活跃 Rust crate + 4 个闭源策略 crate（已注释）：

**核心引擎**

| Crate | 功能 |
|-------|------|
| `taiji-engine` | DAG 管线核心：拓扑排序 + StateStore + DataSource + RiskMonitor + SignalRegistry + 辩论/融合引擎 |
| `taiji-bar` | Tick→K 线聚合 |
| `taiji-realtime` | 实时行情：CTP + WebSocket |
| `taiji-executor` | 订单执行 + 持仓追踪 |

**策略与量化**

| Crate | 功能 |
|-------|------|
| `taiji-backtest` | 回测引擎 + Walk-Forward 验证 |
| `taiji-pattern` | DTW 形态识别 + 三层索引 |
| `taiji-abnormal` | 异常检测评分卡（5 指标融合） |
| `taiji-orderflow` | VPIN + OFI（Welford 在线统计） |
| `taiji-sentiment` | 市场情绪（jieba + 恐惧贪婪指数） |
| `taiji-strategen` | LLM 驱动策略生成（假设→验证→编译→回测） |

**AI / LLM**

| Crate | 功能 |
|-------|------|
| `taiji-llm` | 多 Provider 客户端（OpenAI / Claude / DeepSeek / 本地） |
| `taiji-engine-py` | Python 绑定（PyO3）+ RL 强化学习环境 |

**自媒体视频工厂（Vibe Trading 下游链路）**

| Crate | 功能 |
|-------|------|
| `taiji-content` | K 线渲染 + FFmpeg 视频合成 + 定时任务调度 |
| `taiji-publisher` | 多平台发布（B 站 / Twitter / 微信公众号） |
| `taiji-growth` | 报告生成 + 邮件分发 + 网站发布 |
| `taiji-alert` | 多渠道告警（飞书 Webhook / 桌面通知 / 邮件） |
| `taiji-knowledge-graph` | Petgraph 概念/策略/案例关系图谱 |
| `taiji-blog-gen` | 博客自动生成（tera 模板） |

**入口**

| Crate | 功能 |
|-------|------|
| `taiji-cli` | 独立 CLI binary（零 BitFun 桌面依赖） |
| `taiji-example` | 参考策略实现（MaCross 双均线） |

**闭源策略（已注释，不参与编译）**

`taiji-dvmi`（拐点+双线三态）、`taiji-magnet`（磁体定位）、`taiji-thrust`（三推检测）、`taiji-risk`（风控规则：6 种止损/仓位算法）

> 交易策略生成器（`taiji-strategen`）和 LLM 辩论引擎目前是骨架实现，基础测试已通过，会持续维护更新。

### Skills（技能开源）

**`skills/master-framework/SKILL.md`** — master-framework v16.0.0（三省三域方法论）

- 12 条铁则（每条绑定 公司/军事/航天 三域）
- 10 棵决策树（感知→认知→决策→规划→分解→派发→观察→思考→决策→执行）
- 8 个递归 Agent 角色
- 8 步标准 Phase 工作流（侦察→三文档规划→深度审计→拓扑派发→审查→终审→学习沉淀）
- 37 条实战经验（Phase 1-10 全流程提取）

可被任何 BitFun Agent 加载使用。

### 场景指南

**`docs/methodology/web-coding-workflow.md`** — Vibe Coding 自动化长任务工作流：从 Phase 1 到 Phase 10 的完整实战记录。

关键数据：
- Phase 1-3：Python→Rust 移植五步法，34 R-ID，59 测试
- Phase 4：36 替补 Session 并行侦察 7 大技术领域
- Phase 5：三文档并行拆分，跨文档审计发现 20 个缺口
- Phase 6-7：24 Session 分批并行执行，R-ID 隐式拓扑编排
- Phase 8：Rebase 零冲突迁移，固化一键脚本 `scripts/migrate-taiji.ps1`
- Phase 9：8 维度全并行安全审查，100+ 告警收敛为 4 条根因
- Phase 10：5 门质量门全绿，GitHub CI 上线

### 上游 Bug 修复

- Edit tool token amplification + hang on large files with stale read-state cache → 编辑安全增强，限制 whitespace-normalization 候选生成 + 优化 dry-run 路径（Closes #1650）
- DAG 重复边导致拓扑排序 panic → `add_edge` 去重 + `debug_assert` + 单元测试（Closes #1675）
- Pipeline node panic 传播 → `catch_unwind(AssertUnwindSafe(...))` 隔离（Closes #1676）
- MCP `services-integrations` 4 个 deprecation warning → `from_byte_stream` → `from_bytes_stream` + 移除 `enable_roots()` / `enable_sampling()`（Closes #1677）

### CI / 文档 / 安全

- 4 个 taiji 专属 CI job：`taiji-cargo-check` / `taiji-cargo-test` / `taiji-clippy` / `taiji-cargo-audit`
- `SECURITY.md` / `SECURITY_CN.md` — 新增 Taiji 模块安全策略（安全边界 + 依赖安全 + 敏感信息处理）
- `CONTRIBUTING.md` / `CONTRIBUTING_CN.md` — 新增 Taiji 模块贡献指南
- `docs/plans/phase9-security-report.md` — Phase 9 安全审查归档报告（8 维度审计，10 项 P0 全部修复）
- BeeColony 监控 UI + 创建页面 + 卡片组件（React）
- i18n：中/英/繁 agents 翻译

---

## 验证

| 门 | 命令 | 结果 |
|----|------|------|
| 编译 | `cargo check --workspace` | 零错误，零 warning |
| 测试 | `cargo test`（19 crate） | 400+ 全部通过 |
| Clippy | `cargo clippy --workspace -- -D warnings` | 零 warning |
| 格式 | `cargo fmt --check --all` | 通过 |
| 安全审查 | Phase 9 报告 | P0 10/10 已修复 |
| 闭源隔离 | 4 crate 已注释，CI 排除 | 通过 |

---

## 许可证与合规

- 新增代码：**MIT**（workspace 级继承）
- 新增依赖：均为 MIT / Apache-2.0 兼容
- 无密钥、Token、证书或敏感信息
- 无临时 AI prompt、本地绝对路径、生成草稿文件
- 闭源 crate（4 个）已注释，不参与编译和 CI

---

## 附：关于作者与合作

我是 B 站"**量价仓交易狮**"，全职专业期货交易员。有一套成熟的量价时空交易理论。**没有代码基础，全靠 Vibe Coding。**

**合作提议：**
1. 官方做中转站，收取 coding token 费用，提供服务器等基础设施服务
2. 我这边的增值服务：交易策略、量化方法论、自媒体自动化产出

如果暂时没有兴趣，希望能授权我独立商业化。后续会持续更新并向官方同步进展。

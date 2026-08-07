# LoopX 持续 Issue 修复集成架构

> 状态：Implemented；描述 `feat/loopx-issue-fix` 分支落地的实际架构。
>
> 基线日期：2026-08-06。
>
> 本文记录 BitFun 如何以 host 身份嵌入 LoopX State Kernel 实现持续 issue 修复，
> 以及两侧的边界契约。LoopX 自身的设计哲学以其仓库的
> [state-kernel-domain-state-case-study](https://github.com/huangruiteng/loopx/blob/main/docs/capabilities/issue-fix/state-kernel-domain-state-case-study.zh-CN.md)
> 为权威；本文只记录 BitFun 侧的接入决策与理由。相关代码：
> `src/crates/services/services-integrations/src/loopx_issue_fix/`、
> `src/apps/desktop/src/api/issue_fix_api.rs`、
> `src/web-ui/src/app/components/panels/issue-fix/`。

## 1. 分工总纲

LoopX 是一个 State Kernel：它不写代码、不调模型，只管理控制面事实
（goal、todo、认领/租约、authority、user gate、配额、monitor、运行历史）。
BitFun 作为 host 补齐它缺的三样东西：

| Host 职责 | BitFun 实现 |
| --- | --- |
| 编码能力 | 普通 Agent 会话（真正读代码、打补丁、跑验证、发 PR） |
| 心跳 | 持久 Cron 服务，每 10 分钟向该会话注入一次心跳 prompt |
| 人机界面 | Issue-Fix 面板（只读投影 + 类型化命令）与通知中心 |

一句话分层：**Kernel 管控制权，Domain State 管领域连续性，Capability 管翻译，
host 管执行与人机界面；任何 projection 都只负责展示。**

## 2. 边界契约（解耦的硬规则）

- BitFun 与 LoopX 的唯一交互通道是 `loopx --format json <命令>` 子进程调用
  （`LoopxIssueFix::json_in`）。不 import、不嵌入、不修改 LoopX 源码。
- BitFun 对 LoopX 内部文件的唯一直接读取是 `.loopx/registry.json` 的身份两字段
  （goal id、registered agent）。不解析 `ACTIVE_GOAL_STATE.md`，不读领域账本。
- BitFun 不持久化任何 issue 队列或第二状态机。面板每次刷新从
  `todo list`（+按需 `quota should-run`）重建视图；唯一的本地状态是
  未提交的复选框选择。
- Windows 编码兼容（LoopX subprocess 按局部编码解码 UTF-8 输出）用
  `PYTHONUTF8=1` 环境变量在 host 侧解决——host 适配置于 host，不打补丁。
- **通用性原则**：host 代码与心跳 preamble 必须对任意仓库成立。仓库特定政策
  （工具链、验证命令、路径边界）写入该 goal 的 active state / registry，
  由 agent 运行中读写——这是 LoopX 契约的原文要求
  （"Keep project-specific branching out of the automation prompt"）。

## 3. 状态所有权

| 状态 | 所有者 | 位置 |
| --- | --- | --- |
| issue todo、gate、monitor、配额 | LoopX Kernel | `.loopx/` + `.codex/goals/<goal>/` |
| feasibility / PR lifecycle 观察 | LoopX Domain State | 领域账本（含指纹与 receipt） |
| issue/PR 的真实状态 | GitHub | 外部权威事实源 |
| 心跳调度（cron job、间隔、prompt 快照） | BitFun | `%APPDATA%/bitfun/data/cron/jobs.json` |
| 面板展示 | BitFun（纯投影） | 内存，每次从 Kernel 重建 |

推论：删除 host 会话、应用崩溃、清空 BitFun 缓存都不丢修复进度——
重新 Start 即从 Kernel 断点继续。GitHub 上的人工操作（merge、close）无需
通知 LoopX：下一拍 monitor 回读后自动完成终局收敛。

## 4. 心跳设计

- **注入内容每拍相同**：BitFun host preamble（约 3.4KB，
  `HEARTBEAT_HOST_PREAMBLE`）+ `loopx heartbeat-prompt --compact` 生成的
  生命周期契约（约 6.3KB）。唯一逐拍变化是 cron 服务前置的当前时间行。
  这是特性而非省事：prompt 是无状态调度契约，一切可变状态由 agent 醒来后
  从 CLI 现场读取，避免状态出现第二来源。
- **选 `--compact` 不选 `--thin`**：thin 档委托给 BitFun 会话中不存在的
  LoopX skill pack；compact 档把完整 should_run 生命周期内联。
- **prompt 快照的刷新点**：Start 与每次 gate 应答时重新生成，
  使 LoopX 升级后的契约漂移在自然写入点跟上。
- **单会话续聊**：所有心跳进同一会话（偏离了案例研究的"每拍新会话"理想形态），
  以 preamble 的"Kernel 唯一真源、无视对话早前结论" + autoCompact 缓解。
  权衡理由：每拍新会话会使会话列表无限增长。
- **单飞**：上一拍未结束时新拍合并跳过；应用重启后 job 从 jobs.json 恢复。

### Preamble 各规则的事故出处

preamble 中每条规则都对应一次真实事故或明确需求，修改前先理解出处：

| 规则 | 出处 |
| --- | --- |
| Kernel 唯一真源，无视对话早前结论 | 单会话续聊的漂移风险 |
| 禁止杀死非本回合启动的进程 | agent 清理"陈旧" cargo 进程时把宿主 BitFun 杀掉（两次） |
| fix 分支基远端默认分支，不基 HEAD | 三个 PR 各带上 8000 行特性分支改动 |
| worktree 及构建缓存 terminal closeout 时回收 | worktree target 目录累计 56GB |
| gate 必须 `--unblocks-todo-id` 关联被阻塞 todo | 未关联 gate 在面板上不可见，循环静默空转 |
| 用户车道 todo 文本单行紧凑格式 | 通知中心密度反馈（草稿全文曾被塞进 todo 文本） |
| 仓库政策归 active state | 通用性要求（preamble 曾漂移出 cargo 专有指引） |

## 5. LoopX CLI 调用面与写语义

| 命令 | 调用方 | 写语义 |
| --- | --- | --- |
| `todo list` | 面板 30s 轮询 | **零写入**（dry_run），轮询安全 |
| `quota should-run` | agent 每拍 + 面板手动刷新 | **每次调用追加一条 rollout event**，禁止用于轮询 |
| `heartbeat-prompt --compact` | Start / gate 应答 | 纯生成，无副作用 |
| `todo add`（intake） | Start | 写入；按文本对非 done 项去重，幂等 |
| `todo complete --decision-outcome` | gate 卡片提交 | 写入；仅 approve 消耗 authority |
| `bootstrap` + `register-agent` | 首次 Start 自动执行 | 创建 goal 与 agent lane，幂等入口 |

已知投影限制：`todo list` 只返回活跃窗口，早期已 done 的 intake todo 会被
LoopX 归档出列表，面板对应行退回未选中外观。该行为随关联 PR 被 merge
（issue 关闭、行消失）自愈；根治需 LoopX 暴露 outcome collection 查询（上游事项）。

## 6. 分发与就绪

- **sidecar**：`scripts/prepare-loopx-resource.mjs --build` 用 PyInstaller 打出
  单文件 loopx（零运行时依赖），置于 `resources/loopx/`；打包脚本存在即捆绑。
  运行时解析顺序：`LOOPX_BIN` 显式覆盖 → 捆绑 sidecar → PATH。
  捆绑的核心价值是**契约钉版**：每个 BitFun 版本对应一个验证过的 LoopX 版本。
- **三项体检**：`issue_fix_probe` 报告 loopx（含来源）、gh 安装、gh 登录三项；
  面板对缺失项显示针对性引导。gh 不捆绑（授权状态属于用户）。
- **首次使用**：仓库无 `.loopx` 是正常态（面板显示"尚未连接"而非报错）；
  首次 Start 自动 bootstrap（BitFun 的持续修复 objective、controller 角色）
  并注册 `bitfun-cron` agent lane。
- **平台范围**：当前 GitHub-only（issue 枚举、gh 证据链、PR lifecycle monitor
  均为 GitHub 形态）；非 GitHub 仓库面板显示明确的不支持说明，不发请求。

## 7. 交互不变式

- 面板是只读投影 + 两个类型化写命令（Start intake、gate 决策）。
  聊天回复永远不构成授权；authority 只经 `todo complete` 写入 Kernel。
- 所有 setControl 路径受单调 ticket 保护（慢响应不能复活已答复的 gate）；
  mutation 进行中暂停轮询。
- Stop 只禁用 host 调度（连带清理孤儿/重复 job），不动 Kernel 状态。
- 删除承载 cron job 的会话会先弹确认（说明任务将停止、进度保留）。
- 新 gate / 待审项经通知中心投递（toast + 历史 + 未读角标）；
  一拍产出超过 3 条时合并为摘要卡。

## 8. 已知偏离与上游事项

| 事项 | 状态 |
| --- | --- |
| 单会话续聊 vs 每拍新会话 | 有意偏离，见 §4 |
| compact 契约超 LoopX 自身 6200 字符预算（~6.3K） | 信息性，LoopX 不强制 |
| done todo 归档导致面板行状态回退 | 自愈型；根治待上游 outcome collection |
| `retire-global-goal` 对已删目录的 goal 失败 | 上游小缺陷 |
| sidecar 仅在 Windows x64 构建验证 | macOS/Linux 待 CI 矩阵 |
| orchestrator.rs / repository_context.rs 仅测试引用 | 作为 CLI 契约的可执行文档保留 |

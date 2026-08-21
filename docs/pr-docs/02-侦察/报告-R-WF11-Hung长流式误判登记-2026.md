# 遗留登记 — R-WF-11 P2-8：Hung 派生 watchdog 长流式误判

- 日期：2026（worktree 批次 1 修复）
- 工位：task/rwf11-state（worktree: taiji-wt-rwf11）
- 派发：R-WF-11 批次1（P0×1 + P1×3 + P2×2，增量修复）
- 结论：**登记遗留，本批次不实现**（验收断言不含此项；落盘记录即完成）

---

## 现象

`derive_display_state`（src/crates/execution/agent-runtime/src/session_state.rs L84-122）在
`SessionState::Processing` 下用 `last_progress_at` + `DEFAULT_HUNG_TIMEOUT`（600s）判定 Hung。

问题场景：**长流式输出**（一次模型生成持续 > 10 分钟、每 token 间隔 < 600s 但生成总时长超时）期间，
若 `last_progress_at` 只在「轮次/工具调用」粒度刷新、而未随流式 token 增量推进，则会：
1. 会话实际仍在正常流式输出 → 被误判为 Hung（display_state = "hung"）
2. 前端显示卡死红标，用户误以为超时，可能手动取消

## 根因

- `last_progress_at` 的刷新点粒度粗（processing 态进入时设置一次），流式过程中无 token 级 touch。
- watchdog 超时（`DEFAULT_HUNG_TIMEOUT` = 600s）远小于长任务上限（`max_turns`/模型超时可达小时级）。

## 影响面

- 展示层：七态投影误标 hung；不影响 runtime 状态机（`SessionState::Processing` 不被篡改）。
- 恢复语义：不阻塞任何现有功能；仅显示误导。

## 绕过 / 缓解（现状已具备）

- 前端 `resolveDisplayStateAttention` 对 `SessionDisplayState.HUNG` 不渲染 unread dot（SessionsSection.tsx），
  仅 tooltip 可见，误判视觉冲击有限。
- 若用户在 hung 态下继续交互，新一轮 turn 触发 Processing → `last_progress_at` 重置，误判自愈。

## 修复方向（后续批次候选，不在本批次实现）

1. 流式 token 到达时 touch `last_progress_at`（在 stream processor / round executor 的 chunk 回调里刷新）。
2. 或放宽：`Processing` + 有 `last_active` 活动时不做 hung 判定（改用「无任何活动」而非「无进度标记」）。
3. 或把 `DEFAULT_HUNG_TIMEOUT` 提为可配置（按模型/provider 区分）。

## 验证锚点（修复时参考）

- `session_state.rs::derive_display_state` 单测 `display_state_distinguishes_hung_interrupted_processing`（L253-286）。
- 长流式复现：构造 Processing 超时窗口内持续有 stream event 的 fixture，断言 display_state != hung。

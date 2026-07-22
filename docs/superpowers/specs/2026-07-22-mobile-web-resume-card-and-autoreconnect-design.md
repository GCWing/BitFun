# Mobile-Web Resume Card 错位与重开卡在配对页设计

Date: 2026-07-22  
Scope: `src/mobile-web`（`PairingPage.tsx`, `sessions.scss`）；合同测试可落在 `src/web-ui` 既有 mobile source contract 套件

## 背景

1. 会话列表顶部「继续上次会话」卡片文案居中、meta 左对齐，视觉错位。
2. 关闭 mobile-web 后再打开，页面长期停在「正在连接并配对...」转圈，无法进入会话或重试。

## 根因

### A. Resume Card 错位

- 卡片是 `<button>`，浏览器默认 `text-align: center`。
- 同页 `.session-list__create-btn` 已显式 `text-align: left`，resume card 漏写。
- 标签/标题继承居中；`.session-list__resume-meta` 为 flex 仍靠左 → 与截图一致。

### B. 重开卡在配对

`PairingPage` 自动重连 effect 依赖 `attemptPair`。首次挂载会：

1. `setConnectionStatus('pairing')` 并启动 `attemptPair`
2. `setMobileInstallId(...)` 触发重渲染 → `attemptPair` 引用变化（deps 含 `mobileInstallId`）
3. effect 再次执行：再次 `setConnectionStatus('pairing')` + `setError(null)`，但 `autoReconnectAttemptedRef` 已为 true → **不再发起配对**

若第一次 `attemptPair` 已失败并写入 `error`，步骤 3 会把状态打回 `pairing` 且清空错误，表单不显示 → 永久转圈。

快速失败（离线、二维码过期 404、身份校验拒绝）最容易踩中该竞态。

## 方案

### A. Resume Card UI（方案 A：左对齐横向卡片）

对齐 `.session-list__create-btn`：

- `.session-list__resume-card`：`width: 100%`、`text-align: left`、补齐 button 文本色继承
- `.session-list__resume-body`：`display: flex; flex-direction: column`，稳定纵向节奏
- 不改 DOM 结构（图标 | 文案栈 | 箭头）

### B. 自动重连生命周期

1. **挂载一次性 bootstrap**：用 `attemptPairRef` 持有最新 `attemptPair`；bootstrap effect 仅运行一次（或用 `bootstrappedRef` 防重入），不再把 `attemptPair` 放进依赖导致状态回滚。
2. **仅在真正发起配对时设 `pairing`**：bootstrap / 手动连接进入尝试时设置；禁止「空转」地把状态重置为 `pairing`。
3. **去掉 `mobileInstallId` 对 `attemptPair` 的依赖**：installId 已通过 `options.installId` / `getOrCreateInstallId()` 传入。
4. **世代/挂载守卫**：async 返回后若组件已卸载或已被更新一代配对取代，不再写 store / 调 `onPaired`，避免 StrictMode 或重复尝试污染 UI。
5. **失败必须可恢复**：失败后保持 `connectionStatus === 'error'` 并展示表单 + 错误文案，允许手动重试。

## 非目标

- 不在此任务持久化账号密码或扩展账号模式无密码自动重连（既有产品规则：`auth=account` 禁止无密码自动重连）。
- 不把 pairing target（room/pk）迁入 localStorage（仍依赖扫码 URL hash）。

## 验证

- `pnpm --dir src/mobile-web run type-check`
- `pnpm run build:mobile-web`
- 合同测试：断言 PairingPage 不再在依赖 `attemptPair` 的 effect 里无条件 `setConnectionStatus('pairing')`；存在 mount-once / ref 引导的自动重连路径
- 手工：
  1. 非账号模式配对成功 → 关页再开：应自动配对进入会话，或失败后显示表单而非永转圈
  2. Resume card：标签/标题/meta 左对齐，与图标同一阅读轴
  3. 账号模式（`auth=account`）：重开仍显示密码表单，不自动转圈

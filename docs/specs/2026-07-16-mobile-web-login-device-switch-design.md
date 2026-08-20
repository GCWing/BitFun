# Mobile-Web 账号接续与设备互联 UI/UX 设计

Date: 2026-07-16
Scope: `src/mobile-web`, `src/crates/services/services-integrations` (remote_connect contracts), `src/crates/assembly/core` (remote_connect service)

## 背景与定位

Mobile-web 是手机端的**有限接续平台**：扫码接续单台开启网络中继的 BitFun Desktop，
提供工作区/会话切换与新建、聊天等有限能力。桌面端登录账号后，配对应自动把账号身份
（delegated identity）继承给 mobile-web，使其可以把控制目标切换到同账号下任意在线设备，
切换后仍只提供原有的有限能力（数据面复用 `RemoteCommand`，不做桌面级 Peer Mode
transport 替换，不引入 `peer_control_attach/detach`）。

## 排查结论（根因）

1. **P0 — 账号身份从未到达 mobile**：桌面配对成功后用同一 `correlation_id` 连续发送
   `initial_sync` 与 `delegate_identity` 两帧 room 响应；relay 的 pending 是一次性的
   （`resolve_pending` 移除即失效），第二帧命中 `No pending request` 被丢弃。
   mobile `pair()` 只读取一次 HTTP 响应。结果 `hasDelegatedIdentity` 恒为 false，
   设备列表 / 设备 RPC 全部不可用。（web-ui 的 `accountDelegateToPaired` 定义后从未被
   调用，属死代码，不在本次范围内。）
2. **P1 — 切换后状态未刷新**：`DevicesPage` 仅改 `RelayHttpClient.pairedDeviceId`
   即返回，store 中旧设备的 workspace/sessions/messages 残留；无当前控制设备指示。
3. **P1 — 导航 bug**：`App.tsx` 的 `popstate` 无 `devices` 分支，设备页按系统返回键
   历史栈与 UI 脱节；设备页导航未走 `navigateTo`，无 push/pop 动画。
4. **P1 — UI 脱节**：`DevicesPage` 无任何 SCSS（BEM 类未定义）；
   `session-list__devices-btn` 无样式且 title/aria 硬编码英文；
   `devices.online/offline` 用 emoji；PairingPage 的 3D cube 与整体品牌视觉
   （Logo-ICON + token 化 header）不一致；`devices.*` 存在大量遗留未用 key。

## 方案（已确认）

身份传递改为**拉模型**：配对完成后 mobile 通过 room 通道主动请求账号身份，
单请求单响应，不依赖 relay 多帧能力。

### 后端

1. `services-integrations/src/remote_connect.rs`
   - `RemoteCommand` 新增 `GetDelegatedIdentity`（serde tag `get_delegated_identity`）。
   - `RemoteResponse` 新增 `DelegateIdentity { token, user_id, master_key, device_id }`
     （serde tag `delegate_identity`，与 mobile 既有解析字段完全一致；master_key 为 base64）。
   - `handle_remote_command` 对 `GetDelegatedIdentity` 返回
     `Error("Delegated identity is not available on this host")`（宿主未拦截时的兜底）。
   - 合同测试补充两个 serde tag 断言。
2. `assembly/core/src/service/remote_connect/mod.rs`
   - `CommandReceived` 分支在 `decrypt_command` 成功后拦截 `GetDelegatedIdentity`：
     调用 `delegated_identity_fn`；成功 → 组 `DelegateIdentity` 响应
     （user_id 取 trusted mobile identity，device_id 取本机 device_id），
     经 `encrypt_response(request_id)` 回发；失败/未登录 → `Error`（mobile 视为未登录）。
   - 删除配对成功后推送 `delegate_identity` 第二帧的死代码（从未生效，只产生 relay 告警）。

### Mobile-web

3. `RelayHttpClient.ts` / `RemoteSessionManager.ts`
   - 移除 `pair()` 中的 `delegate_identity` 拦截分支（后端不再在 pair 响应发身份）。
   - 新增 `requestDelegatedIdentity(): Promise<boolean>`：room 通道发
     `{cmd:'get_delegated_identity'}`；`resp==='delegate_identity'` 时写入
     token/masterKey/`pairedDeviceId`，并记录 `homeDeviceId`（扫码桌面）；
     error 响应返回 false（不抛异常）。
   - 命令路由：目标为扫码桌面（`pairedDeviceId === homeDeviceId`）时保持走
     room 通道（已验证路径，且不依赖账号 relay 与配对 relay 同源）；仅切换到
     其它同账号设备时走 `sendDeviceRpc`。
4. `store.ts`
   - 新增 `controlTarget: { deviceId, deviceName } | null` 与 setter；
   - 新增 `resetForDeviceSwitch()`：清空 workspace/assistant/pairedDisplayMode/
     sessions/messages/activeTurn（保留连接与身份状态）；
   - `resetConnectionState` 一并清 `controlTarget`。
5. `PairingPage.tsx`
   - 配对成功后 best-effort 调 `requestDelegatedIdentity()`（失败不阻断接续）；
     成功后异步 `listDevices()` 解析本机设备名写入 `controlTarget`。
   - UI 对齐整体风格：3D cube 换为 `Logo-ICON` 品牌图（含轻呼吸动画），
     右上角补主题切换（与语言切换并列），表单、状态、错误样式全部走既有 token。
6. `DevicesPage.tsx` 重做
   - 挂载时若无身份先尝试 `requestDelegatedIdentity()` 一次（桌面后登录账号的场景
     无需重新扫码）；仍无身份 → 空状态卡（图标 + `noDelegatedIdentity` + 重试按钮）。
   - 列表卡片对齐 sessions 视觉：设备图标、名称、短 id、状态点 + 文案（去 emoji）、
     「当前」徽标与「扫码设备」徽标；离线不可点。
   - 点在线设备：卡片 busy → `peer_mode_ping` 探测 → `pairedDeviceId` 切换 →
     `resetForDeviceSwitch()` → 写 `controlTarget` → pop 返回会话页（重挂载自动重拉）。
   - 失败：内联错误条；401 → `tokenExpired` 文案。
   - 30s 自动刷新 + header 手动刷新按钮。
7. `SessionListPage.tsx`
   - Devices 按钮补样式（同 theme-btn 圈形），title/aria 用 `devices.title`；
     控制目标非扫码设备时按钮呈 accent 态。
   - header 用户行追加当前控制设备名（非扫码设备时显示），语义与 health dot 一致。
8. `App.tsx`
   - devices 页走 `navigateTo('devices','push'/'pop')`，纳入 nav-page 动画；
     `popstate` 增加 devices 分支。
9. i18n `messages.ts`（en-US / zh-CN / zh-TW 三语）
   - `devices.*` 重写：title、noDelegatedIdentity、loading、refresh、noDevices、
     online、offline（文字）、current、pairedDesktop、switchFailed、tokenExpired、
     retry；删除全部遗留未用 key（noSessions/newSession/sendMessage 等）。
10. 样式
    - 新增 `styles/components/devices.scss` 并挂入 `index.scss`；
    - `sessions.scss` 增补 devices-btn 与 header 目标设备名样式；
    - `pairing.scss` 重写品牌区（logo 替代 cube）。
    - 只复用既有 CSS 变量，不新增颜色字面量（规避 theme 审计增长）。

## 错误处理

- 身份请求失败（桌面未登录）：接续流程不受影响；Devices 页展示引导文案与重试。
- 设备切换 ping 失败 / RPC 失败：停留在 Devices 页，内联错误，目标不切换。
- 401（delegated token 失效）：`tokenExpired` 文案提示重新扫码或桌面端重新登录。
- 切换后目标设备离线：既有 reconnect banner 与 health dot 继续生效（ping 走新目标）。

## 验证

- `cargo check --workspace`；`cargo test -p bitfun-services-integrations remote_connect_contracts` 聚焦合同测试。
- `pnpm --dir src/mobile-web run type-check`；`pnpm run build:mobile-web`。
- `pnpm run i18n:audit`（messages.ts 文案变更）。
- 手动路径（PR 说明）：扫码配对 → 自动继承身份 → 设备页可见同账号设备 →
  切换在线设备 → 会话页数据来自新设备且 header 显示目标 → 系统返回键正常 →
  桌面未登录时设备页引导文案。

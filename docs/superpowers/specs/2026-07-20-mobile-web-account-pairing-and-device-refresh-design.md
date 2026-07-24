# Mobile-Web 账号配对门槛与设备列表跟随设计

Date: 2026-07-20  
Scope: `src/mobile-web`, `src/crates/services/services-integrations` (pairing/QR/account), `src/crates/assembly/core` (remote_connect), `src/apps/desktop` (account verifier wiring)

## 背景

1. Mobile-web 设备页在桌面换号后展示「账号授权已过期」死胡同：委派 token 失效后未向配对桌面重新拉取身份。
2. 扫码配对的「用户 ID」原用于 URL 防盗用；桌面已登录 BitFun 账号时，应改为校验与桌面相同的账号密码，且必须是被扫码桌面当前账号。

## 已确认产品规则

| 场景 | Mobile 表单 | 校验 |
|---|---|---|
| 桌面未登录账号 | 仅用户 ID（现状） | `TrustedMobileIdentity`（install_id + user_id） |
| 桌面已登录账号 | 用户名 + 密码 | 与桌面登录同源（challenge + unwrap master_key）；`master_key` 必须等于当前桌面会话；密码不落盘；用户名可预填（含首次） |

设备列表：与桌面登录后列表一致；401 时透明刷新委派身份并重试，不再展示授权过期终态。

## 方案

### A. 配对（账号模式）

1. **QR URL**：桌面已登录时追加 `auth=account&user=<username>`（username 来自 credential hint，非机密）。未登录不带这些参数。
2. **Mobile**：解析到 `auth=account` 时显示密码框；用户名优先 URL，其次本地缓存；**禁用无密码自动重连**；密码仅经 ECDH 加密房间通道提交。
3. **`PairingResponse`**：新增可选 `password`；`user_id` 在账号模式下承载提交的 username。
4. **Desktop 校验（verify-only）**：
   - 注册 `account_pairing_verifier(username, password) -> Result<canonical_user_id>`。
   - 实现：`AccountClient` challenge + unwrap master_key，与当前 `AccountSession.master_key` 比较；一致则返回 `session.user_id`。
   - **不调用** `/api/auth/login`，避免刷新/污染桌面 token。
   - 账号不一致或密码错误 → 统一拒绝文案（不泄露哪一项失败）。
5. **信任绑定**：校验成功后用 **canonical `user_id`**（非明文 username）写入 `TrustedMobileIdentity`，保证重连时 username 提交仍能对齐。

### B. 设备列表跟随

1. `RelayHttpClient`：`clearDelegatedIdentity()`；`requestDelegatedIdentity({ force })`；`listDevices` / `sendDeviceRpc` 遇 HTTP 401 时清身份 → 强制重拉 → **重试一次**。
2. `DevicesPage`：移除 `tokenExpired` 终态；失败走可重试错误或 `noDelegatedIdentity`。
3. 清理或降级 `devices.tokenExpired` i18n（无引用则删）。

## 错误处理

- 账号模式缺密码 / 校验失败：配对失败 + 既有失败次数锁定。
- 桌面在出码后登出：verifier 不可用 → 拒绝账号模式配对（提示桌面重新登录并刷新二维码）。
- 委派刷新后桌面仍未登录：设备页 `noDelegatedIdentity` + 重试。

## 验证

- `cargo test -p bitfun-services-integrations --features remote-connect --test remote_connect_contracts`（不带 `--features remote-connect` 时整个测试文件被 cfg 门掉，0 测试静默通过）
- `cargo check -p bitfun-desktop`
- `pnpm --dir src/mobile-web run type-check`
- 手工：未登录扫码；已登录扫码（预填用户名+密码）；错密码；换号后设备列表刷新无过期页

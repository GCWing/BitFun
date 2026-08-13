# 碰一碰分享文件（Tap to Share Files）需求文档

> 状态：提案 / 未做
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/architecture/platform-portability-design.md`](../architecture/platform-portability-design.md)
> - [`docs/architecture/peer-device-mode.md`](../architecture/peer-device-mode.md)
> - [`harmonyos-pc-drag-drop.md`](./harmonyos-pc-drag-drop.md)
> - [`src/apps/ohos`](../../src/apps/ohos/)

## 背景与需求描述

"碰一碰"是 HarmonyOS 提供的近场分享能力：两台设备轻触（经 NFC / 蓝牙 / Wi-Fi 近场感知）即触发文件 / 内容分享。BitFun 现有的跨设备通道是自托管 relay（零知识、AES-GCM）与 Remote Connect / Peer Device Mode，需要账号配对与网络；缺少**免配对、近场、即触即传**的轻量分享。

当前缺口与诉求：

- 用户无法在两台 BitFun 实例（或 BitFun 与 HarmonyOS 设备）之间"一碰即传"文件，只能走 relay 配对 / 远程工作区 / 拖拽；
- 近场分享无需账号配对、无需联网到厂商云，与"自托管、零厂商云"定位一致；
- 接收侧须有确认与完整性校验，不静默接收未知来源；
- 鸿蒙 PC GUI 是独立产品专题（见 `platform-portability-design.md`），碰一碰是其下能力子项，不提前决定 GUI 整体选型。

本提案新增**碰一碰分享文件**能力：两台设备轻触即发起文件分享，接收侧确认后落盘到工作区，复用既有拖拽落点与文件校验。

## 期望行为

### 1. 发起与接收

- 发起侧：选中文件 / 内容，与另一台设备轻触，发起近场分享；
- 接收侧：弹出接收提示（来源、文件名、大小、类型），用户确认后接收；
- 不静默接收；用户可拒绝。

### 2. 落点与校验

- 接收文件落入当前工作区或用户指定目录（对齐拖拽落点：聊天输入 / 文件面板 / 工作区）；
- 完整性校验（哈希，对齐 market 包校验策略），失败可回滚不残留半截文件；
- 大文件 / 批量有进度与取消。

### 3. 隐私与安全

- 近场分享不联网厂商云；凭据 / 配对信息最小化；
- 接收内容视为第三方来源，落盘前提示（对齐隐私协议第 8 条）；
- 不自动执行接收文件，不自动打开可执行。

### 4. 平台与跨设备

- 优先 HarmonyOS 碰一碰 / 一碰传 / NFC 能力（华为官方 API 为准，本提案不臆测字段）；
- 非 HarmonyOS 平台不支持时显式 unavailable，不静默回退到 relay；
- 与既有 relay / Remote Connect / Peer Device Mode 互补，不替换。

### 5. 远程与不可用态

- 碰一碰是本机近场交互，远程控制场景下声明本地执行；
- 目标鸿蒙版本 / 设备缺少近场能力时显式 unsupported，不借用桌面 / 移动端 / Remote 代执行。

## 非目标 / 范围外

- 不替换 relay / Remote Connect / Peer Device Mode；
- 不在本提案内做隔空传送（发现附近设备、无线传输，见 `harmonyos-pc-airdrop.md`）；
- 不覆盖跨厂商 AirDrop 互操作（除非平台原生支持）；
- 不预先决定鸿蒙 PC GUI 整体选型；
- 不做接收文件的自动解析 / 执行。

## 建议的落地路径（基于现有分层）

1. **Contracts (`src/crates/contracts`)** — 碰一碰分享 DTO / port（来源、文件元数据、进度、确认），行为轻量，不耦合 HarmonyOS 私有 API。
2. **OHOS App (`src/apps/ohos`)** — HarmonyOS 碰一碰 / NFC / 近场分享适配；鸿蒙私有协议不泄漏出适配层。
3. **Services (`src/crates/services`)** — 接收文件落盘、哈希校验、临时文件清理（复用 market 包校验与 artifacts 约定）。
4. **Web UI / ArkUI** — 接收提示 UI、落点选择（复用拖拽落点）。
5. **远程策略** — 碰一碰命令远程策略声明本地执行，在 `remote_workspace_policy` 登记。

### 分层与依赖边界要点

- 严格遵守 `platform-portability-design.md`：鸿蒙 PC GUI 是独立专题，碰一碰是其下能力子项；
- 平台差异只在 app/adapter/service 边界，共享 Runtime 不按 target triple 分叉业务；
- 缺失能力显式 unsupported，不静默借用桌面 / 移动端 / Remote 代执行；
- 不建立巨型 `ohos` feature 或第二套分享协议；
- 复用拖拽落点与文件校验，不新造第二套接收链。

## 设计草案 / 参考示例

- **平台能力参考**：HarmonyOS 碰一碰 / 一碰传 / NFC（华为官方文档为准）。
- **落点参考**：拖拽落点（聊天输入 / 文件面板 / 工作区导航），碰一碰接收复用同构落点。
- **校验参考**：market 包哈希校验、原子落盘、失败回滚。
- **隐私参考**：隐私协议第 8 条（第三方来源提示）。
- **互补参考**：relay（零知识自托管）适合跨网络 / 持久配对；碰一碰适合近场 / 免配对 / 即触即传。

## 是否愿意贡献

- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

## 补充说明

- 与 `harmonyos-pc-drag-drop.md` 的关系：两者都是鸿蒙 PC GUI 下的近场 / 本地交互能力子项，复用文件落点与校验。
- 与 `harmonyos-pc-airdrop.md` 的关系：碰一碰是近场轻触触发，隔空传送是发现附近设备无线传输，互补不替换。
- 与 relay 的关系：relay 适合跨网络持久配对，碰一碰适合近场免配对，二者互补。
- 相关分层入口：`src/apps/ohos`、`docs/architecture/platform-portability-design.md`、`docs/architecture/peer-device-mode.md`。

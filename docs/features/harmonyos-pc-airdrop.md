# 鸿蒙 PC 隔空传送（HarmonyOS PC AirDrop）需求文档

> 状态：提案 / 未做
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/architecture/platform-portability-design.md`](../architecture/platform-portability-design.md)
> - [`docs/architecture/peer-device-mode.md`](../architecture/peer-device-mode.md)
> - [`tap-to-share-files.md`](./tap-to-share-files.md)
> - [`src/apps/ohos`](../../src/apps/ohos/)

## 背景与需求描述

"隔空传送"指无需配对、发现附近设备后无线传输文件的能力（类 AirDrop）。BitFun 现有跨设备通道是自托管 relay（零知识、AES-GCM）与 Remote Connect / Peer Device Mode，需账号配对或联网到自部署服务器；缺少**免配对、近场发现、无线直传**的轻量传输。

当前缺口与诉求：

- 用户无法在鸿蒙 PC 与附近设备（鸿蒙 PC / 手机 / 平板）之间免配对无线传文件，只能走 relay 配对 / 碰一碰近场轻触 / 远程工作区；
- 隔空传送发现附近设备后直传，不联网厂商云，与"自托管、零厂商云"定位一致；
- 接收侧须确认与完整性校验，不静默接收；
- 鸿蒙 PC GUI 是独立产品专题（见 `platform-portability-design.md`），隔空传送是其下能力子项，不提前决定 GUI 整体选型。

本提案新增**鸿蒙 PC 隔空传送**能力：发现附近设备、发起传输、接收确认、落盘校验，复用既有文件落点与校验链。

## 期望行为

### 1. 发现与传输

- 发起侧发现附近可传设备（鸿蒙 PC / 手机 / 平板），列表展示；
- 选中设备发起传输，接收侧弹出确认（来源、文件名、大小、类型）；
- 用户确认后接收，不静默接收；可拒绝。

### 2. 落点与校验

- 接收文件落入当前工作区或用户指定目录（对齐拖拽落点）；
- 完整性校验（哈希，对齐 market 包校验），失败可回滚不残留；
- 大文件 / 批量有进度与取消。

### 3. 隐私与安全

- 发现与传输不联网厂商云；凭据 / 设备标识最小化；
- 接收内容视为第三方来源，落盘前提示（对齐隐私协议第 8 条）；
- 不自动执行接收文件，不自动打开可执行；
- 传输加密，对齐 relay 的零知识姿态（本地派生密钥）。

### 4. 平台与跨设备

- 优先 HarmonyOS 隔空投送 / 近场分享能力（华为官方 API 为准，本提案不臆测字段）；
- 非 HarmonyOS 平台不支持时显式 unavailable，不静默回退到 relay；
- 与 relay / Remote Connect / Peer Device Mode / 碰一碰互补，不替换。

### 5. 远程与不可用态

- 隔空传送是本机近场交互，远程控制场景下声明本地执行；
- 目标鸿蒙版本 / 设备缺少近场发现 / 传输能力时显式 unsupported，不借用桌面 / 移动端 / Remote 代执行。

## 非目标 / 范围外

- 不替换 relay / Remote Connect / Peer Device Mode；
- 不在本提案内做碰一碰（近场轻触触发，见 `tap-to-share-files.md`）；
- 不覆盖跨厂商 AirDrop 互操作（除非平台原生支持）；
- 不预先决定鸿蒙 PC GUI 整体选型；
- 不做接收文件的自动解析 / 执行；
- 不覆盖手机 / 平板移动端本地传输（移动端是另一专题）。

## 建议的落地路径（基于现有分层）

1. **Contracts (`src/crates/contracts`)** — 隔空传送 DTO / port（附近设备、文件元数据、进度、确认），行为轻量，不耦合 HarmonyOS 私有 API。
2. **OHOS App (`src/apps/ohos`)** — HarmonyOS 隔空投送 / 近场分享适配；设备发现、传输握手、加密；鸿蒙私有协议不泄漏出适配层。
3. **Services (`src/crates/services`)** — 接收文件落盘、哈希校验、临时文件清理（复用 market 包校验与 artifacts 约定）。
4. **Web UI / ArkUI** — 设备发现列表、接收提示 UI、落点选择（复用拖拽落点）。
5. **远程策略** — 隔空传送命令远程策略声明本地执行，在 `remote_workspace_policy` 登记。

### 分层与依赖边界要点

- 严格遵守 `platform-portability-design.md`：鸿蒙 PC GUI 是独立专题，隔空传送是其下能力子项；
- 平台差异只在 app/adapter/service 边界，共享 Runtime 不按 target triple 分叉业务；
- 缺失能力显式 unsupported，不静默借用桌面 / 移动端 / Remote 代执行；
- 不建立巨型 `ohos` feature 或第二套传输协议；
- 传输加密对齐 relay 零知识姿态，本地派生密钥，服务器不接触明文；
- 复用文件落点与校验，不新造第二套接收链。

## 设计草案 / 参考示例

- **平台能力参考**：HarmonyOS 隔空投送 / 近场分享 / 设备发现（华为官方文档为准）。
- **落点参考**：拖拽落点（聊天输入 / 文件面板 / 工作区导航），隔空传送接收复用同构落点。
- **校验参考**：market 包哈希校验、原子落盘、失败回滚。
- **加密参考**：relay 的 Argon2id + AES-GCM 零知识姿态，本地派生密钥。
- **隐私参考**：隐私协议第 8 条（第三方来源提示）。
- **互补参考**：relay 适合跨网络 / 持久配对；隔空传送适合近场 / 免配对 / 无线直传；碰一碰适合近场轻触触发。

## 是否愿意贡献

- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

## 补充说明

- 与 `tap-to-share-files.md` 的关系：碰一碰是近场轻触触发，隔空传送是发现附近设备无线传输，互补不替换。
- 与 `harmonyos-pc-drag-drop.md` 的关系：三者都是鸿蒙 PC GUI 下的本地 / 近场交互能力子项，复用文件落点与校验。
- 与 relay 的关系：relay 适合跨网络持久配对，隔空传送适合近场免配对无线直传，二者互补。
- 与远程工作区的关系：隔空传送是本机近场交互，远程策略声明本地执行，不支持时清晰提示。
- 相关分层入口：`src/apps/ohos`、`docs/architecture/platform-portability-design.md`、`docs/architecture/peer-device-mode.md`。

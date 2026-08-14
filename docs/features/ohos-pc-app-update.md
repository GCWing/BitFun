# 鸿蒙 PC 应用更新（HarmonyOS PC App Update）能力需求文档

> 状态：需求 / 提案
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/architecture/platform-portability-design.md`](../architecture/platform-portability-design.md)（鸿蒙 PC 平台移植目标）
> - [`docs/architecture/product-architecture.md`](../architecture/product-architecture.md)（分层与平台适配边界）
> - [`docs/architecture/peer-device-mode.md`](../architecture/peer-device-mode.md)（远程控制下更新归属）
> 现有实现参考：
> - `src/apps/cli/src/self_update.rs`（成熟自更新：清单 / 镜像源选择 / 吞吐量探测 / SHA256+签名 / 进度 / 重启）
> - `src/web-ui/src/infrastructure/update/`（桌面更新 store 状态契约）
> - `src/apps/desktop/src/api/ohos/update.rs`（已有 `check_app_update_ohos` 桥接入口）

### 功能类型
<!-- 请描述功能建议的类型 -->
- [x] 新功能
- [x] 现有功能增强
- [ ] 用户体验优化
- [ ] 性能优化
- [ ] 接口/集成扩展
- [ ] 其他

### 优先级
<!-- 请选择功能建议的优先级 -->
- [ ] 紧急（P0 - 核心需求，强烈期望实现）
- [x] 高（P1 - 重要功能，期望尽快实现）
- [ ] 中（P2 - 有价值的功能，计划实现）
- [ ] 低（P3 - 锦上添花，有时间再做）

### 背景与动机
<!-- 请描述你为什么需要这个功能，解决了什么痛点 -->
- BitFun 已有跨平台桌面（Tauri）、CLI（Linux self_update）、移动 / 平板鸿蒙 HAP（`src/apps/ohos`）等形态；鸿蒙 PC（HarmonyOS NEXT PC）是仓库明确的平台移植目标，但应用更新闭环尚未建立。
- 鸿蒙 PC 上应用更新缺少完整流程：桌面侧已有 `check_app_update_ohos` 桥接，但仅把检查委托给 ArkTS 层，缺少统一的"检查 → 下载 → 校验 → 安装 → 重启 → 恢复"闭环。
- 现有 CLI `self_update.rs` 已是成熟模式（发布清单、GitHub + OpenBitFun 镜像源、基于吞吐量的源选择、SHA256 + 签名 URL、分块下载进度、stall 检测与 fail-over、重启），鸿蒙 PC 自托管通道应复用其策略与常量，而非在鸿蒙侧重造一遍。
- 缺少分发渠道策略：华为应用市场（AppGallery）系统更新 vs 自托管发布通道（GitHub release + OpenBitFun 镜像），二者适用条件与体验不同，需明确何时用哪个。
- 缺少签名校验与完整性保护：鸿蒙 PC 安装包若不校验来源与签名，存在被替换安装包的风险。
- 远程工作场景：远程控制下鸿蒙 PC 的更新应以受控端为准，控制端不得越权触发安装。

### 功能描述
<!-- 请详细描述你期望的功能行为 -->
- **检查更新**：应用启动后台静默检查，或由"检查更新"入口手动触发，查询发布清单获取最新版本信息；启动路径检查须有预算上限（不阻塞首屏），超时按"无更新"降级而非报错。
- **多分发渠道策略**：
  - 若已通过华为应用市场分发，优先走应用市场系统更新，复用系统更新体验与签名链；
  - 自托管通道（GitHub release + OpenBitFun 镜像）作为补充 / 未上架场景，复用 CLI `self_update` 的清单格式、镜像源选择与吞吐量探测。
- **版本清单与完整性**：清单带 `schema_version`、版本号、平台条目、资产（`filename` / `url` / `sha256_url` / `sig_url`）；下载后校验 SHA256，有签名则校验签名；签名缺失按策略降级或 fail-closed 拒绝。
- **下载与进度**：分块下载、按时间间隔上报进度（避免静默卡死）、断点续传、内存上限（防止恶意 / 错误源 OOM）、stall 检测与死链 fail-over 到镜像源。
- **安装与重启**：鸿蒙 PC 通过系统安装能力安装 HAP / 安装包；安装完成后提示重启或按用户选择重启；尽量保留会话 / 设置状态以在重启后恢复。
- **更新状态机**：`idle / checking / available / downloading / verifying / installing / installed / error / up-to-date`；执行状态与加载状态相互独立，UI 不出现无说明的空白态。
- **降级与不可用态**：网络 / 镜像不可用、签名校验失败、系统安装被拒时，给出清晰错误信息与下一步建议，不静默失败；旧版本保持可用。
- **平台无关 + 平台适配**：检查 / 下载 / 校验 / 状态逻辑平台无关（复用 services 层），鸿蒙 PC 特定的安装 / 系统更新能力通过平台适配器暴露；UI 走 adapter / infrastructure 层，不直接调用 host API。
- **远程工作场景**：远程控制下更新以受控端为准；受控端不可更新或被系统策略拒绝时，给出清晰不支持态提示而非静默失败。

### 期望效果 / 使用场景
<!-- 描述该功能在什么场景下使用，以及使用后的预期效果 -->
1. 鸿蒙 PC 用户启动应用，后台静默检查更新（有预算上限，不卡首屏）；有新版本时在合适入口提示。
2. 用户点"检查更新"手动触发，看到新版本说明、大小与来源；确认后下载并显示实时进度。
3. 下载完成自动校验 SHA256 与签名；校验失败时明确报错，旧版本保持可用，不安装被替换的包。
4. 校验通过后通过系统安装能力安装，完成后提示重启；重启后会话 / 设置可恢复。
5. 多源（GitHub + OpenBitFun 镜像）按吞吐量自动选择，慢链 / 死链 fail-over 到镜像，避免单点失败。
6. 远程控制下，鸿蒙 PC 更新以受控端为准，控制端不越权触发安装；受控端不可更新时给出清晰提示。

### 设计草案 / 参考示例
<!-- 如有设计稿、草图或参考的产品示例，请附在此处 -->
- **现有参考实现**：
  - `src/apps/cli/src/self_update.rs`：清单 / 镜像源选择 / 吞吐量探测 / SHA256+签名 / 进度 / 重启——成熟模式，鸿蒙 PC 自托管通道应复用其策略与常量（`PROBE_WINDOW`、`HEALTHY_THROUGHPUT`、`STALL_THROUGHPUT`、`MAX_ARCHIVE_BYTES`、`PROGRESS_INTERVAL`、`AUTO_CHECK_BUDGET`）。
  - `src/web-ui/src/infrastructure/update/updateInstallStore.ts`：桌面更新 store 状态（`downloading` / `installed` / `error` + 进度）——UI 状态契约可复用。
  - `src/apps/desktop/src/api/ohos/update.rs`：`check_app_update_ohos` 已有桌面 → ArkTS 委托入口；鸿蒙 PC 应在此契约上补全完整闭环。
- **行业参照**：HarmonyOS 应用内更新系统 API、华为应用市场更新流程、Tauri updater、Sparkle / appcast 的版本清单与签名校验。
- **落地分层建议**（遵循仓库分层与边界）：
  1. Contracts（`src/crates/contracts`）：发布清单 DTO、更新状态枚举、资产契约（复用 CLI 清单 schema），平台无关、行为轻量。
  2. Services（`src/crates/services/services-integrations`）：清单获取 / 镜像源选择 / 下载 / 校验逻辑（复用 `self_update` 策略），平台无关。
  3. Assembly（`src/crates/assembly`）：按 delivery profile 装配鸿蒙 PC 更新能力，声明对应 capability pack。
  4. Interfaces / App / UI（`src/apps` + `src/web-ui`）：鸿蒙 PC host 暴露更新 command，ArkTS 侧实现系统安装；UI 走 adapter / infrastructure 层，不直接调 host API。
- **签名与完整性**：鸿蒙 PC 安装包须支持签名（`sig_url`），校验失败 fail-closed 拒绝安装；签名缺失仅在明确策略允许时降级，且不得渲染为"校验通过"。

### 是否愿意贡献
<!-- 是否愿意参与该功能的开发或讨论 -->
- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

### 补充说明
<!-- 其他你认为有助于理解功能建议的信息，如相关 Issue 链接、文档等 -->
- 鸿蒙 PC 是仓库平台移植目标（见 [`docs/architecture/platform-portability-design.md`](../architecture/platform-portability-design.md)）；本需求聚焦**鸿蒙 PC 应用更新**，不涉及鸿蒙 PC CLI/TUI——后者是另一个未来平台目标，需单独立项。
- 分发渠道策略（AppGallery 上架 vs 自托管）需法务 / 合规确认后再定，本需求只定义能力与契约。
- 严格复用 CLI `self_update` 模式与常量，避免在鸿蒙 PC 侧重造下载 / 校验 / 源选择逻辑；两处若行为分叉须保持同步（`self_update.rs` 注释已要求与 relay deploy 路径保持一致，本需求同此原则）。
- 远程工作场景遵循仓库"远程兼容"全局规则：本地受限行为在远程场景需有明确不支持态提示，而非静默失败。

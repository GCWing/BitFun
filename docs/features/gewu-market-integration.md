# 接入格物市场（Gewu Market）精品 Skills 功能提案

> 状态：提案 / 未集成
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/architecture/product-architecture.md`](../architecture/product-architecture.md)
> - [`docs/architecture/platform-portability-design.md`](../architecture/platform-portability-design.md)
> - [`deploy/miniapp-market/README.md`](../../deploy/miniapp-market/README.md)
> - [`src/crates/services/services-integrations/AGENTS.md`](../../src/crates/services/services-integrations/AGENTS.md)

## 背景与需求描述

BitFun 的扩展能力分四层：自定义 Agent → MCP / Skills / Hooks → Mini Apps → 源码级改造。其中 **Skills** 是 prompt / resource / instruction 形态的轻量扩展，作为 agent definition 或 harness input 的一部分参与运行。

在 HarmonyOS 生态中，**格物市场** 是华为提供的精品 Skills 分发市场。`HUAWEI BitFun ADE` 的产品定位（见 `src/crates/services/services-integrations/src/privacy/assets/zh-CN.md`）明确包含"融合鸿蒙 Harness、**格物市场精品 Skills**、鸿蒙开发工具链及知识库"。当前仓库尚未接入格物市场，存在以下缺口：

- 用户无法在 BitFun 内浏览 / 检索 / 安装格物市场上的精品 Skills，只能手动复制 Skill 文件到本地 Skills 根目录；
- Skills 的版本更新、来源校验、卸载清理没有受控流程，易残留过期或来源不明的 Skill；
- 开发者无法把自研精品 Skill 经由格物市场分发，只能各自托管；
- 与 MiniApp 市场（`market.openbitfun.com/miniapp`）、Skin 市场（`market.openbitfun.com/skin`）已建成的受控分发链相比，Skills 这条扩展路径缺少等价的接入能力。

因此希望在桌面端（`DeliveryProfile::Desktop` / `ProductFull`，并优先覆盖 HarmonyOS / `HUAWEI BitFun ADE` 形态）新增"接入格物市场精品 Skills"能力，作为 Skills 扩展层的官方分发通道。

## 期望行为

- 在 Settings 或扩展管理区提供"格物市场"入口，可浏览 / 检索精品 Skills 列表与详情；
- 一键安装到本地 Skills 根目录（与现有 `file_watch` 管理的 `bitfun-skills` 目录约定一致），安装后立即可被 Agent 运行时发现；
- 支持检查已安装 Skill 的版本，并在市场有新版本时提示更新；支持卸载并清理对应文件；
- 安装包下载附完整性校验（哈希 / 签名，参照 `miniapp-market-service/src/package.rs` 的校验策略），失败可回滚不残留半截文件；
- 支持开发者从 BitFun 把自研 Skill 提交到格物市场（投稿 / 更新 / 撤回），走格物市场自身的审核与发布流程；
- 账号体系优先复用格物市场 / HarmonyOS 账号体系，不强行新造登录；本地仅保存最小凭据，绝不把凭据写进日志或产物；
- 非桌面 / 非 HarmonyOS 形态（Server / Remote / Web / MobileWeb）默认不启用该能力，符合"按 delivery profile 选择 capability pack"的装配规则；
- 第三方 Skills 的隐私责任明确归属格物市场与 Skill 提供方，前端在安装前给出清晰提示（对齐现有隐私协议第 8 条表述）。

## 非目标 / 范围外

- 不自建第二个 Skills 市场；格物市场是分发方，BitFun 是消费方 / 投稿方；
- 不在本提案内接入 MiniApp / Skin 市场（已有独立服务）；
- 不在本提案内做 Skill 运行时沙箱或权限收窄（Skill 作为 prompt / instruction 注入，安全模型与现有 Skills 一致）；
- 不在本提案内做 Skill 的图形化编辑器或可视化构建；
- 不替换本地手动放置 Skill 的能力，市场安装只是补充分发通道；
- 不在本提案内覆盖 HarmonyOS PC CLI/TUI 形态（按 `platform-portability-design.md`，PC 终端 CLI/TUI 是未来目标，需单独立项）。

## 建议的落地路径（基于现有分层）

依据仓库的分层与边界规则，建议落在：

1. **Contracts (`src/crates/contracts`)** — 在 `core-types` / `runtime-ports` 中定义稳定的格物市场 DTO 与 port trait（如 `GewuSkillManifest`、`GewuSkillListResponse`、`GewuSkillInstallRequest`、`GewuSkillProgressEvent`、投稿相关 DTO），保持行为轻量、不向上依赖。格物市场作为外部系统，是**边界资源**，不得在 contracts 中耦合其私有协议字段。
2. **Adapters (`src/crates/adapters`)** — 新增格物市场协议适配（HTTP 传输、格物市场私有 wire 翻译、鉴权握手、投稿 payload 构造），仅做协议翻译，不承载产品策略。参照现有 `ai-adapters` / `transport` 的边界。
3. **Services (`src/crates/services/services-integrations`)** — 在新增的 `gewu-market` 集成族下实现 `GewuMarketService`：列表 / 详情 / 包下载 / 哈希校验 / 落盘 / 卸载 / 投稿，全部放在显式 feature（如 `gewu-market`）后，`default = []`。下载 / 安装 / 校验逻辑应尽量复用 `miniapp-market-service` 已沉淀的包校验与 artifacts 存储约定，避免重复造轮子。
4. **Assembly (`src/crates/assembly/product-capabilities`)** — 视情况新增 `ProductCapabilityId::GewuMarket`（或并入既有 Skills 扩展能力），按 profile 选择；HarmonyOS / Desktop / ProductFull 选入，其余不选。
5. **Interfaces (`src/crates/interfaces`)** — 在 app-server 协议中暴露格物市场相关请求 / 响应（保持 camelCase wire shape，与 `cloud_speech` 风格一致）。
6. **App / UI (`src/apps/desktop` + `src/web-ui`)** — 桌面 host 注册 `GewuMarketService` 实现并暴露 Tauri command；Web UI 新增市场浏览 / 安装 / 投稿 UI。注意：UI 组件不得直接调用格物市场 HTTP API，必须走 adapter / infrastructure 层。

### 分层与依赖边界要点

- 产品逻辑平台无关：核心 `GewuMarketService` 与 DTO / port 保持平台无关，格物市场私有协议、HarmonyOS 账号握手、Tauri command、Web UI 适配分别落在各自 owner 层；
- Services / Adapters 不得依赖 assembly / core facade / app / UI；
- 格物市场是边界资源，只有注册的 adapter / service 可调用它，其他层只消费 port 与稳定 contracts；
- 新增 feature 必须 `default = []`，避免把格物市场重运行时（HTTP / 鉴权 / 投稿）带入默认编译；
- 投稿链路与浏览 / 安装链路要在 service 内做职责隔离，避免投稿凭据意外流入只读浏览上下文。

## 设计草案 / 参考示例

- **参考本仓库已建成的市场链路**：MiniApp 市场（`miniapp-market-service`）与 Skin 市场（`skin-market-service`）已沉淀 SQLite + artifacts 存储 + GitHub OAuth + 桌面 Bearer token + 审核流程 + 备份 / 恢复演练 + Nginx vhost 的完整受控分发链。格物市场接入应复用其中**包校验、artifacts 目录约定、安装 / 卸载、版本对齐、进度事件**等中性能力，不复用其 GitHub OAuth（格物市场走自有账号体系）。
- **包校验参考**：`miniapp-market-service/src/package.rs` 与 `artifacts.rs` 的哈希校验、原子落盘、失败回滚策略可作为 Skill 包安装的范本。
- **Skills 根目录约定**：与 `file_watch` 服务管理的 `bitfun-skills` 目录一致，安装后由现有 file-watch 机制触发 Agent 运行时重新发现，不新造第二套发现链。
- **交互参考**：MiniApp 市场网页（`market.openbitfun.com/miniapp`）的浏览 / 详情 / 一键安装 / "我的投稿"流程，可作为桌面端格物市场 UI 的交互参考。
- **隐私边界**：格物市场与 Skill 提供方对第三方 Skill 的个人信息处理负责；BitFun 在安装前必须给出清晰提示，对齐 `src/crates/services/services-integrations/src/privacy/assets/zh-CN.md` 第 8 条。
- **远程场景**：用户从一台设备远程控制另一台桌面时，格物市场安装应在受控端执行还是控制端发起，需要在 `src/apps/desktop/src/api/remote_workspace_policy.rs` 中声明明确策略（首版建议 control 端发起、target 端执行，并给出不支持态提示而非静默失败）。

## 是否愿意贡献

- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

## 补充说明

- 本提案严格遵循仓库"产品逻辑平台无关、再通过平台适配器暴露"与"外部系统是边界资源，只有注册 adapter / service 可调用"的规则。
- 格物市场作为华为 HarmonyOS 生态资源，其私有 API、鉴权握手、投稿审核流程属于外部契约；接入前需与格物市场方对齐 API 规格、凭据发放与投稿审核 SLA，不在本提案内臆测其字段。
- 与已建成的 MiniApp / Skin 市场链路（`deploy/miniapp-market/README.md`、`deploy/skin-market/README.md`）保持运维心智一致：专用 checkout、专用 artifacts / backups 目录、root-only secret、 leased push 部署 ref、健康检查与回滚演练等约定，若后续自建格物市场镜像或缓存层，应沿用而非另立。
- HarmonyOS PC CLI/TUI 形态是未来平台目标（见 `platform-portability-design.md`），本提案不覆盖；如后续要覆盖，按该文件要求单独立项，不与桌面端 GUI 接入混为一谈。
- 相关分层入口：`src/crates/contracts/AGENTS.md`、`src/crates/adapters/AGENTS.md`、`src/crates/services/services-integrations/AGENTS.md`、`src/crates/assembly/product-capabilities/AGENTS.md`。

# 桌面端 PC 语音输入（voice-input）功能提案

> 状态：提案 / 未集成
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`docs/architecture/product-architecture.md`](../architecture/product-architecture.md)
> - [`src/crates/assembly/product-capabilities/AGENTS.md`](../../src/crates/assembly/product-capabilities/AGENTS.md)
> - [`src/crates/services/services-integrations/AGENTS.md`](../../src/crates/services/services-integrations/AGENTS.md)

## 背景与需求描述

BitFun 目前是一个跨平台桌面 AI Agent（Windows / macOS / Linux，并正在进行 HarmonyOS 端口移植），用户与 Agent 的交互仅限文本输入。在编码、办公、长任务审查等场景下，纯文本输入存在以下痛点：

- 长段需求 / 会议纪要 / 口述文档时，手打效率低、打断思考流；
- 在 Computer Use、浏览器操作、桌面执行等"手已被占用"场景下，无法边操作边输入；
- 无障碍场景（视障、肢体受限用户）缺乏语音替代输入路径；
- 与"自托管、零厂商云"的产品定位不符的方案是直接接公有云 ASR。

因此希望在桌面端（`DeliveryProfile::Desktop` / `ProductFull`）新增一个 `voice-input` 产品能力，让用户可以通过麦克风把语音转写为文本后注入当前会话输入框，作为文本输入的补充（而非替代）。

## 期望行为

- 在会话输入区提供"语音输入"入口（按住说话 / 点击录音两种模式可选）；
- 录音会话具备完整生命周期：开始 → 采集 → 取消 / 结束 → 转写 → 文本回填到输入框；
- 支持在 Settings 中开关该能力、选择识别模型、配置麦克风设备、设置最长录音时长；
- 默认走**本地 ASR**（模型在用户机器上运行），保证隐私与离线可用；可选项：接入云端 ASR（已有 `cloud_speech` 配置形态可复用 wire 契约）；
- 录音时长、采样率、临时音频文件路径等都有明确上限与清理策略，避免磁盘残留；
- 远端 / Server / Web / MobileWeb 等非桌面 delivery profile 默认不启用该能力，符合现有"按 profile 选择 capability pack"的装配规则。

## 非目标 / 范围外

- 不在本提案内做 TTS（语音播报 Agent 回复）；
- 不做"始终在听"的唤醒词检测（hotword / wake-word），首版只做显式触发的按需录音；
- 不替换文本输入为主交互方式，语音只是补充输入通道；
- 不在本提案内做语音命令（voice command）语义解析，只产出转写文本交给现有会话。

## 建议的落地路径（基于现有分层）

依据仓库的分层与边界规则，建议落在：

1. **Contracts (`src/crates/contracts`)** — 在 `core-types` / `runtime-ports` 中定义稳定的语音转写 DTO 与 port trait（`SpeechTranscribeRequest`、`SpeechListModelsResponse`、`SpeechModelStatus`、`SpeechModelProgressEvent` 等），保持行为轻量、不向上依赖。
2. **Services (`src/crates/services/services-integrations`)** — 在新增的 `speech` 集成族下实现 `SpeechService`：模型下载 / 安装、麦克风采集会话、本地 ASR 推理路由、临时音频文件 IO 与清理，全部放在显式 feature（如 `speech`）后，`default = []`。
3. **Assembly (`src/crates/assembly/product-capabilities`)** — 新增 `ProductCapabilityId::VoiceInput` 与 `VOICE_INPUT_CAPABILITY_PACK`，并将其加入 `DEFAULT_PRODUCT_CAPABILITY_PACKS`（ProductFull / Desktop），其余 profile 不选。
4. **Interfaces (`src/crates/interfaces`)** — 在 app-server 协议中暴露语音相关请求 / 响应（保持 camelCase wire shape 与现有 `cloud_speech` 风格一致）。
5. **App / UI (`src/apps/desktop` + `src/web-ui`)** — 桌面 host 注册语音服务实现并暴露 Tauri command；Web UI 新增麦克风采集适配（`voiceInputAudio.ts` 一类的薄封装）与输入区 UI 入口。注意：UI 组件不得直接调用 Tauri / 浏览器 API，必须走 adapter / infrastructure 层。

### 分层与依赖边界要点

- 产品逻辑平台无关：核心 `SpeechService` 与 DTO / port 保持平台无关，麦克风采集、Tauri command、Web UI 适配分别落在各自 owner 层；
- Services 不得依赖 assembly / core facade / app / UI；
- Assembly 只做 capability pack 事实与装配，不实现具体 ASR 推理；
- 新增 feature 必须 `default = []`，重运行时（ONNX、下载器）由调用方按需启用，避免污染默认编译。

## 设计草案 / 参考示例

- **本地 ASR 引擎候选**：sherpa-onnx（k2-fsa）—— 跨平台、支持 ONNX Runtime、已有中英日韩粤多语种预训练 int8 模型，授权宽松，适合打包进桌面安装包。
- **候选模型**（均为 int8 量化，体积 / 精度折中）：
  - SenseVoice Small int8 —— 多语种（普通话 / 粤语 / 英 / 日 / 韩），体量小、首版默认；
  - Qwen3-ASR-0.6B int8 —— 更高质量，作为可选升级档。
- **模型分发**：首次使用时按 manifest 下载并安装到 `~/.bitfun/speech-models/`（与现有 MiniApp / 远端部署脚本的同构目录约定一致），支持取消与断点续传，附进度事件。
- **交互参考**：OpenAI Whisper 桌面端、微软 Dictate、macOS 系统听写 —— 按住说话 + 点击录音双模式，转写完成后文本落入输入框并由用户决定是否发送（不自动提交）。
- **隐私边界**：默认本地推理；如用户在 Settings 显式选择云端模型，则在 wire 上复用 `cloud_speech` 配置形态，但前端必须明确提示"音频将上传至第三方"。

## 是否愿意贡献

- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

## 补充说明

- 本提案严格遵循仓库"产品逻辑平台无关、再通过平台适配器暴露"的规则。
- 远程工作场景需考虑：当用户从一台设备远程控制另一台桌面时，语音输入是否在受控端执行需要明确策略。首版可在 `src/apps/desktop/src/api/remote_workspace_policy.rs` 中声明为 local-only 并给出清晰的不支持态提示，而非静默失败。
- 仓库已有的 `cloud_speech` 配置 wire 形态（`configId` / `modelName` camelCase）可作为云端选项的复用起点，避免新造协议。
- HarmonyOS PC CLI/TUI 支持是未来平台目标，不属于当前桌面端集成范围；如后续要覆盖，按 [`docs/architecture/platform-portability-design.md`](../architecture/platform-portability-design.md) 单独立项。

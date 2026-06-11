# Voice Service 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 BitFun 添加可插拔的语音输入（STT）和语音播报（TTS）功能，通过统一抽象接口连接可配置的后端 provider，默认使用免费方案。

**架构：**
- 抽象层：`runtime-ports` 定义 `SttProvider` / `TtsProvider` trait
- 实现层：`services-integrations` 实现各 provider（WebSpeech、Edge TTS、faster-whisper 等）
- 集成层：`core` 或 `services-core` 组合 provider 为 `VoiceService`
- 配置层：扩展 `AiConfig` voice 配置，支持通过配置切换 provider

**技术栈：** Rust（本体）+ TypeScript（前端调用）

---

## 文件结构

### 新建文件

| 文件 | 职责 |
|------|------|
| `src/crates/runtime-ports/src/voice/stt.rs` | STT Provider 抽象接口 |
| `src/crates/runtime-ports/src/voice/tts.rs` | TTS Provider 抽象接口 |
| `src/crates/runtime-ports/src/voice/mod.rs` | voice 模块统一导出 |
| `src/crates/services-integrations/src/voice/stt/web_speech.rs` | 浏览器 Web Speech API 实现 |
| `src/crates/services-integrations/src/voice/stt/faster_whisper.rs` | faster-whisper 本地推理实现 |
| `src/crates/services-integrations/src/voice/stt/mod.rs` | STT provider 导出 + 注册 |
| `src/crates/services-integrations/src/voice/tts/edge_tts.rs` | Edge TTS 实现 |
| `src/crates/services-integrations/src/voice/tts/mod.rs` | TTS provider 导出 + 注册 |
| `src/crates/services-integrations/src/voice/mod.rs` | VoiceService 主入口 |
| `src/web-ui/src/infrastructure/voice/VoiceService.ts` | 前端 voice 调用封装 |
| `src/web-ui/src/infrastructure/voice/SttProviderFactory.ts` | 前端 STT provider 工厂 |
| `src/web-ui/src/infrastructure/voice/TtsProviderFactory.ts` | 前端 TTS provider 工厂 |

### 修改文件

| 文件 | 改动 |
|------|------|
| `src/crates/runtime-ports/src/lib.rs` | 导出 voice 模块 |
| `src/crates/runtime-ports/Cargo.toml` | 添加 `stt`, `tts` feature |
| `src/crates/core/src/service/config/types.rs` | 添加 `voice: VoiceConfig` 配置结构 |
| `src/crates/core/src/service/config/mod.rs` | 注册 VoiceConfig |
| `src/crates/services-integrations/src/lib.rs` | 导出 voice 子模块 |
| `src/crates/services-integrations/Cargo.toml` | 添加 voice 相关依赖 |
| `src/apps/desktop/src/api/voice.rs` | Tauri command 暴露 voice API |
| `src/web-ui/src/infrastructure/api/service-api/VoiceAPI.ts` | 前端调用层 |
| `src/web-ui/src/flow_chat/components/ChatInput.tsx` | 集成语音输入按钮 |
| `src/web-ui/src/app/components/BottomBar.tsx` | 集成语音播报开关 |

---

## 实现任务

### 阶段一：Rust 抽象层（runtime-ports）

#### 任务 1：定义 STT Provider trait

**文件：**
- 创建：`src/crates/runtime-ports/src/voice/stt.rs`
- 测试：`src/crates/runtime-ports/tests/voice/test_stt.rs`

- [ ] **步骤 1：编写失败的测试**

```rust
use futures::Stream;

#[tokio::test]
async fn test_stt_provider_trait_object_safe() {
    // 验证 SttProvider 可以通过 dyn 引用使用
    let provider: Box<dyn SttProvider> = Box::new(MockSttProvider::default());
    let stream = provider.start_listening().await;
    assert_pin_stream(stream);
}

#[tokio::test]
async fn test_stt_provider_transcribes() {
    let provider = MockSttProvider::with_transcript("hello world");
    let mut stream = provider.start_listening().await;
    let item = futures::StreamExt::next(&mut stream).await;
    assert_eq!(item.unwrap().as_str(), "hello world");
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cd src/crates/runtime-ports
cargo test --features stt voice
# 预期：编译失败（stt 模块不存在）
```

- [ ] **步骤 3：实现 SttProvider trait**

```rust
// src/crates/runtime-ports/src/voice/stt.rs

use async_trait::async_trait;
use futures::Stream;

/// 配置项，由配置层注入
#[derive(Debug, Clone)]
pub struct SttConfig {
    /// 语言，BCP-47 格式，如 "zh-CN"
    pub language: String,
    /// provider 特定参数
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// 单条识别结果
#[derive(Debug, Clone)]
pub struct SttResult {
    /// 识别的文本
    pub text: String,
    /// 是否为最终结果（false 表示中间结果）
    pub is_final: bool,
    /// 置信度 0-1
    pub confidence: Option<f32>,
}

#[async_trait]
pub trait SttProvider: Send + Sync {
    /// 开始监听，返回实时转写文本流
    async fn start_listening(&self) -> Result<Box<dyn Stream<Item = SttResult> + Send + Unpin>, SttError>;
    /// 停止监听
    async fn stop_listening(&self) -> Result<(), SttError>;
    /// 获取 provider 名称
    fn provider_name(&self) -> &'static str;
}

#[derive(Debug, thiserror::Error)]
pub enum SttError {
    #[error("microphone not available: {0}")]
    MicrophoneNotAvailable(String),
    #[error("permission denied")]
    PermissionDenied,
    #[error("provider error: {0}")]
    ProviderError(String),
    #[error("not supported")]
    NotSupported,
}
```

- [ ] **步骤 4：运行测试验证通过**

```bash
cargo test -p bitfun-runtime-ports voice
# 预期：PASS
```

- [ ] **步骤 5：Commit**

```bash
git add src/crates/runtime-ports/src/voice/stt.rs
git commit -m "feat(voice): add SttProvider trait in runtime-ports"
```

---

#### 任务 2：定义 TTS Provider trait

**文件：**
- 创建：`src/crates/runtime-ports/src/voice/tts.rs`

- [ ] **步骤 1：编写失败的测试**

```rust
#[tokio::test]
async fn test_tts_provider_speaks() {
    let provider = MockTtsProvider::default();
    provider.speak("hello world").await.expect("speak should succeed");
    assert!(provider.spoken_texts().contains(&"hello world".to_string()));
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cargo test -p bitfun-runtime-ports voice
# 预期：编译失败（tts 模块不存在）
```

- [ ] **步骤 3：实现 TtsProvider trait**

```rust
// src/crates/runtime-ports/src/voice/tts.rs

use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TtsConfig {
    pub language: String,
    pub voice: String,
    pub speed: f32,   // 0.5 - 2.0
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[async_trait]
pub trait TtsProvider: Send + Sync {
    /// 播报文本，可流式或等待
    async fn speak(&self, text: &str) -> Result<(), TtsError>;
    /// 停止当前播报
    async fn stop(&self) -> Result<(), TtsError>;
    /// 获取可用音色列表
    fn available_voices(&self) -> Vec<TtsVoice>;
    fn set_voice(&self, voice_id: &str);
    fn provider_name(&self) -> &'static str;
}

#[derive(Debug, Clone)]
pub struct TtsVoice {
    pub id: String,
    pub name: String,
    pub language: String,
    pub gender: &'static str,
}

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("audio output not available: {0}")]
    AudioOutputNotAvailable(String),
    #[error("permission denied")]
    PermissionDenied,
    #[error("provider error: {0}")]
    ProviderError(String),
    #[error("not supported")]
    NotSupported,
}
```

- [ ] **步骤 4：运行测试验证通过**

- [ ] **步骤 5：Commit**

```bash
git add src/crates/runtime-ports/src/voice/tts.rs src/crates/runtime-ports/src/voice/mod.rs
git commit -m "feat(voice): add TtsProvider trait in runtime-ports"
```

---

### 阶段二：Provider 实现（services-integrations）

#### 任务 3：实现 WebSpeech STT Provider（浏览器）

**文件：**
- 创建：`src/crates/services-integrations/src/voice/stt/web_speech.rs`
- 修改：`src/crates/services-integrations/src/voice/stt/mod.rs`

- [ ] **步骤 1：实现 WebSpeechProvider**

WebSpeech API 是浏览器原生 API，需要通过 JS interop 调用：
- 使用 `wasm-bindgen` + `web-sys` 调用浏览器 `webkitSpeechRecognition`
- 结果通过 channel 传回 Rust 端

```rust
// 核心思路：
// 1. WebSpeechProvider 持有一个 Arc<JsValue> 指向 window.SpeechRecognition
// 2. Rust 端通过 wasm 回调接收识别结果
// 3. start_listening() 启动 JS侧监听，返回 Stream
```

- [ ] **步骤 2：在 mod.rs 中注册 provider**

```rust
// src/crates/services-integrations/src/voice/stt/mod.rs

pub struct WebSpeechSttProvider { /* ... */ }

impl SttProvider for WebSpeechSttProvider {
    fn provider_name(&self) -> &'static str { "webspeech" }
    // ...
}

pub fn all_stt_providers() -> Vec<(&'static str, fn() -> Box<dyn SttProvider>)> {
    vec![
        ("webspeech", || Box::new(WebSpeechSttProvider::new())),
        // future: ("faster-whisper", || Box::new(FasterWhisperProvider::new())),
    ]
}
```

- [ ] **步骤 3：Commit**

```bash
git add src/crates/services-integrations/src/voice/stt/
git commit -m "feat(voice): add WebSpeech STT provider"
```

---

#### 任务 4：实现 Edge TTS Provider

**文件：**
- 创建：`src/crates/services-integrations/src/voice/tts/edge_tts.rs`
- 修改：`src/crates/services-integrations/src/voice/tts/mod.rs`

- [ ] **步骤 1：实现 EdgeTtsProvider**

Edge TTS 是微软的免费 TTS 服务，通过 HTTP 调用：
- 发送 HTTP 请求到 Edge TTS endpoint
- 获取音频数据后通过 `rodio` 或 `cpal` 播放
- 或者返回 audio bytes 让调用方处理

```rust
pub struct EdgeTtsProvider {
    config: TtsConfig,
    http_client: reqwest::Client,
}

impl TtsProvider for EdgeTtsProvider {
    fn provider_name(&self) -> &'static str { "edge" }
    // 调用 Edge TTS API，下载音频，播放
}
```

- [ ] **步骤 2：Commit**

```bash
git add src/crates/services-integrations/src/voice/tts/
git commit -m "feat(voice): add Edge TTS provider"
```

---

### 阶段三：配置集成（core config）

#### 任务 5：扩展 AiConfig 添加 VoiceConfig

**文件：**
- 修改：`src/crates/core/src/service/config/types.rs`

- [ ] **步骤 1：添加 VoiceConfig 结构**

```rust
/// Voice 功能配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VoiceConfig {
    /// 是否启用语音输入
    pub stt_enabled: bool,
    /// 是否启用语音播报
    pub tts_enabled: bool,
    /// STT provider: "webspeech" | "faster-whisper" | "groq" | "openai"
    pub stt_provider: String,
    /// TTS provider: "edge" | "elevenlabs" | "openai"
    pub tts_provider: String,
    /// 语言
    pub language: String,
    /// TTS 音色
    pub tts_voice: String,
    /// TTS 语速 0.5-2.0
    pub tts_speed: f32,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            stt_enabled: false,
            tts_enabled: false,
            stt_provider: "webspeech".to_string(),
            tts_provider: "edge".to_string(),
            language: "zh-CN".to_string(),
            tts_voice: "zh-CN-XiaoxiaoNeural".to_string(),
            tts_speed: 1.0,
        }
    }
}
```

- [ ] **步骤 2：将 voice 配置挂到 AiConfig**

在 `AiConfig` 结构体中添加 `pub voice: VoiceConfig`，并设置默认值。

- [ ] **步骤 3：Commit**

```bash
git add src/crates/core/src/service/config/types.rs
git commit -m "feat(voice): add VoiceConfig to AiConfig"
```

---

### 阶段四：Tauri API 暴露（desktop）

#### 任务 6：暴露 Voice API

**文件：**
- 创建/修改：`src/apps/desktop/src/api/voice.rs`

- [ ] **步骤 1：实现 Tauri command**

```rust
#[tauri::command]
pub async fn voice_start_listening(
    state: State<'_, AppState>,
) -> Result<String, String>;

#[tauri::command]
pub async fn voice_speak(
    state: State<'_, AppState>,
    text: String,
) -> Result<(), String>;

#[tauri::command]
pub async fn voice_stop(state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
pub async fn voice_get_status(
    state: State<'_, AppState>,
) -> Result<VoiceStatus, String>;
```

- [ ] **步骤 2：Commit**

```bash
git add src/apps/desktop/src/api/voice.rs
git commit -m "feat(voice): add Tauri voice commands"
```

---

### 阶段五：前端集成

#### 任务 7：前端 VoiceService 封装

**文件：**
- 创建：`src/web-ui/src/infrastructure/voice/VoiceService.ts`
- 创建：`src/web-ui/src/infrastructure/voice/SttProviderFactory.ts`
- 创建：`src/web-ui/src/infrastructure/voice/TtsProviderFactory.ts`

- [ ] **步骤 1：实现 VoiceService**

```typescript
export class VoiceService {
  private sttProvider: SttProvider;
  private ttsProvider: TtsProvider;

  constructor(config: VoiceConfig) {
    this.sttProvider = SttProviderFactory.create(config.sttProvider, config);
    this.ttsProvider = TtsProviderFactory.create(config.ttsProvider, config);
  }

  startListening(onChunk: (text: string) => void): void;
  stopListening(): void;
  speak(text: string): Promise<void>;
  stopSpeaking(): void;
  isListening(): boolean;
}
```

- [ ] **步骤 2：在 ChatInput 中添加语音按钮**

在 `ChatInput.tsx` 添加：
- 麦克风图标按钮（按住说话 / 点击切换）
- 识别中的视觉反馈（音频波形动画）
- 识别结果自动填入输入框

- [ ] **步骤 3：Commit**

```bash
git add src/web-ui/src/infrastructure/voice/
git add src/web-ui/src/flow_chat/components/ChatInput.tsx
git commit -m "feat(voice): frontend VoiceService and ChatInput integration"
```

---

### 阶段六：测试验证

#### 任务 8：端到端验证

- [ ] **步骤 1：验证 Web UI 编译通过**

```bash
cd src/web-ui
pnpm run type-check
# 预期：无类型错误
```

- [ ] **步骤 2：验证 Rust 编译通过**

```bash
cargo check --workspace
# 预期：编译成功
```

- [ ] **步骤 3：手动测试流程**

1. 打开 ChatInput，按住麦克风按钮说话
2. 松开后文字自动填入输入框
3. 发送消息，AI 回复自动播报（如果 TTS 开启）
4. 通过配置切换不同 provider

---

## 自检清单

- [ ] STT/TTS trait 在 runtime-ports 中定义，无具体实现依赖
- [ ] 各 provider 实现独立，新增 provider 只需实现 trait
- [ ] 默认配置使用免费 provider（WebSpeech + Edge TTS），零 API Key 可用
- [ ] 配置通过 `AiConfig.voice` 集中管理，支持运行时切换
- [ ] 前端通过 `VoiceService` 封装调用，与 Tauri command 解耦
- [ ] ChatInput 集成语音输入，不破坏现有键盘输入流程
- [ ] 所有 provider 可通过 feature flag 开关，避免不需要时增加二进制大小

---

## 执行方式

计划已完成并保存到 `docs/superpowers/plans/2026-06-03-voice-service-design.md`。两种执行方式：

**1. 子代理驱动（推荐）** - 每个任务调度一个新的子代理，任务间进行审查，快速迭代

**2. 内联执行** - 在当前会话中使用 executing-plans 执行任务，批量执行并设有检查点

**选哪种方式？**
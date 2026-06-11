use bitfun_runtime_ports::voice::tts::{TtsConfig, TtsError, TtsProvider, TtsVoice};
use async_trait::async_trait;

pub struct EdgeTtsProvider {
    config: TtsConfig,
}

impl EdgeTtsProvider {
    pub fn new(config: TtsConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl TtsProvider for EdgeTtsProvider {
    async fn speak(&self, text: &str) -> Result<(), TtsError> {
        Err(TtsError::NotSupported)
    }

    async fn stop(&self) -> Result<(), TtsError> {
        Err(TtsError::NotSupported)
    }

    fn available_voices(&self) -> Vec<TtsVoice> {
        vec![
            TtsVoice {
                id: "zh-CN-XiaoxiaoNeural".to_string(),
                name: "Xiaoxiao".to_string(),
                language: "zh-CN".to_string(),
                gender: "female",
            },
            TtsVoice {
                id: "zh-CN-YunxiNeural".to_string(),
                name: "Yunxi".to_string(),
                language: "zh-CN".to_string(),
                gender: "male",
            },
        ]
    }

    fn set_voice(&self, _voice_id: &str) {}

    fn provider_name(&self) -> &'static str {
        "edge"
    }
}

pub fn all_tts_providers() -> Vec<(&'static str, fn(TtsConfig) -> Box<dyn TtsProvider>)> {
    vec![("edge", |config| Box::new(EdgeTtsProvider::new(config)))]
}
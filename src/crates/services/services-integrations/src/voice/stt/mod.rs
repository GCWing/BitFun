use bitfun_runtime_ports::voice::stt::{SttConfig, SttError, SttProvider, SttResult};
use futures::Stream;

pub struct WebSpeechSttProvider {
    config: SttConfig,
}

impl WebSpeechSttProvider {
    pub fn new(config: SttConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl SttProvider for WebSpeechSttProvider {
    async fn start_listening(
        &self,
    ) -> Result<Box<dyn Stream<Item = SttResult> + Send + Unpin>, SttError> {
        Err(SttError::NotSupported)
    }

    async fn stop_listening(&self) -> Result<(), SttError> {
        Err(SttError::NotSupported)
    }

    fn provider_name(&self) -> &'static str {
        "webspeech"
    }
}

pub fn all_stt_providers() -> Vec<(&'static str, fn(SttConfig) -> Box<dyn SttProvider>)> {
    vec![("webspeech", |config| Box::new(WebSpeechSttProvider::new(config)))]
}

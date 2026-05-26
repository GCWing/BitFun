//! AI infrastructure
//!
//! Provides AI clients and related services

pub mod client_factory;
pub mod tool_call_accumulator;

use std::time::Duration;

pub use bitfun_ai_adapters::providers;
pub use bitfun_ai_adapters::stream as ai_stream_handlers;

pub use bitfun_ai_adapters::{
    AIClient, StreamOptions, StreamResponse, DEFAULT_STREAM_IDLE_TIMEOUT_SECS,
    DEFAULT_STREAM_TTFT_TIMEOUT_SECS, REASONING_STREAM_TTFT_TIMEOUT_SECS,
};
pub use client_factory::{
    get_global_ai_client_factory, initialize_global_ai_client_factory, AIClientFactory,
};

use crate::service::config::types::{AIConfig, AIModelConfig, ReasoningMode};

pub fn build_stream_options(config: &AIConfig) -> StreamOptions {
    build_stream_options_for_model(config, None)
}

pub fn build_stream_options_for_model(
    config: &AIConfig,
    model_config: Option<&AIModelConfig>,
) -> StreamOptions {
    let idle_timeout = config.stream_idle_timeout_secs.map(Duration::from_secs);

    let base_ttft_secs = config
        .stream_ttft_timeout_secs
        .or(Some(DEFAULT_STREAM_TTFT_TIMEOUT_SECS));

    let ttft_secs = match (base_ttft_secs, model_config) {
        (Some(secs), Some(model))
            if matches!(
                model.effective_reasoning_mode(),
                ReasoningMode::Enabled | ReasoningMode::Adaptive
            ) =>
        {
            Some(secs.max(REASONING_STREAM_TTFT_TIMEOUT_SECS))
        }
        (secs, _) => secs,
    };

    StreamOptions {
        idle_timeout,
        ttft_timeout: ttft_secs.map(Duration::from_secs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::config::types::AIModelConfig;

    #[test]
    fn reasoning_models_use_extended_ttft_timeout() {
        let config = AIConfig::default();
        let mut model = AIModelConfig::default();
        model.reasoning_mode = Some(ReasoningMode::Enabled);

        let options = build_stream_options_for_model(&config, Some(&model));

        assert_eq!(
            options.ttft_timeout,
            Some(Duration::from_secs(REASONING_STREAM_TTFT_TIMEOUT_SECS))
        );
        assert_eq!(
            options.idle_timeout,
            Some(Duration::from_secs(DEFAULT_STREAM_IDLE_TIMEOUT_SECS))
        );
    }
}

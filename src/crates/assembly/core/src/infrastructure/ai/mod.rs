//! AI infrastructure
//!
//! Provides AI clients and related services

pub mod client_factory;
pub mod tool_call_accumulator;

use std::time::Duration;

pub use bitfun_ai_adapters::providers;
pub use bitfun_ai_adapters::stream as ai_stream_handlers;

pub use bitfun_ai_adapters::{AIClient, StreamOptions, StreamResponse};
pub use client_factory::{
    get_global_ai_client_factory, initialize_global_ai_client_factory, AIClientFactory,
};

use crate::service::config::types::{AIConfig, AIModelConfig};

pub fn build_stream_options(config: &AIConfig) -> StreamOptions {
    build_stream_options_for_model(config, None)
}

pub fn build_stream_options_for_model(
    config: &AIConfig,
    _model_config: Option<&AIModelConfig>,
) -> StreamOptions {
    let idle_timeout = config.stream_idle_timeout_secs.map(Duration::from_secs);

    StreamOptions {
        idle_timeout,
        ttft_timeout: config.stream_ttft_timeout_secs.map(Duration::from_secs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::config::types::AIModelConfig;

    #[test]
    fn model_reasoning_mode_does_not_override_stream_timeouts() {
        let config = AIConfig::default();
        let mut model = AIModelConfig::default();
        model.reasoning_mode = Some(crate::service::config::types::ReasoningMode::Enabled);

        let options = build_stream_options_for_model(&config, Some(&model));

        assert_eq!(options.ttft_timeout, Some(Duration::from_secs(600)));
        assert_eq!(options.idle_timeout, Some(Duration::from_secs(600)));
    }

    #[test]
    fn explicit_none_stream_timeouts_mean_wait_indefinitely() {
        let mut config = AIConfig::default();
        config.stream_idle_timeout_secs = None;
        config.stream_ttft_timeout_secs = None;

        let options = build_stream_options_for_model(&config, None);

        assert_eq!(options.ttft_timeout, None);
        assert_eq!(options.idle_timeout, None);
    }
}

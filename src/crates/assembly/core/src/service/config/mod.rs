//! Unified configuration service module
//!
//! A complete configuration management system based on the Provider mechanism.

#[cfg(feature = "product-full")]
pub mod agent_profile_project_store;
pub mod app_language;
pub mod factory;
pub mod global;
pub mod manager;
#[cfg(feature = "product-full")]
pub mod mode_config_canonicalizer;
pub mod project_permission_store;
pub mod providers;
pub mod service;
pub mod types;

pub use app_language::{
    get_app_language, get_app_language_code, short_model_user_language_instruction,
};
pub use factory::ConfigFactory;
pub use global::{
    get_global_config_service, initialize_global_config, reload_global_config,
    subscribe_config_updates, ConfigUpdateEvent, GlobalConfigManager,
};
pub use manager::{ConfigManager, ConfigManagerSettings, ConfigStatistics};
#[cfg(feature = "product-full")]
pub use mode_config_canonicalizer::{
    canonicalize_agent_profile_configs, AgentProfileConfigCanonicalizationReport,
    AgentProfileConfigUpdateInfo,
};
pub use providers::ConfigProviderRegistry;
pub use service::{ConfigExport, ConfigHealthStatus, ConfigImportResult, ConfigService};
pub use types::*;

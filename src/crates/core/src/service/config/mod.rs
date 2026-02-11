//! Unified configuration service module
//!
//! A complete configuration management system based on the Provider mechanism.

pub mod factory;
pub mod global;
pub mod manager;
pub mod providers;
pub mod service;
pub mod tool_config_sync;
pub mod types;

pub use factory::ConfigFactory;
pub use global::{
    get_global_config_service, initialize_global_config, reload_global_config,
    subscribe_config_updates, ConfigUpdateEvent, GlobalConfigManager,
};
pub use manager::{ConfigManager, ConfigManagerSettings, ConfigStatistics};
pub use providers::ConfigProviderRegistry;
pub use service::{ConfigExport, ConfigHealthStatus, ConfigImportResult, ConfigService};
pub use tool_config_sync::{sync_tool_configs, ModeSyncInfo, SyncReport};
pub use types::*;

//! Unleash feature flag integration for taiji-engine.
//!
//! Provides kill-switch, A/B experiment grouping, and remote configuration
//! via a self-hosted Unleash server. Local evaluation (<1ms) is powered by
//! `unleash-yggdrasil`'s in-memory compiled state; offline cache path is
//! reserved for future disk-backed toggle persistence.
//!
//! Architecture (Wave A-C from C4-行为分析.md):
//!   Wave A: Unleash self-hosted + unleash-client SDK integration
//!   Wave B: kill switch + gradual rollout + remote config
//!   Wave C: A/B experiment (strategy comparison)

use std::path::PathBuf;
use std::sync::Arc;

use unleash_client::unleash::Unleash;
use unleash_client::unleash_yggdrasil::Context;

/// Feature flag client wrapping the Unleash Rust SDK.
///
/// Uses local evaluation: toggles are fetched in background polling cycles
/// and evaluated against an in-memory compiled state. When the Unleash
/// server is unreachable, the last-fetched state is retained — providing
/// natural offline resilience without an explicit file cache.
pub struct FeatureFlags {
    client: Arc<Unleash>,
    #[allow(dead_code)]
    offline_cache_path: Option<PathBuf>,
}

impl FeatureFlags {
    /// Create a new feature-flags client connected to a self-hosted Unleash instance.
    ///
    /// # Parameters
    /// - `api_url`: Unleash server API URL, e.g. `"http://localhost:4242/api/"`.
    /// - `app_name`: Application name registered in Unleash.
    /// - `instance_id`: Unique instance identifier (auto-generated if `None`).
    /// - `token`: Unleash API token (client key from `INIT_ADMIN_API_TOKENS`).
    /// - `offline_cache_path`: Reserved for future disk-backed toggle persistence.
    pub fn new(
        api_url: String,
        app_name: String,
        instance_id: Option<String>,
        token: String,
        offline_cache_path: Option<PathBuf>,
    ) -> Self {
        let client = Unleash::new(
            api_url,
            app_name,
            token,
            instance_id,
            None, // refresh_interval: default 15s
            None, // features_query: get all features
            None, // disable_metrics: false (send usage metrics)
        );

        FeatureFlags {
            client: Arc::new(client),
            offline_cache_path,
        }
    }

    /// Start background polling for feature toggle updates.
    ///
    /// This spawns an async loop that fetches toggle definitions from the
    /// Unleash server every `refresh_interval` (default 15s) and updates
    /// the in-memory compiled state. Call once at application startup.
    pub async fn start(&self) {
        self.client.start().await;
    }

    /// Stop background polling.
    pub fn stop(&self) {
        self.client.stop();
    }

    /// Kill switch: check whether a named strategy is enabled.
    ///
    /// Returns `false` when:
    /// - The toggle does not exist on the Unleash server.
    /// - The toggle exists but `enabled: false`.
    /// - The toggle's strategy constraints do not match the current context.
    ///
    /// Usage in pipeline node execution:
    /// ```ignore
    /// if !feature_flags.is_strategy_enabled("magnet_strategy") {
    ///     return Ok(()); // skip this node
    /// }
    /// ```
    pub fn is_strategy_enabled(&self, strategy_name: &str) -> bool {
        let mut ctx = Context::default();
        self.client.is_enabled(strategy_name, &mut ctx)
    }

    /// Get the A/B experiment variant name for a feature flag.
    ///
    /// Returns `Some(variant_name)` when the toggle is enabled and a
    /// non-disabled variant is assigned. Returns `None` when the toggle
    /// is off, unknown, or assigned the implicit "disabled" variant.
    ///
    /// Use the variant name to branch strategy logic:
    /// ```ignore
    /// match feature_flags.get_variant("magnet_algorithm") {
    ///     Some(v) if v == "v2_fast" => run_fast_path(),
    ///     Some(v) if v == "v3_precise" => run_precise_path(),
    ///     _ => run_default_path(), // includes None + "disabled" + unknown variants
    /// }
    /// ```
    pub fn get_variant(&self, flag_name: &str) -> Option<String> {
        let mut ctx = Context::default();
        let variant = self.client.get_variant(flag_name, &mut ctx);
        if variant.feature_enabled && variant.name != "disabled" {
            Some(variant.name)
        } else {
            None
        }
    }

    /// Read a remote configuration value from a variant's payload.
    ///
    /// Unleash feature toggles can carry a `payload` (type + value) on
    /// each variant. This method reads the `value` field and falls back
    /// to `default` when the toggle is unknown, disabled, or has no payload.
    ///
    /// Typical use: strategy parameters that operators can tune without
    /// redeploying the application.
    ///
    /// ```ignore
    /// let threshold: f64 = feature_flags
    ///     .get_config_value("magnet_threshold", "0.5")
    ///     .parse()
    ///     .unwrap_or(0.5);
    /// ```
    pub fn get_config_value(&self, key: &str, default: &str) -> String {
        let mut ctx = Context::default();
        let variant = self.client.get_variant(key, &mut ctx);
        variant
            .payload
            .map(|p| p.value)
            .unwrap_or_else(|| default.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_with_offline_cache_path() {
        let temp = std::env::temp_dir().join("taiji-unleash-test-cache");
        let _ = std::fs::create_dir_all(&temp);

        let ff = FeatureFlags::new(
            "http://localhost:4242/api/".into(),
            "taiji-test".into(),
            Some("test-instance-001".into()),
            "default:development.unleash-insecure-api-token".into(),
            Some(temp.clone()),
        );

        // Construction succeeds even without a running Unleash server —
        // the SDK initialises the in-memory engine in an empty state.
        assert!(ff.offline_cache_path.is_some());

        // Kill-switch: unknown toggles default to false (safe-off).
        assert!(!ff.is_strategy_enabled("nonexistent_strategy"));

        // A/B variant: unknown flags return None.
        assert_eq!(ff.get_variant("nonexistent_flag"), None);

        // Remote config: unknown keys fall back to the provided default.
        assert_eq!(
            ff.get_config_value("nonexistent_key", "fallback"),
            "fallback"
        );

        // Clean up.
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn offline_cache_path_none_is_ok() {
        let ff = FeatureFlags::new(
            "http://localhost:4242/api/".into(),
            "taiji-test".into(),
            None,
            "default:development.test-token".into(),
            None,
        );

        assert!(ff.offline_cache_path.is_none());
        // Defaults still work in offline mode.
        assert_eq!(
            ff.get_config_value("any_key", "safe_default"),
            "safe_default"
        );
    }

    #[test]
    fn kill_switch_unknown_strategy_returns_false() {
        let ff = FeatureFlags::new(
            "http://localhost:4242/api/".into(),
            "taiji-test".into(),
            Some("i-01".into()),
            "default:development.test-token".into(),
            None,
        );

        // Safety: never enable a strategy the server hasn't defined.
        assert!(!ff.is_strategy_enabled("magnet_strategy"));
        assert!(!ff.is_strategy_enabled("grid_trading_v2"));
    }
}

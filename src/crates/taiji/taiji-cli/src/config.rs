//! Product configuration: product.toml loading, deepMerge, and FeatureGate.
//!
//! Architecture:
//!   product.toml (base)  ────┐
//!   product.{tier}.toml  ──┬─┘
//!                          ▼
//!                   deepMerge → ResolvedConfig
//!                          │
//!                    FeatureGate::require()
//!                          │
//!                          ▼
//!                    CLI / MCP / ACP entry

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;
use tracing::info;

// ── Public types ──

/// Product tier enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Tier {
    Free,
    Standard,
    Ultimate,
}

impl Tier {
    /// Parse tier from a string (case-insensitive).
    #[allow(dead_code)]
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "free" => Some(Self::Free),
            "standard" => Some(Self::Standard),
            "ultimate" => Some(Self::Ultimate),
            _ => None,
        }
    }

    /// Return the config file suffix for this tier.
    pub(crate) fn config_suffix(&self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Standard => "standard",
            Self::Ultimate => "ultimate",
        }
    }
}

/// Result of a feature gate check.
#[derive(Debug)]
pub(crate) enum GateResult {
    Ok,
    UpgradeRequired {
        #[allow(dead_code)]
        feature: String,
        #[allow(dead_code)]
        current_tier: Tier,
        #[allow(dead_code)]
        message: String,
    },
}

/// Resolved product configuration after deepMerge.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedConfig {
    pub(crate) tier: Tier,
    pub(crate) features: HashMap<String, bool>,
    #[allow(dead_code)]
    pub(crate) data_sources: Vec<String>,
}

// ── Internal TOML types ──

#[derive(Debug, Deserialize)]
struct ProductConfigFile {
    #[allow(dead_code)]
    product: ProductMeta,
    tiers: HashMap<String, TierConfig>,
}

#[derive(Debug, Deserialize)]
struct ProductMeta {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    version: String,
}

#[derive(Debug, Deserialize, Clone)]
struct TierConfig {
    #[allow(dead_code)]
    label: String,
    features: HashMap<String, bool>,
    data: Option<TierDataConfig>,
}

#[derive(Debug, Deserialize, Clone)]
struct TierDataConfig {
    sources: Vec<String>,
}

// ── deepMerge ──

/// Deep-merge two feature maps: overlay values override base values.
/// This is the core of the tier override mechanism.
fn deep_merge_features(
    base: &HashMap<String, bool>,
    overlay: &HashMap<String, bool>,
) -> HashMap<String, bool> {
    let mut merged = base.clone();
    for (k, v) in overlay {
        merged.insert(k.clone(), *v);
    }
    merged
}

// ── Compile-time embedded config ──

/// product.toml embedded at compile time via `include_str!`.
/// This is the PRIMARY config source — works even when the file is missing
/// at runtime (e.g. in a bundled binary where product.toml is not shipped).
/// Path relative to `taiji-cli/src/config.rs` → `taiji/product.toml`.
const EMBEDDED_PRODUCT_TOML: &str = include_str!("../../product.toml");

/// Try to load product.toml from runtime filesystem; fall back to compile-time embed.
///
/// Strategy (priority):
///   1. Runtime file `{exe_dir}/product.toml` (for hot-reload / user customisation)
///   2. Runtime file `{CARGO_MANIFEST_DIR}/../product.toml` (dev workspace)
///   3. Compile-time `include_str!("../../product.toml")` (bundled — always available)
fn load_product_toml_str(tier: &Tier) -> Result<(String, Option<String>), String> {
    let tier_suffix = tier.config_suffix();
    let embedded = EMBEDDED_PRODUCT_TOML;

    // --- Primary: runtime filesystem read (allows hot-reload) ---
    let runtime_result = (|| -> Option<(String, Option<String>)> {
        // Strategy 1: alongside the executable
        let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        let base_path = exe_dir.join("product.toml");
        if base_path.exists() {
            let base = std::fs::read_to_string(&base_path).ok()?;
            let overlay_path = exe_dir.join(format!("product.{}.toml", tier_suffix));
            let overlay = if overlay_path.exists() {
                std::fs::read_to_string(&overlay_path).ok()
            } else {
                None
            };
            return Some((base, overlay));
        }
        // Strategy 2: dev workspace (CARGO_MANIFEST_DIR parent)
        let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent()?.to_path_buf();
        let base_path = dev_path.join("product.toml");
        if base_path.exists() {
            let base = std::fs::read_to_string(&base_path).ok()?;
            let overlay_path = dev_path.join(format!("product.{}.toml", tier_suffix));
            let overlay = if overlay_path.exists() {
                std::fs::read_to_string(&overlay_path).ok()
            } else {
                None
            };
            return Some((base, overlay));
        }
        None
     })();

    match runtime_result {
        Some((base, overlay)) => Ok((base, overlay)),
        None => {
            // --- Fallback: compile-time embedded ---
            info!("Falling back to compile-time embedded product.toml");
            // The embedded file contains all tiers in one file, so no separate overlay.
            Ok((embedded.to_string(), None))
        }
    }
}

/// Load the product configuration for a given tier.
///
/// Uses runtime filesystem as primary, compile-time `include_str!` as fallback.
/// This ensures the binary always has a valid config, even without shipping
/// `product.toml` alongside the executable.
pub(crate) fn load_product_config(tier: Tier) -> Result<ResolvedConfig, String> {
    let (base_content, overlay_content) = load_product_toml_str(&tier)?;

    // Parse base config
    let base: ProductConfigFile = toml::from_str(&base_content)
        .map_err(|e| format!("Failed to parse product.toml (base): {}", e))?;

    // Resolve the tier key name (lowercase)
    let tier_key = format!("{:?}", tier).to_lowercase();
    let tier_cfg = base.tiers.get(&tier_key)
        .ok_or_else(|| format!("Tier '{}' not found in product.toml", tier_key))?;

    let mut features = tier_cfg.features.clone();
    let data_sources = tier_cfg.data.clone()
        .map(|d| d.sources)
        .unwrap_or_default();

    // Apply overlay if present
    if let Some(overlay_str) = overlay_content {
        if let Ok(overlay) = toml::from_str::<ProductConfigFile>(&overlay_str) {
            if let Some(overlay_tier) = overlay.tiers.get(&tier_key) {
                features = deep_merge_features(&features, &overlay_tier.features);
            }
        }
    }

    info!("Product config loaded: tier={:?}, features={}", tier, features.len());
    Ok(ResolvedConfig {
        tier,
        features,
        data_sources,
    })
}

/// FeatureGate check.
///
/// Call this at every CLI/MCP/ACP entry point before executing the operation.
/// Returns `GateResult::Ok` if the feature is enabled, or
/// `GateResult::UpgradeRequired` with a message if not.
pub(crate) fn require(feature: &str, config: &ResolvedConfig) -> GateResult {
    match config.features.get(feature) {
        Some(true) => GateResult::Ok,
        Some(false) => GateResult::UpgradeRequired {
            feature: feature.to_string(),
            current_tier: config.tier,
            message: format!(
                "请升级到更高版本以使用「{}」功能（当前版本：{:?}）",
                feature, config.tier
            ),
        },
        None => GateResult::Ok, // unknown features default to allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_from_str() {
        assert_eq!(Tier::from_str("free"), Some(Tier::Free));
        assert_eq!(Tier::from_str("Standard"), Some(Tier::Standard));
        assert_eq!(Tier::from_str("ULTIMATE"), Some(Tier::Ultimate));
        assert_eq!(Tier::from_str("unknown"), None);
    }

    #[test]
    fn test_deep_merge_features() {
        let mut base = HashMap::new();
        base.insert("cli.backtest".into(), true);
        base.insert("cli.mcp".into(), true);

        let mut overlay = HashMap::new();
        overlay.insert("cli.mcp".into(), false);
        overlay.insert("cli.acp".into(), true);

        let merged = deep_merge_features(&base, &overlay);
        assert_eq!(merged.get("cli.backtest"), Some(&true));  // from base
        assert_eq!(merged.get("cli.mcp"), Some(&false));      // overlay overrides
        assert_eq!(merged.get("cli.acp"), Some(&true));       // from overlay
    }

    #[test]
    fn test_require_feature_enabled() {
        let mut features = HashMap::new();
        features.insert("cli.signal".into(), true);
        let config = ResolvedConfig {
            tier: Tier::Free,
            features,
            data_sources: vec![],
        };
        match require("cli.signal", &config) {
            GateResult::Ok => {} // expected
            _ => panic!("feature should be enabled"),
        }
    }

    #[test]
    fn test_require_feature_disabled() {
        let mut features = HashMap::new();
        features.insert("cli.backtest".into(), false);
        let config = ResolvedConfig {
            tier: Tier::Free,
            features,
            data_sources: vec![],
        };
        match require("cli.backtest", &config) {
            GateResult::UpgradeRequired { feature, .. } => {
                assert_eq!(feature, "cli.backtest");
            }
            _ => panic!("should require upgrade"),
        }
    }

    #[test]
    fn test_require_unknown_feature_defaults_ok() {
        let config = ResolvedConfig {
            tier: Tier::Free,
            features: HashMap::new(),
            data_sources: vec![],
        };
        match require("cli.nonexistent", &config) {
            GateResult::Ok => {} // expected: unknown features default to allowed
            _ => panic!("unknown feature should default to Ok"),
        }
    }
}

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Re-exported from [`taiji_content::DateRange`], the canonical definition.
pub use taiji_content::DateRange;

/// Walk-forward cross-validation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardConfig {
    /// Number of folds (default 4).
    #[serde(default = "default_folds")]
    pub folds: usize,
    /// Train ratio per fold (default 0.75, i.e. 75% train / 25% test).
    #[serde(default = "default_train_ratio")]
    pub train_ratio: f64,
}

fn default_folds() -> usize {
    4
}

fn default_train_ratio() -> f64 {
    0.75
}

/// Backtest engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    /// Instruments to backtest.
    pub instruments: Vec<String>,
    /// Date range for CSV data.
    pub date_range: DateRange,
    /// Initial account capital.
    pub initial_capital: f64,
    /// Commission per lot (single-side, e.g. 3.0 for rb).
    pub commission_per_lot: f64,
    /// Slippage in minimum ticks.
    pub slippage_ticks: u32,
    /// Path to pipeline YAML template.
    pub pipeline_template: PathBuf,
    /// Optional walk-forward validation config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub walk_forward: Option<WalkForwardConfig>,
    /// Multiplier per instrument (contract size, e.g. 10 for rb, 15 for ag).
    /// Defaults to 10.0 if not specified for an instrument.
    #[serde(default)]
    pub contract_multipliers: std::collections::HashMap<String, f64>,
}

impl BacktestConfig {
    /// Get contract multiplier for an instrument, defaults to 10.0.
    pub fn multiplier(&self, instrument: &str) -> f64 {
        self.contract_multipliers
            .get(instrument)
            .copied()
            .unwrap_or(10.0)
    }

    /// Clone the config, replacing instruments with a single instrument.
    /// Used for parallel backtest: one config per instrument.
    pub fn with_instrument(&self, instrument: &str) -> Self {
        let mut cfg = self.clone();
        cfg.instruments = vec![instrument.to_string()];
        cfg
    }
}

impl Default for WalkForwardConfig {
    fn default() -> Self {
        Self {
            folds: 4,
            train_ratio: 0.75,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_default_walk_forward_config() {
        let cfg = WalkForwardConfig::default();
        assert_eq!(cfg.folds, 4);
        assert!((cfg.train_ratio - 0.75).abs() < 1e-9);
    }

    #[test]
    fn test_multiplier_default() {
        let cfg = BacktestConfig {
            instruments: vec!["rb9999".into()],
            date_range: DateRange {
                start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            },
            initial_capital: 100_000.0,
            commission_per_lot: 3.0,
            slippage_ticks: 1,
            pipeline_template: PathBuf::from("pipeline.yaml"),
            walk_forward: None,
            contract_multipliers: std::collections::HashMap::new(),
        };
        assert!((cfg.multiplier("rb9999") - 10.0).abs() < 1e-9);
    }

    #[test]
    fn test_with_instrument_clones_and_replaces() {
        let cfg = BacktestConfig {
            instruments: vec!["rb9999".into(), "ag2506".into()],
            date_range: DateRange {
                start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            },
            initial_capital: 100_000.0,
            commission_per_lot: 3.0,
            slippage_ticks: 1,
            pipeline_template: PathBuf::from("pipeline.yaml"),
            walk_forward: None,
            contract_multipliers: std::collections::HashMap::new(),
        };
        let single = cfg.with_instrument("fg2509");
        assert_eq!(single.instruments, vec!["fg2509"]);
        assert_eq!(single.initial_capital, cfg.initial_capital);
        assert_eq!(single.date_range.start, cfg.date_range.start);
        assert_eq!(single.pipeline_template, cfg.pipeline_template);
        // Original unchanged
        assert_eq!(cfg.instruments.len(), 2);
    }
}

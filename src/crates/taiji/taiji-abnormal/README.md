# taiji-abnormal — Anomaly Detection Scorecard

Five indicator ComputeNodes (vol_regime, vol_anomaly, corr_fracture, gap_alert, trend_accel) plus a `ScorecardFusionNode` for weighted fusion. All metrics computed from OHLCV bars — zero L2 dependency, fully online per‑`on_bar()`. Includes internal statistical helpers: mean, std_dev, Pearson r, percentile, linear regression.

## Usage

```rust
use taiji_abnormal::scorecard::ScorecardFusionNode;
use taiji_abnormal::AbnormalWeights;
use taiji_engine::node::NodeConfig;

let mut node = ScorecardFusionNode::new("abnormal_fusion");
node.on_init(&NodeConfig::from_json(json!({
    "vol_regime": 0.25,
    "vol_anomaly": 0.25,
    "corr_fracture": 0.20,
    "gap_alert": 0.15,
    "trend_accel": 0.15,
    "warn_threshold": 0.5,
    "emergency_threshold": 0.8,
}))?, &mut state)?;

// Feed bars — on_calculate() returns AlertLevel + weighted score
let signals = node.on_calculate(&mut state)?;
```

```bash
cargo add taiji-abnormal
```

## Modules

| Module | Description |
|--------|-------------|
| `vol_regime` | Volume regime indicator (high/normal/low) |
| `vol_anomaly` | Volume anomaly (spike vs normal relative to history) |
| `corr_fracture` | Correlation fracture between instruments |
| `gap_alert` | Gap open detection with score |
| `trend_accel` | Trend acceleration / deceleration |
| `scorecard` | `ScorecardFusionNode` — weighted fusion → `AlertLevel` |

# taiji-backtest — Backtest Engine

CSV replay → Pipeline → signal matching → performance stats. Walk-forward cross-validation, trade record tracking, and statistical analysis (Sharpe, MaxDD, WinRate, Profit Factor).

## Usage

```rust
use taiji_backtest::config::BacktestConfig;
use taiji_backtest::runner::BacktestRunner;
use taiji_engine::config::PipelineConfig;

let pipeline_config = PipelineConfig::from_yaml(&std::fs::read_to_string("pipeline.yaml")?)?;
let backtest_config = BacktestConfig {
    instruments: vec!["rb9999".into()],
    date_range: taiji_backtest::config::DateRange {
        start: "2026-01-01".into(),
        end: "2026-12-31".into(),
    },
    initial_capital: 1_000_000.0,
    commission_per_lot: 3.0,
    slippage_ticks: 1,
    pipeline_template: "pipeline.yaml".into(),
    ..Default::default()
};

let runner = BacktestRunner::new(pipeline_config, backtest_config);
let result = runner.run()?;
println!("Sharpe: {:.2}, MaxDD: {:.2}%", result.stats.sharpe, result.stats.max_drawdown_pct);
```

```bash
cargo add taiji-backtest
```

## Modules

| Module | Description |
|--------|-------------|
| `config` | `BacktestConfig`, `DateRange`, `WalkForwardConfig` |
| `runner` | `BacktestRunner`, `BacktestResult` |
| `stats` | `PerformanceStats` — Sharpe, MaxDD, WinRate, ProfitFactor, Alpha |
| `trade_record` | `TradeRecord`, `Direction` |
| `walk_forward` | `WalkForwardValidator`, `WalkForwardReport` |

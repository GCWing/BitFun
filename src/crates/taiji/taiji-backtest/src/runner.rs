use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use taiji_engine::config::PipelineConfig;
use taiji_engine::pipeline::Pipeline;
use taiji_engine::types::signal::{Signal, SignalAction};
use taiji_engine::types::tick::TickData;

use crate::config::BacktestConfig;
use crate::stats::PerformanceStats;
use crate::trade_record::{Direction, TradeRecord};
use crate::walk_forward::{WalkForwardReport, WalkForwardValidator};

/// Complete backtest result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    /// All trades (open + closed).
    pub trades: Vec<TradeRecord>,
    /// Performance statistics.
    pub stats: PerformanceStats,
    /// Equity curve (equity after each closed trade, starts with initial capital).
    pub equity_curve: Vec<f64>,
    /// Drawdown curve (drawdown at each step of equity curve).
    pub drawdown_curve: Vec<f64>,
    /// Walk-forward validation report (if enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub walk_forward: Option<WalkForwardReport>,
}

/// Backtest engine: replay CSV ticks through a Pipeline and match signals into trades.
pub struct BacktestRunner {
    config: BacktestConfig,
    trades: Vec<TradeRecord>,
    equity_curve: Vec<f64>,
    /// Path to CSV tick data file. If None, derived from config conventions.
    csv_path: Option<PathBuf>,
}

/// Parsed CSV result: ticks with optional time range.
type CsvParseResult =
    Result<(Vec<TickData>, Option<DateTime<Utc>>, Option<DateTime<Utc>>), anyhow::Error>;

impl BacktestRunner {
    /// Create a new backtest runner.
    pub fn new(config: BacktestConfig) -> Self {
        let mut equity = Vec::with_capacity(1024);
        equity.push(config.initial_capital);
        Self {
            config,
            trades: Vec::new(),
            equity_curve: equity,
            csv_path: None,
        }
    }

    /// Set the CSV data path explicitly.
    pub fn set_csv_path(&mut self, path: PathBuf) {
        self.csv_path = Some(path);
    }

    /// Run the backtest.
    ///
    /// 1. Load and parse CSV tick data.
    /// 2. Build Pipeline from config template.
    /// 3. Feed ticks → collect signals.
    /// 4. Match signals into trades.
    /// 5. Compute stats.
    /// 6. Optionally run walk-forward validation.
    pub async fn run(&mut self) -> Result<BacktestResult, anyhow::Error> {
        let csv_path = self.resolve_csv_path()?;
        let csv_content = std::fs::read_to_string(&csv_path)?;
        self.run_with_csv(&csv_content)
    }

    /// Run backtest with pre-loaded CSV content.
    ///
    /// Used for parallel execution via [`run_parallel`] — each rayon thread
    /// receives pre-loaded CSV data and builds its own Pipeline.
    pub fn run_with_csv(&mut self, csv_content: &str) -> Result<BacktestResult, anyhow::Error> {
        // Parse CSV
        let (ticks, _start_dt, _end_dt) = self.parse_csv(csv_content)?;

        // Build pipeline
        let yaml_str = std::fs::read_to_string(&self.config.pipeline_template)?;
        let pipeline_config = PipelineConfig::from_yaml(&yaml_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse pipeline config: {}", e))?;
        let mut pipeline = Pipeline::from_config(pipeline_config)
            .map_err(|e| anyhow::anyhow!("Failed to create pipeline: {}", e))?;

        // Feed ticks and collect signals
        let all_signals = self.feed_ticks(&mut pipeline, &ticks)?;

        // Match signals into trades
        self.match_trades(&all_signals);

        // Force-close any open positions at last tick price
        if !ticks.is_empty() {
            let last_price = ticks.last().unwrap().last_price;
            let last_time = self.tick_time(ticks.last().unwrap());
            self.force_close_all(last_time, last_price);
        }

        // Compute drawdown curve
        let drawdown_curve = self.compute_drawdown_curve();

        // Compute stats
        let pnls: Vec<f64> = self.trades.iter().filter_map(|t| t.pnl).collect();
        let stats =
            PerformanceStats::compute(&pnls, &self.equity_curve, self.config.initial_capital, None);

        // Walk-forward validation
        let walk_forward = if let Some(ref wf_config) = self.config.walk_forward {
            let validator = WalkForwardValidator::new(wf_config.clone());
            let start_date = self.config.date_range.start;
            let end_date = self.config.date_range.end;
            // For walk-forward, use lightweight TickDataRef
            let tick_refs: Vec<crate::walk_forward::types::TickDataRef> = ticks
                .iter()
                .map(|t| crate::walk_forward::types::TickDataRef {
                    timestamp: self.tick_time(t),
                    instrument: t.instrument.clone(),
                    price: t.last_price,
                })
                .collect();
            let wf_report = validator.validate(
                &self.config.instruments[0],
                start_date,
                end_date,
                &tick_refs,
            );
            Some(wf_report)
        } else {
            None
        };

        Ok(BacktestResult {
            trades: self.trades.clone(),
            stats,
            equity_curve: self.equity_curve.clone(),
            drawdown_curve,
            walk_forward,
        })
    }

    /// Multi-instrument parallel backtest via rayon work-stealing.
    ///
    /// Phase 1: tokio concurrent CSV loading for all instruments.
    /// Phase 2: rayon parallel backtest computation (each instrument gets its
    /// own Pipeline built from the per-config `pipeline_template` path).
    ///
    /// # Rayon thread count
    ///
    /// Rayon defaults to [`std::thread::available_parallelism`] threads, which
    /// is guaranteed ≤ CPU core count. No explicit thread-pool tuning needed
    /// unless 30+ instruments are run on a machine with fewer cores — rayon's
    /// work-stealing naturally balances the load.
    pub fn run_parallel(
        configs: Vec<BacktestConfig>,
    ) -> Result<Vec<BacktestResult>, anyhow::Error> {
        use rayon::prelude::*;

        if configs.is_empty() {
            return Ok(vec![]);
        }

        // Resolve CSV paths via convention (same as resolve_csv_path without
        // explicit csv_path override)
        let csv_paths: Vec<PathBuf> = configs
            .iter()
            .map(|cfg| {
                let parent = cfg.pipeline_template.parent().unwrap_or(Path::new("."));
                let inst = cfg
                    .instruments
                    .first()
                    .map(|s| s.as_str())
                    .unwrap_or("unknown");
                let csv_name = format!(
                    "{}_{}_{}.csv",
                    inst, cfg.date_range.start, cfg.date_range.end
                );
                parent.join("csv").join(csv_name)
            })
            .collect();

        // Phase 1: Concurrent CSV loading via tokio
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| anyhow::anyhow!("Failed to create tokio runtime: {}", e))?;
        let csv_contents: Vec<String> = rt.block_on(async {
            let mut handles = Vec::with_capacity(csv_paths.len());
            for path in &csv_paths {
                let path = path.clone();
                handles.push(tokio::task::spawn(async move {
                    tokio::fs::read_to_string(&path)
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path.display(), e))
                }));
            }
            let mut results = Vec::with_capacity(handles.len());
            for h in handles {
                results.push(
                    h.await
                        .map_err(|e| anyhow::anyhow!("tokio join error: {}", e))??,
                );
            }
            Ok::<Vec<String>, anyhow::Error>(results)
        })?;

        // Phase 2: Parallel backtest via rayon
        configs
            .into_par_iter()
            .zip(csv_contents.into_par_iter())
            .map(|(config, csv_content)| {
                let mut runner = BacktestRunner::new(config);
                runner.run_with_csv(&csv_content)
            })
            .collect::<Result<Vec<_>, _>>()
    }

    // ── Private helpers ──

    fn resolve_csv_path(&self) -> Result<PathBuf, anyhow::Error> {
        if let Some(ref p) = self.csv_path {
            return Ok(p.clone());
        }
        // Convention: csv/{instrument}_{date_range}.csv in same dir as pipeline template
        let parent = self
            .config
            .pipeline_template
            .parent()
            .unwrap_or(Path::new("."));
        let inst = self
            .config
            .instruments
            .first()
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        let csv_name = format!(
            "{}_{}_{}.csv",
            inst, self.config.date_range.start, self.config.date_range.end
        );
        Ok(parent.join("csv").join(csv_name))
    }

    /// Parse CSV into Vec<TickData>. Returns (ticks, start_dt, end_dt).
    fn parse_csv(&self, csv_content: &str) -> CsvParseResult {
        let lines: Vec<&str> = csv_content.lines().collect();
        if lines.is_empty() {
            anyhow::bail!("CSV file is empty");
        }

        let header_fields = parse_csv_line(lines[0]);
        let mut column_map: HashMap<String, usize> = HashMap::new();
        for (i, col) in header_fields.iter().enumerate() {
            column_map.insert(col.clone(), i);
        }

        let mut ticks: Vec<TickData> = Vec::with_capacity(lines.len().saturating_sub(1));

        for line in lines.iter().skip(1) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let fields = parse_csv_line(line);
            let field_refs: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();

            let timestamp_ms = get_csv_f64(&field_refs, &column_map, "timestamp")
                .map(|v| v as i64)
                .or_else(|| {
                    get_csv_str(&field_refs, &column_map, "created_at").and_then(parse_created_at)
                })
                .unwrap_or(0);

            let tick = TickData {
                instrument: get_csv_str(&field_refs, &column_map, "instrument")
                    .or_else(|| get_csv_str(&field_refs, &column_map, "symbol"))
                    .unwrap_or("")
                    .to_string(),
                last_price: get_csv_f64(&field_refs, &column_map, "price")
                    .unwrap_or(0.0)
                    .max(0.0),
                open_price: get_csv_f64(&field_refs, &column_map, "open")
                    .unwrap_or(0.0)
                    .max(0.0),
                highest_price: get_csv_f64(&field_refs, &column_map, "high")
                    .unwrap_or(0.0)
                    .max(0.0),
                lowest_price: get_csv_f64(&field_refs, &column_map, "low")
                    .unwrap_or(0.0)
                    .max(0.0),
                volume: get_csv_f64_alt(&field_refs, &column_map, &["cum_volume", "volume"])
                    .unwrap_or(0.0)
                    .max(0.0),
                turnover: get_csv_f64_alt(&field_refs, &column_map, &["cum_amount", "amount"])
                    .unwrap_or(0.0)
                    .max(0.0),
                open_interest: get_csv_f64_alt(
                    &field_refs,
                    &column_map,
                    &["cum_position", "open_interest"],
                )
                .unwrap_or(0.0)
                .max(0.0),
                bid_price1: get_csv_f64_alt(&field_refs, &column_map, &["bid_p", "bid_price1"])
                    .unwrap_or(0.0)
                    .max(0.0),
                ask_price1: get_csv_f64_alt(&field_refs, &column_map, &["ask_p", "ask_price1"])
                    .unwrap_or(0.0)
                    .max(0.0),
                trade_type: get_csv_f64(&field_refs, &column_map, "trade_type"),
                timestamp_ms,
                ..Default::default()
            };

            ticks.push(tick);
        }

        let start_dt = ticks.first().map(|t| self.tick_time(t));
        let end_dt = ticks.last().map(|t| self.tick_time(t));

        Ok((ticks, start_dt, end_dt))
    }

    /// Feed ticks through pipeline, collecting all signals.
    fn feed_ticks(
        &mut self,
        pipeline: &mut Pipeline,
        ticks: &[TickData],
    ) -> Result<Vec<Signal>, anyhow::Error> {
        let mut signals: Vec<Signal> = Vec::new();

        for tick in ticks {
            match pipeline.feed_tick_direct(tick) {
                Ok(result) => {
                    signals.extend(result.signals);
                }
                Err(e) => {
                    // Log and continue — single tick errors shouldn't abort the run
                    eprintln!("Warning: feed_tick error: {}", e);
                }
            }
        }

        Ok(signals)
    }

    /// Match signals into trades. Simple one-position-per-instrument model.
    fn match_trades(&mut self, signals: &[Signal]) {
        // Open positions keyed by instrument
        let mut positions: HashMap<String, (usize, SignalAction)> = HashMap::new();
        let mut trade_seq: usize = 0;

        for signal in signals {
            let multiplier = self.config.multiplier(&signal.instrument);
            // Entry-direction slippage only for open signals; exit slippage is applied later.
            let fill_price = match signal.action {
                SignalAction::Long => {
                    (signal.entry.unwrap_or(0.0) + self.config.slippage_ticks as f64).max(0.0)
                }
                SignalAction::Short => {
                    (signal.entry.unwrap_or(0.0) - self.config.slippage_ticks as f64).max(0.0)
                }
                // Close signals: no entry-direction slippage; exit slippage applied separately.
                _ => signal.entry.unwrap_or(0.0).max(0.0),
            };

            let entry_time = signal.timestamp;
            let confidence = Some(signal.confidence);
            let volume = signal.size.map(|s| s as u32).unwrap_or(1);

            match signal.action {
                SignalAction::Long | SignalAction::Short => {
                    // Close existing position if direction differs
                    if let Some(&(pos_idx, ref pos_dir)) = positions.get(&signal.instrument) {
                        let should_close = matches!(
                            (&signal.action, pos_dir),
                            (SignalAction::Long, SignalAction::Short)
                                | (SignalAction::Short, SignalAction::Long)
                                | (SignalAction::Long, SignalAction::CloseLong)
                                | (SignalAction::Short, SignalAction::CloseShort)
                        );
                        if should_close {
                            let exit_price =
                                self.apply_exit_slippage(fill_price, signal.direction());
                            self.trades[pos_idx].close(
                                entry_time,
                                exit_price,
                                "signal_reverse",
                                multiplier,
                            );
                            // Subtract commission from equity
                            let net_pnl = self.trades[pos_idx].pnl.unwrap_or(0.0)
                                - 2.0 * self.config.commission_per_lot;
                            self.trades[pos_idx].pnl = Some(net_pnl);
                            let last_eq = *self
                                .equity_curve
                                .last()
                                .unwrap_or(&self.config.initial_capital);
                            self.equity_curve.push(last_eq + net_pnl);
                            positions.remove(&signal.instrument);
                        }
                    }

                    // Open new position
                    let dir = match signal.action {
                        SignalAction::Long => Direction::Long,
                        SignalAction::Short => Direction::Short,
                        _ => unreachable!(),
                    };
                    trade_seq += 1;
                    let trade = TradeRecord::open(
                        trade_seq,
                        &signal.instrument,
                        entry_time,
                        dir,
                        fill_price,
                        volume,
                        confidence,
                    );
                    self.trades.push(trade);
                    positions.insert(
                        signal.instrument.clone(),
                        (self.trades.len() - 1, signal.action.clone()),
                    );
                }
                SignalAction::CloseLong | SignalAction::CloseShort => {
                    if let Some(&(pos_idx, _)) = positions.get(&signal.instrument) {
                        let exit_price = self.apply_exit_slippage(fill_price, signal.direction());
                        self.trades[pos_idx].close(
                            entry_time,
                            exit_price,
                            "signal_close",
                            multiplier,
                        );
                        let net_pnl = self.trades[pos_idx].pnl.unwrap_or(0.0)
                            - 2.0 * self.config.commission_per_lot;
                        self.trades[pos_idx].pnl = Some(net_pnl);
                        let last_eq = *self
                            .equity_curve
                            .last()
                            .unwrap_or(&self.config.initial_capital);
                        self.equity_curve.push(last_eq + net_pnl);
                        positions.remove(&signal.instrument);
                    }
                }
                SignalAction::Hold => { /* no-op */ }
            }
        }
    }

    /// Force-close all open positions at end of backtest.
    fn force_close_all(&mut self, exit_time: DateTime<Utc>, last_price: f64) {
        for trade in self.trades.iter_mut() {
            if trade.exit_time.is_none() {
                let multiplier = self.config.multiplier(&trade.instrument);
                trade.close(exit_time, last_price, "eos", multiplier);
                let net_pnl = trade.pnl.unwrap_or(0.0) - 2.0 * self.config.commission_per_lot;
                trade.pnl = Some(net_pnl);
                let last_eq = *self
                    .equity_curve
                    .last()
                    .unwrap_or(&self.config.initial_capital);
                self.equity_curve.push(last_eq + net_pnl);
            }
        }
    }

    fn apply_exit_slippage(&self, price: f64, direction: Option<Direction>) -> f64 {
        match direction {
            Some(Direction::Long) => price - self.config.slippage_ticks as f64,
            Some(Direction::Short) => price + self.config.slippage_ticks as f64,
            None => price,
        }
    }

    fn compute_drawdown_curve(&self) -> Vec<f64> {
        if self.equity_curve.is_empty() {
            return vec![];
        }
        let mut peak = self.equity_curve[0];
        self.equity_curve
            .iter()
            .map(|&eq| {
                if eq > peak {
                    peak = eq;
                }
                if peak > 0.0 {
                    (peak - eq) / peak
                } else {
                    0.0
                }
            })
            .collect()
    }

    fn tick_time(&self, tick: &TickData) -> DateTime<Utc> {
        chrono::DateTime::from_timestamp_millis(tick.timestamp_ms).unwrap_or_else(Utc::now)
    }
}

// ── Signal direction helper ──

trait SignalExt {
    fn direction(&self) -> Option<Direction>;
}

impl SignalExt for Signal {
    fn direction(&self) -> Option<Direction> {
        match self.action {
            SignalAction::Long => Some(Direction::Long),
            SignalAction::Short => Some(Direction::Short),
            SignalAction::CloseLong => Some(Direction::Long),
            SignalAction::CloseShort => Some(Direction::Short),
            SignalAction::Hold => None,
        }
    }
}

// ── CSV parsing helpers (shared with taiji-cli pattern) ──

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    fields.push(current.trim().to_string());
    fields
}

fn get_csv_f64(fields: &[&str], col_map: &HashMap<String, usize>, name: &str) -> Option<f64> {
    col_map
        .get(name)
        .and_then(|&idx| fields.get(idx))
        .and_then(|s| s.trim().parse::<f64>().ok())
}

fn get_csv_str<'a>(
    fields: &'a [&str],
    col_map: &HashMap<String, usize>,
    name: &str,
) -> Option<&'a str> {
    col_map
        .get(name)
        .and_then(|&idx| fields.get(idx).map(|s| s.trim()))
}

fn get_csv_f64_alt(
    fields: &[&str],
    col_map: &HashMap<String, usize>,
    names: &[&str],
) -> Option<f64> {
    for name in names {
        let v = get_csv_f64(fields, col_map, name);
        if v.is_some() {
            return v;
        }
    }
    None
}

fn parse_created_at(s: &str) -> Option<i64> {
    let rfc3339 = s.trim().replace(' ', "T");
    chrono::DateTime::parse_from_rfc3339(&rfc3339)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_backtest_runner_new() {
        let cfg = BacktestConfig {
            instruments: vec!["rb9999".into()],
            date_range: crate::config::DateRange {
                start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            },
            initial_capital: 100_000.0,
            commission_per_lot: 3.0,
            slippage_ticks: 1,
            pipeline_template: PathBuf::from("pipeline.yaml"),
            walk_forward: None,
            contract_multipliers: HashMap::new(),
        };
        let runner = BacktestRunner::new(cfg);
        assert_eq!(runner.equity_curve.len(), 1);
        assert!((runner.equity_curve[0] - 100_000.0).abs() < 1e-9);
        assert!(runner.trades.is_empty());
    }

    #[test]
    fn test_match_trades_long_open_close() {
        use chrono::TimeZone;
        use taiji_engine::types::signal::SignalAction;

        let cfg = BacktestConfig {
            instruments: vec!["rb9999".into()],
            date_range: crate::config::DateRange {
                start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 1, 2).unwrap(),
            },
            initial_capital: 100_000.0,
            commission_per_lot: 0.0,
            slippage_ticks: 1,
            pipeline_template: PathBuf::from("pipeline.yaml"),
            walk_forward: None,
            contract_multipliers: HashMap::new(),
        };
        let mut runner = BacktestRunner::new(cfg);

        let signals = vec![
            Signal {
                timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap(),
                instrument: "rb9999".into(),
                freq: taiji_engine::types::bar::Freq::F1,
                action: SignalAction::Long,
                entry: Some(4000.0),
                stop_loss: None,
                take_profit: Some(4100.0),
                size: Some(2.0),
                source: "test_node".into(),
                confidence: 0.9,
                metadata: HashMap::new(),
                disclaimer: None,
            },
            Signal {
                timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 14, 0, 0).unwrap(),
                instrument: "rb9999".into(),
                freq: taiji_engine::types::bar::Freq::F1,
                action: SignalAction::CloseLong,
                entry: Some(4100.0),
                stop_loss: None,
                take_profit: None,
                size: Some(2.0),
                source: "test_node".into(),
                confidence: 0.8,
                metadata: HashMap::new(),
                disclaimer: None,
            },
        ];

        runner.match_trades(&signals);

        assert_eq!(runner.trades.len(), 1);
        let trade = &runner.trades[0];
        assert_eq!(trade.instrument, "rb9999");
        assert_eq!(trade.direction, Direction::Long);
        assert_eq!(trade.entry_price, 4001.0); // 4000 + 1 tick slippage
        assert!(trade.exit_price.is_some());
        assert_eq!(trade.exit_price.unwrap(), 4099.0); // 4100 - 1 tick slippage
        assert_eq!(trade.exit_reason, "signal_close");
        // PnL = (4099 - 4001) * 2 * 10 = 1960
        assert!((trade.pnl.unwrap() - 1960.0).abs() < 1e-9);
    }

    // ── run_parallel tests ──

    #[test]
    fn test_run_parallel_two_instruments() {
        use std::io::Write;

        let tmp = std::env::temp_dir().join("taiji_parallel_test");
        let _ = std::fs::create_dir_all(&tmp);
        let csv_dir = tmp.join("csv");
        let _ = std::fs::create_dir_all(&csv_dir);

        // Write a minimal pipeline YAML (no-op nodes)
        let pipeline_yaml = tmp.join("pipeline.yaml");
        let yaml_content = r#"
name: "parallel_test"
version: "1.0"
bar_gen:
  modes: ["time"]
  time_freqs: ["1m"]
data_source:
  type: "none"
  config: {}
nodes:
  - id: "n1"
    type: "ma_cross"
    config: {}
    input_keys: []
    output_keys: ["signals:n1"]
"#;
        std::fs::write(&pipeline_yaml, yaml_content).unwrap();

        // Write CSV for rb9999 (2 ticks, one bar → no signal from ma_cross)
        let csv_rb = csv_dir.join("rb9999_2026-01-01_2026-01-01.csv");
        let mut f = std::fs::File::create(&csv_rb).unwrap();
        writeln!(f, "instrument,price,volume,open_interest,created_at").unwrap();
        writeln!(f, "rb9999,4000.0,100.0,100000.0,2026-01-01T09:00:00+08:00").unwrap();
        writeln!(f, "rb9999,4010.0,200.0,100000.0,2026-01-01T09:01:00+08:00").unwrap();

        // Write CSV for ag2506
        let csv_ag = csv_dir.join("ag2506_2026-01-01_2026-01-01.csv");
        let mut f = std::fs::File::create(&csv_ag).unwrap();
        writeln!(f, "instrument,price,volume,open_interest,created_at").unwrap();
        writeln!(f, "ag2506,5000.0,50.0,50000.0,2026-01-01T09:00:00+08:00").unwrap();
        writeln!(f, "ag2506,5010.0,100.0,50000.0,2026-01-01T09:01:00+08:00").unwrap();

        let base_cfg = BacktestConfig {
            instruments: vec!["rb9999".into(), "ag2506".into()],
            date_range: crate::config::DateRange {
                start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            },
            initial_capital: 100_000.0,
            commission_per_lot: 0.0,
            slippage_ticks: 0,
            pipeline_template: pipeline_yaml.clone(),
            walk_forward: None,
            contract_multipliers: HashMap::new(),
        };

        let configs: Vec<BacktestConfig> = base_cfg
            .instruments
            .iter()
            .map(|inst| base_cfg.with_instrument(inst))
            .collect();

        let results = BacktestRunner::run_parallel(configs).unwrap();
        assert_eq!(results.len(), 2, "should return one result per instrument");

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_run_parallel_empty_configs() {
        let results = BacktestRunner::run_parallel(vec![]).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_rayon_thread_count_within_cpu_cores() {
        let pool = rayon::ThreadPoolBuilder::new().build().unwrap();
        let n = pool.current_num_threads();
        let cores = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1);
        assert!(
            n <= cores,
            "rayon thread count {} should be <= CPU cores {}",
            n,
            cores
        );
    }
}

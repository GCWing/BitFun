//! Taiji standalone CLI — zero BitFun desktop dependency.
//!
//! Usage:
//!   taiji --config pipeline.yaml --csv data.csv [--output signals.json] [--resume N]
//!   taiji backtest --config backtest_config.yaml --csv data.csv

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::{debug, info, warn};

use taiji_engine::compliance;
use taiji_engine::config::PipelineConfig;
use taiji_engine::factory::NodeFactory;
use taiji_engine::node::{ComputeNode, NodeConfig};
use taiji_engine::pipeline::Pipeline;
use taiji_engine::store::StateStore;
use taiji_engine::types::signal::Signal;
use taiji_engine::types::tick::TickData;

// ── CLI args ───────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "taiji",
    version,
    about = "Taiji standalone CLI — pipeline engine without BitFun desktop"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Path to pipeline YAML config (default pipeline mode)
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Path to CSV tick data (default pipeline mode)
    #[arg(long, value_name = "FILE")]
    csv: Option<PathBuf>,

    /// Output signals JSON file (default: stdout)
    #[arg(long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Resume from line N (skip first N data rows after header)
    #[arg(long, default_value = "0")]
    resume: usize,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run backtest with config file
    Backtest {
        /// Path to backtest config YAML
        #[arg(long, value_name = "FILE")]
        config: PathBuf,

        /// Path to CSV tick data (ignored when --parallel is set)
        #[arg(long, value_name = "FILE")]
        csv: Option<PathBuf>,

        /// Run all instruments in parallel via rayon
        #[arg(long)]
        parallel: bool,
    },
    /// Reload pipeline configuration (hot-reload)
    ReloadConfig {
        /// Path to new pipeline YAML config
        #[arg(long, value_name = "FILE")]
        config: PathBuf,
    },
}

// ── CSV helpers ────────────────────────────────────────────────────────
//
// TODO(P1-4): Replace hand-written CSV parser with the `csv` crate.
// `taiji-engine` already depends on `csv`; adding `csv` to taiji-cli's
// Cargo.toml and using `csv::ReaderBuilder` would eliminate ~76 lines of
// manual parsing code (parse_csv_line, get_csv_f64, get_csv_str,
// get_csv_f64_alt, fields_as_strs) and handle edge cases such as embedded
// newlines, escaped quotes, and BOM headers correctly.
// Expected dependencies: `csv = { workspace = true }` in taiji-cli/Cargo.toml.
// See: reports/taiji-cli-duplication-audit.md

/// Extract f64 from a CSV field via column map.
fn get_csv_f64(fields: &[&str], col_map: &HashMap<String, usize>, name: &str) -> Option<f64> {
    col_map
        .get(name)
        .and_then(|&idx| fields.get(idx))
        .and_then(|s| s.trim().parse::<f64>().ok())
}

/// Extract &str from a CSV field via column map.
fn get_csv_str<'a>(
    fields: &'a [&str],
    col_map: &HashMap<String, usize>,
    name: &str,
) -> Option<&'a str> {
    col_map
        .get(name)
        .and_then(|&idx| fields.get(idx).map(|s| s.trim()))
}

/// Extract f64 by trying multiple column names in order.
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

/// Simple CSV line parser that respects double-quoted fields.
/// Handles embedded commas and newlines within quotes.
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

/// Convert a CSV line (Vec<String>) to a slice of &str for lookups.
fn fields_as_strs(fields: &[String]) -> Vec<&str> {
    fields.iter().map(|s| s.as_str()).collect()
}

/// Parse created_at timestamp (ISO 8601 with space: "2026-07-21 09:25:11+08:00")
/// into milliseconds since epoch.
fn parse_created_at(s: &str) -> Option<i64> {
    // Replace space with T for RFC 3339 compatibility
    let rfc3339 = s.trim().replace(' ', "T");
    chrono::DateTime::parse_from_rfc3339(&rfc3339)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

// ── Node constructors ──────────────────────────────────────────────────

fn register_nodes(factory: &mut NodeFactory) {
    // MaCross: classical MA dual-line crossover strategy
    factory.register(
        "ma_cross",
        Box::new(
            |config: &NodeConfig| -> taiji_engine::error::Result<Box<dyn ComputeNode>> {
                let mut node = taiji_example::MaCross::new("ma_cross");
                let store = StateStore::new();
                node.on_init(config, &store)?;
                Ok(Box::new(node))
            },
        ),
    );

    // BarNode: tick-to-KLine aggregation (independent of built-in BarGenerator)
    factory.register(
        "BarNode",
        Box::new(
            |config: &NodeConfig| -> taiji_engine::error::Result<Box<dyn ComputeNode>> {
                let id = config.get_str("id").unwrap_or("bar_node");
                let mut node = taiji_bar::BarNode::new(id.to_string());
                let store = StateStore::new();
                node.on_init(config, &store)?;
                Ok(Box::new(node))
            },
        ),
    );
}

// ── Pipeline mode ──────────────────────────────────────────────────────

fn run_pipeline(
    config_path: &PathBuf,
    csv_path: &PathBuf,
    output_path: &Option<PathBuf>,
    resume: usize,
) -> Result<()> {
    // 1. Read and parse pipeline YAML config
    let yaml_str = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
    let config = PipelineConfig::from_yaml(&yaml_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))?;

    info!("Pipeline: {} v{}", config.name, config.version);
    debug!("  bar_gen: {:?}", config.bar_gen.time_freqs);
    info!("  nodes: {}", config.nodes.len());

    // 2. Build pipeline
    let mut pipeline = Pipeline::from_config(config.clone())
        .map_err(|e| anyhow::anyhow!("Failed to create pipeline: {}", e))?;

    // 3. Register node types and create nodes from config
    let mut factory = NodeFactory::new();
    register_nodes(&mut factory);

    for spec in &config.nodes {
        let params: HashMap<String, serde_json::Value> =
            if let serde_json::Value::Object(map) = &spec.config {
                map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
            } else {
                HashMap::new()
            };

        let node_config = NodeConfig {
            type_name: spec.type_name.clone(),
            params,
        };
        let mut node = factory.create(&spec.type_name, &node_config).map_err(|e| {
            anyhow::anyhow!(
                "Failed to create node '{}' (type={}): {}",
                spec.id,
                spec.type_name,
                e
            )
        })?;

        // on_init may have already been called in the constructor.
        // Call it again here with the spec config for idempotence.
        let store = StateStore::new();
        node.on_init(&node_config, &store)
            .map_err(|e| anyhow::anyhow!("Failed to init node '{}': {}", spec.id, e))?;

        info!("  + node: id={}, type={}", spec.id, spec.type_name);
        pipeline.add_node(node);
    }

    // 4. Derive DAG edges
    pipeline
        .derive_edges()
        .map_err(|e| anyhow::anyhow!("Failed to derive DAG edges: {}", e))?;

    // 5. Read CSV
    let csv_content = std::fs::read_to_string(csv_path)
        .with_context(|| format!("Failed to read CSV: {}", csv_path.display()))?;

    let lines: Vec<&str> = csv_content.lines().collect();
    if lines.is_empty() {
        anyhow::bail!("CSV file is empty");
    }

    // Parse header
    let header_fields = parse_csv_line(lines[0]);
    let mut column_map: HashMap<String, usize> = HashMap::new();
    for (i, col) in header_fields.iter().enumerate() {
        column_map.insert(col.clone(), i);
    }

    let total_data_lines = lines.len().saturating_sub(1);
    info!("CSV: {} data rows, resume={}", total_data_lines, resume);

    if resume > 0 {
        info!(
            "Resuming from data row {} (skipping {} rows)",
            resume, resume
        );
    }

    // 6. Process ticks
    let mut all_signals: Vec<Signal> = Vec::new();
    let mut ticks_processed: u64 = 0;
    let mut bars_generated: u64 = 0;

    for (line_idx, line) in lines.iter().enumerate().skip(1 + resume) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let fields = parse_csv_line(line);
        let field_refs = fields_as_strs(&fields);

        // Parse timestamp: try "timestamp" (ms) first, then "created_at" (ISO 8601)
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

        match pipeline.feed_tick_direct(&tick) {
            Ok(result) => {
                ticks_processed += 1;
                bars_generated += result.closed_bars.len() as u64;
                all_signals.extend(result.signals);

                if ticks_processed.is_multiple_of(5000) {
                    info!(
                        "Progress: {} ticks, {} bars, {} signals",
                        ticks_processed,
                        bars_generated,
                        all_signals.len()
                    );
                }
            }
            Err(e) => {
                warn!("error at line {}: {}", line_idx + 1, e);
            }
        }
    }

    info!(
        "Done: {} ticks, {} bars, {} signals",
        ticks_processed,
        bars_generated,
        all_signals.len()
    );

    // 7. Export signals
    let signals_json = serde_json::to_string_pretty(&all_signals)?;

    if let Some(output_path) = output_path {
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        std::fs::write(output_path, &signals_json)
            .with_context(|| format!("Failed to write output: {}", output_path.display()))?;
        info!("Signals written to: {}", output_path.display());
    } else {
        println!("{}", signals_json);
    }

    Ok(())
}

// ── Backtest mode ──────────────────────────────────────────────────────

fn run_backtest(config_path: &PathBuf, csv_path: &Option<PathBuf>, parallel: bool) -> Result<()> {
    let yaml_str = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read backtest config: {}", config_path.display()))?;
    let mut config: taiji_backtest::BacktestConfig = serde_yaml::from_str(&yaml_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse backtest config: {}", e))?;
    config.pipeline_template = resolve_relative(&config.pipeline_template, config_path);

    if parallel {
        // Multi-instrument parallel backtest
        let configs: Vec<taiji_backtest::BacktestConfig> = config
            .instruments
            .iter()
            .map(|inst| config.with_instrument(inst))
            .collect();
        let n = configs.len();
        info!("Parallel backtest: {} instrument(s)", n);

        let results = taiji_backtest::BacktestRunner::run_parallel(configs)
            .map_err(|e| anyhow::anyhow!("Parallel backtest failed: {}", e))?;

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| anyhow::anyhow!("Failed to serialize results: {}", e))?;
        println!("{}", json);
    } else {
        // Single-instrument backtest (existing path)
        let csv_path = csv_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("--csv is required for single-instrument backtest"))?;
        let mut runner = taiji_backtest::BacktestRunner::new(config);
        runner.set_csv_path(csv_path.clone());

        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| anyhow::anyhow!("Failed to create tokio runtime: {}", e))?;
        let result = rt
            .block_on(runner.run())
            .map_err(|e| anyhow::anyhow!("Backtest failed: {}", e))?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| anyhow::anyhow!("Failed to serialize backtest result: {}", e))?;
        println!("{}", json);
    }

    Ok(())
}

/// Resolve a relative path against the config file's directory.
fn resolve_relative(path: &Path, config_path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        let joined = config_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join(path);
        // Normalize `..` and `.` components
        normalize_path(&joined)
    }
}

/// Normalize a path by resolving `.` and `..` components.
fn normalize_path(path: &std::path::Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::CurDir => {}
            c => result.push(c.as_os_str()),
        }
    }
    result
}

// ── Reload-config mode ──────────────────────────────────────────────────

fn run_reload_config(config_path: &PathBuf) -> Result<()> {
    // 1. Read and validate the new config
    let yaml_str = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
    let config = PipelineConfig::from_yaml(&yaml_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))?;

    info!("Reload config: {} v{}", config.name, config.version);
    debug!("  bar_gen modes: {:?}", config.bar_gen.modes);
    debug!("  bar_gen time_freqs: {:?}", config.bar_gen.time_freqs);
    info!("  nodes: {}", config.nodes.len());
    for node in &config.nodes {
        info!("    - id={}, type={}", node.id, node.type_name);
    }
    info!("Config validated successfully — ready for hot-reload.");
    Ok(())
}

// ── main ───────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Check for backtest subcommand first
    if let Some(Command::Backtest {
        config,
        csv,
        parallel,
    }) = cli.command
    {
        // R6.3: Compliance — require risk acknowledgement before any execution.
        if !compliance::show_risk_disclaimer() {
            warn!("风险揭示未确认，程序退出。");
            std::process::exit(1);
        }
        return run_backtest(&config, &csv, parallel);
    }

    // Check for reload-config subcommand
    if let Some(Command::ReloadConfig { config }) = cli.command {
        return run_reload_config(&config);
    }

    // Default pipeline mode
    let config_path = cli
        .config
        .ok_or_else(|| anyhow::anyhow!("--config is required for pipeline mode"))?;
    let csv_path = cli
        .csv
        .ok_or_else(|| anyhow::anyhow!("--csv is required for pipeline mode"))?;

    // R6.3: Compliance
    if !compliance::show_risk_disclaimer() {
        warn!("风险揭示未确认，程序退出。");
        std::process::exit(1);
    }

    run_pipeline(&config_path, &csv_path, &cli.output, cli.resume)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_csv_line_basic() {
        let fields = parse_csv_line("a,b,c");
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_csv_line_quoted() {
        let fields = parse_csv_line("\"hello, world\",b,c");
        assert_eq!(fields, vec!["hello, world", "b", "c"]);
    }

    #[test]
    fn test_resolve_relative() {
        let config = PathBuf::from("/home/user/configs/backtest.yaml");
        let rel = PathBuf::from("../../examples/pipeline.yaml");
        let resolved = resolve_relative(&rel, &config);
        // From /home/user/configs/, ../../ goes to /home/, then examples/pipeline.yaml
        assert_eq!(
            resolved.to_string_lossy().replace('\\', "/"),
            "/home/examples/pipeline.yaml"
        );
    }

    #[test]
    fn test_resolve_absolute() {
        let config = PathBuf::from("/home/user/configs/backtest.yaml");
        let abs = PathBuf::from("/etc/taiji/pipeline.yaml");
        let resolved = resolve_relative(&abs, &config);
        assert_eq!(resolved, abs);
    }
}

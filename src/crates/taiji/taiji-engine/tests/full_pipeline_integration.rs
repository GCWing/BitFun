use std::collections::HashMap;
use std::path::PathBuf;

use taiji_engine::config::*;
use taiji_engine::node::{ComputeNode, NodeConfig};
use taiji_engine::pipeline::Pipeline;
use taiji_engine::source::datasource::{DataSource, DataSourceConfig};
use taiji_engine::source::replay::CsvReplaySource;
use taiji_engine::store::StateStore;
use taiji_engine::types::tick::TickData;

use taiji_bar::BarNode;
use taiji_example::MaCross;

// ── helpers ────────────────────────────────────────────────────────────

fn golden_csv_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../test_data/golden_tick/20260721/a2609/a2609_golden_20260721.csv")
}

/// Convert CsvReplaySource RawTick → TickData, matching the mapping in
/// Pipeline::feed_tick() (pipeline/mod.rs:152-167).
fn raw_tick_to_tick_data(raw: &taiji_engine::types::tick::RawTick) -> TickData {
    TickData {
        instrument: raw.instrument.clone(),
        timestamp_ms: raw.timestamp,
        last_price: raw.fields.get("price").copied().unwrap_or(0.0),
        open_price: raw.fields.get("open").copied().unwrap_or(0.0),
        highest_price: raw.fields.get("high").copied().unwrap_or(0.0),
        lowest_price: raw.fields.get("low").copied().unwrap_or(0.0),
        volume: raw
            .fields
            .get("cum_volume")
            .or_else(|| raw.fields.get("volume"))
            .copied()
            .unwrap_or(0.0),
        turnover: raw
            .fields
            .get("cum_amount")
            .or_else(|| raw.fields.get("amount"))
            .copied()
            .unwrap_or(0.0),
        open_interest: raw
            .fields
            .get("cum_position")
            .or_else(|| raw.fields.get("open_interest"))
            .copied()
            .unwrap_or(0.0),
        trade_type: raw.fields.get("trade_type").copied(),
        ..Default::default()
    }
}

// ── full_pipeline_csv_to_signal ────────────────────────────────────────

/// 全链路集成测试：CSV → CsvReplaySource → BarGenerator → DAG(BarNode+MaCross) → Signal
///
/// 使用 RealData（golden_tick CSV）、MaCross（零太极公式）、BarNode（真实 BarNode）、
/// 以及 Pipeline 现有 API（from_config + add_node + feed_tick_direct）。
#[test]
#[ignore]
fn full_pipeline_csv_to_signal() {
    // ── 1. 构建 PipelineConfig（BarNode + MaCross） ──
    let config = PipelineConfig {
        name: "integration-test".into(),
        version: "1.0".into(),
        bar_gen: BarGenConfig {
            modes: vec!["time".into()],
            time_freqs: vec!["1m".into()],
        },
        data_source: DataSourceSpec {
            type_name: "csv_replay".into(),
            config: serde_json::json!({}),
        },
        nodes: vec![
            NodeSpec {
                id: "bar_node".into(),
                type_name: "BarNode".into(),
                config: serde_json::json!({"freq": "1m"}),
                input_keys: vec![],
                output_keys: vec!["bars:1m".into()],
            },
            NodeSpec {
                id: "ma_cross".into(),
                type_name: "ma_cross".into(),
                config: serde_json::json!({"fast_period": 5, "slow_period": 20}),
                input_keys: vec!["bars:1m".into()],
                output_keys: vec!["signals:ma_cross".into()],
            },
        ],
    };

    // ── 2. Pipeline::from_config(config) ──
    let mut pipeline = Pipeline::from_config(config).expect("create pipeline");

    // ── 3. 注册 BarNode + MaCross → add_node → derive_edges ──
    let mut bar_node = BarNode::new("bar_node".into());
    let mut bar_config = NodeConfig::new();
    bar_config
        .params
        .insert("freq".into(), serde_json::json!("1m"));
    bar_node
        .on_init(&bar_config, &StateStore::new())
        .expect("init BarNode");
    pipeline.add_node(Box::new(bar_node));

    let mut ma_cross = MaCross::new("ma_cross");
    let mut ma_config = NodeConfig::new();
    ma_config
        .params
        .insert("fast_period".into(), serde_json::json!(5));
    ma_config
        .params
        .insert("slow_period".into(), serde_json::json!(20));
    ma_cross
        .on_init(&ma_config, &StateStore::new())
        .expect("init MaCross");
    pipeline.add_node(Box::new(ma_cross));

    pipeline.derive_edges().expect("derive DAG edges");

    // ── 4. CsvReplaySource 读 golden_tick CSV → connect ──
    let csv_path = golden_csv_path();
    assert!(
        csv_path.exists(),
        "golden tick CSV not found: {}",
        csv_path.display()
    );

    let mut source = CsvReplaySource::new(&csv_path, "csv:integration".into());
    let mut ds_params = HashMap::new();
    ds_params.insert(
        "csv_path".to_string(),
        serde_json::Value::String(csv_path.to_string_lossy().to_string()),
    );
    let ds_config = DataSourceConfig {
        type_name: "csv_replay".into(),
        params: ds_params,
    };
    source.connect(&ds_config).expect("connect CSV source");

    // ── 5. 循环 next_raw → feed_tick_direct ──
    let mut tick_count: u64 = 0;
    let mut all_signals = Vec::new();

    loop {
        let raw = match source.next_raw() {
            Ok(Some(r)) => r,
            Ok(None) => break,
            Err(e) => {
                eprintln!("CSV read error at tick {}: {}", tick_count + 1, e);
                break;
            }
        };

        let tick = raw_tick_to_tick_data(&raw);

        match pipeline.feed_tick_direct(&tick) {
            Ok(result) => {
                tick_count += 1;
                all_signals.extend(result.signals);
            }
            Err(e) => {
                eprintln!("feed_tick_direct error at tick {}: {}", tick_count + 1, e);
            }
        }
    }

    // ── 6. 断言：tick_count > 0, signals 非空 ──
    assert!(tick_count > 0, "should process at least one tick");
    assert!(
        !all_signals.is_empty(),
        "should generate at least one signal (got {} ticks, {} bars in state)",
        tick_count,
        pipeline.status().total_bars,
    );

    eprintln!(
        "full_pipeline_csv_to_signal: {} ticks, {} signals",
        tick_count,
        all_signals.len()
    );
}

//! E2E 全链路集成测试：golden_tick CSV → Pipeline → MA交叉 → 信号融合 → 模拟成交 → TradeRecord JSON
//! 风控由闭源 RiskMonitorChain 插件（通过 NodeFactory 注册）处理，此测试跳过风控步骤。
//!
//! 测试流程：
//! 1. 加载 test_data/golden_tick/ 中的 CSV
//! 2. CsvReplaySource → Pipeline::feed_tick_direct()
//! 3. Pipeline DAG（BarNode → MaCross）
//! 4. 信号输出 → FusionEngine 融合
//! 5. RiskMonitorChain 风控过滤
//! 6. OrderManager 模拟成交 → TradeRecord
//! 7. 验证 TradeRecord JSON Schema

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

use taiji_engine::config::*;
use taiji_engine::fusion::{AgentOutput, AgentWeights, Direction as FusionDirection, FusionEngine};
use taiji_engine::node::{ComputeNode, NodeConfig};
use taiji_engine::pipeline::Pipeline;
use taiji_engine::source::datasource::{DataSource, DataSourceConfig};
use taiji_engine::source::replay::CsvReplaySource;
use taiji_engine::store::StateStore;
use taiji_engine::types::signal::{Signal, SignalAction};
use taiji_engine::types::tick::TickData;

use taiji_backtest::TradeRecord;
use taiji_executor::types::{
    Direction as ExecDirection, Fill, Offset, OrderRequest, OrderStatus, OrderType,
};
use taiji_executor::OrderManager;

use taiji_bar::BarNode;
use taiji_example::MaCross;

// ── helpers ────────────────────────────────────────────────────────────

/// Resolve golden_tick CSV path.  Prefer `TAIJI_GOLDEN_TICK_CSV` env var;
/// otherwise fall back to `test_data/golden_tick/20260721/a2609/a2609_golden_20260721.csv`
/// relative to the workspace root.
fn golden_csv_path() -> PathBuf {
    if let Ok(p) = std::env::var("TAIJI_GOLDEN_TICK_CSV") {
        let pb = PathBuf::from(&p);
        if pb.exists() {
            return pb;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../test_data/golden_tick/20260721/a2609/a2609_golden_20260721.csv")
}

/// Convert CsvReplaySource RawTick → TickData, matching the mapping in
/// Pipeline::feed_tick() (pipeline/mod.rs:162-176).
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

/// Convert a Pipeline [`Signal`] into a fusion [`AgentOutput`].
fn signal_to_agent_output(signal: &Signal) -> AgentOutput {
    let direction = match signal.action {
        SignalAction::Long => FusionDirection::Long,
        SignalAction::Short => FusionDirection::Short,
        _ => FusionDirection::Neutral,
    };
    AgentOutput {
        agent_id: signal.source.clone(),
        direction,
        confidence: signal.confidence,
    }
}

/// Map [`SignalAction`] to executor direction + offset.
fn signal_to_exec_params(action: &SignalAction) -> Option<(ExecDirection, Offset)> {
    match action {
        SignalAction::Long => Some((ExecDirection::Buy, Offset::Open)),
        SignalAction::Short => Some((ExecDirection::Sell, Offset::Open)),
        SignalAction::CloseLong => Some((ExecDirection::Sell, Offset::Close)),
        SignalAction::CloseShort => Some((ExecDirection::Buy, Offset::Close)),
        SignalAction::Hold => None,
    }
}

/// Map [`SignalAction`] to backtest [`taiji_backtest::Direction`].
fn signal_to_trade_direction(action: &SignalAction) -> Option<taiji_backtest::Direction> {
    match action {
        SignalAction::Long | SignalAction::CloseShort => Some(taiji_backtest::Direction::Long),
        SignalAction::Short | SignalAction::CloseLong => Some(taiji_backtest::Direction::Short),
        SignalAction::Hold => None,
    }
}

/// Simple JSON schema validation for `TradeRecord[]`.
fn validate_trade_records_json(trades: &[TradeRecord]) {
    // Roundtrip
    let json = serde_json::to_string_pretty(trades).expect("serialize TradeRecord[] to JSON");
    let parsed: Vec<TradeRecord> =
        serde_json::from_str(&json).expect("deserialize TradeRecord[] from JSON");
    assert_eq!(parsed.len(), trades.len(), "roundtrip length mismatch");

    // Schema-level field presence
    let v: Value = serde_json::from_str(&json).unwrap();
    let arr = v.as_array().expect("top-level should be array");
    for (i, item) in arr.iter().enumerate() {
        let obj = item.as_object().unwrap_or_else(|| {
            panic!("trade[{}]: not a JSON object", i);
        });
        assert!(
            obj.contains_key("trade_id"),
            "trade[{}]: missing trade_id",
            i
        );
        assert!(
            obj.contains_key("instrument"),
            "trade[{}]: missing instrument",
            i
        );
        assert!(
            obj.contains_key("entry_time"),
            "trade[{}]: missing entry_time",
            i
        );
        assert!(
            obj.contains_key("direction"),
            "trade[{}]: missing direction",
            i
        );
        assert!(
            obj.contains_key("entry_price"),
            "trade[{}]: missing entry_price",
            i
        );
        assert!(obj.contains_key("volume"), "trade[{}]: missing volume", i);
    }

    // Per-record invariants
    for (i, t) in trades.iter().enumerate() {
        assert!(!t.trade_id.is_empty(), "trade[{}]: trade_id empty", i);
        assert!(!t.instrument.is_empty(), "trade[{}]: instrument empty", i);
        assert!(t.volume > 0, "trade[{}]: volume is 0", i);
        assert!(t.entry_price > 0.0, "trade[{}]: entry_price is 0", i);
        assert!(
            t.pnl.is_some(),
            "trade[{}]: pnl is None (should be closed)",
            i
        );
    }
}

// ── core pipeline runner ───────────────────────────────────────────────

/// Run the full E2E pipeline and return generated [`TradeRecord`]s.
async fn run_e2e_pipeline() -> Result<Vec<TradeRecord>, String> {
    // ── 1. Build PipelineConfig (BarNode + MaCross) ──
    let config = PipelineConfig {
        name: "e2e-full-trading".into(),
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

    // ── 2. Pipeline::from_config ──
    let mut pipeline = Pipeline::from_config(config).map_err(|e| e.to_string())?;

    // ── 3. Register BarNode + MaCross → add_node → derive_edges ──
    let mut bar_node = BarNode::new("bar_node".into());
    let mut bar_config = NodeConfig::new();
    bar_config
        .params
        .insert("freq".into(), serde_json::json!("1m"));
    bar_node
        .on_init(&bar_config, &StateStore::new())
        .map_err(|e| e.to_string())?;
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
        .map_err(|e| e.to_string())?;
    pipeline.add_node(Box::new(ma_cross));

    pipeline.derive_edges().map_err(|e| e.to_string())?;

    // ── 4. CsvReplaySource 读 golden_tick CSV → connect ──
    let csv_path = golden_csv_path();
    assert!(
        csv_path.exists(),
        "golden tick CSV not found: {}",
        csv_path.display()
    );

    let mut source = CsvReplaySource::new(&csv_path, "csv:e2e".into());
    let mut ds_params = HashMap::new();
    ds_params.insert(
        "csv_path".to_string(),
        serde_json::Value::String(csv_path.to_string_lossy().to_string()),
    );
    let ds_config = DataSourceConfig {
        type_name: "csv_replay".into(),
        params: ds_params,
    };
    source.connect(&ds_config).map_err(|e| e.to_string())?;

    // ── 5. 循环 next_raw → feed_tick_direct ──
    let mut tick_count: u64 = 0;
    let mut all_signals: Vec<Signal> = Vec::new();
    // Track last-known instrument & price to fill signals that omit them
    // (MaCross signals have empty instrument and no entry price).
    let mut last_instrument: String = String::new();
    let mut last_price: f64 = 0.0;

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

        // Track context for signals that omit instrument / entry
        if !tick.instrument.is_empty() {
            last_instrument = tick.instrument.clone();
        }
        if tick.last_price > 0.0 {
            last_price = tick.last_price;
        }

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

    assert!(tick_count > 0, "should process at least one tick");
    assert!(
        !all_signals.is_empty(),
        "should generate at least one signal (got {} ticks)",
        tick_count,
    );

    eprintln!(
        "E2E pipeline: {} ticks, {} raw signals",
        tick_count,
        all_signals.len()
    );

    // ── 6. FusionEngine — 将所有信号转为 AgentOutput → fuse ──
    let agent_outputs: Vec<AgentOutput> = all_signals.iter().map(signal_to_agent_output).collect();

    let fusion_engine = FusionEngine::new(AgentWeights::default(), None);
    let fusion_result = fusion_engine
        .fuse(&agent_outputs)
        .await
        .map_err(|e| e.to_string())?;

    eprintln!(
        "fusion: direction={:?} confidence={:.4} score={:.4} phase={:?}",
        fusion_result.direction,
        fusion_result.confidence,
        fusion_result.fusion_score,
        fusion_result.phase,
    );

    // ── 7. Signal → OrderManager → TradeRecord ──
    // Risk check skipped — closed-source RiskMonitorChain is a plugin via NodeFactory
    let order_mgr = OrderManager::new();
    let mut trades: Vec<TradeRecord> = Vec::new();

    for (i, signal) in all_signals.iter().enumerate() {
        // 跳过非可执行信号
        if signal.action == SignalAction::Hold {
            continue;
        }

        // 使用跟踪到的 instrument / 价格填补 MaCross 信号的空缺
        let instrument = if signal.instrument.is_empty() {
            &last_instrument
        } else {
            &signal.instrument
        };
        if instrument.is_empty() {
            eprintln!("signal[{}]: no instrument available, skip", i);
            continue;
        }

        let entry_price = signal.entry.unwrap_or(last_price);
        if entry_price <= 0.0 {
            eprintln!("signal[{}]: no entry price available, skip", i);
            continue;
        }

        // Stop-loss / take-profit: default to +/- 2% of entry
        let tp = signal.take_profit.unwrap_or(entry_price * 1.02);
        let sl = signal.stop_loss.unwrap_or(entry_price * 0.98);

        // Risk check skipped — closed-source RiskMonitorChain is a plugin via NodeFactory

        // ── 7b. 下单 + 模拟成交 ──
        let (exec_dir, offset) = match signal_to_exec_params(&signal.action) {
            Some(p) => p,
            None => continue,
        };

        let volume = signal.size.map(|s| s as u32).unwrap_or(1).max(1);

        let order_req = OrderRequest {
            order_id: format!("e2e-ord-{:04}", i),
            instrument: instrument.clone(),
            direction: exec_dir,
            offset,
            price: entry_price,
            volume,
            order_type: OrderType::Limit,
        };

        let ack = order_mgr.submit(order_req);
        assert_eq!(
            ack.status,
            OrderStatus::Submitted,
            "order should be submitted"
        );

        // 模拟全部成交
        let fill = Fill {
            order_id: ack.order_id.clone(),
            price: entry_price,
            volume,
            time: signal.timestamp.to_rfc3339(),
        };
        let fill_ack = order_mgr.on_fill(&fill).expect("fill should succeed");
        assert_eq!(
            fill_ack.status,
            OrderStatus::Filled,
            "order should be filled"
        );

        // ── 7c. 生成 TradeRecord ──
        let trade_dir = match signal_to_trade_direction(&signal.action) {
            Some(d) => d,
            None => continue,
        };

        let mut trade = TradeRecord::open(
            trades.len() + 1,
            instrument,
            signal.timestamp,
            trade_dir,
            entry_price,
            volume,
            Some(signal.confidence),
        );

        // 用止盈/止损价平仓（按方向选择）
        let (exit_price, exit_reason) = match signal.action {
            SignalAction::Long | SignalAction::CloseShort => (tp, "tp"),
            SignalAction::Short | SignalAction::CloseLong => (sl, "sl"),
            SignalAction::Hold => (entry_price, "signal_reverse"),
        };

        trade.close(signal.timestamp, exit_price, exit_reason, 10.0);
        trades.push(trade);
    }

    Ok(trades)
}

// ── tests ──────────────────────────────────────────────────────────────

/// 全链路 E2E 集成测试。
///
/// ```text
/// golden_tick CSV → CsvReplaySource → Pipeline::feed_tick_direct()
///   → DAG (BarNode → MaCross) → Signal[]
///   → FusionEngine (weighted vote) → OrderManager
///   → TradeRecord[] → JSON schema validation
/// ```
///
/// # 超时
/// 通过 `tokio::time::timeout` 确保 60s 内完成，防止死循环或 hang。
#[tokio::test]
async fn e2e_full_trading() {
    let result = tokio::time::timeout(Duration::from_secs(60), run_e2e_pipeline()).await;

    match result {
        Ok(Ok(trades)) => {
            assert!(
                !trades.is_empty(),
                "should produce at least one trade record"
            );
            validate_trade_records_json(&trades);
            eprintln!("e2e_full_trading: {} trade records generated", trades.len());
        }
        Ok(Err(e)) => panic!("E2E pipeline error: {}", e),
        Err(_elapsed) => panic!("E2E pipeline timed out after 60s"),
    }
}

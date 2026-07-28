use std::collections::HashMap;
use taiji_engine::source::adapter::SchemaAdapter;
use taiji_engine::types::tick::RawTick;

#[test]
fn test_ctp_field_mapping() {
    let mut adapter = SchemaAdapter::new();

    // 注册 CTP 字段映射
    adapter.register_source(
        "ctp".into(),
        vec![
            ("LastPrice", "last_price", true),
            ("Volume", "volume", true),
            ("OpenInterest", "open_interest", true),
            ("OpenPrice", "open_price", true),
            ("HighestPrice", "highest_price", true),
            ("LowestPrice", "lowest_price", true),
            ("Turnover", "turnover", true),
            ("PreSettlementPrice", "pre_settlement_price", false),
            ("UpperLimitPrice", "upper_limit_price", false),
            ("LowerLimitPrice", "lower_limit_price", false),
            ("BidPrice1", "bid_price1", false),
            ("AskPrice1", "ask_price1", false),
            ("AveragePrice", "average_price", false),
        ],
    );

    let mut fields = HashMap::new();
    fields.insert("LastPrice".into(), 4500.0);
    fields.insert("Volume".into(), 12345.0);
    fields.insert("OpenInterest".into(), 50000.0);
    fields.insert("OpenPrice".into(), 4480.0);
    fields.insert("HighestPrice".into(), 4520.0);
    fields.insert("LowestPrice".into(), 4470.0);
    fields.insert("Turnover".into(), 55000000.0);

    let raw = RawTick {
        instrument: "ag2611".into(),
        source_id: "ctp:0".into(),
        fields,
        timestamp: 1700000000000,
        sequence: Some(1),
    };

    let (tick, _missing) = adapter.adapt(&"ctp".into(), raw);

    assert_eq!(tick.last_price, 4500.0);
    assert_eq!(tick.volume, 12345.0);
    assert_eq!(tick.open_interest, 50000.0);
    assert_eq!(tick.open_price, 4480.0);
    assert_eq!(tick.highest_price, 4520.0);
    assert_eq!(tick.lowest_price, 4470.0);
    assert_eq!(tick.turnover, 55000000.0);

    // 未提供的 required 字段不应产生 missing 报告（as last_price is set）
    println!(
        "Mapped: last_price={}, vol={}, oi={}",
        tick.last_price, tick.volume, tick.open_interest
    );
}

#[test]
fn test_missing_non_required_fields() {
    let mut adapter = SchemaAdapter::new();
    adapter.register_source(
        "test".into(),
        vec![
            ("Present", "last_price", true),
            ("Optional", "pre_settlement_price", false),
        ],
    );

    let mut fields = HashMap::new();
    fields.insert("Present".into(), 100.0);
    // Optional not provided

    let raw = RawTick {
        instrument: "test".into(),
        source_id: "test:0".into(),
        fields,
        timestamp: 0,
        sequence: None,
    };

    let (tick, missing) = adapter.adapt(&"test".into(), raw);

    assert_eq!(tick.last_price, 100.0);
    assert_eq!(tick.pre_settlement_price, 0.0); // default, not estimated
                                                // Optional field missing should NOT appear in missing list (non-required)
    assert!(missing.is_empty());
}

#[test]
fn test_unknown_source() {
    let adapter = SchemaAdapter::new();
    let raw = RawTick {
        instrument: "test".into(),
        source_id: "unknown".into(),
        fields: HashMap::new(),
        timestamp: 0,
        sequence: None,
    };
    let (tick, _missing) = adapter.adapt(&"unknown".into(), raw);
    // Should not panic, just return default TickData
    assert_eq!(tick.last_price, 0.0);
}

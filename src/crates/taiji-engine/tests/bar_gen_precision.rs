use taiji_engine::pipeline::bar_gen::{AggMode, BarGenerator};
use taiji_engine::types::bar::Freq;
use taiji_engine::types::tick::TickData;

/// 构造 tick
fn tick(ts_ms: i64, price: f64, vol: f64, amount: f64, oi: f64, bid: f64, ask: f64) -> TickData {
    TickData {
        instrument: "test".into(),
        timestamp_ms: ts_ms,
        last_price: price,
        volume: vol,
        turnover: amount,
        open_interest: oi,
        bid_price1: bid,
        ask_price1: ask,
        ..Default::default()
    }
}

#[test]
fn test_single_1m_bar_ohlc() {
    // 1 分钟内 2 个 tick → 第 3 个 tick 跨边界，闭合含前 2 个 tick 的 bar
    // 前两个 tick: 22:13:20 ~ 22:13:50 UTC → 桶 22:13
    let t1 = tick(1700000000_000, 100.0, 10.0, 1000.0, 5000.0, 99.9, 100.1);
    let t2 = tick(1700000030_000, 101.0, 20.0, 2000.0, 5005.0, 100.9, 101.1);
    // 下一分钟 22:14:10 UTC → 桶 22:14，触发旧 bar 闭合（t3 本身进入新 bar）
    let t3 = tick(1700000050_000, 99.0, 30.0, 3000.0, 5010.0, 98.9, 99.1);

    let mut bg = BarGenerator::new("test".into(), vec![AggMode::Time], vec![Freq::F1]);
    bg.update_tick(&t1);
    bg.update_tick(&t2);
    bg.update_tick(&t3); // 闭合在此次调用中发生，被闭 bar 只含 t1+t2

    let bars = bg.bars(&Freq::F1);
    assert_eq!(bars.len(), 1, "one bar should be closed");
    let bar = &bars[0];
    assert_eq!(bar.open, 100.0, "open");
    assert_eq!(bar.high, 101.0, "high");
    assert_eq!(bar.low, 100.0, "low");
    assert_eq!(bar.close, 101.0, "close");
    // vol 增量: 0 + (20-10) = 10
    assert_eq!(bar.vol, 10.0, "vol");
    // amount 增量: 0 + (2000-1000) = 1000
    assert_eq!(bar.amount, 1000.0, "amount");
    assert_eq!(bar.open_interest, Some(5005.0), "oi = last tick oi (t2)");
}

#[test]
fn test_multi_freq_simultaneous_close() {
    // 同时闭合 1m 和 5m bar
    let base = 1700000000_000i64;
    let mut bg = BarGenerator::new("test".into(), vec![AggMode::Time], vec![Freq::F1, Freq::F5]);

    // Feed 5 minutes of ticks
    for m in 0..5 {
        for s in 0..3 {
            let ts = base + (m * 60 + s * 20) as i64 * 1000;
            let price = 100.0 + m as f64 + s as f64 * 0.1;
            bg.update_tick(&tick(
                ts,
                price,
                (m * 10 + s) as f64,
                0.0,
                5000.0,
                price - 0.1,
                price + 0.1,
            ));
        }
    }

    let bars_1m = bg.bars(&Freq::F1);
    let bars_5m = bg.bars(&Freq::F5);
    // 由于 base 时间在 22:13:20，每 20s 一个 tick，第 3 个 tick（base+40s）在 22:14:00
    // 即每分钟末的 tick 正好跨边界，因此 5 分钟产生 5 个边界跨越 → 5 根闭合 1m bar
    assert_eq!(bars_1m.len(), 5, "should have exactly 5 completed 1m bars");
    // base+120s=22:15:20 跨过 5 分钟边界（桶 22:15 ≠ 22:10）→ 闭合 1 根 5m bar
    assert_eq!(bars_5m.len(), 1, "should have exactly 1 completed 5m bar");
}

#[test]
fn test_delta_from_ctp_l1() {
    // 验证主动买卖方向判定
    // LastPrice >= AskPrice1 → 主动买
    let t_buy = tick(1700000000_000, 100.1, 10.0, 0.0, 5000.0, 99.9, 100.1);
    // LastPrice <= BidPrice1 → 主动卖
    let t_sell = tick(1700000001_000, 99.9, 20.0, 0.0, 5000.0, 99.9, 100.1);
    // 中间价 → 无法判定
    let t_mid = tick(1700000002_000, 100.0, 30.0, 0.0, 5000.0, 99.9, 100.1);

    let mut bg = BarGenerator::new("test".into(), vec![AggMode::Time], vec![Freq::F1]);
    bg.update_tick(&t_buy);
    bg.update_tick(&t_sell);
    bg.update_tick(&t_mid);
    // 触发闭合
    bg.update_tick(&tick(1700000060_000, 100.0, 40.0, 0.0, 5000.0, 99.9, 100.1));

    let bars = bg.bars(&Freq::F1);
    assert_eq!(bars.len(), 1);
    let bar = &bars[0];
    // delta: +1 (buy) + (-1) (sell) + 0 (mid) = 0 → None
    assert_eq!(bar.delta, None, "net zero delta should be None");
    // vol: 0 + (20-10) + (30-20) = 20
    assert_eq!(bar.vol, 20.0, "vol");
}

#[test]
fn test_delta_net_positive() {
    // 纯主动买 → delta 应有正数
    let t1 = tick(1700000000_000, 100.1, 10.0, 0.0, 5000.0, 99.9, 100.1);
    let t2 = tick(1700000001_000, 100.2, 20.0, 0.0, 5000.0, 99.9, 100.1);

    let mut bg = BarGenerator::new("test".into(), vec![AggMode::Time], vec![Freq::F1]);
    bg.update_tick(&t1);
    bg.update_tick(&t2);
    bg.update_tick(&tick(1700000060_000, 100.0, 30.0, 0.0, 5000.0, 99.9, 100.1));

    let bars = bg.bars(&Freq::F1);
    assert_eq!(bars.len(), 1);
    // +1 + +1 = +2
    assert_eq!(bars[0].delta, Some(2.0), "two buys should give delta=2.0");
}

#[test]
fn test_no_tick_no_bar() {
    // 没有 tick 输入时，bars 为空
    let bg = BarGenerator::new("test".into(), vec![AggMode::Time], vec![Freq::F1]);
    assert!(bg.bars(&Freq::F1).is_empty());
}

#[test]
fn test_oi_zero_not_recorded() {
    // oi=0 时，PartialBar 将其视为 None（不记录）
    let t1 = tick(1700000000_000, 100.0, 10.0, 1000.0, 0.0, 99.9, 100.1);
    let t2 = tick(1700000060_000, 101.0, 20.0, 2000.0, 0.0, 100.9, 101.1);

    let mut bg = BarGenerator::new("test".into(), vec![AggMode::Time], vec![Freq::F1]);
    bg.update_tick(&t1);
    let closed = bg.update_tick(&t2);

    assert_eq!(closed.len(), 1);
    let (_freq, bar) = &closed[0];
    assert_eq!(bar.open_interest, None, "oi=0 should be treated as None");
}

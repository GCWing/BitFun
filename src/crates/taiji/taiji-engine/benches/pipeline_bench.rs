use std::time::Instant;

/// 基准：BarGenerator 吞吐量
#[test]
fn bench_bar_gen_throughput() {
    use taiji_engine::pipeline::bar_gen::{AggMode, BarGenerator};
    use taiji_engine::types::bar::Freq;

    let mut bg = BarGenerator::new(
        "bench".into(),
        vec![AggMode::Time],
        vec![Freq::F1, Freq::F5, Freq::F15],
    );

    let start = Instant::now();
    let n = 1000;

    for i in 0..n {
        let tick = taiji_engine::types::tick::TickData {
            instrument: "bench".into(),
            timestamp_ms: 1700000000000 + i as i64 * 1000,
            last_price: 100.0 + (i % 100) as f64 * 0.01,
            volume: (i * 10) as f64,
            turnover: (i * 1000) as f64,
            open_interest: 5000.0,
            ask_price1: 100.1,
            bid_price1: 99.9,
            ..Default::default()
        };
        bg.update_tick(&tick);
    }

    let elapsed = start.elapsed();
    let throughput = n as f64 / elapsed.as_secs_f64();
    println!(
        "BarGenerator: {} ticks in {:?} ({:.0} ticks/s)",
        n, elapsed, throughput
    );
    assert!(
        throughput > 100.0,
        "throughput too low: {:.0} ticks/s",
        throughput
    );
}

/// 基准：DAG 拓扑排序
#[test]
fn bench_dag_sort() {
    use taiji_engine::dag::Dag;

    let mut dag = Dag::new();
    // 构建 50 节点链
    for i in 0..49 {
        dag.add_edge(format!("n{}", i), format!("n{}", i + 1));
    }

    let start = Instant::now();
    for _ in 0..1000 {
        let _ = dag.sort().unwrap();
    }
    let elapsed = start.elapsed();
    println!("DAG sort (50 nodes × 1000): {:?}", elapsed);
    assert!(elapsed.as_secs_f64() < 1.0, "DAG sort too slow");
}

/// 基准：StateStore 读写
#[test]
fn bench_state_store() {
    use taiji_engine::store::StateStore;
    use taiji_engine::types::state::StateValue;

    let store = StateStore::new();
    let start = Instant::now();

    for i in 0..10000 {
        store.set(
            format!("key_{}", i),
            StateValue::F64(i as f64),
            "bench".into(),
        );
    }
    let write_elapsed = start.elapsed();

    let start = Instant::now();
    for i in 0..10000 {
        let val: Option<f64> = store.get(&format!("key_{}", i));
        assert_eq!(val, Some(i as f64));
    }
    let read_elapsed = start.elapsed();

    println!(
        "StateStore: 10000 writes in {:?} ({:.0} ops/s), reads in {:?} ({:.0} ops/s)",
        write_elapsed,
        10000.0 / write_elapsed.as_secs_f64(),
        read_elapsed,
        10000.0 / read_elapsed.as_secs_f64(),
    );
}

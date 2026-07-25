# taiji-cli

**路径**: src/crates/taiji/taiji-cli
**描述**: Taiji standalone CLI — zero BitFun desktop dependency

## 依赖
- 内部: taiji-engine, taiji-bar, taiji-example, taiji-backtest
- 外部: clap, serde, serde_json, serde_yaml, anyhow, chrono, tokio, tracing, tracing-subscriber

## 模块结构
- `main.rs` — CLI binary + CSV 解析 + Pipeline/Backtest/Reload 三种运行模式

## 核心函数
- `run_pipeline()` — 管道模式：YAML 配置 + CSV → Pipeline 执行 → 输出信号
- `run_backtest()` — 回测模式（支持单品种/多品种并行- rayon）
- `run_reload_config()` — 配置热加载验证
- `register_nodes()` — 注册 BarNode / MaCross 节点类型

## 核心类型
- `Cli` — clap CLI 参数（config/csv/output/resume 子命令）
- `Command` — 子命令枚举（Backtest / ReloadConfig）

## 属于领域
- CLI / integration

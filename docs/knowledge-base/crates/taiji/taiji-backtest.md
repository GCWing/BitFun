# taiji-backtest

**路径**: src/crates/taiji/taiji-backtest
**描述**: Backtest engine (MIT)

## 依赖
- 内部: taiji-content, taiji-engine
- 外部: serde, serde_json, serde_yaml, chrono, statrs, anyhow, thiserror, rayon, tokio

## 模块结构
- `config` — BacktestConfig / WalkForwardConfig YAML 配置
- `runner` — BacktestRunner + BacktestResult（CSV replay → Pipeline → signal matching）
- `stats` — PerformanceStats（8 个指标：Sharpe, MaxDD, WinRate 等）
- `trade_record` — TradeRecord 单笔交易追踪（含 PnL）
- `walk_forward` — WalkForwardValidator 滚动交叉验证

## 核心类型
- `BacktestRunner` — 主回测循环
- `PerformanceStats` — 回测绩效指标
- `WalkForwardValidator` — Walk-Forward 验证器
- `TradeRecord` — 交易记录
- `BacktestConfig` — YAML 驱动的回测配置

## 核心函数
- `BacktestRunner::run()` — 执行回测
- `BacktestRunner::run_parallel()` — 多品种并行回测（rayon）

## 属于领域
- backtest / evaluation

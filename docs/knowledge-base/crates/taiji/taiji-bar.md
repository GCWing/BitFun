# taiji-bar

**路径**: src/crates/taiji/taiji-bar
**描述**: Tick-to-KLine aggregation engine (ref: czsc BarGenerator)

## 依赖
- 内部: taiji-engine
- 外部: chrono

## 模块结构
- `lib.rs` — 单个 BarNode 实现 ComputeNode

## 核心类型
- `BarNode` — 实现 ComputeNode，接收 tick 按时间边界聚合为 RawBar

## 核心函数
- `BarNode::new(id)` — 创建 BarNode 实例
- `BarNode::on_tick(&mut self, tick, state)` — 接收逐笔 tick，委托 BarGenerator 聚合

## 属于领域
- data / pipeline

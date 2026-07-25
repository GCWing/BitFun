# taiji-strategy-template

**路径**: src/crates/taiji/taiji-strategy-template
**描述**: Taiji strategy template — 复制此 crate 创建你自己的闭源策略

## 依赖
- 内部: taiji-engine
- 外部: chrono, serde_json, serde

## 模块结构
- `lib.rs` — DualThrust 通道突破策略模板

## 核心类型
- `DualThrust` — 通道突破策略（Range = Max(HH-LC, HC-LL)，上下轨突破信号）
- `DualThrustParams` — 策略参数（lookback/k1/k2/max_position/day_open/night_open）

## 核心函数
- `DualThrust::new()` — 创建策略
- `DualThrust::evaluate(bar)` — 核心策略逻辑（替换此方法实现自定义策略）
- `DualThrust::calc_range()` — 计算 Range

## 属于领域
- template / strategy

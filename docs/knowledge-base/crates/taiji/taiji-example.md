# taiji-example

**路径**: src/crates/taiji/taiji-example
**描述**: Taiji example strategies — reference ComputeNode patterns

## 依赖
- 内部: taiji-engine
- 外部: chrono

## 模块结构
- `lib.rs` — MaCross 示例策略

## 核心类型
- `MaCross` — MA 双均线金叉/死叉策略（fast_period=5, slow_period=20）

## 核心函数
- `MaCross::new()` — 创建策略实例
- `MaCross::on_bar()` — 接收 K 线，缓存 close 序列
- `MaCross::on_calculate()` — 计算均线交叉信号（Long/Short）

## 属于领域
- example / strategy

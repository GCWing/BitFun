# taiji-abnormal

**路径**: src/crates/taiji/taiji-abnormal
**描述**: Abnormal flow detection scoring card (5 indicators + fusion)

## 依赖
- 内部: taiji-engine
- 外部: serde, serde_json, chrono, statrs

## 模块结构
- `vol_regime` — 波动率体制识别 ComputeNode
- `vol_anomaly` — 成交量异常检测 ComputeNode
- `corr_fracture` — 相关性断裂检测 ComputeNode
- `gap_alert` — 跳空缺口告警 ComputeNode
- `trend_accel` — 趋势加速检测 ComputeNode
- `scorecard` — ScorecardFusionNode 加权融合

## 核心类型
- `AbnormalWeights` — 5 指标权重配置
- `AlertThresholds` — 告警阈值（warn/reduce/emergency）
- `AbnormalLevel` — 异常等级（Normal/Warn/Reduce/Emergency）
- `AbnormalIndicator` — 异常指标 trait

## 核心函数
- `compute_score(bars, lookback)` — 从 OHLCV 计算异常分数 0-100

## 属于领域
- risk / analysis

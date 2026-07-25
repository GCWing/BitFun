# taiji-pattern

**路径**: src/crates/taiji/taiji-pattern
**描述**: Chart pattern recognition — DTW engine + three-layer index

## 依赖
- 内部: taiji-engine
- 外部: serde, serde_json, ndarray

## 模块结构
- `dtw` — DtwEngine: 加权 Euclidean DTW + LB_Keogh 下界剪枝
- `index` — PatternIndex: 三层索引（signature → LB_Keogh → DTW）
- `node` — PatternMatchNode: ComputeNode 接入 DAG 流水线

## 核心类型
- `DtwEngine` — DTW 匹配引擎
- `PatternIndex` — 三层模式索引
- `PatternMatch` — 匹配结果
- `PatternMatchNode` — DAG 节点包装

## 属于领域
- pattern / analysis

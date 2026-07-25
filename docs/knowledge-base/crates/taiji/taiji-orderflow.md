# taiji-orderflow

**路径**: src/crates/taiji/taiji-orderflow
**描述**: Order flow analysis: VPIN + OFI with Welford online statistics (MIT)

## 依赖
- 内部: taiji-engine
- 外部: serde_json

## 模块结构
- `welford` — 单遍均值/方差/CDF 在线统计（O(1) 空间）
- `vpin` — VPIN: volume-bucket 知情交易概率 + CDF 毒性评分
- `ofi` — OFI: 5 档订单流不平衡 + 买卖方向信号

## 核心类型
- `VpinNode` — VPIN 计算 ComputeNode
- `OfiNode` — OFI 计算 ComputeNode
- `WelfordStats` — Welford 在线统计

## 属于领域
- orderflow / analysis

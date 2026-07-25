# taiji-executor

**路径**: src/crates/taiji/taiji-executor
**描述**: Taiji execution bridge — order placement, position tracking, and CTP integration

## 依赖
- 内部: 无
- 外部: serde, serde_json, tokio, async-trait, dashmap

## 模块结构
- `bridge` — ExecutionBridge trait（订单执行桥梁抽象）
- `order_mgr` — OrderManager + OrderState 订单管理
- `position` — PositionTracker 持仓追踪
- `types` — 类型定义（re-export）

## 核心类型
- `ExecutionBridge` — 执行桥梁 trait（CTP 集成接口）
- `OrderManager` — 订单管理器
- `PositionTracker` — 持仓追踪器
- `OrderState` — 订单状态枚举

## 属于领域
- execution / trading

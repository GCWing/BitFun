# taiji-quant 量化引擎 Crates 索引

> 21 个 crate，覆盖行情接入→数据聚合→策略生成→回测→执行→发布全链路

## 核心引擎

| crate | 描述 | 依赖（内部） |
|-------|------|-------------|
| [taiji-engine](taiji-engine.md) | DAG 管线核心 | llm, bar, example, backtest, executor |
| [taiji-bar](taiji-bar.md) | Tick → K线聚合 | engine |
| [taiji-llm](taiji-llm.md) | LLM 客户端 | — |

## 策略 & 分析

| crate | 描述 | 依赖 |
|-------|------|------|
| [taiji-strategen](taiji-strategen.md) | LLM 驱动策略生成 | engine, backtest, llm |
| [taiji-pattern](taiji-pattern.md) | 图表模式识别（DTW） | engine |
| [taiji-abnormal](taiji-abnormal.md) | 异常检测评分卡 | engine |
| [taiji-sentiment](taiji-sentiment.md) | 市场情绪分析 | engine |
| [taiji-orderflow](taiji-orderflow.md) | 订单流分析（VPIN+OFI） | engine |

## 执行 & 回测

| crate | 描述 | 依赖 |
|-------|------|------|
| [taiji-backtest](taiji-backtest.md) | 回测引擎 | engine, content |
| [taiji-executor](taiji-executor.md) | 订单执行桥接 | — |
| [taiji-realtime](taiji-realtime.md) | 实时行情 | engine |

## 内容 & 发布

| crate | 描述 | 依赖 |
|-------|------|------|
| [taiji-content](taiji-content.md) | 视频/TTS/K线合成 | engine |
| [taiji-publisher](taiji-publisher.md) | 多平台发布 | content |
| [taiji-growth](taiji-growth.md) | 增长运营 | content, engine |
| [taiji-alert](taiji-alert.md) | 多渠道告警 | — |
| [taiji-knowledge-graph](taiji-knowledge-graph.md) | 知识图谱 | engine |
| [taiji-blog-gen](taiji-blog-gen.md) | 博客生成（二进制） | growth |

## 工具 & 示例

| crate | 描述 | 依赖 |
|-------|------|------|
| [taiji-engine-py](taiji-engine-py.md) | Python RL 绑定 | engine |
| [taiji-example](taiji-example.md) | 示例策略 | engine |
| [taiji-strategy-template](taiji-strategy-template.md) | 策略模板 | engine |
| [taiji-cli](taiji-cli.md) | 量化 CLI | engine, bar, example, backtest |

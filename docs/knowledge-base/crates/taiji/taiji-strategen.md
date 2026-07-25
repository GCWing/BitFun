# taiji-strategen

**路径**: src/crates/taiji/taiji-strategen
**描述**: Strategy generator — LLM-driven hypothesis → validate → compile → backtest → refine pipeline (MIT)

## 依赖
- 内部: taiji-engine, taiji-backtest, taiji-llm
- 外部: serde, serde_json, serde_yaml, tokio, async-trait, rand, statrs, anyhow, thiserror, chrono

## 模块结构
- `hypothesis` — Hypothesis / Condition / PositionSizing / RiskParams / HypothesisValidator
- `compiler` — StrategyCompiler: Hypothesis → PipelineConfig YAML
- `pipeline` — StrategyGenPipeline: 五阶段流水线编排
- `analyzer` — ResultAnalyzer: Deflated Sharpe Ratio + Monte Carlo
- `refiner` — HypothesisRefiner: LLM 反馈优化（最多 5 轮）

## 核心类型
- `StrategyGenPipeline` — 五阶段生成流水线
- `Hypothesis` — 交易假设
- `HypothesisValidator` — 假设验证器（类型安全/合理性/前视偏差）
- `StrategyCompiler` — 假设→可执行配置编译器
- `ResultAnalyzer` — 回测结果分析器
- `HypothesisRefiner` — 假设优化器

## 核心函数
- `StrategyGenPipeline::run()` — 执行完整策略生成流程
- `StrategyCompiler::compile()` — 编译假设为 PipelineConfig

## 属于领域
- strategy / AI generation

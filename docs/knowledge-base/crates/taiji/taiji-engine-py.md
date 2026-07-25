# taiji-engine-py

**路径**: src/crates/taiji/taiji-engine-py
**描述**: Python bindings for taiji-engine via PyO3

## 依赖
- 内部: taiji-engine
- 外部: pyo3, parking_lot, chrono, serde_yaml

## 模块结构
- `python/types_py` — TickDataPy/RawBarPy/SignalPy Python 类型
- `python/engine_py` — PipelinePy Python 绑定
- `obs_builder` — RL 环境观察构建器
- `reward_calculator` — RL 奖励计算器
- `rl_env` — TaijiRLEnv Gymnasium 风格强化学习环境
- `cache` — 缓存

## 核心类型
- `TaijiRLEnv` — Python 可调用的强化学习环境
- `ObsBuilder` — 观察（状态）构建器
- `RewardCalculator` — 奖励函数计算器
- `PipelinePy` — Python 端的 Pipeline 包装

## 属于领域
- python binding / RL

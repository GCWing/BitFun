# taiji-engine-py — Python Bindings for taiji-engine

PyO3 `cdylib` exposing `Pipeline`, `TickData`, `RawBar`, and `Signal` to Python. Enables RL training (stable-baselines3) and Jupyter notebook workflows with the Rust engine.

## Architecture Position

```
taiji-engine (rlib)
  └── taiji-engine-py (cdylib + PyO3) → import _native
```

## Exposed Classes

| Python Class | Rust Source | Description |
|-------------|-------------|-------------|
| `PipelinePy` | `engine_py.rs` | Pipeline lifecycle: new, feed_tick, serialize_state |
| `TickDataPy` | `types_py.rs` | 47-field CTP-aligned tick data |
| `RawBarPy` | `types_py.rs` | OHLCV bar with delta and open interest |
| `SignalPy` | `types_py.rs` | Trading signal with action, entry, stop-loss, take-profit |

## Quick Start

```python
import _native

pipeline = _native.PipelinePy("pipeline_config.yaml")
signal = pipeline.feed_tick(tick_dict)
state_json = pipeline.serialize_state()
```

## Related

- `taiji-engine` — core Rust engine
- `taiji-rl-env` (Phase 6) — Gymnasium environment wrapping PipelinePy

## License

SPDX-License-Identifier: Apache-2.0 OR MIT

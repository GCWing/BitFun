# taiji-cli — Standalone Pipeline CLI

Zero BitFun desktop dependency. Runs taiji-engine pipeline against CSV tick data and outputs signals as JSON.

## CLI Usage

```
taiji --config pipeline.yaml --csv data.csv [--output signals.json] [--resume N]
```

| Argument | Description |
|----------|-------------|
| `--config` | Pipeline YAML config (required) |
| `--csv` | CSV tick data file (required) |
| `--output` | Output signals JSON path (default: stdout) |
| `--resume` | Skip first N data rows after header |

## Pipeline Flow

1. Parse YAML `PipelineConfig`
2. Register `BarNode` (taiji-bar) + `MaCross` (taiji-example) node types
3. Build DAG, derive edges from `input_keys`/`output_keys`
4. Parse CSV → `TickData` → `Pipeline::feed_tick_direct()`
5. Collect `Signal[]` → JSON output

## Example

```bash
taiji --config examples/ma_cross.yaml --csv data/rb9999_2026.csv --output signals.json
```

## Related

- `taiji-engine` — core pipeline engine
- `taiji-bar` — BarNode implementation
- `taiji-example` — reference ComputeNode (MaCross)

## License

SPDX-License-Identifier: Apache-2.0 OR MIT

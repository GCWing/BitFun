# taiji-pattern — Chart Pattern Recognition

Multi-dimensional Dynamic Time Warping (DTW) with weighted Euclidean distance and LB_Keogh lower-bound filtering. Three-layer index (signature → LB_Keogh → full DTW) plus a `PatternMatchNode` ComputeNode.

## Usage

```rust
use taiji_pattern::dtw::DtwEngine;
use taiji_pattern::index::PatternIndex;

let engine = DtwEngine::new(&[1.0, 1.0, 0.5]); // feature weights: [O,H,L,C,V]
let mut index = PatternIndex::new(engine);

index.insert("head_and_shoulders".into(), template_bars);
let matches = index.search(&query_bars, 5);
for m in matches {
    println!("  {}: distance={:.4}", m.pattern_id, m.distance);
}
```

```bash
cargo add taiji-pattern
```

## Modules

| Module | Description |
|--------|-------------|
| `dtw` | `DtwEngine` — weighted Euclidean DTW + LB_Keogh lower bound |
| `index` | `PatternIndex` — three-layer search with signature pre-filtering |
| `node` | `PatternMatchNode` — `ComputeNode` that feeds bars into the index |

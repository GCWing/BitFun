# taiji-sentiment — Market Sentiment Analysis

Chinese text sentiment via jieba-rs word segmentation + financial sentiment lexicon with adverb/negation handling. Also provides a Fear & Greed Index (FGI) from 5 factors: HV20, momentum, OI, basis, NLP. Exposes a `SentimentNode` ComputeNode.

## Usage

```rust
use taiji_sentiment::tokenizer::SentimentTokenizer;

let tokenizer = SentimentTokenizer::new()?;
let result = tokenizer.analyze("央行降准利好市场，但外部风险不可忽视")?;
println!("score={:.3}, confidence={:.3}", result.score, result.confidence);
```

```rust
use taiji_sentiment::fgi::FearGreedIndex;
let fgi = FearGreedIndex::new();
let value = fgi.compute(hv20, momentum, oi_change, basis, nlp_score);
println!("FGI={:.0} ({})", value.value, value.category);
```

```bash
cargo add taiji-sentiment
```

## Modules

| Module | Description |
|--------|-------------|
| `tokenizer` | `SentimentTokenizer` — jieba-rs + financial lexicon + negation handling |
| `fgi` | `FearGreedIndex` — 5-factor FGI with 0-100 scale and category labels |
| `node` | `SentimentNode` — `ComputeNode` for DAG integration |

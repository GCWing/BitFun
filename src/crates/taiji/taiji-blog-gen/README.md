# taiji-blog-gen — Agent Analysis → Hugo Blog Post CLI

Reads 7-Agent analysis JSON, maps tags from agent output fields, renders Tera templates, and writes Hugo-compatible Markdown with front matter.

## Architecture Position

```
taiji-growth (Tera, ContentAsset)
  └── taiji-blog-gen (binary CLI)
```

## CLI Usage

```
taiji-blog-gen --input agent_output.json --output-dir posts/
taiji-blog-gen --batch --input-dir exports/ --output-dir posts/
```

## Tag Auto-Mapping

| Agent Field | Tag Rule |
|-------------|----------|
| `structure_agent.trend_direction` | 多头趋势 / 空头趋势 / 震荡 |
| `magnet_agent.magnet_valid` | 磁体共振 / 无磁体 |
| `thrust_agent.thrust_count >= 1` | 三推 |
| `resonance_agent.resonance_level >= HIGH` | 共振 |

## Templates

| Template | Output |
|----------|--------|
| `daily_post.tera` | Daily analysis blog post |
| `weekly_summary.tera` | Weekly market summary |
| `special_topic.tera` | Deep-dive topic analysis |

## License

SPDX-License-Identifier: Apache-2.0 OR MIT

# taiji-sentiment

**路径**: src/crates/taiji/taiji-sentiment
**描述**: Market sentiment analysis (MIT)

## 依赖
- 内部: taiji-engine
- 外部: serde, serde_json, jieba-rs

## 模块结构
- `tokenizer` — jieba-rs 分词 + 金融情绪词典 + 程度副词/否定翻转
- `fgi` — Fear & Greed Index（HV20/动量/OI/基差/NLP 五因子）
- `node` — SentimentNode（ComputeNode 实现）

## 核心类型
- `SentimentTokenizer` — 情绪分词器
- `SentimentResult` — 情绪分析结果
- `SentimentEntry` — 词典条目
- `FearGreedIndex` — 恐惧贪婪指数
- `FgiCategory` — FGI 分类
- `SentimentNode` — DAG 节点

## 属于领域
- sentiment / analysis

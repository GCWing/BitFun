//! taiji-sentiment — 市场情绪分析（MIT）。
//!
//! 提供三个核心模块：
//! - [`tokenizer`]: jieba-rs 分词 + 金融情绪词典 + 程度副词/否定翻转
//! - [`fgi`]: Fear & Greed Index（HV20/动量/OI/基差/NLP 五因子）
//! - [`node`]: SentimentNode（实现 ComputeNode，接入太极 DAG 流水线）

pub mod fgi;
pub mod node;
pub mod tokenizer;

pub use fgi::{FearGreedIndex, FgiCategory};
pub use node::SentimentNode;
pub use tokenizer::{SentimentEntry, SentimentResult, SentimentTokenizer};

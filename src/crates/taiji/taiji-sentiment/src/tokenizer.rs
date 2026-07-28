//! 情绪分词器 — jieba-rs + 金融情绪词典 + 程度副词 + 否定翻转。
//!
//! 参考：SnowNLP（MIT）/ cnsenti（MIT），中文情感分析，适配期货语境。

use std::collections::{HashMap, HashSet};

use jieba_rs::Jieba;
use serde::Deserialize;

// ── 词典条目 ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct SentimentEntry {
    pub word: String,
    pub polarity: f64,
    pub category: String,
}

// ── 情绪结果 ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SentimentResult {
    /// 综合情绪得分 (-1.0 ~ 1.0)
    pub score: f64,
    /// 置信度 (0.0 ~ 1.0)，基于匹配词数与总词数之比
    pub confidence: f64,
    /// 匹配到的正面词
    pub positive_words: Vec<String>,
    /// 匹配到的负面词
    pub negative_words: Vec<String>,
    /// 匹配到的政策/宏观关键词
    pub policy_keywords: Vec<String>,
}

// ── 分词器 ──────────────────────────────────────────────────────────────

/// 金融情绪分词器。
///
/// 基于 jieba-rs 分词，结合自建金融情绪词典、程度副词修饰和否定翻转，
/// 输出综合情绪得分。
pub struct SentimentTokenizer {
    tokenizer: Jieba,
    sentiment_dict: HashMap<String, SentimentEntry>,
    degree_words: HashMap<String, f64>,
    negation_words: HashSet<String>,
}

impl SentimentTokenizer {
    /// 从 JSON 字节数组加载情绪词典，使用内置的程度副词和否定词表。
    pub fn from_json(json_bytes: &[u8]) -> Result<Self, String> {
        let entries: Vec<SentimentEntry> =
            serde_json::from_slice(json_bytes).map_err(|e| format!("解析词典失败: {e}"))?;

        let sentiment_dict: HashMap<String, SentimentEntry> =
            entries.into_iter().map(|e| (e.word.clone(), e)).collect();

        Ok(Self {
            tokenizer: Jieba::new(),
            sentiment_dict,
            degree_words: default_degree_words(),
            negation_words: default_negation_words(),
        })
    }

    /// 内联构造（测试用）。
    pub fn new(dict: HashMap<String, SentimentEntry>) -> Self {
        Self {
            tokenizer: Jieba::new(),
            sentiment_dict: dict,
            degree_words: default_degree_words(),
            negation_words: default_negation_words(),
        }
    }

    /// 分析文本情绪。
    ///
    /// 算法：
    /// 1. jieba 分词（开启 HMM 新词发现）
    /// 2. 遍历分词结果，匹配情绪词典
    /// 3. 对匹配到的情绪词，检查前置词是否为程度副词 → 强度修饰
    /// 4. 检查前置词是否包含否定词（1-2 词窗口）→ 极性翻转
    /// 5. 累加后归一化 = score / (matched_count + epsilon)
    pub fn analyze(&self, text: &str) -> SentimentResult {
        let words: Vec<&str> = self.tokenizer.cut(text, true);

        let mut total_score = 0.0;
        let mut matched_count = 0u32;
        let mut positive_words: Vec<String> = Vec::new();
        let mut negative_words: Vec<String> = Vec::new();
        let mut policy_keywords: Vec<String> = Vec::new();

        for (i, &w) in words.iter().enumerate() {
            let Some(entry) = self.sentiment_dict.get(w) else {
                continue;
            };

            let mut polarity = entry.polarity;

            // 程度副词修饰：向前查找最近的程度副词
            if i > 0 {
                let prev = words[i - 1];
                if let Some(&multiplier) = self.degree_words.get(prev) {
                    polarity *= multiplier;
                }
            }

            // 否定翻转：向前 1-2 词窗口内出现否定词则取反
            let negated = (i >= 1 && self.negation_words.contains(words[i - 1]))
                || (i >= 2 && self.negation_words.contains(words[i - 2]));
            if negated {
                polarity = -polarity;
            }

            total_score += polarity;
            matched_count += 1;

            if polarity > 0.0 {
                positive_words.push(w.to_string());
            } else if polarity < 0.0 {
                negative_words.push(w.to_string());
            }

            if entry.category == "policy" {
                policy_keywords.push(w.to_string());
            }
        }

        let score = if matched_count > 0 {
            (total_score / matched_count as f64).clamp(-1.0, 1.0)
        } else {
            0.0
        };

        let confidence = if words.is_empty() {
            0.0
        } else {
            (matched_count as f64 / words.len() as f64).clamp(0.0, 1.0)
        };

        SentimentResult {
            score,
            confidence,
            positive_words,
            negative_words,
            policy_keywords,
        }
    }
}

// ── 内置程度副词表 ──────────────────────────────────────────────────────

fn default_degree_words() -> HashMap<String, f64> {
    [
        // 极度
        ("极其", 2.0),
        ("非常", 2.0),
        ("极度", 2.0),
        ("绝对", 2.0),
        // 超
        ("特别", 1.8),
        ("尤为", 1.8),
        ("异常", 1.8),
        // 很
        ("十分", 1.5),
        ("相当", 1.5),
        ("挺", 1.3),
        ("很", 1.2),
        // 较
        ("较", 1.1),
        ("更为", 1.1),
        ("越发", 1.1),
        // 稍
        ("稍微", 0.5),
        ("略微", 0.5),
        ("稍稍", 0.5),
        ("有点", 0.3),
        ("有些", 0.3),
        // 欠
        ("不大", 0.2),
        ("不太", 0.2),
        ("不怎么", 0.2),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

// ── 内置否定词表 ────────────────────────────────────────────────────────

fn default_negation_words() -> HashSet<String> {
    [
        "不", "没", "无", "非", "未", "莫", "勿", "别", "否", "休", "不要", "没有", "未必", "并非",
        "并非", "从不", "绝不",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dict() -> HashMap<String, SentimentEntry> {
        [
            ("看好", 0.8, "sentiment"),
            ("下跌", -0.7, "price"),
            ("突破", 0.6, "technical"),
            ("强势", 0.7, "technical"),
            ("熊市", -1.0, "market"),
            ("降息", 0.9, "policy"),
            ("利空", -0.8, "fundamental"),
            ("拉升", 0.8, "order_flow"),
        ]
        .into_iter()
        .map(|(w, p, c)| {
            (
                w.to_string(),
                SentimentEntry {
                    word: w.to_string(),
                    polarity: p,
                    category: c.to_string(),
                },
            )
        })
        .collect()
    }

    #[test]
    fn test_basic_tokenization() {
        let t = SentimentTokenizer::new(test_dict());
        let r = t.analyze("市场强势突破，看好后市");
        assert!(r.score > 0.0, "正面文本应得正分，实际: {}", r.score);
        assert!(!r.positive_words.is_empty(), "应有正面词匹配");
    }

    #[test]
    fn test_negative_text() {
        let t = SentimentTokenizer::new(test_dict());
        let r = t.analyze("市场下跌，利空频繁，进入熊市");
        assert!(r.score < 0.0, "负面文本应得负分，实际: {}", r.score);
        assert!(!r.negative_words.is_empty(), "应有负面词匹配");
    }

    #[test]
    fn test_negation_flip_1() {
        let t = SentimentTokenizer::new(test_dict());
        // "不看好" → 看好(0.8) 被"不"翻转 → -0.8
        let r = t.analyze("不看好后市");
        assert!(r.score < 0.0, "「不看好」应为负面，实际: {}", r.score);
    }

    #[test]
    fn test_negation_flip_2() {
        let t = SentimentTokenizer::new(test_dict());
        // "没有突破" → 突破(0.6) 被"没有"翻转 → -0.6
        let r = t.analyze("价格没有突破阻力位");
        assert!(r.score < 0.0, "「没有突破」应为负面，实际: {}", r.score);
    }

    #[test]
    fn test_degree_modifier() {
        let t = SentimentTokenizer::new(test_dict());
        // "非常强势" → 强势(0.7) * 非常(2.0) = 1.4 → clamp → 1.0（单个词贡献，平均后）
        let r1 = t.analyze("非常强势");
        let r2 = t.analyze("强势");
        assert!(r1.score.abs() >= r2.score.abs(), "程度副词应增强极性");
    }

    #[test]
    fn test_policy_keywords() {
        let t = SentimentTokenizer::new(test_dict());
        let r = t.analyze("央行降息刺激经济");
        assert!(
            r.policy_keywords.contains(&"降息".to_string()),
            "应识别政策关键词「降息」"
        );
    }

    #[test]
    fn test_empty_text() {
        let t = SentimentTokenizer::new(test_dict());
        let r = t.analyze("");
        assert_eq!(r.score, 0.0);
        assert_eq!(r.confidence, 0.0);
    }

    #[test]
    fn test_neutral_text() {
        let t = SentimentTokenizer::new(test_dict());
        let r = t.analyze("今日螺纹钢期货主力合约");
        assert_eq!(r.score, 0.0);
        assert!(r.positive_words.is_empty());
        assert!(r.negative_words.is_empty());
    }

    #[test]
    fn test_confidence_computation() {
        let t = SentimentTokenizer::new(test_dict());
        // "拉升" is the only matched word out of 4 total
        let r = t.analyze("主力拉升螺纹钢");
        assert!(
            r.confidence > 0.0 && r.confidence <= 1.0,
            "置信度应在 (0,1] 之间，实际: {}",
            r.confidence
        );
    }
}

//! SentimentNode — 实现 ComputeNode，将情绪分析接入太极 DAG 流水线。
//!
//! 输入：StateStore 中的文本（key 可配置，默认 `sentiment:text`）
//! 输出：`sentiment:score` / `sentiment:confidence` / `sentiment:fgi`

use std::sync::Arc;

use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::signal::Signal;
use taiji_engine::types::state::{StateKey, StateValue};

use crate::fgi::FearGreedIndex;
use crate::tokenizer::SentimentTokenizer;

/// 情绪计算节点。
///
/// 配置参数（NodeConfig）：
/// - `text_key` (str): 读取文本的 StateKey，默认 `"sentiment:text"`
/// - `fgi_compute` (bool): 是否计算 FGI，默认 `false`
pub struct SentimentNode {
    id: NodeId,
    tokenizer: Arc<SentimentTokenizer>,
    text_key: StateKey,
    fgi_compute: bool,
}

impl SentimentNode {
    pub fn new(id: NodeId, tokenizer: Arc<SentimentTokenizer>) -> Self {
        Self {
            id,
            tokenizer,
            text_key: "sentiment:text".into(),
            fgi_compute: false,
        }
    }
}

impl ComputeNode for SentimentNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }

    fn name(&self) -> &'static str {
        "SentimentNode"
    }

    fn input_keys(&self) -> Vec<StateKey> {
        vec![self.text_key.clone()]
    }

    fn output_keys(&self) -> Vec<StateKey> {
        let mut keys = vec!["sentiment:score".into(), "sentiment:confidence".into()];
        if self.fgi_compute {
            keys.push("sentiment:fgi".into());
        }
        keys
    }

    fn on_init(&mut self, config: &NodeConfig, _state: &StateStore) -> Result<()> {
        if let Some(key) = config.get_str("text_key") {
            self.text_key = key.to_string();
        }
        if let Some(flag) = config.get_str("fgi_compute") {
            self.fgi_compute = flag == "true";
        }
        Ok(())
    }

    fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
        // 情绪节点不处理 bar 数据
        Ok(())
    }

    fn on_calculate(&mut self, state: &StateStore) -> Result<Vec<Signal>> {
        // 读取文本
        let text: String = match state.get_raw(&self.text_key) {
            Some(StateValue::Json(v)) => v.as_str().unwrap_or_default().to_string(),
            Some(StateValue::Custom(tag, bytes)) if tag == "text" => {
                String::from_utf8_lossy(&bytes).to_string()
            }
            _ => {
                // 无文本输入时输出中性值
                self.write_defaults(state);
                return Ok(vec![]);
            }
        };

        if text.is_empty() {
            self.write_defaults(state);
            return Ok(vec![]);
        }

        // 执行情绪分析
        let result = self.tokenizer.analyze(&text);

        // 写入结果
        state.set(
            "sentiment:score".into(),
            StateValue::F64(result.score),
            self.id(),
        );
        state.set(
            "sentiment:confidence".into(),
            StateValue::F64(result.confidence),
            self.id(),
        );

        // 可选：计算 FGI
        if self.fgi_compute {
            // FGI 需要 5 个因子，此处从 StateStore 读取数值因子
            // 若缺失则使用默认中性值 50.0
            let hv20 = state
                .get::<f64>(&"sentiment factor:hv20".into())
                .unwrap_or(50.0);
            let momentum = state
                .get::<f64>(&"sentiment factor:momentum".into())
                .unwrap_or(50.0);
            let oi_change = state
                .get::<f64>(&"sentiment factor:oi_change".into())
                .unwrap_or(50.0);
            let basis = state
                .get::<f64>(&"sentiment factor:basis".into())
                .unwrap_or(50.0);
            let nlp = ((result.score + 1.0) / 2.0 * 100.0).clamp(0.0, 100.0);

            let fgi = FearGreedIndex::compute(hv20, momentum, oi_change, basis, nlp);
            state.set("sentiment:fgi".into(), StateValue::F64(fgi), self.id());
        }

        Ok(vec![])
    }

    fn subscribed_freqs(&self) -> Vec<Freq> {
        // 情绪分析不依赖特定周期
        vec![]
    }
}

impl SentimentNode {
    fn write_defaults(&self, state: &StateStore) {
        state.set("sentiment:score".into(), StateValue::F64(0.0), self.id());
        state.set(
            "sentiment:confidence".into(),
            StateValue::F64(0.0),
            self.id(),
        );
        if self.fgi_compute {
            state.set("sentiment:fgi".into(), StateValue::F64(50.0), self.id());
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::SentimentEntry;
    use std::collections::HashMap;

    fn make_tokenizer() -> Arc<SentimentTokenizer> {
        let mut dict = HashMap::new();
        dict.insert(
            "看好".into(),
            SentimentEntry {
                word: "看好".into(),
                polarity: 0.8,
                category: "sentiment".into(),
            },
        );
        dict.insert(
            "下跌".into(),
            SentimentEntry {
                word: "下跌".into(),
                polarity: -0.7,
                category: "price".into(),
            },
        );
        Arc::new(SentimentTokenizer::new(dict))
    }

    #[test]
    fn test_node_outputs_score_and_confidence() {
        let tokenizer = make_tokenizer();
        let store = StateStore::new();
        let mut node = SentimentNode::new("sent1".into(), tokenizer);

        // 写入文本
        store.set(
            "sentiment:text".into(),
            StateValue::Json(serde_json::Value::String("看好后市".into())),
            "upstream".into(),
        );

        node.on_calculate(&store).unwrap();

        let score: f64 = store.get(&"sentiment:score".into()).unwrap_or(0.0);
        let confidence: f64 = store.get(&"sentiment:confidence".into()).unwrap_or(0.0);

        assert!(score > 0.0, "正面文本应输出正分，实际: {}", score);
        assert!(confidence > 0.0, "置信度应 >0，实际: {}", confidence);
    }

    #[test]
    fn test_node_no_text_defaults() {
        let tokenizer = make_tokenizer();
        let store = StateStore::new();
        let mut node = SentimentNode::new("sent1".into(), tokenizer);

        node.on_calculate(&store).unwrap();

        let score: f64 = store.get(&"sentiment:score".into()).unwrap_or(1.0);
        assert_eq!(score, 0.0, "无文本时应输出 0");
        let confidence: f64 = store.get(&"sentiment:confidence".into()).unwrap_or(1.0);
        assert_eq!(confidence, 0.0, "无文本时置信度应为 0");
    }

    #[test]
    fn test_node_fgi_output() {
        let tokenizer = make_tokenizer();
        let store = StateStore::new();
        let mut node = SentimentNode::new("sent1".into(), tokenizer);
        node.fgi_compute = true;

        // 写入文本和因子
        store.set(
            "sentiment:text".into(),
            StateValue::Json(serde_json::Value::String("市场强势".into())),
            "upstream".into(),
        );
        store.set(
            "sentiment factor:hv20".into(),
            StateValue::F64(60.0),
            "factor".into(),
        );
        store.set(
            "sentiment factor:momentum".into(),
            StateValue::F64(70.0),
            "factor".into(),
        );
        store.set(
            "sentiment factor:oi_change".into(),
            StateValue::F64(55.0),
            "factor".into(),
        );
        store.set(
            "sentiment factor:basis".into(),
            StateValue::F64(45.0),
            "factor".into(),
        );

        node.on_calculate(&store).unwrap();

        let fgi: f64 = store.get(&"sentiment:fgi".into()).unwrap_or(-1.0);
        assert!(
            (0.0..=100.0).contains(&fgi),
            "FGI 应在 0-100 范围内，实际: {}",
            fgi
        );
    }

    #[test]
    fn test_node_on_init_config() {
        let tokenizer = make_tokenizer();
        let store = StateStore::new();
        let mut node = SentimentNode::new("sent1".into(), tokenizer);

        let mut config = NodeConfig::new();
        config.params.insert(
            "text_key".into(),
            serde_json::Value::String("news:text".into()),
        );
        config.params.insert(
            "fgi_compute".into(),
            serde_json::Value::String("true".into()),
        );

        node.on_init(&config, &store).unwrap();

        assert_eq!(node.text_key, "news:text");
        assert!(node.fgi_compute);
    }
}

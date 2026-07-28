use serde::{Deserialize, Serialize};

/// 交易决策输出 —— Agent 产出的结构化决策结果。
///
/// 所有 7 个分析 Agent 的结论汇总后，由 decision_agent 产出此结构。
/// direction / confidence / reasoning 为必填字段。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionOutput {
    /// 交易方向："long" | "short" | "hold"
    pub direction: String,
    /// 置信度 [0.0, 1.0]
    pub confidence: f64,
    /// 决策推理过程（自然语言）
    pub reasoning: String,
    /// 支撑决策的关键信号列表
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_signals: Vec<String>,
    /// 识别到的风险列表
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<String>,
}

/// LLM 流式输出的增量块。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChunk {
    /// 本次增量的文本片段
    pub delta: String,
    /// 流结束标记
    pub done: bool,
    /// 完成原因（流结束时有效）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_output_serde_roundtrip() {
        let original = DecisionOutput {
            direction: "long".into(),
            confidence: 0.85,
            reasoning: "三推衰竭 + 磁体共振，多周期确认".into(),
            key_signals: vec!["triple_push_exhaustion".into(), "magnet_resonance".into()],
            risks: vec!["gap_risk".into()],
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: DecisionOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_decision_output_minimal() {
        let json = r#"{"direction":"hold","confidence":0.5,"reasoning":"无明确信号"}"#;
        let parsed: DecisionOutput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.direction, "hold");
        assert_eq!(parsed.confidence, 0.5);
        assert!(parsed.key_signals.is_empty());
        assert!(parsed.risks.is_empty());
    }

    #[test]
    fn test_decision_output_missing_required_fields() {
        // 缺少 reasoning（必填）
        let json = r#"{"direction":"long","confidence":0.9}"#;
        let result: Result<DecisionOutput, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}

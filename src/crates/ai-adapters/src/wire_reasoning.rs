//! Extract thinking / effort values from the final provider request body.

use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireReasoningFields {
    pub thinking: Option<String>,
    pub effort: Option<String>,
}

pub fn extract_wire_reasoning_fields(body: &Value) -> WireReasoningFields {
    WireReasoningFields {
        thinking: extract_wire_thinking(body),
        effort: extract_wire_effort(body),
    }
}

fn extract_wire_thinking(body: &Value) -> Option<String> {
    body.pointer("/thinking/type")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            body.get("enable_thinking")
                .and_then(Value::as_bool)
                .map(|enabled| enabled.to_string())
        })
        .or_else(|| {
            body.pointer("/generationConfig/thinkingConfig/includeThoughts")
                .and_then(Value::as_bool)
                .map(|enabled| enabled.to_string())
        })
}

fn extract_wire_effort(body: &Value) -> Option<String> {
    body.get("reasoning_effort")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            body.pointer("/reasoning/effort")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            body.pointer("/output_config/effort")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_openai_chat_deepseek_fields() {
        let body = json!({
            "model": "deepseek-v4-pro",
            "thinking": { "type": "enabled" },
            "reasoning_effort": "max"
        });
        let fields = extract_wire_reasoning_fields(&body);
        assert_eq!(fields.thinking.as_deref(), Some("enabled"));
        assert_eq!(fields.effort.as_deref(), Some("max"));
    }

    #[test]
    fn extracts_responses_effort_field() {
        let body = json!({
            "model": "gpt-5",
            "reasoning": { "effort": "high" }
        });
        let fields = extract_wire_reasoning_fields(&body);
        assert!(fields.thinking.is_none());
        assert_eq!(fields.effort.as_deref(), Some("high"));
    }

    #[test]
    fn extracts_anthropic_adaptive_fields() {
        let body = json!({
            "model": "claude-sonnet-4-6",
            "thinking": { "type": "adaptive" },
            "output_config": { "effort": "medium" }
        });
        let fields = extract_wire_reasoning_fields(&body);
        assert_eq!(fields.thinking.as_deref(), Some("adaptive"));
        assert_eq!(fields.effort.as_deref(), Some("medium"));
    }

    #[test]
    fn returns_none_when_fields_absent() {
        let body = json!({ "model": "gpt-4o", "messages": [] });
        let fields = extract_wire_reasoning_fields(&body);
        assert!(fields.thinking.is_none());
        assert!(fields.effort.is_none());
    }
}

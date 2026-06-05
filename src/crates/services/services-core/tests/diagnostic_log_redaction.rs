use bitfun_services_core::diagnostics::{
    redact_diagnostic_log_text, redact_diagnostic_log_text_with_report,
};

#[test]
fn redacts_model_payloads_without_removing_routing_metadata() {
    let input = r#"[2026-05-13T10:38:21.837][DEBUG][ai::openai] Request body:
{
  "model": "kimi-k2.6",
  "api_key": "sk-secret-token",
  "messages": [
    {"role": "user", "content": "please review C:\\Users\\limit\\private\\file.rs"}
  ],
  "tools": [{"name": "Read"}],
  "tool_call": {"name": "Read", "arguments": "{\"path\":\"C:\\Users\\limit\\private\\file.rs\"}"}
}
Authorization: Bearer live-provider-token
"#;

    let report = redact_diagnostic_log_text_with_report(input);

    assert!(report.redaction_count >= 4);
    assert!(report.text.contains("[ai::openai]"));
    assert!(report.text.contains("\"model\": \"kimi-k2.6\""));
    assert!(report.text.contains("\"role\": \"user\""));
    assert!(report.text.contains("\"name\": \"Read\""));
    assert!(!report.text.contains("sk-secret-token"));
    assert!(!report.text.contains("live-provider-token"));
    assert!(!report.text.contains("please review"));
    assert!(!report.text.contains("C:\\Users\\limit"));
    assert!(report.text.contains("<redacted"));
}

#[test]
fn redacts_anthropic_stream_payloads_but_keeps_event_shape() {
    let input = r#"[TRACE][ai::anthropic_stream_response] Anthropic SSE: Event { event: "content_block_delta", data: "{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"-US secret argument\"}}", id: "", retry: None }
[TRACE][ai::anthropic_stream_response] Anthropic unified response: UnifiedResponse { text: Some("private answer"), reasoning_content: Some("hidden reasoning"), thinking_signature: "<omitted>", tool_call: Some(UnifiedToolCall { tool_call_index: Some(0), id: Some("toolu_1"), name: Some("Read"), arguments: Some("{\"path\":\"D:\\workspace\\secret\\main.rs\"}"), arguments_is_snapshot: false }), usage: None, finish_reason: None, provider_metadata: "<omitted>" }
"#;

    let redacted = redact_diagnostic_log_text(input);

    assert!(redacted.contains("Anthropic SSE"));
    assert!(redacted.contains("event: \"content_block_delta\""));
    assert!(redacted.contains("tool_call_index: Some(0)"));
    assert!(redacted.contains("name: Some(\"Read\")"));
    assert!(!redacted.contains("-US secret argument"));
    assert!(!redacted.contains("private answer"));
    assert!(!redacted.contains("hidden reasoning"));
    assert!(!redacted.contains("D:\\workspace\\secret"));
}

#[test]
fn handles_large_log_text_without_dropping_lines() {
    let mut input = String::new();
    for index in 0..2_000 {
        input.push_str(&format!(
            "[TRACE][webview] event={index} payload={{\"prompt\":\"secret prompt {index}\",\"path\":\"C:\\\\Users\\\\limit\\\\secret-{index}.txt\"}}\n"
        ));
    }

    let report = redact_diagnostic_log_text_with_report(&input);

    assert_eq!(report.text.lines().count(), 2_000);
    assert!(report.redaction_count >= 4_000);
    assert!(report.text.contains("[TRACE][webview] event=1999"));
    assert!(!report.text.contains("secret prompt"));
    assert!(!report.text.contains("C:\\\\Users\\\\limit"));
}

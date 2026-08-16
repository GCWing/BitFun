use bitfun_ai_adapters::providers::openai::OpenAIMessageConverter;
use bitfun_ai_adapters::Message;
use serde_json::json;

#[test]
fn chat_completions_preserves_empty_json_array_as_text() {
    let messages = OpenAIMessageConverter::convert_messages(vec![Message::user("[]".to_string())]);

    assert_eq!(messages[0]["content"], json!("[]"));
}

#[test]
fn responses_preserves_empty_json_array_as_text() {
    let (_, input) =
        OpenAIMessageConverter::convert_messages_to_responses_input(vec![Message::user(
            "[]".to_string(),
        )]);

    assert_eq!(input[0]["content"][0]["type"], json!("input_text"));
    assert_eq!(input[0]["content"][0]["text"], json!("[]"));
}

use bitfun_agent_tools::{InputValidator, ToolResult, ValidationResult};
use serde_json::json;

#[test]
fn validation_result_default_preserves_success_contract() {
    assert!(ValidationResult::default().result);
    assert_eq!(ValidationResult::default().message, None);
}

#[test]
fn input_validator_preserves_required_field_error() {
    let result = InputValidator::new(&json!({}))
        .validate_required("path")
        .finish();

    assert!(!result.result);
    assert_eq!(result.message.as_deref(), Some("path is required"));
    assert_eq!(result.error_code, Some(400));
}

#[test]
fn tool_result_ok_keeps_result_shape() {
    let result = ToolResult::ok(json!({"ok": true}), Some("done".to_string()));
    let value = serde_json::to_value(result).expect("serialize tool result");

    assert_eq!(value["type"], "result");
    assert_eq!(value["data"]["ok"], true);
    assert_eq!(value["result_for_assistant"], "done");
}

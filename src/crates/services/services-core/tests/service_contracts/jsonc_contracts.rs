use openbitfun_services_core::jsonc::strip_jsonc;

#[test]
fn jsonc_normalization_preserves_string_tokens_and_removes_trailing_commas() {
    let normalized = strip_jsonc(
        r#"{
          // line comment
          "url": "https://example.invalid/a//b",
          "marker": "/* keep */",
          "items": ["a", "b",],
          /* block
             comment */
          "nested": {"enabled": true,},
        }"#,
    );
    let value: serde_json::Value = serde_json::from_str(&normalized).expect("normalized json");

    assert_eq!(value["url"], "https://example.invalid/a//b");
    assert_eq!(value["marker"], "/* keep */");
    assert_eq!(value["items"], serde_json::json!(["a", "b"]));
    assert_eq!(value["nested"]["enabled"], true);
}

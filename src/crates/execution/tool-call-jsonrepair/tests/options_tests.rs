use bitfun_tool_call_jsonrepair::{
    jsonrepair, jsonrepair_to_writer_with_options, jsonrepair_with_options, JsonRepairErrorKind,
    RepairOptions,
};

#[test]
fn default_options_match_jsonrepair() {
    let input = "{name: 'Ada', active: True}";

    assert_eq!(
        jsonrepair_with_options(input, RepairOptions::default()).unwrap(),
        jsonrepair(input).unwrap()
    );
}

#[test]
fn strict_mode_passes_valid_json_unchanged() {
    let input = r#"{"name": "Ada", "active": true, "skills": ["rust"]}"#;

    assert_eq!(
        jsonrepair_with_options(input, RepairOptions::strict()).unwrap(),
        input
    );
}

#[test]
fn strict_mode_rejects_repairable_inputs() {
    for input in [
        "{name: 'Ada'}",
        "{\n  // comment\n  \"name\": \"Ada\"\n}",
        "```json\n{\"name\":\"Ada\"}\n```",
        "{\"active\": True}",
        "callback({\"name\":\"Ada\"});",
        "{\"value\": NaN}",
        "{\"items\": [1,2,]}",
        "{\"a\":1}\n{\"b\":2}",
    ] {
        let err = jsonrepair_with_options(input, RepairOptions::strict())
            .expect_err(&format!("expected strict error for {input:?}"));
        assert_eq!(err.kind, JsonRepairErrorKind::StrictModeViolation);
        assert!(err.line > 0);
        assert!(err.column > 0);
    }
}

#[test]
fn strict_mode_is_available_for_writer_helpers() {
    let mut output = Vec::new();
    let err =
        jsonrepair_to_writer_with_options("{name: 'Ada'}", &mut output, RepairOptions::strict())
            .unwrap_err();

    assert!(matches!(
        err,
        bitfun_tool_call_jsonrepair::JsonRepairWriteError::Repair(repair_err)
            if repair_err.kind == JsonRepairErrorKind::StrictModeViolation
    ));
    assert!(output.is_empty());
}

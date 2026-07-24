#![cfg(feature = "serde")]

use bitfun_tool_call_jsonrepair::{
    jsonrepair_parse, jsonrepair_parse_with_options, jsonrepair_value,
    jsonrepair_value_with_options, JsonRepairErrorKind, JsonRepairParseError, RepairOptions,
};

#[test]
fn repairs_and_returns_value() {
    let value = jsonrepair_value("{name: 'Ada', active: True, attempts: [1,2,],}").unwrap();

    assert_eq!(value["name"], "Ada");
    assert_eq!(value["active"], true);
    assert_eq!(value["attempts"][1], 2);
}

#[derive(Debug, serde::Deserialize, PartialEq)]
struct User {
    name: String,
    active: bool,
}

#[test]
fn repairs_and_deserializes_target_type() {
    let user: User = jsonrepair_parse("{name: 'Ada', active: True}").unwrap();

    assert_eq!(
        user,
        User {
            name: "Ada".to_string(),
            active: true,
        }
    );
}

#[test]
fn preserves_repair_errors() {
    let err = jsonrepair_value(r#""\u00""#).unwrap_err();

    assert!(matches!(err, JsonRepairParseError::Repair(_)));
}

#[test]
fn strict_options_apply_to_serde_helpers() {
    let value = jsonrepair_value_with_options(
        r#"{"name": "Ada", "active": true}"#,
        RepairOptions::strict(),
    )
    .unwrap();
    assert_eq!(value["name"], "Ada");

    let err = jsonrepair_parse_with_options::<User>(
        "{name: 'Ada', active: True}",
        RepairOptions::strict(),
    )
    .unwrap_err();

    assert!(matches!(
        err,
        JsonRepairParseError::Repair(repair_err)
            if repair_err.kind == JsonRepairErrorKind::StrictModeViolation
    ));
}

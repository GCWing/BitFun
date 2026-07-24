use bitfun_tool_call_jsonrepair::jsonrepair;
use serde_json::Value;

const CORPUS: &str = include_str!("fixtures/parity_cases.json");

#[test]
fn parity_fixture_corpus_repairs_expected_cases() {
    let corpus: Value = serde_json::from_str(CORPUS).unwrap();
    assert_eq!(corpus["schema_version"], 1);

    let cases = corpus["cases"]
        .as_array()
        .expect("parity corpus must contain a cases array");

    assert!(!cases.is_empty(), "parity corpus should not be empty");

    for case in cases {
        let name = required_str(case, "name");
        let category = required_str(case, "category");
        let input = required_str(case, "input");
        let expected = required_str(case, "expected");

        validate_divergence_metadata(name, case);

        let repaired =
            jsonrepair(input).unwrap_or_else(|err| panic!("{name} ({category}) failed: {err}"));

        assert_eq!(
            repaired, expected,
            "parity fixture {name} ({category}) did not match"
        );
    }
}

fn required_str<'a>(case: &'a Value, field: &str) -> &'a str {
    case[field]
        .as_str()
        .unwrap_or_else(|| panic!("parity case must include string field `{field}`: {case:?}"))
}

fn validate_divergence_metadata(name: &str, case: &Value) {
    match &case["divergence"] {
        Value::Null => {}
        Value::Object(divergence) => {
            assert!(
                divergence.get("name").and_then(Value::as_str).is_some(),
                "parity case {name} divergence must include a name"
            );
            assert!(
                divergence.get("reason").and_then(Value::as_str).is_some(),
                "parity case {name} divergence must include a reason"
            );
        }
        _ => panic!("parity case {name} divergence must be null or an object"),
    }
}

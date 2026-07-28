//! Safe JSON deserialization utilities (P1-8).
//!
//! Provides depth-limited JSON parsing to prevent stack overflow attacks
//! from deeply nested JSON payloads. The depth limit is set to 100 layers,
//! which is more than sufficient for any legitimate trading data while
//! blocking DoS vectors.

use serde::de::DeserializeOwned;
use serde_json::Value;

/// Maximum JSON nesting depth allowed.
pub const MAX_JSON_DEPTH: usize = 100;

/// Recursively check that a `serde_json::Value` tree does not exceed
/// `max_depth`. Returns `Ok(())` if within limits, `Err(msg)` otherwise.
fn check_depth(value: &Value, max_depth: usize, current_depth: usize) -> Result<(), String> {
    if current_depth > max_depth {
        return Err(format!(
            "JSON nesting depth {} exceeds limit {}",
            current_depth, max_depth
        ));
    }
    match value {
        Value::Array(arr) => {
            for item in arr {
                check_depth(item, max_depth, current_depth + 1)?;
            }
        }
        Value::Object(map) => {
            for (_k, v) in map {
                check_depth(v, max_depth, current_depth + 1)?;
            }
        }
        _ => {} // scalars and null don't add depth
    }
    Ok(())
}

/// Parse a JSON string with a depth limit.
///
/// Parses the JSON and verifies the nesting depth does not exceed
/// [`MAX_JSON_DEPTH`] (100). Returns the deserialized type on success,
/// or an error message if the JSON is too deeply nested or invalid.
pub fn from_json_str_limited<T: DeserializeOwned>(s: &str) -> Result<T, String> {
    // First parse into a generic Value to check depth
    let value: Value =
        serde_json::from_str(s).map_err(|e| format!("JSON parse error: {}", e))?;
    check_depth(&value, MAX_JSON_DEPTH, 0)?;
    // Re-parse into target type (avoids serde_json::from_value allocation overhead
    // for types that need it, but guarantees depth was checked first)
    serde_json::from_value(value).map_err(|e| format!("JSON deserialize error: {}", e))
}

/// Parse a YAML string with a depth limit.
///
/// YAML can represent deeply nested structures too. We parse it through
/// serde_json::Value (via serde_yaml → serde_json conversion) to apply
/// the same depth check.
pub fn from_yaml_str_limited<T: DeserializeOwned>(s: &str) -> Result<T, String> {
    let value: Value =
        serde_yaml::from_str(s).map_err(|e| format!("YAML parse error: {}", e))?;
    check_depth(&value, MAX_JSON_DEPTH, 0)?;
    serde_json::from_value(value).map_err(|e| format!("YAML deserialize error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct SimpleConfig {
        name: String,
        value: i32,
    }

    #[test]
    fn test_shallow_json() {
        let result: SimpleConfig =
            from_json_str_limited(r#"{"name": "test", "value": 42}"#).unwrap();
        assert_eq!(result.name, "test");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn test_deeply_nested_json_rejected() {
        // Build a JSON string with depth > MAX_JSON_DEPTH (100) but
        // within serde_json's default recursion limit (128).
        let mut s = String::new();
        let depth = 110;
        for _ in 0..depth {
            s.push_str("{\"nested\":");
        }
        s.push_str("42");
        for _ in 0..depth {
            s.push('}');
        }
        let result: Result<Value, _> = from_json_str_limited(&s);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("depth"));
    }

    #[test]
    fn test_invalid_json() {
        let result: Result<Value, _> = from_json_str_limited("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_array_depth() {
        let result: Result<Value, _> =
            from_json_str_limited("[[[[[[[[[[[[[[[[[[[[[42]]]]]]]]]]]]]]]]]]]]]");
        // This should work because depth is only ~21
        assert!(result.is_ok());
    }
}

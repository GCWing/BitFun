//! Shared JSON argument helpers for Tauri-shaped HostInvoke payloads.

use serde_json::Value;

/// Extract the `request` object from Tauri-style params (`{ request: { ... } }`).
/// Falls back to the top-level object when `request` is absent.
pub(crate) fn request_value(args: &Value) -> &Value {
    args.get("request").unwrap_or(args)
}

/// Look up a field by camelCase key, falling back to snake_case.
fn field<'a>(obj: &'a Value, camel: &str, snake: &str) -> Option<&'a Value> {
    obj.get(camel).or_else(|| obj.get(snake))
}

pub(crate) fn get_string(obj: &Value, camel: &str) -> Result<String, String> {
    let snake = camel_to_snake(camel);
    field(obj, camel, &snake)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Missing or invalid '{camel}' field"))
}

pub(crate) fn optional_string(obj: &Value, camel: &str) -> Option<String> {
    let snake = camel_to_snake(camel);
    field(obj, camel, &snake)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub(crate) fn optional_bool(obj: &Value, camel: &str) -> Option<bool> {
    let snake = camel_to_snake(camel);
    field(obj, camel, &snake).and_then(|v| v.as_bool())
}

pub(crate) fn get_usize(obj: &Value, camel: &str) -> Result<usize, String> {
    let snake = camel_to_snake(camel);
    let value =
        field(obj, camel, &snake).ok_or_else(|| format!("Missing or invalid '{camel}' field"))?;
    value
        .as_u64()
        .map(|n| n as usize)
        .or_else(|| value.as_i64().filter(|n| *n >= 0).map(|n| n as usize))
        .ok_or_else(|| format!("Missing or invalid '{camel}' field"))
}

fn camel_to_snake(camel: &str) -> String {
    let mut out = String::with_capacity(camel.len() + 4);
    for (i, ch) in camel.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prefers_camel_then_snake() {
        let camel = json!({ "workspacePath": "/a" });
        let snake = json!({ "workspace_path": "/b" });
        assert_eq!(get_string(&camel, "workspacePath").unwrap(), "/a");
        assert_eq!(get_string(&snake, "workspacePath").unwrap(), "/b");
        assert_eq!(
            optional_string(&camel, "workspacePath").as_deref(),
            Some("/a")
        );
    }
}

//! Browser snapshot presentation, independent of transport and page IO.
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) fn resolve_element_js(selector: &str) -> String {
    let selector = if selector.starts_with("@e") {
        format!("[data-cdp-ref={}]", json!(selector))
    } else {
        selector.to_string()
    };
    format!(
        "const el = ({})({});",
        include_str!("resolve_element.js"),
        json!(selector)
    )
}

/// An evaluation failure must never look like a successful empty page.
pub(super) fn parse_snapshot_result(result: &Value) -> Result<Value, String> {
    let text = result
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(Value::as_str)
        .ok_or("Browser snapshot returned no JSON string")?;
    let parsed: Value = serde_json::from_str(text)
        .map_err(|e| format!("Browser snapshot returned invalid JSON: {e}"))?;
    let elements = parsed
        .get("elements")
        .and_then(Value::as_array)
        .ok_or("Browser snapshot is missing its elements array")?;
    let mut refs = std::collections::BTreeSet::new();
    for element in elements {
        let reference = element
            .get("ref")
            .and_then(Value::as_str)
            .filter(|r| !r.is_empty())
            .ok_or("Browser snapshot element is missing its ref")?;
        if !refs.insert(reference) {
            return Err(format!(
                "Browser snapshot contains duplicate ref {reference}"
            ));
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_dom_shape_falls_back_past_empty_labels_and_retains_refs() {
        let mut value = json!({"url":"https://example.test", "elements":[
            {"ref":"@e1", "tag":"button", "ariaLabel":"", "placeholder":"", "text":"保存", "scope":"shadow", "frame_path":"iframe[0]", "backend_node_id":42},
            {"ref":"@e2", "tag":"input", "ariaLabel":null, "placeholder":" Search ", "text":"", "value":"current query", "disabled":true},
            {"ref":"@e3", "tag":"input", "ariaLabel":"  ", "placeholder":"", "text":"", "name":"email", "checked":false}
        ], "offscreen_count":3, "cross_origin_frames":1});
        let originals = value["elements"].clone();
        attach_snapshot_text(&mut value);
        let text = value["snapshot"].as_str().unwrap();
        for expected in [
            "保存",
            "Search",
            "email",
            "current query",
            "[disabled]",
            "checked=false",
            "scope=shadow",
            "iframe[0]",
            "3 more",
            "1 cross-origin",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        assert_eq!(value["elements"], originals);
        for element in originals.as_array().unwrap() {
            assert_eq!(value["refs"][element["ref"].as_str().unwrap()], *element);
        }
    }

    #[test]
    fn malformed_snapshot_is_an_error_but_empty_page_is_valid() {
        for value in [
            json!({}),
            json!({"result":{"value":"bad"}}),
            json!({"result":{"value":"{}"}}),
            json!({"result":{"value":"{\"elements\":[{\"ref\":\"@e1\"},{\"ref\":\"@e1\"}]}"}}),
        ] {
            assert!(parse_snapshot_result(&value).is_err());
        }
        assert!(parse_snapshot_result(&json!({"result":{"value":"{\"elements\":[]}"}})).is_ok());
    }

    #[test]
    #[ignore = "run through scripts/test-browser-snapshot.mjs with a real Chromium payload"]
    fn real_browser_snapshot_survives_rust_presentation() {
        let path = std::env::var("OPENBITFUN_BROWSER_SNAPSHOT_FIXTURE")
            .expect("real browser fixture path");
        let source = std::fs::read_to_string(path).unwrap();
        let mut parsed = parse_snapshot_result(&json!({"result":{"value":source}})).unwrap();
        let elements = parsed["elements"].clone();
        attach_snapshot_text(&mut parsed);
        let text = parsed["snapshot"].as_str().unwrap();
        for expected in [
            "保存",
            "Search label",
            "current query",
            "[disabled]",
            "checked=true",
            "Nested",
            "Shadow frame",
        ] {
            assert!(
                text.contains(expected),
                "missing actual browser context {expected}: {text}"
            );
        }
        assert!(!parsed.to_string().contains("must-not-appear"));
        assert_eq!(
            parsed["refs"].as_object().unwrap().len(),
            elements.as_array().unwrap().len()
        );
        for element in elements.as_array().unwrap() {
            assert_eq!(parsed["refs"][element["ref"].as_str().unwrap()], *element);
        }
    }
}

pub(super) fn attach_snapshot_text(parsed: &mut Value) {
    let Some(elements) = parsed.get("elements").and_then(|v| v.as_array()) else {
        return;
    };
    let mut lines = Vec::<String>::new();
    let mut refs = BTreeMap::<String, Value>::new();
    for element in elements {
        let reference = element.get("ref").and_then(|v| v.as_str()).unwrap_or("");
        let tag = element
            .get("tag")
            .and_then(|v| v.as_str())
            .unwrap_or("element");
        let role = element.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let text = ["ariaLabel", "label", "placeholder", "text", "name"]
            .iter()
            .filter_map(|key| element.get(key).and_then(Value::as_str))
            .map(str::trim)
            .find(|s| !s.is_empty())
            .unwrap_or("");
        let type_text = element.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let frame_path = element
            .get("frame_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let scope = element
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("document");
        let mut label = if role.is_empty() {
            tag.to_string()
        } else {
            role.to_string()
        };
        if !type_text.is_empty() {
            label.push(':');
            label.push_str(type_text);
        }
        let mut line = if reference.is_empty() {
            format!("- {}", label)
        } else {
            format!("- {} [{}]", label, reference)
        };
        if !text.is_empty() {
            let clipped = if text.chars().count() > 80 {
                format!("{}...", text.chars().take(77).collect::<String>())
            } else {
                text.to_string()
            };
            line.push(' ');
            line.push_str(&serde_json::to_string(&clipped).unwrap_or_else(|_| "\"\"".to_string()));
            if element.get("text_truncated").and_then(Value::as_bool) == Some(true) {
                line.push_str(" [text truncated]");
            }
        }
        if !id.is_empty() {
            line.push_str(&format!(" id={}", id));
        }
        if element.get("disabled").and_then(Value::as_bool) == Some(true) {
            line.push_str(" [disabled]");
        }
        for key in ["checked", "selected", "expanded"] {
            if let Some(value) = element.get(key).filter(|v| !v.is_null()) {
                line.push_str(&format!(" {key}={value}"));
            }
        }
        if type_text != "password" {
            if let Some(value) = element
                .get("value")
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
            {
                let clipped: String = value.chars().take(120).collect();
                line.push_str(&format!(" value={}", json!(clipped)));
                if value.chars().count() > 120
                    || element.get("value_truncated").and_then(Value::as_bool) == Some(true)
                {
                    line.push_str(" [value truncated]");
                }
            }
        }
        if scope != "document" || !frame_path.is_empty() {
            line.push_str(&format!(" scope={}", scope));
            if !frame_path.is_empty() {
                line.push_str(&format!(" frame={}", frame_path));
            }
        }
        lines.push(line);
        if !reference.is_empty() {
            refs.insert(reference.to_string(), element.clone());
        }
    }
    let offscreen = parsed
        .get("offscreen_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if offscreen > 0 {
        lines.push(format!(
            "- note: {} more interactive element(s) exist outside the current viewport and are NOT listed above; scroll toward them and snapshot again to get refs for them",
            offscreen
        ));
    }
    let cross_origin_frames = parsed
        .get("cross_origin_frames")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if cross_origin_frames > 0 {
        lines.push(format!(
            "- note: this page contains {} cross-origin iframe(s) whose contents cannot be inspected; elements inside them are absent here and cannot be targeted by @eN refs",
            cross_origin_frames
        ));
    }
    if let Some(obj) = parsed.as_object_mut() {
        obj.insert("snapshot".to_string(), json!(lines.join("\n")));
        obj.insert("refs".to_string(), json!(refs));
    }
}

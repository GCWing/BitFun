use crate::types::{AIConfig, RemoteModelInfo};
use std::time::Instant;

pub(crate) fn merge_json_value(target: &mut serde_json::Value, overlay: serde_json::Value) {
    match (target, overlay) {
        (serde_json::Value::Object(target_map), serde_json::Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                let entry = target_map.entry(key).or_insert(serde_json::Value::Null);
                merge_json_value(entry, value);
            }
        }
        (target_slot, overlay_value) => {
            *target_slot = overlay_value;
        }
    }
}

pub(crate) fn is_trim_custom_request_body_mode(config: &AIConfig) -> bool {
    config.custom_request_body_mode.as_deref() == Some("trim")
}

pub(crate) fn build_request_body_subset(
    source: &serde_json::Value,
    top_level_keys: &[&str],
    nested_fields: &[(&str, &str)],
) -> serde_json::Value {
    let mut subset = serde_json::Map::new();

    if let Some(source_obj) = source.as_object() {
        for key in top_level_keys {
            if let Some(value) = source_obj.get(*key) {
                subset.insert((*key).to_string(), value.clone());
            }
        }
    }

    for (parent, child) in nested_fields {
        let Some(child_value) = source
            .get(*parent)
            .and_then(serde_json::Value::as_object)
            .and_then(|parent_obj| parent_obj.get(*child))
            .cloned()
        else {
            continue;
        };

        let parent_entry = subset
            .entry((*parent).to_string())
            .or_insert_with(|| serde_json::json!({}));

        if !parent_entry.is_object() {
            *parent_entry = serde_json::json!({});
        }

        parent_entry
            .as_object_mut()
            .expect("protected request subset parent must be object")
            .insert((*child).to_string(), child_value);
    }

    serde_json::Value::Object(subset)
}

pub(crate) fn dedupe_remote_models(models: Vec<RemoteModelInfo>) -> Vec<RemoteModelInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();

    for model in models {
        if seen.insert(model.id.clone()) {
            deduped.push(model);
        }
    }

    deduped
}

pub(crate) fn normalize_base_url_for_discovery(base_url: &str) -> String {
    base_url
        .trim()
        .trim_end_matches('#')
        .trim_end_matches('/')
        .to_string()
}

pub(crate) fn elapsed_ms_u64(started_at: Instant) -> u64 {
    started_at.elapsed().as_millis() as u64
}

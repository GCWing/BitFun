//! Read compatibility for the retired built-in Agent/profile identities.
//! Migration is atomic: ambiguous collisions return an error before any write.

use openbitfun_core_types::agent_identity::canonical_agent_config_id;
use serde_json::{Map, Value};

pub fn canonicalize_agent_profile_keys(
    profiles: &Map<String, Value>,
) -> Result<Map<String, Value>, String> {
    let mut result = Map::new();
    for (old_id, value) in profiles {
        let id = canonical_agent_config_id(old_id);
        let mut value = value.clone();
        if let Some(object) = value.as_object_mut() {
            if object.contains_key("profile_id") {
                object.insert("profile_id".into(), Value::String(id.into()));
            }
        }
        if let Some(existing) = result.get_mut(id) {
            merge_compatible(existing, value).map_err(|()| {
                format!("Conflicting Agent profile aliases for '{id}' (including '{old_id}'); original configuration was preserved")
            })?;
        } else {
            result.insert(id.into(), value);
        }
    }
    Ok(result)
}

fn merge_compatible(existing: &mut Value, incoming: Value) -> Result<(), ()> {
    if existing == &incoming || incoming.is_null() {
        return Ok(());
    }
    if existing.is_null() {
        *existing = incoming;
        return Ok(());
    }
    if let (Value::Object(existing), Value::Object(incoming)) = (existing, incoming) {
        for (key, value) in incoming {
            if let Some(previous) = existing.get_mut(&key) {
                merge_compatible(previous, value)?;
            } else {
                existing.insert(key, value);
            }
        }
        Ok(())
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn migrates_legacy_keys_and_references_without_changing_settings() {
        let raw = json!({
            "coding_shared": {"profile_id": "coding_shared", "removed_tools": ["Bash"], "unknown_future_field": true},
            "Standard": {"enabled_user_skills": ["user::docs"]},
            "Ultra": {"profile_id": "Ultra", "subagent_overrides": {"SwarmWorker": "disabled"}},
            "plugin::agentic": {"profile_id": "plugin::agentic"}
        });
        let migrated = canonicalize_agent_profile_keys(raw.as_object().unwrap()).unwrap();
        assert_eq!(migrated["Standard"]["profile_id"], "Standard");
        assert_eq!(migrated["Standard"]["removed_tools"], json!(["Bash"]));
        assert_eq!(
            migrated["Standard"]["enabled_user_skills"],
            json!(["user::docs"])
        );
        assert_eq!(migrated["Standard"]["unknown_future_field"], true);
        assert_eq!(migrated["Ultimate"]["profile_id"], "Ultimate");
        assert!(migrated.contains_key("plugin::agentic"));
        assert!(!migrated.contains_key("coding_shared"));
        assert_eq!(
            canonicalize_agent_profile_keys(&migrated).unwrap(),
            migrated
        );
    }

    #[test]
    fn conflicting_aliases_do_not_mutate_original_records() {
        let raw =
            json!({"agentic": {"removed_tools": ["Bash"]}, "Standard": {"removed_tools": []}});
        let before = raw.clone();
        assert!(canonicalize_agent_profile_keys(raw.as_object().unwrap()).is_err());
        assert_eq!(raw, before);
    }

    #[test]
    fn typed_profiles_round_trip_legacy_keys_and_future_fields() {
        let ai: crate::types::AIConfig = serde_json::from_value(json!({
            "agent_profiles": {
                "coding_shared": {"removed_tools": ["Bash"], "future_policy": {"enabled": false}},
                "Ultra": {"profile_id": "Ultra", "enabled_user_skills": ["user::docs"]},
                "custom::agentic": {"disabled_user_skills": ["user::private"]}
            }
        }))
        .unwrap();
        assert_eq!(ai.agent_profiles["Standard"].profile_id, "Standard");
        assert_eq!(ai.agent_profiles["Ultimate"].profile_id, "Ultimate");
        let saved = serde_json::to_value(&ai).unwrap();
        assert_eq!(
            saved["agent_profiles"]["Standard"]["future_policy"],
            json!({"enabled": false})
        );
        assert_eq!(
            saved["agent_profiles"]["Standard"]["removed_tools"],
            json!(["Bash"])
        );
        assert!(saved["agent_profiles"].get("coding_shared").is_none());
        assert!(saved["agent_profiles"].get("custom::agentic").is_some());
        let reloaded: crate::types::AIConfig = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(serde_json::to_value(reloaded).unwrap(), saved);
    }

    #[test]
    fn legacy_selector_preferences_keep_their_selection_strategy() {
        use crate::types::{AppFlowChatConfig, ChatInputDefaultModeStrategy};
        let fixed: AppFlowChatConfig = serde_json::from_value(json!({
            "default_mode_id": "agentic", "last_mode_id": "Ultra"
        }))
        .unwrap();
        assert_eq!(fixed.default_mode_strategy, None);
        assert_eq!(fixed.default_mode_id.as_deref(), Some("Standard"));
        assert_eq!(fixed.last_mode_id.as_deref(), Some("Ultimate"));
        let following: AppFlowChatConfig = serde_json::from_value(json!({
            "default_mode_strategy": "follow_last", "last_mode_id": "minimal",
            "show_permission_mode_control": false
        }))
        .unwrap();
        let written = serde_json::to_value(&following).unwrap();
        assert_eq!(written["last_mode_id"], "Minimal");
        let reloaded: AppFlowChatConfig = serde_json::from_value(written).unwrap();
        assert_eq!(
            reloaded.default_mode_strategy,
            Some(ChatInputDefaultModeStrategy::FollowLast)
        );
        assert!(!reloaded.show_permission_mode_control);
    }
}

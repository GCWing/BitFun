//! Group chat membership back-index on session `custom_metadata`.
//!
//! When a member joins/leaves a group (or a group is deleted), the member
//! session's `custom_metadata.groupChats` array is updated so rooms can be
//! resolved back from a session (reverse lookup). The merge is applied through
//! the same object-merge semantics as `merge_session_custom_metadata`
//! (metadata.rs:375-385) so concurrent patches never drop each other's keys.
//!
//! The `groupChats` key does not collide with the lineage-preserved keys
//! (lineage.rs:12-20: kind / parentSessionId / parentRequestId /
//! parentDialogTurnId / parentTurnIndex / parentToolCallId / subagentType).
//!
//! Contract: type-contract v1.3 §1.3 (R-GC-05).

use serde_json::{json, Map as JsonMap, Value as JsonValue};

/// Custom-metadata key holding the room ids a session belongs to.
pub const GROUP_CHATS_METADATA_KEY: &str = "groupChats";

/// Adds `room_id` to the `groupChats` array carried by `custom_metadata`
/// (dedup, no-op when already present). Returns the replacement value for
/// `custom_metadata`, ready to be persisted with the same object-merge
/// semantics as `merge_session_custom_metadata`.
pub fn add_room_to_group_chats(custom_metadata: Option<&JsonValue>, room_id: &str) -> JsonValue {
    let mut rooms = group_chats_of(custom_metadata);
    if !rooms.iter().any(|existing| existing == room_id) {
        rooms.push(room_id.to_string());
    }
    merge_group_chats_into(custom_metadata, rooms)
}

/// Removes `room_id` from the `groupChats` array carried by `custom_metadata`.
/// Returns the replacement value for `custom_metadata`; the `groupChats` key is
/// dropped entirely when the last room id is removed.
pub fn remove_room_from_group_chats(
    custom_metadata: Option<&JsonValue>,
    room_id: &str,
) -> JsonValue {
    let remaining: Vec<String> = group_chats_of(custom_metadata)
        .into_iter()
        .filter(|existing| existing != room_id)
        .collect();
    merge_group_chats_into(custom_metadata, remaining)
}

/// Returns the current `groupChats` array (empty when absent or malformed).
pub fn group_chats_of(custom_metadata: Option<&JsonValue>) -> Vec<String> {
    let Some(JsonValue::Object(map)) = custom_metadata else {
        return Vec::new();
    };
    match map.get(GROUP_CHATS_METADATA_KEY) {
        Some(JsonValue::Array(values)) => values
            .iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// Applies object-level merge (mirrors metadata.rs merge_session_custom_metadata):
/// `groupChats` is set to the given array; unrelated keys are preserved; when
/// the array is empty the key is removed. Returns the resulting metadata value.
fn merge_group_chats_into(custom_metadata: Option<&JsonValue>, rooms: Vec<String>) -> JsonValue {
    let rooms = if rooms.is_empty() {
        None
    } else {
        Some(JsonValue::Array(
            rooms.into_iter().map(JsonValue::String).collect(),
        ))
    };
    match custom_metadata {
        Some(JsonValue::Object(map)) => {
            let mut updated: JsonMap<String, JsonValue> = map.clone();
            match rooms {
                Some(rooms) => {
                    updated.insert(GROUP_CHATS_METADATA_KEY.to_string(), rooms);
                }
                None => {
                    updated.remove(GROUP_CHATS_METADATA_KEY);
                }
            }
            if updated.is_empty() {
                json!({})
            } else {
                JsonValue::Object(updated)
            }
        }
        _ => match rooms {
            Some(rooms) => json!({ GROUP_CHATS_METADATA_KEY: rooms }),
            None => json!({}),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn add_room_creates_group_chats_when_metadata_is_absent() {
        let patched = add_room_to_group_chats(None, "room-1");
        assert_eq!(patched, json!({ "groupChats": ["room-1"] }));
    }

    #[test]
    fn add_room_appends_and_dedups() {
        let metadata = json!({ "otherKey": "keep", "groupChats": ["room-1"] });
        let patched = add_room_to_group_chats(Some(&metadata), "room-2");
        assert_eq!(
            patched,
            json!({ "otherKey": "keep", "groupChats": ["room-1", "room-2"] })
        );

        let patched_dup = add_room_to_group_chats(Some(&metadata), "room-1");
        assert_eq!(
            patched_dup,
            json!({ "otherKey": "keep", "groupChats": ["room-1"] })
        );
    }

    #[test]
    fn add_room_preserves_unrelated_keys() {
        let metadata = json!({ "kind": "subagent", "groupChats": [] });
        let patched = add_room_to_group_chats(Some(&metadata), "room-1");
        assert_eq!(
            patched,
            json!({ "kind": "subagent", "groupChats": ["room-1"] })
        );
    }

    #[test]
    fn remove_room_keeps_other_rooms_and_preserves_keys() {
        let metadata = json!({ "otherKey": "keep", "groupChats": ["room-1", "room-2"] });
        let patched = remove_room_from_group_chats(Some(&metadata), "room-1");
        assert_eq!(
            patched,
            json!({ "otherKey": "keep", "groupChats": ["room-2"] })
        );
    }

    #[test]
    fn remove_last_room_drops_the_key_entirely() {
        let metadata = json!({ "otherKey": "keep", "groupChats": ["room-1"] });
        let patched = remove_room_from_group_chats(Some(&metadata), "room-1");
        assert_eq!(patched, json!({ "otherKey": "keep" }));
    }

    #[test]
    fn remove_room_when_metadata_is_absent_returns_empty_object() {
        let patched = remove_room_from_group_chats(None, "room-1");
        assert_eq!(patched, json!({}));
    }

    #[test]
    fn group_chats_of_reads_array_and_tolerates_malformed() {
        assert_eq!(group_chats_of(None), Vec::<String>::new());
        assert_eq!(
            group_chats_of(Some(&json!({ "groupChats": ["a", "b"] }))),
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(
            group_chats_of(Some(&json!({ "groupChats": "not-an-array" }))),
            Vec::<String>::new()
        );
        assert_eq!(
            group_chats_of(Some(&json!({ "groupChats": [42, "a"] }))),
            vec!["a".to_string()]
        );
    }

    #[test]
    fn group_chats_key_does_not_collide_with_lineage_preserved_keys() {
        // lineage.rs:12-20 preserves 7 keys; groupChats is an 8th, disjoint key.
        let lineage_keys = [
            "kind",
            "parentSessionId",
            "parentRequestId",
            "parentDialogTurnId",
            "parentTurnIndex",
            "parentToolCallId",
            "subagentType",
        ];
        assert!(!lineage_keys.contains(&GROUP_CHATS_METADATA_KEY));
    }

    #[test]
    fn merge_semantics_match_merge_session_custom_metadata() {
        // The service-layer merge (metadata.rs:375-385) replaces keys in a
        // patch object; our helpers produce exactly such a patch-compatible
        // result: unrelated keys preserved, groupChats replaced wholesale.
        let metadata = json!({ "otherKey": "keep", "groupChats": ["room-1"] });
        let patched = add_room_to_group_chats(Some(&metadata), "room-1");
        let mut merged = metadata.clone();
        merged["groupChats"] = patched["groupChats"].clone();
        assert_eq!(merged, metadata, "dedup add must be a no-op through merge");
    }
}

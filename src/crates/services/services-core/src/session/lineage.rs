//! Session lineage and branch metadata mutation rules.

use super::types::{
    DialogTurnData, SessionMetadata, SessionRelationship, SessionRelationshipKind, SessionStatus,
};
use bitfun_core_types::SessionKind;
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::{HashMap, HashSet};

const LINEAGE_CUSTOM_METADATA_KEYS: &[&str] = &[
    "kind",
    "parentSessionId",
    "parentRequestId",
    "parentDialogTurnId",
    "parentTurnIndex",
    "parentToolCallId",
    "subagentType",
];

const BRANCH_EXCLUDED_TAGS: &[&str] = &["btw", "review", "deep_review", "miniapp", "subagent"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct SubagentRelationshipFacts {
    kind: SessionRelationshipKind,
    parent_session_id: String,
    parent_dialog_turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBranchRequest {
    pub source_session_id: String,
    pub source_turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBranchResult {
    pub session_id: String,
    pub session_name: String,
    pub agent_type: String,
}

#[derive(Debug, Clone)]
pub struct BranchSessionMetadataFacts<'a> {
    pub source_metadata: &'a SessionMetadata,
    pub target_session_id: String,
    pub target_session_name: String,
    pub target_agent_type: String,
    pub source_session_id: &'a str,
    pub source_turn_id: &'a str,
    pub source_turn_index: usize,
    pub branched_turns: &'a [DialogTurnData],
    pub now_ms: u64,
}

pub fn apply_session_lineage(metadata: &mut SessionMetadata, relationship: SessionRelationship) {
    metadata.relationship = Some(relationship);
    metadata.custom_metadata = strip_lineage_custom_metadata(metadata.custom_metadata.take());
}

fn strip_lineage_custom_metadata(value: Option<JsonValue>) -> Option<JsonValue> {
    let Some(JsonValue::Object(mut metadata)) = value else {
        return None;
    };

    for key in LINEAGE_CUSTOM_METADATA_KEYS {
        metadata.remove(*key);
    }

    (!metadata.is_empty()).then_some(JsonValue::Object(metadata))
}

fn extract_subagent_relationship(metadata: &SessionMetadata) -> Option<SubagentRelationshipFacts> {
    let relationship = metadata.relationship.as_ref();
    let custom_metadata = metadata.custom_metadata.as_ref();

    let kind = relationship
        .and_then(|value| value.kind.clone())
        .or_else(|| {
            custom_metadata
                .and_then(|value| value.get("kind"))
                .and_then(|value| value.as_str())
                .and_then(|value| match value {
                    "subagent" => Some(SessionRelationshipKind::Subagent),
                    _ => None,
                })
        })?;

    let parent_session_id = relationship
        .and_then(|value| value.parent_session_id.clone())
        .or_else(|| {
            custom_metadata
                .and_then(|value| value.get("parentSessionId"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })?;

    let parent_dialog_turn_id = relationship
        .and_then(|value| value.parent_dialog_turn_id.clone())
        .or_else(|| {
            custom_metadata
                .and_then(|value| value.get("parentDialogTurnId"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })?;

    Some(SubagentRelationshipFacts {
        kind,
        parent_session_id,
        parent_dialog_turn_id,
    })
}

pub fn collect_hidden_subagent_cascade(
    metadata_list: impl IntoIterator<Item = SessionMetadata>,
    parent_session_id: &str,
    parent_dialog_turn_ids: &HashSet<String>,
) -> Vec<String> {
    if parent_session_id.trim().is_empty() || parent_dialog_turn_ids.is_empty() {
        return Vec::new();
    }

    let mut child_session_ids_by_parent: HashMap<String, Vec<String>> = HashMap::new();
    let mut root_session_ids = Vec::new();

    for metadata in metadata_list {
        let Some(relationship) = extract_subagent_relationship(&metadata) else {
            continue;
        };

        if relationship.kind != SessionRelationshipKind::Subagent {
            continue;
        }

        child_session_ids_by_parent
            .entry(relationship.parent_session_id.clone())
            .or_default()
            .push(metadata.session_id.clone());

        if relationship.parent_session_id == parent_session_id
            && parent_dialog_turn_ids.contains(&relationship.parent_dialog_turn_id)
        {
            root_session_ids.push(metadata.session_id);
        }
    }

    let mut visited = HashSet::new();
    let mut ordered_session_ids = Vec::new();

    for root_session_id in root_session_ids {
        collect_subagent_post_order(
            &root_session_id,
            &child_session_ids_by_parent,
            &mut visited,
            &mut ordered_session_ids,
        );
    }

    ordered_session_ids
}

fn collect_subagent_post_order(
    session_id: &str,
    child_session_ids_by_parent: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    ordered_session_ids: &mut Vec<String>,
) {
    if !visited.insert(session_id.to_string()) {
        return;
    }

    if let Some(child_session_ids) = child_session_ids_by_parent.get(session_id) {
        for child_session_id in child_session_ids {
            collect_subagent_post_order(
                child_session_id,
                child_session_ids_by_parent,
                visited,
                ordered_session_ids,
            );
        }
    }

    ordered_session_ids.push(session_id.to_string());
}

pub fn build_branched_session_metadata(facts: BranchSessionMetadataFacts<'_>) -> SessionMetadata {
    let mut metadata = facts.source_metadata.clone();
    metadata.session_id = facts.target_session_id;
    metadata.session_name = facts.target_session_name;
    metadata.agent_type = facts.target_agent_type;
    metadata.created_by = None;
    metadata.session_kind = SessionKind::Standard;
    metadata.created_at = facts.now_ms;
    metadata.last_active_at = facts.now_ms;
    metadata.last_finished_at = None;
    metadata.turn_count = facts.branched_turns.len();
    metadata.message_count = facts
        .branched_turns
        .iter()
        .map(estimate_turn_message_count)
        .sum();
    metadata.tool_call_count = facts
        .branched_turns
        .iter()
        .map(DialogTurnData::count_tool_calls)
        .sum();
    metadata.status = SessionStatus::Active;
    metadata.snapshot_session_id = None;
    metadata
        .tags
        .retain(|tag| !BRANCH_EXCLUDED_TAGS.contains(&tag.as_str()));
    metadata.custom_metadata = build_branch_custom_metadata(
        facts.source_metadata.custom_metadata.as_ref(),
        facts.source_session_id,
        facts.source_turn_id,
        facts.source_turn_index,
    );
    metadata.relationship = None;
    metadata.todos = None;
    metadata.deep_review_run_manifest = None;
    metadata.unread_completion = None;
    metadata.needs_user_attention = None;
    metadata
}

fn estimate_turn_message_count(turn: &DialogTurnData) -> usize {
    1 + turn
        .model_rounds
        .iter()
        .map(|round| round.text_items.len())
        .sum::<usize>()
}

fn strip_child_session_metadata(value: Option<&JsonValue>) -> Option<JsonValue> {
    let Some(JsonValue::Object(existing)) = value else {
        return None;
    };

    let mut next = existing.clone();
    for key in [
        "kind",
        "parentSessionId",
        "parentRequestId",
        "parentDialogTurnId",
        "parentTurnIndex",
    ] {
        next.remove(key);
    }
    Some(JsonValue::Object(next))
}

fn build_branch_custom_metadata(
    source_metadata: Option<&JsonValue>,
    source_session_id: &str,
    source_turn_id: &str,
    source_turn_index: usize,
) -> Option<JsonValue> {
    let mut base = match strip_child_session_metadata(source_metadata) {
        Some(JsonValue::Object(map)) => map,
        _ => JsonMap::new(),
    };

    base.insert(
        "forkOrigin".to_string(),
        serde_json::json!({
            "sessionId": source_session_id,
            "turnId": source_turn_id,
            "turnIndex": source_turn_index + 1,
        }),
    );

    Some(JsonValue::Object(base))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{
        ModelRoundData, SessionMetadata, SessionRelationship, SessionRelationshipKind,
        TextItemData, ToolCallData, ToolItemData, UserMessageData,
    };
    use serde_json::json;

    fn metadata(session_id: &str) -> SessionMetadata {
        SessionMetadata::new(
            session_id.to_string(),
            format!("Session {session_id}"),
            "agentic".to_string(),
            "model".to_string(),
        )
    }

    fn turn(session_id: &str, turn_id: &str, turn_index: usize) -> DialogTurnData {
        let mut turn = DialogTurnData::new(
            turn_id.to_string(),
            turn_index,
            session_id.to_string(),
            UserMessageData {
                id: format!("user-{turn_id}"),
                content: format!("prompt {turn_id}"),
                timestamp: 1,
                metadata: None,
            },
        );
        turn.model_rounds.push(ModelRoundData {
            id: format!("round-{turn_id}"),
            turn_id: turn_id.to_string(),
            round_index: 0,
            round_group_id: None,
            timestamp: 1,
            text_items: vec![
                TextItemData {
                    id: "text-1".to_string(),
                    content: "answer".to_string(),
                    is_streaming: false,
                    timestamp: 1,
                    is_markdown: true,
                    order_index: None,
                    is_subagent_item: None,
                    parent_task_tool_id: None,
                    subagent_session_id: None,
                    status: None,
                    attempt_id: None,
                    attempt_index: None,
                },
                TextItemData {
                    id: "text-2".to_string(),
                    content: "details".to_string(),
                    is_streaming: false,
                    timestamp: 1,
                    is_markdown: true,
                    order_index: None,
                    is_subagent_item: None,
                    parent_task_tool_id: None,
                    subagent_session_id: None,
                    status: None,
                    attempt_id: None,
                    attempt_index: None,
                },
            ],
            tool_items: vec![ToolItemData {
                id: "tool-1".to_string(),
                tool_name: "Read".to_string(),
                tool_call: ToolCallData {
                    id: "call-1".to_string(),
                    input: json!({ "file_path": "src/lib.rs" }),
                },
                tool_result: None,
                ai_intent: None,
                start_time: 1,
                end_time: Some(2),
                duration_ms: Some(1),
                queue_wait_ms: None,
                preflight_ms: None,
                confirmation_wait_ms: None,
                execution_ms: None,
                order_index: None,
                is_subagent_item: None,
                parent_task_tool_id: None,
                subagent_session_id: None,
                subagent_dialog_turn_id: None,
                attempt_id: None,
                attempt_index: None,
                subagent_model_id: None,
                subagent_model_display_name: None,
                status: Some("completed".to_string()),
                interruption_reason: None,
            }],
            thinking_items: Vec::new(),
            start_time: 1,
            end_time: Some(2),
            duration_ms: Some(1),
            provider_id: None,
            model_id: None,
            model_alias: None,
            first_chunk_ms: None,
            first_visible_output_ms: None,
            stream_duration_ms: None,
            attempt_count: None,
            failure_category: None,
            token_details: None,
            status: "completed".to_string(),
        });
        turn.mark_completed();
        turn
    }

    #[test]
    fn apply_session_lineage_sets_relationship_and_removes_legacy_projection() {
        let mut value = metadata("child");
        value.custom_metadata = Some(json!({
            "kind": "subagent",
            "parentSessionId": "old-parent",
            "parentRequestId": "old-request",
            "parentDialogTurnId": "old-turn",
            "parentTurnIndex": 1,
            "parentToolCallId": "old-tool",
            "subagentType": "Explore",
            "preserved": "kept"
        }));

        apply_session_lineage(
            &mut value,
            SessionRelationship {
                kind: Some(SessionRelationshipKind::DeepReview),
                parent_session_id: Some("parent".to_string()),
                parent_request_id: Some("request".to_string()),
                parent_dialog_turn_id: Some("turn".to_string()),
                parent_turn_index: Some(2),
                parent_tool_call_id: None,
                subagent_type: None,
            },
        );

        assert_eq!(
            value
                .relationship
                .as_ref()
                .and_then(|value| value.kind.clone()),
            Some(SessionRelationshipKind::DeepReview)
        );
        let custom_metadata = value
            .custom_metadata
            .expect("preserved metadata should remain");
        assert_eq!(custom_metadata["preserved"], "kept");
        assert!(custom_metadata.get("kind").is_none());
        assert!(custom_metadata.get("parentSessionId").is_none());
        assert!(custom_metadata.get("subagentType").is_none());
    }

    #[test]
    fn collect_hidden_subagent_cascade_returns_post_order_matches() {
        let mut root = metadata("child-root");
        root.relationship = Some(SessionRelationship {
            kind: Some(SessionRelationshipKind::Subagent),
            parent_session_id: Some("parent".to_string()),
            parent_request_id: None,
            parent_dialog_turn_id: Some("turn-2".to_string()),
            parent_turn_index: Some(2),
            parent_tool_call_id: None,
            subagent_type: None,
        });

        let mut grandchild = metadata("grandchild");
        grandchild.custom_metadata = Some(json!({
            "kind": "subagent",
            "parentSessionId": "child-root",
            "parentDialogTurnId": "turn-3"
        }));

        let unrelated = metadata("other");
        let parent_turns = HashSet::from(["turn-2".to_string()]);

        let cascade = collect_hidden_subagent_cascade(
            vec![root, grandchild, unrelated],
            "parent",
            &parent_turns,
        );

        assert_eq!(
            cascade,
            vec!["grandchild".to_string(), "child-root".to_string()]
        );
    }

    #[test]
    fn build_branched_session_metadata_resets_child_state_and_counts_turns() {
        let mut source = metadata("source");
        source.created_by = Some("creator".to_string());
        source.session_kind = SessionKind::Subagent;
        source.status = SessionStatus::Completed;
        source.snapshot_session_id = Some("snapshot".to_string());
        source.tags = vec![
            "btw".to_string(),
            "kept".to_string(),
            "deep_review".to_string(),
        ];
        source.custom_metadata = Some(json!({
            "parentSessionId": "legacy-parent",
            "parentDialogTurnId": "legacy-turn",
            "preserved": "value"
        }));
        source.relationship = Some(SessionRelationship {
            kind: Some(SessionRelationshipKind::Subagent),
            parent_session_id: Some("parent".to_string()),
            parent_request_id: None,
            parent_dialog_turn_id: Some("turn".to_string()),
            parent_turn_index: Some(1),
            parent_tool_call_id: None,
            subagent_type: None,
        });
        source.todos = Some(json!([{ "id": "todo" }]));
        source.deep_review_run_manifest = Some(json!({ "run": "manifest" }));
        source.unread_completion = Some("completed".to_string());
        source.needs_user_attention = Some("ask_user".to_string());
        let turns = vec![turn("target", "turn-1", 0), turn("target", "turn-2", 1)];

        let branched = build_branched_session_metadata(BranchSessionMetadataFacts {
            source_metadata: &source,
            target_session_id: "target".to_string(),
            target_session_name: "Target".to_string(),
            target_agent_type: "coding".to_string(),
            source_session_id: "source",
            source_turn_id: "turn-2",
            source_turn_index: 1,
            branched_turns: &turns,
            now_ms: 42,
        });

        assert_eq!(branched.session_id, "target");
        assert_eq!(branched.session_name, "Target");
        assert_eq!(branched.agent_type, "coding");
        assert_eq!(branched.session_kind, SessionKind::Standard);
        assert_eq!(branched.created_at, 42);
        assert_eq!(branched.last_active_at, 42);
        assert_eq!(branched.turn_count, 2);
        assert_eq!(branched.message_count, 6);
        assert_eq!(branched.tool_call_count, 2);
        assert_eq!(branched.status, SessionStatus::Active);
        assert_eq!(branched.tags, vec!["kept".to_string()]);
        assert!(branched.relationship.is_none());
        assert!(branched.todos.is_none());
        assert!(branched.deep_review_run_manifest.is_none());
        assert!(branched.unread_completion.is_none());
        assert!(branched.needs_user_attention.is_none());

        let custom_metadata = branched
            .custom_metadata
            .expect("fork metadata should exist");
        assert_eq!(custom_metadata["preserved"], "value");
        assert!(custom_metadata.get("parentSessionId").is_none());
        assert_eq!(
            custom_metadata["forkOrigin"],
            json!({
                "sessionId": "source",
                "turnId": "turn-2",
                "turnIndex": 2
            })
        );
    }
}

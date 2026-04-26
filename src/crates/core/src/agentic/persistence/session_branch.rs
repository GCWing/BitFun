use super::manager::PersistenceManager;
use crate::agentic::core::{Session, SessionKind};
use crate::service::session::{DialogTurnData, SessionStatus};
use crate::util::errors::{BitFunError, BitFunResult};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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

impl PersistenceManager {
    pub async fn branch_session(
        &self,
        workspace_path: &Path,
        request: &SessionBranchRequest,
    ) -> BitFunResult<SessionBranchResult> {
        let source_session = self
            .load_session(workspace_path, &request.source_session_id)
            .await?;
        let source_metadata = self
            .load_session_metadata(workspace_path, &request.source_session_id)
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!(
                    "Source session metadata not found: {}",
                    request.source_session_id
                ))
            })?;
        let source_turns = self
            .load_session_turns(workspace_path, &request.source_session_id)
            .await?;

        if source_turns.is_empty() {
            return Err(BitFunError::Validation(
                "Source session has no persisted turns to branch".to_string(),
            ));
        }

        let source_turn_index = source_turns
            .iter()
            .position(|turn| turn.turn_id == request.source_turn_id)
            .ok_or_else(|| {
                BitFunError::NotFound(format!(
                    "Source turn not found in persisted session: {}",
                    request.source_turn_id
                ))
            })?;

        let target_session_name = source_session.session_name.clone();
        let target_agent_type = source_session.agent_type.clone();

        let mut target_session = Session::new(
            target_session_name.clone(),
            target_agent_type.clone(),
            source_session.config.clone(),
        );
        target_session.created_by = None;
        target_session.kind = SessionKind::Standard;
        target_session.snapshot_session_id = None;
        target_session.compression_state = source_session.compression_state.clone();
        let target_session_id = target_session.session_id.clone();

        self.save_session(workspace_path, &target_session).await?;

        let branch_result = async {
            let branched_turns = source_turns
                .iter()
                .take(source_turn_index + 1)
                .enumerate()
                .map(|(new_index, turn)| {
                    let mut branched_turn = turn.clone();
                    branched_turn.session_id = target_session_id.clone();
                    branched_turn.turn_index = new_index;
                    branched_turn
                })
                .collect::<Vec<_>>();

            for (new_index, source_turn) in
                source_turns.iter().take(source_turn_index + 1).enumerate()
            {
                if let Some(messages) = self
                    .load_turn_context_snapshot(
                        workspace_path,
                        &request.source_session_id,
                        source_turn.turn_index,
                    )
                    .await?
                {
                    self.save_turn_context_snapshot(
                        workspace_path,
                        &target_session_id,
                        new_index,
                        &messages,
                    )
                    .await?;
                }
            }

            for turn in &branched_turns {
                self.save_dialog_turn(workspace_path, turn).await?;
            }

            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let mut branched_metadata = source_metadata.clone();
            branched_metadata.session_id = target_session_id.clone();
            branched_metadata.session_name = target_session_name.clone();
            branched_metadata.agent_type = target_agent_type.clone();
            branched_metadata.created_by = None;
            branched_metadata.session_kind = SessionKind::Standard;
            branched_metadata.created_at = now_ms;
            branched_metadata.last_active_at = now_ms;
            branched_metadata.turn_count = branched_turns.len();
            branched_metadata.message_count =
                branched_turns.iter().map(estimate_turn_message_count).sum();
            branched_metadata.tool_call_count = branched_turns
                .iter()
                .map(DialogTurnData::count_tool_calls)
                .sum();
            branched_metadata.status = SessionStatus::Active;
            branched_metadata.snapshot_session_id = None;
            branched_metadata.tags = branched_metadata
                .tags
                .into_iter()
                .filter(|tag| tag != "btw" && tag != "review" && tag != "deep_review")
                .collect();
            branched_metadata.custom_metadata = build_branch_custom_metadata(
                source_metadata.custom_metadata.as_ref(),
                &request.source_session_id,
                &request.source_turn_id,
                source_turn_index,
            );
            branched_metadata.todos = None;
            branched_metadata.unread_completion = None;
            branched_metadata.needs_user_attention = None;

            self.save_session_metadata(workspace_path, &branched_metadata)
                .await?;

            Ok::<(), BitFunError>(())
        }
        .await;

        if let Err(error) = branch_result {
            let _ = self
                .delete_session(workspace_path, &target_session_id)
                .await;
            return Err(error);
        }

        Ok(SessionBranchResult {
            session_id: target_session_id,
            session_name: target_session_name,
            agent_type: target_agent_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{PersistenceManager, SessionBranchRequest};
    use crate::agentic::core::{Message, Session, SessionKind};
    use crate::infrastructure::PathManager;
    use crate::service::session::{DialogTurnData, UserMessageData};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use uuid::Uuid;

    struct TestWorkspace {
        path: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("bitfun-session-branch-test-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&path).expect("test workspace should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn build_turn(
        session_id: &str,
        turn_id: &str,
        turn_index: usize,
        content: &str,
    ) -> DialogTurnData {
        let mut turn = DialogTurnData::new(
            turn_id.to_string(),
            turn_index,
            session_id.to_string(),
            UserMessageData {
                id: format!("user-{}", turn_id),
                content: content.to_string(),
                timestamp: turn_index as u64,
                metadata: None,
            },
        );
        turn.mark_completed();
        turn
    }

    #[tokio::test]
    async fn branch_session_copies_turns_snapshots_and_lineage_metadata() {
        let workspace = TestWorkspace::new();
        let manager = PersistenceManager::new(Arc::new(PathManager::new().expect("path manager")))
            .expect("persistence manager");

        let mut source_session = Session::new(
            "Source Title".to_string(),
            "agentic".to_string(),
            Default::default(),
        );
        source_session.kind = SessionKind::Standard;
        manager
            .save_session(workspace.path(), &source_session)
            .await
            .expect("source session should save");

        let turn_0 = build_turn(&source_session.session_id, "turn-0", 0, "first");
        let turn_1 = build_turn(&source_session.session_id, "turn-1", 1, "second");
        manager
            .save_dialog_turn(workspace.path(), &turn_0)
            .await
            .expect("turn 0 should save");
        manager
            .save_dialog_turn(workspace.path(), &turn_1)
            .await
            .expect("turn 1 should save");

        manager
            .save_turn_context_snapshot(
                workspace.path(),
                &source_session.session_id,
                0,
                &[Message::user("snapshot-0".to_string())],
            )
            .await
            .expect("snapshot 0 should save");
        manager
            .save_turn_context_snapshot(
                workspace.path(),
                &source_session.session_id,
                1,
                &[Message::user("snapshot-1".to_string())],
            )
            .await
            .expect("snapshot 1 should save");

        let mut source_metadata = manager
            .load_session_metadata(workspace.path(), &source_session.session_id)
            .await
            .expect("metadata load should succeed")
            .expect("source metadata should exist");
        source_metadata.tags = vec!["btw".to_string(), "review".to_string(), "kept".to_string()];
        source_metadata.custom_metadata = Some(serde_json::json!({
            "parentSessionId": "legacy-parent",
            "preservedKey": "preserved-value"
        }));
        source_metadata.todos = Some(serde_json::json!([{ "id": "todo-1" }]));
        source_metadata.unread_completion = Some("completed".to_string());
        source_metadata.needs_user_attention = Some("ask_user".to_string());
        manager
            .save_session_metadata(workspace.path(), &source_metadata)
            .await
            .expect("source metadata update should save");

        let result = manager
            .branch_session(
                workspace.path(),
                &SessionBranchRequest {
                    source_session_id: source_session.session_id.clone(),
                    source_turn_id: "turn-0".to_string(),
                },
            )
            .await
            .expect("branch should succeed");

        assert_ne!(result.session_id, source_session.session_id);
        assert_eq!(result.session_name, "Source Title");
        assert_eq!(result.agent_type, "agentic");

        let branched_turns = manager
            .load_session_turns(workspace.path(), &result.session_id)
            .await
            .expect("branched turns should load");
        assert_eq!(branched_turns.len(), 1);
        assert_eq!(branched_turns[0].turn_id, "turn-0");
        assert_eq!(branched_turns[0].turn_index, 0);
        assert_eq!(branched_turns[0].session_id, result.session_id);

        let branched_snapshot = manager
            .load_turn_context_snapshot(workspace.path(), &result.session_id, 0)
            .await
            .expect("branched snapshot load should succeed")
            .expect("branched snapshot should exist");
        assert_eq!(branched_snapshot.len(), 1);
        assert!(matches!(
            &branched_snapshot[0].content,
            crate::agentic::core::MessageContent::Text(text) if text == "snapshot-0"
        ));

        let branched_metadata = manager
            .load_session_metadata(workspace.path(), &result.session_id)
            .await
            .expect("branched metadata load should succeed")
            .expect("branched metadata should exist");
        assert_eq!(branched_metadata.session_name, "Source Title");
        assert_eq!(branched_metadata.session_kind, SessionKind::Standard);
        assert_eq!(branched_metadata.tags, vec!["kept".to_string()]);
        assert!(branched_metadata.todos.is_none());
        assert!(branched_metadata.unread_completion.is_none());
        assert!(branched_metadata.needs_user_attention.is_none());

        let custom_metadata = branched_metadata
            .custom_metadata
            .expect("branch should record custom metadata");
        assert_eq!(custom_metadata["preservedKey"], "preserved-value");
        assert!(custom_metadata.get("parentSessionId").is_none());
        assert_eq!(
            custom_metadata["forkOrigin"],
            serde_json::json!({
                "sessionId": source_session.session_id,
                "turnId": "turn-0",
                "turnIndex": 1
            })
        );
    }
}

use super::manager::PersistenceManager;
use crate::agentic::core::{Session, SessionKind};
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_services_core::session::{
    build_branched_session_metadata, format_branch_session_name, resolve_branch_session_lineage,
    BranchSessionMetadataFacts,
};
pub use bitfun_services_core::session::{SessionBranchRequest, SessionBranchResult};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

impl PersistenceManager {
    pub async fn branch_session(
        &self,
        workspace_path: &Path,
        request: &SessionBranchRequest,
    ) -> BitFunResult<SessionBranchResult> {
        bitfun_core_types::validate_session_id(&request.source_session_id)
            .map_err(BitFunError::Validation)?;
        let branch_allocation_lock = self
            .get_session_branch_allocation_lock(workspace_path)
            .await;
        let _branch_allocation_guard = branch_allocation_lock.lock().await;

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
        let metadata_list = self
            .list_session_metadata_including_internal(workspace_path)
            .await?;
        let branch_lineage = resolve_branch_session_lineage(
            &source_metadata,
            &source_session.session_name,
            &metadata_list,
        );
        let source_turns = self
            .load_session_turns(workspace_path, &request.source_session_id)
            .await?;
        let source_prompt_cache = self
            .load_prompt_cache(workspace_path, &request.source_session_id)
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

        let target_session_name =
            format_branch_session_name(&branch_lineage.base_session_name, branch_lineage.ordinal);
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
                if let Some(snapshot) = self
                    .load_turn_skill_agent_snapshot(
                        workspace_path,
                        &request.source_session_id,
                        source_turn.turn_index,
                    )
                    .await?
                {
                    self.save_turn_skill_agent_snapshot(
                        workspace_path,
                        &target_session_id,
                        new_index,
                        &snapshot,
                    )
                    .await?;
                }
            }

            for turn in &branched_turns {
                self.save_dialog_turn(workspace_path, turn).await?;
            }

            self.copy_compression_transcripts_through(
                workspace_path,
                &request.source_session_id,
                &target_session_id,
                source_turn_index,
            )
            .await?;

            if let Some(cache) = source_prompt_cache.as_ref() {
                self.save_prompt_cache(workspace_path, &target_session_id, cache)
                    .await?;
            }
            if let Some(snapshot) = self
                .load_skill_agent_baseline_override_snapshot(
                    workspace_path,
                    &request.source_session_id,
                )
                .await?
            {
                self.save_skill_agent_baseline_override_snapshot(
                    workspace_path,
                    &target_session_id,
                    &snapshot,
                )
                .await?;
            }

            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let branched_metadata = build_branched_session_metadata(BranchSessionMetadataFacts {
                source_metadata: &source_metadata,
                target_session_id: target_session_id.clone(),
                target_session_name: target_session_name.clone(),
                target_agent_type: target_agent_type.clone(),
                source_session_id: &request.source_session_id,
                source_turn_id: &request.source_turn_id,
                source_turn_index,
                branched_turns: &branched_turns,
                branch_lineage: &branch_lineage,
                now_ms,
            });

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
    use crate::agentic::session::{
        CachedSystemPrompt, CachedUserContext, SessionPromptCache, SystemPromptCacheIdentity,
        UserContextCacheIdentity,
    };
    use crate::agentic::skill_agent_snapshot::{SkillSnapshotEntry, TurnSkillAgentSnapshot};
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

        fn path_manager(&self) -> Arc<PathManager> {
            Arc::new(PathManager::with_user_root_for_tests(
                self.path.join("user-root"),
            ))
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

    async fn rename_persisted_session(
        manager: &PersistenceManager,
        workspace_path: &Path,
        session_id: &str,
        session_name: &str,
    ) {
        let mut session = manager
            .load_session(workspace_path, session_id)
            .await
            .expect("session should load for rename");
        session.session_name = session_name.to_string();
        manager
            .save_session(workspace_path, &session)
            .await
            .expect("renamed session should save");

        let mut metadata = manager
            .load_session_metadata(workspace_path, session_id)
            .await
            .expect("metadata should load for rename")
            .expect("metadata should exist for rename");
        metadata.session_name = session_name.to_string();
        manager
            .save_session_metadata(workspace_path, &metadata)
            .await
            .expect("renamed metadata should save");
    }

    #[tokio::test]
    async fn branch_session_copies_turns_snapshots_and_lineage_metadata() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");

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

        let kept_transcript = manager
            .create_compression_transcript(
                workspace.path(),
                &source_session.session_id,
                0,
                "compression-kept",
                "auto",
            )
            .await
            .expect("kept transcript should create")
            .expect("kept transcript should exist");
        let omitted_transcript = manager
            .create_compression_transcript(
                workspace.path(),
                &source_session.session_id,
                1,
                "compression-omitted",
                "auto",
            )
            .await
            .expect("omitted transcript should create")
            .expect("omitted transcript should exist");

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

        let source_prompt_cache = SessionPromptCache {
            system_prompt: Some(CachedSystemPrompt::new(
                SystemPromptCacheIdentity::new("template:agentic_mode"),
                "system prompt",
            )),
            user_context: Some(CachedUserContext::new(
                UserContextCacheIdentity::new("workspace_context|workspace_instructions"),
                "user context",
            )),
        };
        manager
            .save_prompt_cache(
                workspace.path(),
                &source_session.session_id,
                &source_prompt_cache,
            )
            .await
            .expect("source prompt cache should save");

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
        source_metadata.relationship = Some(
            serde_json::from_value(serde_json::json!({
                "kind": "deep_review",
                "parentSessionId": "structured-parent",
                "parentRequestId": "structured-request",
                "parentDialogTurnId": "structured-turn",
                "parentTurnIndex": 4
            }))
            .expect("relationship should deserialize"),
        );
        source_metadata.deep_review_run_manifest = Some(serde_json::json!({
            "reviewMode": "deep",
            "coreReviewers": [{ "subagentId": "ReviewBusinessLogic" }]
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
        assert_eq!(result.session_name, "Source Title (1)");
        assert_eq!(result.agent_type, "agentic");

        let branched_turns = manager
            .load_session_turns(workspace.path(), &result.session_id)
            .await
            .expect("branched turns should load");
        assert_eq!(branched_turns.len(), 1);
        assert_eq!(branched_turns[0].turn_id, "turn-0");
        assert_eq!(branched_turns[0].turn_index, 0);
        assert_eq!(branched_turns[0].session_id, result.session_id);

        let target_transcript_dir =
            manager.compression_transcripts_dir(workspace.path(), &result.session_id);
        let kept_name = kept_transcript
            .transcript_path
            .file_name()
            .expect("kept file name")
            .to_owned();
        let kept_meta_name = kept_transcript
            .meta_path
            .file_name()
            .expect("kept metadata file name")
            .to_owned();
        assert!(target_transcript_dir.join(kept_name).exists());
        assert!(target_transcript_dir.join(kept_meta_name).exists());
        assert!(!target_transcript_dir
            .join(
                omitted_transcript
                    .transcript_path
                    .file_name()
                    .expect("omitted file name")
            )
            .exists());
        assert!(!target_transcript_dir
            .join(
                omitted_transcript
                    .meta_path
                    .file_name()
                    .expect("omitted metadata file name")
            )
            .exists());

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

        let branched_prompt_cache = manager
            .load_prompt_cache(workspace.path(), &result.session_id)
            .await
            .expect("branched prompt cache load should succeed")
            .expect("branched prompt cache should exist");
        assert_eq!(branched_prompt_cache, source_prompt_cache);

        let branched_metadata = manager
            .load_session_metadata(workspace.path(), &result.session_id)
            .await
            .expect("branched metadata load should succeed")
            .expect("branched metadata should exist");
        assert_eq!(branched_metadata.session_name, "Source Title (1)");
        assert_eq!(branched_metadata.session_kind, SessionKind::Standard);
        assert_eq!(branched_metadata.tags, vec!["kept".to_string()]);
        assert!(branched_metadata.relationship.is_none());
        assert!(branched_metadata.deep_review_run_manifest.is_none());
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
                "turnIndex": 1,
                "baseTitle": "Source Title"
            })
        );
    }

    #[tokio::test]
    async fn branch_session_advances_the_family_title_without_growing_suffixes() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
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
        manager
            .save_dialog_turn(
                workspace.path(),
                &build_turn(&source_session.session_id, "turn-0", 0, "first"),
            )
            .await
            .expect("source turn should save");

        let first_branch = manager
            .branch_session(
                workspace.path(),
                &SessionBranchRequest {
                    source_session_id: source_session.session_id.clone(),
                    source_turn_id: "turn-0".to_string(),
                },
            )
            .await
            .expect("first branch should succeed");
        assert_eq!(first_branch.session_name, "Source Title (1)");

        let nested_branch = manager
            .branch_session(
                workspace.path(),
                &SessionBranchRequest {
                    source_session_id: first_branch.session_id,
                    source_turn_id: "turn-0".to_string(),
                },
            )
            .await
            .expect("nested branch should succeed");
        assert_eq!(nested_branch.session_name, "Source Title (2)");

        let sibling_branch = manager
            .branch_session(
                workspace.path(),
                &SessionBranchRequest {
                    source_session_id: source_session.session_id.clone(),
                    source_turn_id: "turn-0".to_string(),
                },
            )
            .await
            .expect("sibling branch should succeed");
        assert_eq!(sibling_branch.session_name, "Source Title (3)");

        let nested_metadata = manager
            .load_session_metadata(workspace.path(), &nested_branch.session_id)
            .await
            .expect("nested metadata should load")
            .expect("nested metadata should exist");
        assert_eq!(
            nested_metadata.custom_metadata.and_then(|metadata| {
                metadata
                    .get("forkOrigin")
                    .and_then(|origin| origin.get("baseTitle"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            }),
            Some("Source Title".to_string())
        );
    }

    #[tokio::test]
    async fn branch_session_respects_inherited_and_unrelated_renamed_suffixes() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
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
        manager
            .save_dialog_turn(
                workspace.path(),
                &build_turn(&source_session.session_id, "turn-0", 0, "first"),
            )
            .await
            .expect("source turn should save");

        let first_branch = manager
            .branch_session(
                workspace.path(),
                &SessionBranchRequest {
                    source_session_id: source_session.session_id.clone(),
                    source_turn_id: "turn-0".to_string(),
                },
            )
            .await
            .expect("first branch should succeed");
        assert_eq!(first_branch.session_name, "Source Title (1)");

        rename_persisted_session(
            &manager,
            workspace.path(),
            &first_branch.session_id,
            "Source Title (2)",
        )
        .await;
        let inherited_suffix_branch = manager
            .branch_session(
                workspace.path(),
                &SessionBranchRequest {
                    source_session_id: first_branch.session_id.clone(),
                    source_turn_id: "turn-0".to_string(),
                },
            )
            .await
            .expect("inherited suffix branch should succeed");
        assert_eq!(inherited_suffix_branch.session_name, "Source Title (3)");

        rename_persisted_session(
            &manager,
            workspace.path(),
            &first_branch.session_id,
            "Another title (2)",
        )
        .await;
        let unrelated_suffix_branch = manager
            .branch_session(
                workspace.path(),
                &SessionBranchRequest {
                    source_session_id: first_branch.session_id,
                    source_turn_id: "turn-0".to_string(),
                },
            )
            .await
            .expect("unrelated suffix branch should succeed");
        assert_eq!(
            unrelated_suffix_branch.session_name,
            "Another title (2) (1)"
        );
    }

    #[tokio::test]
    async fn concurrent_branches_allocate_distinct_workspace_title_ordinals() {
        let workspace = TestWorkspace::new();
        let manager = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager"),
        );
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
        manager
            .save_dialog_turn(
                workspace.path(),
                &build_turn(&source_session.session_id, "turn-0", 0, "first"),
            )
            .await
            .expect("source turn should save");

        let mut second_source_session = Session::new(
            "Source Title".to_string(),
            "agentic".to_string(),
            Default::default(),
        );
        second_source_session.kind = SessionKind::Standard;
        manager
            .save_session(workspace.path(), &second_source_session)
            .await
            .expect("second source session should save");
        manager
            .save_dialog_turn(
                workspace.path(),
                &build_turn(&second_source_session.session_id, "turn-0", 0, "first"),
            )
            .await
            .expect("second source turn should save");

        let source_session_id = source_session.session_id.clone();
        let first_manager = Arc::clone(&manager);
        let second_manager = Arc::clone(&manager);
        let first_request = SessionBranchRequest {
            source_session_id: source_session_id.clone(),
            source_turn_id: "turn-0".to_string(),
        };
        let second_request = SessionBranchRequest {
            source_session_id: second_source_session.session_id,
            source_turn_id: "turn-0".to_string(),
        };
        let (first_result, second_result) = tokio::join!(
            first_manager.branch_session(workspace.path(), &first_request),
            second_manager.branch_session(workspace.path(), &second_request),
        );

        let mut session_names = vec![
            first_result
                .expect("first concurrent branch should succeed")
                .session_name,
            second_result
                .expect("second concurrent branch should succeed")
                .session_name,
        ];
        session_names.sort();
        assert_eq!(
            session_names,
            vec![
                "Source Title (1)".to_string(),
                "Source Title (2)".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn branch_session_copies_skill_agent_baseline_override_snapshot() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");

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
        manager
            .save_dialog_turn(workspace.path(), &turn_0)
            .await
            .expect("turn 0 should save");

        let baseline_override = TurnSkillAgentSnapshot {
            skills: vec![SkillSnapshotEntry {
                name: "interactive-debug".to_string(),
                description: "debug helper".to_string(),
                location: "/skills/interactive-debug".to_string(),
            }],
            subagents: Vec::new(),
        };
        manager
            .save_skill_agent_baseline_override_snapshot(
                workspace.path(),
                &source_session.session_id,
                &baseline_override,
            )
            .await
            .expect("baseline override should save");

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

        let branched_override = manager
            .load_skill_agent_baseline_override_snapshot(workspace.path(), &result.session_id)
            .await
            .expect("branched override should load")
            .expect("branched override should exist");
        assert_eq!(branched_override, baseline_override);
    }
}

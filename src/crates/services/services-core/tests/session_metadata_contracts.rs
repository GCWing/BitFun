use bitfun_services_core::session::{
    build_session_index_snapshot, refresh_session_metadata_from_turns, remove_session_index_entry,
    try_refresh_session_metadata_for_saved_turn, upsert_session_index_entry, DialogTurnData,
    DialogTurnKind, ModelRoundData, SessionKind, SessionMetadata, StoredSessionIndexFile,
    TextItemData, ToolCallData, ToolItemData, TurnStatus, UserMessageData,
};

fn metadata(session_id: &str) -> SessionMetadata {
    SessionMetadata::new(
        session_id.to_string(),
        session_id.to_string(),
        "agent".to_string(),
        "model".to_string(),
    )
}

fn user_message(content: &str) -> UserMessageData {
    UserMessageData {
        id: format!("user-{content}"),
        content: content.to_string(),
        timestamp: 0,
        metadata: None,
    }
}

fn text_item(id: &str) -> TextItemData {
    TextItemData {
        id: id.to_string(),
        content: id.to_string(),
        is_streaming: false,
        timestamp: 0,
        is_markdown: true,
        order_index: None,
        is_subagent_item: None,
        parent_task_tool_id: None,
        subagent_session_id: None,
        status: None,
        attempt_id: None,
        attempt_index: None,
    }
}

fn tool_item(id: &str) -> ToolItemData {
    ToolItemData {
        id: id.to_string(),
        tool_name: "Read".to_string(),
        tool_call: ToolCallData {
            input: serde_json::json!({ "path": "README.md" }),
            id: format!("call-{id}"),
        },
        tool_result: None,
        ai_intent: None,
        start_time: 0,
        end_time: Some(1),
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
    }
}

fn round(turn_id: &str, text_count: usize, tool_count: usize) -> ModelRoundData {
    ModelRoundData {
        id: format!("round-{turn_id}"),
        turn_id: turn_id.to_string(),
        round_index: 0,
        round_group_id: None,
        timestamp: 0,
        text_items: (0..text_count)
            .map(|index| text_item(&format!("{turn_id}-text-{index}")))
            .collect(),
        tool_items: (0..tool_count)
            .map(|index| tool_item(&format!("{turn_id}-tool-{index}")))
            .collect(),
        thinking_items: Vec::new(),
        start_time: 0,
        end_time: Some(1),
        duration_ms: Some(1),
        provider_id: None,
        model_config_id: None,
        effective_model_name: None,
        first_chunk_ms: None,
        first_visible_output_ms: None,
        stream_duration_ms: None,
        attempt_count: None,
        attempt_diagnostics: vec![],
        failure_category: None,
        token_details: None,
        status: "completed".to_string(),
    }
}

fn turn(
    session_id: &str,
    turn_index: usize,
    text_count: usize,
    tool_count: usize,
) -> DialogTurnData {
    let turn_id = format!("turn-{turn_index}");
    let mut turn = DialogTurnData::new(
        turn_id.clone(),
        turn_index,
        session_id.to_string(),
        user_message(&format!("prompt-{turn_index}")),
    );
    turn.model_rounds
        .push(round(&turn_id, text_count, tool_count));
    turn
}

fn finished_turn(
    session_id: &str,
    turn_index: usize,
    status: TurnStatus,
    end_time: u64,
) -> DialogTurnData {
    let mut turn = turn(session_id, turn_index, 1, 0);
    turn.status = status;
    turn.end_time = Some(end_time);
    turn.duration_ms = Some(end_time.saturating_sub(turn.start_time));
    turn
}

#[test]
fn deferred_tool_item_serializes_only_its_wire_invocation() {
    let mut item = tool_item("deferred");
    item.tool_name = "CallDeferredTool".to_string();
    item.tool_call.input = serde_json::json!({
        "tool_name": "WebFetch",
        "args": { "url": "https://example.test" }
    });

    let value = serde_json::to_value(&item).expect("serialize tool item");
    assert_eq!(value["toolName"], "CallDeferredTool");
    assert_eq!(value["toolCall"]["input"], item.tool_call.input);
    assert!(value.get("effectiveToolName").is_none());
    assert!(value.get("effectiveToolInput").is_none());
}

#[test]
fn full_refresh_recomputes_metadata_counters_from_turns() {
    let mut metadata = metadata("session-1");

    refresh_session_metadata_from_turns(
        &mut metadata,
        "D:/workspace/project",
        &[turn("session-1", 0, 2, 1), turn("session-1", 1, 1, 2)],
        42,
    );

    assert_eq!(metadata.turn_count, 2);
    assert_eq!(metadata.message_count, 5);
    assert_eq!(metadata.tool_call_count, 3);
    assert_eq!(metadata.last_active_at, 42);
    assert_eq!(
        metadata.workspace_path.as_deref(),
        Some("D:/workspace/project")
    );
}

#[test]
fn saved_turn_refresh_updates_incrementally_for_append_and_replace() {
    let mut metadata = metadata("session-1");
    metadata.turn_count = 1;
    metadata.message_count = 2;
    metadata.tool_call_count = 1;

    assert!(try_refresh_session_metadata_for_saved_turn(
        &mut metadata,
        "D:/workspace/project",
        None,
        &turn("session-1", 1, 2, 2),
        50,
    ));
    assert_eq!(metadata.turn_count, 2);
    assert_eq!(metadata.message_count, 5);
    assert_eq!(metadata.tool_call_count, 3);

    assert!(try_refresh_session_metadata_for_saved_turn(
        &mut metadata,
        "D:/workspace/project",
        Some(&turn("session-1", 1, 2, 2)),
        &turn("session-1", 1, 1, 1),
        60,
    ));
    assert_eq!(metadata.turn_count, 2);
    assert_eq!(metadata.message_count, 4);
    assert_eq!(metadata.tool_call_count, 2);
    assert_eq!(metadata.last_active_at, 60);
}

#[test]
fn saved_turn_refresh_sets_last_finished_at_for_completed_user_dialog() {
    let mut metadata = metadata("session-1");
    let turn = finished_turn("session-1", 0, TurnStatus::Completed, 123);

    assert!(try_refresh_session_metadata_for_saved_turn(
        &mut metadata,
        "D:/workspace/project",
        None,
        &turn,
        200,
    ));

    assert_eq!(metadata.last_finished_at, Some(123));
}

#[test]
fn saved_turn_refresh_ignores_in_progress_turn_for_last_finished_at() {
    let mut metadata = metadata("session-1");
    let turn = turn("session-1", 0, 1, 0);

    assert!(try_refresh_session_metadata_for_saved_turn(
        &mut metadata,
        "D:/workspace/project",
        None,
        &turn,
        200,
    ));

    assert_eq!(metadata.last_finished_at, None);
}

#[test]
fn saved_turn_refresh_keeps_newer_last_finished_at() {
    let mut metadata = metadata("session-1");
    metadata.turn_count = 1;
    metadata.last_finished_at = Some(500);
    let turn = finished_turn("session-1", 1, TurnStatus::Completed, 300);

    assert!(try_refresh_session_metadata_for_saved_turn(
        &mut metadata,
        "D:/workspace/project",
        None,
        &turn,
        600,
    ));

    assert_eq!(metadata.last_finished_at, Some(500));
}

#[test]
fn full_refresh_uses_latest_terminal_user_dialog_finish_time() {
    let mut metadata = metadata("session-1");
    metadata.last_finished_at = Some(1);

    refresh_session_metadata_from_turns(
        &mut metadata,
        "D:/workspace/project",
        &[
            finished_turn("session-1", 0, TurnStatus::Completed, 100),
            finished_turn("session-1", 1, TurnStatus::Error, 300),
            finished_turn("session-1", 2, TurnStatus::Cancelled, 200),
        ],
        400,
    );

    assert_eq!(metadata.last_finished_at, Some(300));
}

#[test]
fn full_refresh_ignores_non_user_dialog_turns_for_last_finished_at() {
    let mut metadata = metadata("session-1");

    let mut local_command = finished_turn("session-1", 0, TurnStatus::Completed, 100);
    local_command.kind = DialogTurnKind::LocalCommand;
    let mut manual_compaction = finished_turn("session-1", 1, TurnStatus::Completed, 200);
    manual_compaction.kind = DialogTurnKind::ManualCompaction;

    refresh_session_metadata_from_turns(
        &mut metadata,
        "D:/workspace/project",
        &[local_command, manual_compaction],
        300,
    );

    assert_eq!(metadata.last_finished_at, None);
}

#[test]
fn full_refresh_clears_last_finished_at_without_terminal_user_dialog() {
    let mut metadata = metadata("session-1");
    metadata.last_finished_at = Some(500);

    refresh_session_metadata_from_turns(
        &mut metadata,
        "D:/workspace/project",
        &[turn("session-1", 0, 1, 0)],
        600,
    );

    assert_eq!(metadata.last_finished_at, None);
}

#[test]
fn saved_turn_refresh_rejects_gaps_and_session_mismatches() {
    let mut metadata = metadata("session-1");
    metadata.turn_count = 1;

    assert!(!try_refresh_session_metadata_for_saved_turn(
        &mut metadata,
        "D:/workspace/project",
        None,
        &turn("session-1", 2, 1, 0),
        50,
    ));
    assert!(!try_refresh_session_metadata_for_saved_turn(
        &mut metadata,
        "D:/workspace/project",
        Some(&turn("other-session", 0, 1, 0)),
        &turn("session-1", 0, 1, 0),
        50,
    ));
}

#[test]
fn index_snapshot_keeps_visible_sessions_but_counts_all_metadata_files() {
    let mut first_tied = metadata("first-tied");
    first_tied.last_active_at = 1_000;
    let mut second_tied = metadata("second-tied");
    second_tied.last_active_at = 1_000;
    let mut older = metadata("older");
    older.last_active_at = 500;
    let mut internal = metadata("internal");
    internal.session_kind = SessionKind::Subagent;

    let (index, visible_sessions) =
        build_session_index_snapshot(vec![first_tied, older, internal, second_tied], 99);

    assert_eq!(index.updated_at, 99);
    assert_eq!(index.metadata_file_count, 4);
    assert_eq!(index.sessions.len(), 3);
    assert_eq!(
        visible_sessions
            .iter()
            .map(|metadata| metadata.session_id.as_str())
            .collect::<Vec<_>>(),
        vec!["first-tied", "second-tied", "older"]
    );
}

#[test]
fn index_entry_upsert_and_remove_preserve_sorting_and_counts() {
    let mut older = metadata("older");
    older.last_active_at = 10;
    let existing = StoredSessionIndexFile::with_metadata_file_count(1, vec![older], 1);

    let mut newer = metadata("newer");
    newer.last_active_at = 20;
    let index = upsert_session_index_entry(Some(existing), &newer, true, 99, 2);

    assert_eq!(index.metadata_file_count, 2);
    assert_eq!(
        index
            .sessions
            .iter()
            .map(|metadata| metadata.session_id.as_str())
            .collect::<Vec<_>>(),
        vec!["newer", "older"]
    );

    let index = remove_session_index_entry(Some(index), "newer", -1, 100)
        .expect("existing index should remain");
    assert_eq!(index.metadata_file_count, 1);
    assert_eq!(index.updated_at, 100);
    assert_eq!(index.sessions[0].session_id, "older");
}

#![cfg(feature = "remote-connect")]

use bitfun_events::{AgenticEvent, ToolEventData};
use bitfun_runtime_ports::AgentSubmissionSource;
use bitfun_services_integrations::remote_connect::{
    build_remote_image_attachment, build_remote_image_contexts,
    build_remote_image_submission_request, build_remote_session_create_request,
    build_remote_submission_request, make_slim_tool_params, remote_file_display_name,
    remote_model_catalog_poll_delta, remote_no_change_poll_response,
    remote_persisted_poll_response, remote_session_restore_target, remote_snapshot_poll_response,
    resolve_remote_agent_type, resolve_remote_cancel_decision,
    resolve_remote_execution_image_contexts, resolve_remote_file_chunk_range,
    should_send_remote_model_catalog, ActiveTurnSnapshot, ChatImageAttachment, ChatMessage,
    ChatMessageItem, ImageAttachment, RemoteCancelDecision, RemoteCommand,
    RemoteConnectSubmissionSource, RemoteDefaultModelsConfig, RemoteImageContext,
    RemoteModelCatalog, RemoteModelConfig, RemoteResponse, RemoteSessionStateTracker,
    RemoteToolStatus, TrackerEvent, REMOTE_FILE_MAX_CHUNK_BYTES, REMOTE_FILE_MAX_READ_BYTES,
};

#[test]
fn remote_connect_submission_contract_preserves_relay_source_and_turn_id() {
    let request = build_remote_submission_request(
        "session-1",
        "hello from phone",
        Some("turn-1".to_string()),
        RemoteConnectSubmissionSource::Relay,
    );

    assert_eq!(request.session_id, "session-1");
    assert_eq!(request.message, "hello from phone");
    assert_eq!(request.turn_id.as_deref(), Some("turn-1"));
    assert_eq!(request.source, Some(AgentSubmissionSource::RemoteRelay));
    assert!(request.attachments.is_empty());
}

#[test]
fn remote_connect_submission_contract_preserves_bot_source() {
    let request = build_remote_submission_request(
        "session-2",
        "hello from bot",
        None,
        RemoteConnectSubmissionSource::Bot,
    );

    assert_eq!(request.source, Some(AgentSubmissionSource::Bot));
    assert!(request.turn_id.is_none());
}

#[test]
fn remote_connect_image_attachment_contract_preserves_portable_metadata() {
    let image = ImageAttachment {
        name: "clip.png".to_string(),
        data_url: "data:image/png;base64,abc".to_string(),
    };

    let attachment = build_remote_image_attachment(1, &image);
    let json = serde_json::to_value(attachment).expect("serialize image attachment");

    assert_eq!(json["kind"], "remote_image");
    assert_eq!(json["id"], "remote-image-2");
    assert_eq!(json["metadata"]["name"], "clip.png");
    assert_eq!(json["metadata"]["dataUrl"], "data:image/png;base64,abc");
}

#[test]
fn remote_connect_image_submission_request_preserves_existing_source_and_turn_shape() {
    let image = ImageAttachment {
        name: "clip.png".to_string(),
        data_url: "data:image/png;base64,abc".to_string(),
    };

    let request = build_remote_image_submission_request(
        "session-3",
        "hello with image",
        Some("turn-3".to_string()),
        RemoteConnectSubmissionSource::Relay,
        &[image],
    );

    assert_eq!(request.session_id, "session-3");
    assert_eq!(request.message, "hello with image");
    assert_eq!(request.turn_id.as_deref(), Some("turn-3"));
    assert_eq!(request.source, Some(AgentSubmissionSource::RemoteRelay));
    assert_eq!(request.attachments.len(), 1);
    assert_eq!(request.attachments[0].kind, "remote_image");
    assert_eq!(request.attachments[0].id, "remote-image-1");
    assert_eq!(
        request.attachments[0].metadata["dataUrl"],
        "data:image/png;base64,abc"
    );
}

#[test]
fn remote_connect_image_context_policy_preserves_legacy_fallback_shape() {
    let images = vec![
        ImageAttachment {
            name: "clip.png".to_string(),
            data_url: "data:image/png;base64,abc".to_string(),
        },
        ImageAttachment {
            name: "raw".to_string(),
            data_url: "not-a-data-url".to_string(),
        },
    ];

    let contexts = build_remote_image_contexts(Some(&images));

    assert_eq!(contexts.len(), 2);
    assert!(contexts[0].id.starts_with("remote_img_"));
    assert_eq!(contexts[0].image_path, None);
    assert_eq!(
        contexts[0].data_url.as_deref(),
        Some("data:image/png;base64,abc")
    );
    assert_eq!(contexts[0].mime_type, "image/png");
    assert_eq!(contexts[0].metadata.as_ref().unwrap()["name"], "clip.png");
    assert_eq!(contexts[0].metadata.as_ref().unwrap()["source"], "remote");
    assert_eq!(contexts[1].mime_type, "image/png");
}

#[test]
fn remote_connect_image_context_policy_prefers_explicit_contexts() {
    let legacy_images = vec![ImageAttachment {
        name: "legacy.png".to_string(),
        data_url: "data:image/png;base64,legacy".to_string(),
    }];
    let explicit = RemoteImageContext {
        id: "ctx-1".to_string(),
        image_path: Some("D:/workspace/project/screenshot.png".to_string()),
        data_url: None,
        mime_type: "image/png".to_string(),
        metadata: Some(serde_json::json!({ "source": "desktop" })),
    };

    let contexts = resolve_remote_execution_image_contexts(
        Some(&legacy_images),
        Some(vec![explicit.clone()]),
        build_remote_image_contexts,
    );

    assert_eq!(contexts, vec![explicit]);
}

#[test]
fn remote_connect_cancel_and_restore_policy_preserve_runtime_decisions() {
    assert_eq!(
        remote_session_restore_target(false, Some("D:/workspace/project")),
        Some("D:/workspace/project")
    );
    assert_eq!(
        remote_session_restore_target(true, Some("D:/workspace/project")),
        None
    );
    assert_eq!(remote_session_restore_target(false, None), None);

    assert_eq!(
        resolve_remote_cancel_decision(Some("turn-current"), Some("turn-current")),
        RemoteCancelDecision::CancelCurrent("turn-current".to_string())
    );
    assert_eq!(
        resolve_remote_cancel_decision(Some("turn-current"), None),
        RemoteCancelDecision::CancelCurrent("turn-current".to_string())
    );
    assert_eq!(
        resolve_remote_cancel_decision(Some("turn-current"), Some("turn-stale")),
        RemoteCancelDecision::StaleRequestedTurn
    );
    assert_eq!(
        resolve_remote_cancel_decision(None, Some("turn-finished")),
        RemoteCancelDecision::AlreadyFinished
    );
    assert_eq!(
        resolve_remote_cancel_decision(None, None),
        RemoteCancelDecision::NoRunningTask
    );
}

#[test]
fn remote_connect_file_transfer_policy_preserves_limits_and_chunk_ranges() {
    assert_eq!(REMOTE_FILE_MAX_READ_BYTES, 30 * 1024 * 1024);
    assert_eq!(REMOTE_FILE_MAX_CHUNK_BYTES, 3 * 1024 * 1024);
    assert_eq!(REMOTE_FILE_MAX_CHUNK_BYTES % 3, 0);

    let range = resolve_remote_file_chunk_range(10_000_000, 5, REMOTE_FILE_MAX_CHUNK_BYTES + 99);
    assert_eq!(range.start, 5);
    assert_eq!(range.end, 5 + REMOTE_FILE_MAX_CHUNK_BYTES as usize);
    assert_eq!(range.chunk_size, REMOTE_FILE_MAX_CHUNK_BYTES);

    let tail = resolve_remote_file_chunk_range(100, 95, 30);
    assert_eq!(tail.start, 95);
    assert_eq!(tail.end, 100);
    assert_eq!(tail.chunk_size, 5);

    let past_end = resolve_remote_file_chunk_range(100, 150, 30);
    assert_eq!(past_end.start, 100);
    assert_eq!(past_end.end, 100);
    assert_eq!(past_end.chunk_size, 0);
}

#[test]
fn remote_connect_file_transfer_policy_preserves_name_fallback() {
    assert_eq!(remote_file_display_name(Some("report.md")), "report.md");
    assert_eq!(remote_file_display_name(None), "file");
    assert_eq!(remote_file_display_name(Some("")), "file");
}

#[test]
fn remote_connect_session_create_contract_preserves_workspace_binding() {
    let request = build_remote_session_create_request(
        "Remote Session",
        "agentic",
        Some("D:/workspace/project"),
        RemoteConnectSubmissionSource::Relay,
    );

    assert_eq!(request.session_name, "Remote Session");
    assert_eq!(request.agent_type, "agentic");
    assert_eq!(
        request.workspace_path.as_deref(),
        Some("D:/workspace/project")
    );
    assert_eq!(request.metadata["source"], "remote_relay");
}

#[test]
fn remote_connect_agent_type_mapping_preserves_current_mobile_aliases() {
    assert_eq!(resolve_remote_agent_type(Some("code")), "agentic");
    assert_eq!(resolve_remote_agent_type(Some("agentic")), "agentic");
    assert_eq!(resolve_remote_agent_type(Some("Agentic")), "agentic");
    assert_eq!(resolve_remote_agent_type(Some("cowork")), "Cowork");
    assert_eq!(resolve_remote_agent_type(Some("Cowork")), "Cowork");
    assert_eq!(resolve_remote_agent_type(Some("plan")), "Plan");
    assert_eq!(resolve_remote_agent_type(Some("Plan")), "Plan");
    assert_eq!(resolve_remote_agent_type(Some("debug")), "debug");
    assert_eq!(resolve_remote_agent_type(Some("Debug")), "debug");
    assert_eq!(resolve_remote_agent_type(Some("unknown")), "agentic");
    assert_eq!(resolve_remote_agent_type(None), "agentic");
}

#[test]
fn remote_connect_message_dtos_keep_current_wire_shape() {
    let image = ImageAttachment {
        name: "clip.png".to_string(),
        data_url: "data:image/png;base64,abc".to_string(),
    };
    let chat = ChatMessage {
        id: "msg-1".to_string(),
        role: "assistant".to_string(),
        content: "done".to_string(),
        timestamp: "1".to_string(),
        metadata: None,
        tools: Some(vec![RemoteToolStatus {
            id: "tool-1".to_string(),
            name: "bash".to_string(),
            status: "running".to_string(),
            duration_ms: None,
            start_ms: Some(42),
            input_preview: Some("{\"cmd\":\"git status\"}".to_string()),
            tool_input: None,
        }]),
        thinking: None,
        items: Some(vec![ChatMessageItem {
            item_type: "tool".to_string(),
            content: None,
            tool: None,
            is_subagent: Some(false),
        }]),
        images: Some(vec![ChatImageAttachment {
            name: image.name.clone(),
            data_url: image.data_url.clone(),
        }]),
    };

    let json = serde_json::to_value(chat).expect("serialize chat message");

    assert_eq!(json["id"], "msg-1");
    assert_eq!(json["tools"][0]["start_ms"], 42);
    assert_eq!(json["items"][0]["type"], "tool");
    assert_eq!(json["images"][0]["data_url"], "data:image/png;base64,abc");
}

#[test]
fn remote_connect_command_wire_shape_lives_in_owner_contract() {
    let command = RemoteCommand::SendMessage {
        session_id: "session-1".to_string(),
        content: "hello".to_string(),
        agent_type: Some("code".to_string()),
        images: Some(vec![ImageAttachment {
            name: "clip.png".to_string(),
            data_url: "data:image/png;base64,abc".to_string(),
        }]),
        image_contexts: Some(vec![RemoteImageContext {
            id: "ctx-1".to_string(),
            image_path: Some("D:/workspace/project/screenshot.png".to_string()),
            data_url: None,
            mime_type: "image/png".to_string(),
            metadata: Some(serde_json::json!({ "source": "remote" })),
        }]),
    };
    let json = serde_json::to_value(command).expect("serialize send command");

    assert_eq!(json["cmd"], "send_message");
    assert_eq!(json["session_id"], "session-1");
    assert_eq!(json["agent_type"], "code");
    assert_eq!(json["images"][0]["name"], "clip.png");
    assert_eq!(json["image_contexts"][0]["id"], "ctx-1");
    assert_eq!(
        json["image_contexts"][0]["image_path"],
        "D:/workspace/project/screenshot.png"
    );
    assert!(json.get("imageContexts").is_none());

    let cancel = serde_json::to_value(RemoteCommand::CancelTask {
        session_id: "session-1".to_string(),
        turn_id: Some("turn-1".to_string()),
    })
    .expect("serialize cancel command");
    assert_eq!(cancel["cmd"], "cancel_task");
    assert_eq!(cancel["turn_id"], "turn-1");

    let poll = serde_json::to_value(RemoteCommand::PollSession {
        session_id: "session-1".to_string(),
        since_version: 7,
        known_msg_count: 3,
        known_model_catalog_version: Some(11),
    })
    .expect("serialize poll command");
    assert_eq!(poll["cmd"], "poll_session");
    assert_eq!(poll["since_version"], 7);
    assert_eq!(poll["known_msg_count"], 3);
    assert_eq!(poll["known_model_catalog_version"], 11);
}

#[test]
fn remote_connect_response_wire_shape_lives_in_owner_contract() {
    let active_turn = ActiveTurnSnapshot {
        turn_id: "turn-1".to_string(),
        status: "active".to_string(),
        text: String::new(),
        thinking: String::new(),
        tools: vec![RemoteToolStatus {
            id: "tool-1".to_string(),
            name: "Read".to_string(),
            status: "running".to_string(),
            duration_ms: None,
            start_ms: Some(42),
            input_preview: Some("{\"path\":\"README.md\"}".to_string()),
            tool_input: None,
        }],
        round_index: 2,
        items: Some(vec![ChatMessageItem {
            item_type: "tool".to_string(),
            content: None,
            tool: None,
            is_subagent: None,
        }]),
    };

    let poll = serde_json::to_value(RemoteResponse::SessionPoll {
        version: 8,
        changed: true,
        session_state: Some("running".to_string()),
        title: Some("session title".to_string()),
        new_messages: None,
        total_msg_count: None,
        active_turn: Some(active_turn),
        model_catalog: Box::new(Some(sample_remote_model_catalog(11))),
    })
    .expect("serialize poll response");

    assert_eq!(poll["resp"], "session_poll");
    assert_eq!(poll["version"], 8);
    assert_eq!(poll["active_turn"]["turn_id"], "turn-1");
    assert_eq!(
        poll["active_turn"]["tools"][0]["input_preview"],
        "{\"path\":\"README.md\"}"
    );
    assert_eq!(poll["model_catalog"]["version"], 11);
    assert_eq!(
        poll["model_catalog"]["default_models"]["primary"],
        "model-1"
    );
    assert!(poll.get("new_messages").is_none());

    let sent = serde_json::to_value(RemoteResponse::MessageSent {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
    })
    .expect("serialize sent response");
    assert_eq!(sent["resp"], "message_sent");
    assert_eq!(sent["turn_id"], "turn-1");
}

fn sample_remote_model_catalog(version: u64) -> RemoteModelCatalog {
    RemoteModelCatalog {
        version,
        models: vec![RemoteModelConfig {
            id: "model-1".to_string(),
            name: "Model One".to_string(),
            provider: "openai".to_string(),
            base_url: "https://api.example.com".to_string(),
            model_name: "gpt-test".to_string(),
            context_window: Some(128_000),
            enabled: true,
            capabilities: vec!["text_chat".to_string()],
            enable_thinking_process: false,
            reasoning_mode: Some("default".to_string()),
            reasoning_effort: None,
            thinking_budget_tokens: None,
        }],
        default_models: RemoteDefaultModelsConfig {
            primary: Some("model-1".to_string()),
            ..RemoteDefaultModelsConfig::default()
        },
        session_model_id: Some("model-1".to_string()),
    }
}

#[test]
fn remote_connect_tracker_preserves_streaming_snapshot_contract() {
    let tracker = RemoteSessionStateTracker::new("session-1".to_string());

    tracker.handle_agentic_event(&AgenticEvent::DialogTurnStarted {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        turn_index: 0,
        user_input: "hello".to_string(),
        original_user_input: None,
        user_message_metadata: None,
        subagent_parent_info: None,
    });
    tracker.handle_agentic_event(&AgenticEvent::ModelRoundStarted {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        round_id: "round-1".to_string(),
        round_index: 3,
        subagent_parent_info: None,
        model_id: None,
    });
    tracker.handle_agentic_event(&AgenticEvent::ThinkingChunk {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        round_id: "round-1".to_string(),
        content: "<thinking>plan".to_string(),
        is_end: false,
        subagent_parent_info: None,
    });
    tracker.handle_agentic_event(&AgenticEvent::TextChunk {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        round_id: "round-1".to_string(),
        text: "answer".to_string(),
        subagent_parent_info: None,
    });

    let snapshot = tracker
        .snapshot_active_turn()
        .expect("active turn snapshot");

    assert_eq!(tracker.session_state(), "running");
    assert_eq!(snapshot.turn_id, "turn-1");
    assert_eq!(snapshot.status, "active");
    assert_eq!(snapshot.round_index, 3);
    assert_eq!(snapshot.text, "");
    assert_eq!(snapshot.thinking, "");
    let items = snapshot.items.expect("ordered streaming items");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].item_type, "thinking");
    assert_eq!(items[0].content.as_deref(), Some("plan"));
    assert_eq!(items[1].item_type, "text");
    assert_eq!(items[1].content.as_deref(), Some("answer"));
}

#[test]
fn remote_connect_tracker_keeps_subagent_items_out_of_parent_accumulators() {
    let tracker = RemoteSessionStateTracker::new("parent-session".to_string());
    let subagent_parent_info = Some(bitfun_events::SubagentParentInfo {
        tool_call_id: "task-1".to_string(),
        session_id: "parent-session".to_string(),
        dialog_turn_id: "parent-turn".to_string(),
    });

    tracker.initialize_active_turn("parent-turn".to_string());
    tracker.handle_agentic_event(&AgenticEvent::TextChunk {
        session_id: "child-session".to_string(),
        turn_id: "child-turn".to_string(),
        round_id: "round-1".to_string(),
        text: "child text".to_string(),
        subagent_parent_info,
    });

    assert_eq!(tracker.accumulated_text(), "");
    let snapshot = tracker
        .snapshot_active_turn()
        .expect("active turn snapshot");
    let items = snapshot.items.expect("subagent item");
    assert_eq!(items[0].content.as_deref(), Some("child text"));
    assert_eq!(items[0].is_subagent, Some(true));
}

#[tokio::test]
async fn remote_connect_tracker_broadcasts_tool_and_turn_events() {
    let tracker = RemoteSessionStateTracker::new("session-1".to_string());
    let mut events = tracker.subscribe();

    tracker.handle_agentic_event(&AgenticEvent::DialogTurnStarted {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        turn_index: 0,
        user_input: "hello".to_string(),
        original_user_input: None,
        user_message_metadata: None,
        subagent_parent_info: None,
    });
    tracker.handle_agentic_event(&AgenticEvent::ToolEvent {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        tool_event: ToolEventData::Started {
            tool_id: "tool-1".to_string(),
            tool_name: "AskUserQuestion".to_string(),
            params: serde_json::json!({ "questions": [] }),
            timeout_seconds: None,
        },
        subagent_parent_info: None,
    });
    tracker.handle_agentic_event(&AgenticEvent::DialogTurnCancelled {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        subagent_parent_info: None,
    });

    match events.recv().await.expect("tool started event") {
        TrackerEvent::ToolStarted {
            tool_id,
            tool_name,
            params,
        } => {
            assert_eq!(tool_id, "tool-1");
            assert_eq!(tool_name, "AskUserQuestion");
            assert!(params.is_some());
        }
        other => panic!("unexpected event: {other:?}"),
    }
    match events.recv().await.expect("turn cancelled event") {
        TrackerEvent::TurnCancelled { turn_id } => assert_eq!(turn_id, "turn-1"),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn remote_connect_tracker_keeps_finished_turn_snapshot_until_persistence_finalizes() {
    let tracker = RemoteSessionStateTracker::new("session-1".to_string());

    tracker.handle_agentic_event(&AgenticEvent::DialogTurnStarted {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        turn_index: 0,
        user_input: "hello".to_string(),
        original_user_input: None,
        user_message_metadata: None,
        subagent_parent_info: None,
    });
    tracker.handle_agentic_event(&AgenticEvent::TextChunk {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        round_id: "round-1".to_string(),
        text: "answer".to_string(),
        subagent_parent_info: None,
    });
    tracker.mark_persistence_clean();

    tracker.handle_agentic_event(&AgenticEvent::DialogTurnCompleted {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        total_rounds: 1,
        total_tools: 0,
        duration_ms: 42,
        subagent_parent_info: None,
        partial_recovery_reason: None,
        success: Some(true),
        finish_reason: Some("stop".to_string()),
    });

    assert_eq!(tracker.session_state(), "idle");
    assert!(tracker.is_turn_finished());
    assert!(tracker.is_persistence_dirty());
    let snapshot = tracker
        .snapshot_active_turn()
        .expect("finished snapshot remains until persistence catches up");
    assert_eq!(snapshot.status, "completed");
    assert_eq!(snapshot.turn_id, "turn-1");

    tracker.finalize_completed_turn();
    assert!(tracker.snapshot_active_turn().is_none());
    assert_eq!(tracker.accumulated_text(), "");
}

#[test]
fn remote_connect_model_catalog_delta_preserves_poll_invalidation_policy() {
    let unchanged =
        remote_model_catalog_poll_delta(Some(sample_remote_model_catalog(11)), Some(11));
    assert!(!unchanged.changed);
    assert!(unchanged.catalog.is_none());

    let changed = remote_model_catalog_poll_delta(Some(sample_remote_model_catalog(12)), Some(11));
    assert!(changed.changed);
    assert_eq!(changed.catalog.expect("changed catalog").version, 12);

    let initial_catalog =
        remote_model_catalog_poll_delta(Some(sample_remote_model_catalog(13)), None);
    assert!(initial_catalog.changed);
    assert_eq!(
        initial_catalog.catalog.expect("initial catalog").version,
        13
    );

    let unavailable_after_known_version = remote_model_catalog_poll_delta(None, Some(11));
    assert!(unavailable_after_known_version.changed);
    assert!(unavailable_after_known_version.catalog.is_none());

    let unavailable_initial = remote_model_catalog_poll_delta(None, None);
    assert!(!unavailable_initial.changed);
    assert!(unavailable_initial.catalog.is_none());
}

#[test]
fn remote_connect_poll_helpers_preserve_delta_and_completion_policy() {
    let tracker = RemoteSessionStateTracker::new("session-1".to_string());

    assert!(!should_send_remote_model_catalog(
        Some(&sample_remote_model_catalog(11)),
        Some(11)
    ));
    assert!(should_send_remote_model_catalog(
        Some(&sample_remote_model_catalog(12)),
        Some(11)
    ));

    let no_change =
        serde_json::to_value(remote_no_change_poll_response(7)).expect("serialize no-change poll");
    assert_eq!(no_change["resp"], "session_poll");
    assert_eq!(no_change["changed"], false);
    assert!(no_change.get("active_turn").is_none());

    tracker.handle_agentic_event(&AgenticEvent::DialogTurnStarted {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        turn_index: 0,
        user_input: "hello".to_string(),
        original_user_input: None,
        user_message_metadata: None,
        subagent_parent_info: None,
    });
    tracker.handle_agentic_event(&AgenticEvent::TextChunk {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        round_id: "round-1".to_string(),
        text: "answer".to_string(),
        subagent_parent_info: None,
    });
    tracker.mark_persistence_clean();

    let snapshot = serde_json::to_value(remote_snapshot_poll_response(
        &tracker,
        tracker.version(),
        Some(sample_remote_model_catalog(13)),
    ))
    .expect("serialize snapshot poll");
    assert_eq!(snapshot["changed"], true);
    assert_eq!(snapshot["active_turn"]["turn_id"], "turn-1");
    assert!(snapshot.get("new_messages").is_none());
    assert_eq!(snapshot["model_catalog"]["version"], 13);

    tracker.handle_agentic_event(&AgenticEvent::DialogTurnCompleted {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        total_rounds: 1,
        total_tools: 0,
        duration_ms: 42,
        subagent_parent_info: None,
        partial_recovery_reason: None,
        success: Some(true),
        finish_reason: Some("stop".to_string()),
    });

    let waiting_for_persistence = serde_json::to_value(remote_persisted_poll_response(
        &tracker,
        tracker.version(),
        Vec::new(),
        0,
        None,
    ))
    .expect("serialize completed poll without assistant message");
    assert!(waiting_for_persistence.get("new_messages").is_none());
    assert_eq!(
        waiting_for_persistence["active_turn"]["status"],
        "completed"
    );
    assert!(tracker.snapshot_active_turn().is_some());

    let assistant_message = ChatMessage {
        id: "msg-2".to_string(),
        role: "assistant".to_string(),
        content: "answer".to_string(),
        timestamp: "2".to_string(),
        metadata: None,
        tools: None,
        thinking: None,
        items: None,
        images: None,
    };
    let with_persisted_message = serde_json::to_value(remote_persisted_poll_response(
        &tracker,
        tracker.version(),
        vec![assistant_message],
        2,
        None,
    ))
    .expect("serialize completed poll with assistant message");
    assert_eq!(
        with_persisted_message["new_messages"][0]["role"],
        "assistant"
    );
    assert_eq!(with_persisted_message["total_msg_count"], 2);
    assert!(with_persisted_message.get("active_turn").is_none());
    assert!(tracker.snapshot_active_turn().is_none());
}

#[test]
fn remote_connect_tracker_ignores_unrelated_direct_session_events() {
    let tracker = RemoteSessionStateTracker::new("session-1".to_string());

    tracker.handle_agentic_event(&AgenticEvent::DialogTurnStarted {
        session_id: "session-2".to_string(),
        turn_id: "turn-2".to_string(),
        turn_index: 0,
        user_input: "hello".to_string(),
        original_user_input: None,
        user_message_metadata: None,
        subagent_parent_info: None,
    });
    tracker.handle_agentic_event(&AgenticEvent::TextChunk {
        session_id: "session-2".to_string(),
        turn_id: "turn-2".to_string(),
        round_id: "round-1".to_string(),
        text: "other answer".to_string(),
        subagent_parent_info: None,
    });

    assert_eq!(tracker.version(), 0);
    assert_eq!(tracker.session_state(), "idle");
    assert!(tracker.snapshot_active_turn().is_none());
    assert_eq!(tracker.accumulated_text(), "");
}

#[test]
fn remote_connect_tool_preview_slimming_keeps_short_fields_and_drops_large_strings() {
    let preview = make_slim_tool_params(&serde_json::json!({
        "path": "README.md",
        "content": "x".repeat(201),
        "line": 12
    }))
    .expect("object preview");
    let preview_json: serde_json::Value =
        serde_json::from_str(&preview).expect("preview remains json object");

    assert_eq!(preview_json["path"], "README.md");
    assert_eq!(preview_json["line"], 12);
    assert!(preview_json.get("content").is_none());

    let long_text = "a".repeat(260);
    let text_preview =
        make_slim_tool_params(&serde_json::Value::String(long_text)).expect("string preview");
    assert_eq!(text_preview.len(), 200);

    assert!(make_slim_tool_params(&serde_json::json!(42)).is_none());
}

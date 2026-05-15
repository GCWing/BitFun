#![cfg(feature = "remote-connect")]

use bitfun_events::{AgenticEvent, ToolEventData};
use bitfun_runtime_ports::AgentSubmissionSource;
use bitfun_services_integrations::remote_connect::{
    ChatImageAttachment, ChatMessage, ChatMessageItem, ImageAttachment,
    RemoteConnectSubmissionSource, RemoteSessionStateTracker, RemoteToolStatus, TrackerEvent,
    build_remote_image_attachment, build_remote_image_submission_request,
    build_remote_session_create_request, build_remote_submission_request, make_slim_tool_params,
    resolve_remote_agent_type,
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

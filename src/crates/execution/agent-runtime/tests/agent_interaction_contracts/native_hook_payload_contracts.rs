//! Native agent hook stdin payload contracts.
//!
//! Field names and per-event fields must stay identical to the Codex hook
//! process interface so existing hook scripts keep working.

use openbitfun_agent_runtime::native_hooks::{
    AgentHookEvent, AgentHookEventPayload, AgentHookPayload, AgentHookPayloadCommon,
    AgentHookPermissionMode,
};
use serde_json::json;

fn common() -> AgentHookPayloadCommon {
    AgentHookPayloadCommon {
        session_id: "session-1".to_string(),
        transcript_path: None,
        cwd: "/workspace".to_string(),
        model: "model-x".to_string(),
        permission_mode: AgentHookPermissionMode::Default,
        turn_id: Some("turn-1".to_string()),
    }
}

fn payload(event: AgentHookEventPayload) -> serde_json::Value {
    AgentHookPayload {
        common: common(),
        event,
    }
    .to_json()
}

#[test]
fn common_fields_are_present_for_every_event() {
    let events = [
        AgentHookEventPayload::SessionStart {
            source: "startup".to_string(),
        },
        AgentHookEventPayload::SessionEnd {
            reason: "other".to_string(),
        },
        AgentHookEventPayload::UserPromptSubmit {
            prompt: "hi".to_string(),
        },
        AgentHookEventPayload::PreToolUse {
            tool_name: "Bash".to_string(),
            tool_use_id: "call-1".to_string(),
            tool_input: json!({"command": "ls"}),
        },
        AgentHookEventPayload::Stop {
            stop_hook_active: false,
            last_assistant_message: None,
        },
    ];

    for event in events {
        let expected_event_name = event.event().as_str().to_string();
        let value = payload(event);
        assert_eq!(value["session_id"], json!("session-1"));
        assert_eq!(value["transcript_path"], json!(null));
        assert_eq!(value["cwd"], json!("/workspace"));
        assert_eq!(value["hook_event_name"], json!(expected_event_name));
        assert_eq!(value["model"], json!("model-x"));
        assert_eq!(value["permission_mode"], json!("default"));
    }
}

#[test]
fn turn_id_is_present_only_for_turn_scoped_events() {
    let session_start = payload(AgentHookEventPayload::SessionStart {
        source: "resume".to_string(),
    });
    assert!(session_start.get("turn_id").is_none());

    let session_end = payload(AgentHookEventPayload::SessionEnd {
        reason: "other".to_string(),
    });
    assert!(session_end.get("turn_id").is_none());

    let pre_tool_use = payload(AgentHookEventPayload::PreToolUse {
        tool_name: "Bash".to_string(),
        tool_use_id: "call-1".to_string(),
        tool_input: json!({}),
    });
    assert_eq!(pre_tool_use["turn_id"], json!("turn-1"));
}

#[test]
fn permission_mode_vocabulary_matches_codex() {
    let modes = [
        (AgentHookPermissionMode::Default, "default"),
        (AgentHookPermissionMode::AcceptEdits, "acceptEdits"),
        (AgentHookPermissionMode::Plan, "plan"),
        (AgentHookPermissionMode::DontAsk, "dontAsk"),
        (
            AgentHookPermissionMode::BypassPermissions,
            "bypassPermissions",
        ),
    ];
    for (mode, expected) in modes {
        let value = AgentHookPayload {
            common: AgentHookPayloadCommon {
                permission_mode: mode,
                ..common()
            },
            event: AgentHookEventPayload::Stop {
                stop_hook_active: false,
                last_assistant_message: None,
            },
        }
        .to_json();
        assert_eq!(value["permission_mode"], json!(expected));
    }
}

#[test]
fn event_specific_fields_use_codex_names() {
    let session_start = payload(AgentHookEventPayload::SessionStart {
        source: "compact".to_string(),
    });
    assert_eq!(session_start["source"], json!("compact"));

    let session_end = payload(AgentHookEventPayload::SessionEnd {
        reason: "other".to_string(),
    });
    assert_eq!(session_end["reason"], json!("other"));

    let prompt = payload(AgentHookEventPayload::UserPromptSubmit {
        prompt: "do the thing".to_string(),
    });
    assert_eq!(prompt["prompt"], json!("do the thing"));

    let pre_tool_use = payload(AgentHookEventPayload::PreToolUse {
        tool_name: "Bash".to_string(),
        tool_use_id: "call-1".to_string(),
        tool_input: json!({"command": "ls -la"}),
    });
    assert_eq!(pre_tool_use["tool_name"], json!("Bash"));
    assert_eq!(pre_tool_use["tool_use_id"], json!("call-1"));
    assert_eq!(pre_tool_use["tool_input"], json!({"command": "ls -la"}));

    let permission_request = payload(AgentHookEventPayload::PermissionRequest {
        tool_name: "Write".to_string(),
        tool_input: json!({"file_path": "/tmp/x"}),
    });
    assert_eq!(permission_request["tool_name"], json!("Write"));
    assert_eq!(
        permission_request["tool_input"],
        json!({"file_path": "/tmp/x"})
    );
    // PermissionRequest carries no tool_use_id in the Codex contract.
    assert!(permission_request.get("tool_use_id").is_none());

    let post_tool_use = payload(AgentHookEventPayload::PostToolUse {
        tool_name: "Read".to_string(),
        tool_use_id: "call-2".to_string(),
        tool_input: json!({"file_path": "/tmp/x"}),
        tool_response: json!({"result": "contents", "is_error": false}),
    });
    assert_eq!(post_tool_use["tool_name"], json!("Read"));
    assert_eq!(post_tool_use["tool_use_id"], json!("call-2"));
    assert_eq!(
        post_tool_use["tool_response"],
        json!({"result": "contents", "is_error": false})
    );

    for event in [
        AgentHookEventPayload::PreCompact {
            trigger: "auto".to_string(),
        },
        AgentHookEventPayload::PostCompact {
            trigger: "manual".to_string(),
        },
    ] {
        let expected = match &event {
            AgentHookEventPayload::PreCompact { trigger }
            | AgentHookEventPayload::PostCompact { trigger } => trigger.clone(),
            _ => unreachable!(),
        };
        let value = payload(event);
        assert_eq!(value["trigger"], json!(expected));
    }

    let subagent_start = payload(AgentHookEventPayload::SubagentStart {
        agent_id: "agent-1".to_string(),
        agent_type: "reviewer".to_string(),
    });
    assert_eq!(subagent_start["agent_id"], json!("agent-1"));
    assert_eq!(subagent_start["agent_type"], json!("reviewer"));

    let subagent_stop = payload(AgentHookEventPayload::SubagentStop {
        agent_id: "agent-1".to_string(),
        agent_type: "reviewer".to_string(),
        agent_transcript_path: None,
        stop_hook_active: true,
        last_assistant_message: Some("done".to_string()),
    });
    assert_eq!(subagent_stop["agent_transcript_path"], json!(null));
    assert_eq!(subagent_stop["stop_hook_active"], json!(true));
    assert_eq!(subagent_stop["last_assistant_message"], json!("done"));

    let stop = payload(AgentHookEventPayload::Stop {
        stop_hook_active: true,
        last_assistant_message: Some("final".to_string()),
    });
    assert_eq!(stop["stop_hook_active"], json!(true));
    assert_eq!(stop["last_assistant_message"], json!("final"));
}

#[test]
fn optional_last_assistant_message_is_omitted_when_absent() {
    let stop = payload(AgentHookEventPayload::Stop {
        stop_hook_active: false,
        last_assistant_message: None,
    });
    assert!(stop.get("last_assistant_message").is_none());
}

#[test]
fn transcript_path_is_serialized_when_available() {
    let value = AgentHookPayload {
        common: AgentHookPayloadCommon {
            transcript_path: Some("/tmp/transcript.jsonl".to_string()),
            ..common()
        },
        event: AgentHookEventPayload::SessionStart {
            source: "startup".to_string(),
        },
    }
    .to_json();
    assert_eq!(value["transcript_path"], json!("/tmp/transcript.jsonl"));
}

#[test]
fn matcher_context_matches_the_documented_events() {
    let cases: Vec<(AgentHookEventPayload, Option<&str>)> = vec![
        (
            AgentHookEventPayload::PreToolUse {
                tool_name: "Bash".to_string(),
                tool_use_id: "c".to_string(),
                tool_input: json!({}),
            },
            Some("Bash"),
        ),
        (
            AgentHookEventPayload::PermissionRequest {
                tool_name: "Write".to_string(),
                tool_input: json!({}),
            },
            Some("Write"),
        ),
        (
            AgentHookEventPayload::PostToolUse {
                tool_name: "Read".to_string(),
                tool_use_id: "c".to_string(),
                tool_input: json!({}),
                tool_response: json!({}),
            },
            Some("Read"),
        ),
        (
            AgentHookEventPayload::PreCompact {
                trigger: "auto".to_string(),
            },
            Some("auto"),
        ),
        (
            AgentHookEventPayload::PostCompact {
                trigger: "manual".to_string(),
            },
            Some("manual"),
        ),
        (
            AgentHookEventPayload::SessionStart {
                source: "resume".to_string(),
            },
            Some("resume"),
        ),
        (
            AgentHookEventPayload::SubagentStart {
                agent_id: "a".to_string(),
                agent_type: "reviewer".to_string(),
            },
            Some("reviewer"),
        ),
        (
            AgentHookEventPayload::SubagentStop {
                agent_id: "a".to_string(),
                agent_type: "reviewer".to_string(),
                agent_transcript_path: None,
                stop_hook_active: false,
                last_assistant_message: None,
            },
            Some("reviewer"),
        ),
        // No matcher filtering for these events.
        (
            AgentHookEventPayload::UserPromptSubmit {
                prompt: "p".to_string(),
            },
            None,
        ),
        (
            AgentHookEventPayload::Stop {
                stop_hook_active: false,
                last_assistant_message: None,
            },
            None,
        ),
        (
            AgentHookEventPayload::SessionEnd {
                reason: "other".to_string(),
            },
            None,
        ),
    ];

    for (event, expected) in cases {
        let event_name = event.event();
        assert_eq!(
            event.matcher_value(),
            expected,
            "{event_name} matcher context mismatch"
        );
    }
}

#[test]
fn event_names_render_exactly_as_configured() {
    assert_eq!(AgentHookEvent::PreToolUse.as_str(), "PreToolUse");
    assert_eq!(
        AgentHookEvent::PermissionRequest.as_str(),
        "PermissionRequest"
    );
    assert_eq!(AgentHookEvent::PostToolUse.as_str(), "PostToolUse");
    assert_eq!(AgentHookEvent::PreCompact.as_str(), "PreCompact");
    assert_eq!(AgentHookEvent::PostCompact.as_str(), "PostCompact");
    assert_eq!(AgentHookEvent::SessionStart.as_str(), "SessionStart");
    assert_eq!(AgentHookEvent::SessionEnd.as_str(), "SessionEnd");
    assert_eq!(
        AgentHookEvent::UserPromptSubmit.as_str(),
        "UserPromptSubmit"
    );
    assert_eq!(AgentHookEvent::SubagentStart.as_str(), "SubagentStart");
    assert_eq!(AgentHookEvent::SubagentStop.as_str(), "SubagentStop");
    assert_eq!(AgentHookEvent::Stop.as_str(), "Stop");
    for event in AgentHookEvent::ALL {
        assert_eq!(AgentHookEvent::parse(event.as_str()), Some(event));
    }
    assert_eq!(AgentHookEvent::parse("NotAnEvent"), None);
}

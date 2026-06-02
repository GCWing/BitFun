use bitfun_agent_runtime::scheduler::{
    resolve_background_delivery_action, BackgroundDeliveryAction, BackgroundDeliveryFacts,
    DialogRoundInjectionInterrupt, NoopDialogRoundPreemptSource, SessionRoundInjectionBuffer,
    SessionRoundYieldFlags, TurnOutcome, TurnOutcomeQueueAction, TurnOutcomeStatus,
};
use bitfun_runtime_ports::{
    DialogQueuePriority, DialogRoundPreemptSource, DialogSessionStateFact, DialogTriggerSource,
    RoundInjection, RoundInjectionKind, RoundInjectionTarget,
};
use std::sync::Arc;
use std::time::SystemTime;

#[test]
fn background_delivery_injects_when_session_is_processing() {
    let action = resolve_background_delivery_action(BackgroundDeliveryFacts {
        session_state: DialogSessionStateFact::Processing,
    });

    assert_eq!(action, BackgroundDeliveryAction::InjectIntoRunningTurn);
}

#[test]
fn background_delivery_starts_agent_session_follow_up_when_session_is_not_processing() {
    for session_state in [
        DialogSessionStateFact::Missing,
        DialogSessionStateFact::Idle,
        DialogSessionStateFact::Error,
    ] {
        let action = resolve_background_delivery_action(BackgroundDeliveryFacts { session_state });

        assert_eq!(
            action,
            BackgroundDeliveryAction::SubmitAgentSessionFollowUp {
                queue_priority: DialogQueuePriority::Low,
                skip_tool_confirmation: true,
            }
        );
    }
}

#[test]
fn background_delivery_follow_up_uses_agent_session_source_semantics() {
    let action = resolve_background_delivery_action(BackgroundDeliveryFacts {
        session_state: DialogSessionStateFact::Missing,
    });

    let policy = action
        .follow_up_submission_policy()
        .expect("follow-up action should expose submission policy");

    assert_eq!(policy.trigger_source, DialogTriggerSource::AgentSession);
    assert_eq!(policy.queue_priority, DialogQueuePriority::Low);
    assert!(policy.skip_tool_confirmation);
}

#[test]
fn background_delivery_injection_does_not_expose_follow_up_policy() {
    let action = resolve_background_delivery_action(BackgroundDeliveryFacts {
        session_state: DialogSessionStateFact::Processing,
    });

    assert_eq!(action.follow_up_submission_policy(), None);
}

#[test]
fn turn_outcome_status_reply_and_queue_policy_are_portable() {
    let completed = TurnOutcome::Completed {
        turn_id: "turn-complete".to_string(),
        final_response: "done".to_string(),
    };
    assert_eq!(completed.turn_id(), "turn-complete");
    assert_eq!(completed.status(), TurnOutcomeStatus::Completed);
    assert_eq!(completed.status_str(), "completed");
    assert_eq!(completed.reply_text(), "done");
    assert_eq!(
        completed.queue_action(),
        TurnOutcomeQueueAction::DispatchNext
    );

    let empty_completed = TurnOutcome::Completed {
        turn_id: "turn-empty".to_string(),
        final_response: "  ".to_string(),
    };
    assert_eq!(empty_completed.reply_text(), "(no final text response)");

    let cancelled = TurnOutcome::Cancelled {
        turn_id: "turn-cancel".to_string(),
    };
    assert_eq!(cancelled.status(), TurnOutcomeStatus::Cancelled);
    assert!(cancelled.reply_text().contains("cancelled"));
    assert_eq!(
        cancelled.queue_action(),
        TurnOutcomeQueueAction::DispatchNext
    );

    let failed = TurnOutcome::Failed {
        turn_id: "turn-fail".to_string(),
        error: "network offline".to_string(),
    };
    assert_eq!(failed.status(), TurnOutcomeStatus::Failed);
    assert!(failed.reply_text().contains("network offline"));
    assert_eq!(failed.queue_action(), TurnOutcomeQueueAction::ClearQueue);
}

#[test]
fn round_yield_flags_are_session_scoped_and_clearable() {
    let noop = NoopDialogRoundPreemptSource;
    assert!(!noop.should_yield_after_round("s1"));

    let flags = SessionRoundYieldFlags::default();
    flags.request_yield("s1");

    assert!(flags.should_yield_after_round("s1"));
    assert!(!flags.should_yield_after_round("s2"));

    flags.clear_yield_after_round("s1");
    assert!(!flags.should_yield_after_round("s1"));
}

#[test]
fn round_injection_buffer_drains_only_messages_for_the_active_turn() {
    let buffer = Arc::new(SessionRoundInjectionBuffer::default());
    buffer.push("s1", exact_turn_msg("turn-a", "first"));
    buffer.push("s1", exact_turn_msg("turn-b", "other"));
    buffer.push("s1", current_turn_msg("background"));

    let interrupt =
        DialogRoundInjectionInterrupt::new("s1".to_string(), "turn-a".to_string(), buffer.clone());
    assert!(interrupt.should_interrupt());

    let drained = buffer.drain_for_turn("s1", "turn-a");
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].content, "first");
    assert_eq!(drained[1].content, "background");
    assert_eq!(buffer.pending_count("s1"), 1);

    let remaining = buffer.drain_for_turn("s1", "turn-b");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].content, "other");
    assert_eq!(buffer.pending_count("s1"), 0);
}

fn exact_turn_msg(turn_id: &str, content: &str) -> RoundInjection {
    RoundInjection {
        id: format!("id-{turn_id}-{content}"),
        kind: RoundInjectionKind::UserSteering,
        target: RoundInjectionTarget::ExactTurn(turn_id.to_string()),
        content: content.to_string(),
        display_content: content.to_string(),
        created_at: SystemTime::now(),
    }
}

fn current_turn_msg(content: &str) -> RoundInjection {
    RoundInjection {
        id: format!("id-current-{content}"),
        kind: RoundInjectionKind::BackgroundResult,
        target: RoundInjectionTarget::CurrentRunningTurn,
        content: content.to_string(),
        display_content: content.to_string(),
        created_at: SystemTime::now(),
    }
}

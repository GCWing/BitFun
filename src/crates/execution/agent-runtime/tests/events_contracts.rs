use bitfun_agent_runtime::events::{
    session_state_label, turn_outcome_kind, turn_outcome_status_kind, FinishReason,
};
use bitfun_agent_runtime::scheduler::{TurnOutcome, TurnOutcomeStatus};
use bitfun_runtime_ports::{DialogSessionStateFact, DialogTurnOutcomeKind};

#[test]
fn finish_reason_display_preserves_wire_labels() {
    assert_eq!(FinishReason::Complete.as_str(), "complete");
    assert_eq!(FinishReason::ToolCalls.as_str(), "tool_calls");
    assert_eq!(FinishReason::Cancelled.as_str(), "cancelled");
    assert_eq!(FinishReason::Error.as_str(), "error");

    assert_eq!(FinishReason::Complete.to_string(), "complete");
    assert_eq!(FinishReason::ToolCalls.to_string(), "tool_calls");
    assert_eq!(FinishReason::Cancelled.to_string(), "cancelled");
    assert_eq!(FinishReason::Error.to_string(), "error");
}

#[test]
fn session_state_labels_match_existing_event_wire_values() {
    assert_eq!(session_state_label(DialogSessionStateFact::Idle), "idle");
    assert_eq!(
        session_state_label(DialogSessionStateFact::Processing),
        "processing"
    );
    assert_eq!(session_state_label(DialogSessionStateFact::Error), "error");
    assert_eq!(
        session_state_label(DialogSessionStateFact::Missing),
        "missing"
    );
}

#[test]
fn turn_outcome_kind_matches_existing_reply_policy_contract() {
    assert_eq!(
        turn_outcome_status_kind(TurnOutcomeStatus::Completed),
        DialogTurnOutcomeKind::Completed
    );
    assert_eq!(
        turn_outcome_status_kind(TurnOutcomeStatus::Cancelled),
        DialogTurnOutcomeKind::Cancelled
    );
    assert_eq!(
        turn_outcome_status_kind(TurnOutcomeStatus::Failed),
        DialogTurnOutcomeKind::Failed
    );

    let completed = TurnOutcome::Completed {
        turn_id: "turn-complete".to_string(),
        final_response: "done".to_string(),
    };
    let cancelled = TurnOutcome::Cancelled {
        turn_id: "turn-cancelled".to_string(),
    };
    let failed = TurnOutcome::Failed {
        turn_id: "turn-failed".to_string(),
        error: "network offline".to_string(),
    };

    assert_eq!(
        turn_outcome_kind(&completed),
        DialogTurnOutcomeKind::Completed
    );
    assert_eq!(
        turn_outcome_kind(&cancelled),
        DialogTurnOutcomeKind::Cancelled
    );
    assert_eq!(turn_outcome_kind(&failed), DialogTurnOutcomeKind::Failed);
}

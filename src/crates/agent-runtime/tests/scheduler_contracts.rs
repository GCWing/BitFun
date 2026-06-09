use bitfun_agent_runtime::scheduler::{
    resolve_background_delivery_action, BackgroundDeliveryAction, BackgroundDeliveryFacts,
};
use bitfun_runtime_ports::{DialogQueuePriority, DialogSessionStateFact, DialogTriggerSource};

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

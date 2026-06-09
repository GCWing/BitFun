//! Scheduler owner decisions.

use bitfun_runtime_ports::{
    DialogQueuePriority, DialogSessionStateFact, DialogSubmissionPolicy, DialogTriggerSource,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackgroundDeliveryFacts {
    pub session_state: DialogSessionStateFact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundDeliveryAction {
    InjectIntoRunningTurn,
    SubmitAgentSessionFollowUp {
        queue_priority: DialogQueuePriority,
        skip_tool_confirmation: bool,
    },
}

impl BackgroundDeliveryAction {
    pub const fn follow_up_submission_policy(self) -> Option<DialogSubmissionPolicy> {
        match self {
            Self::InjectIntoRunningTurn => None,
            Self::SubmitAgentSessionFollowUp {
                queue_priority,
                skip_tool_confirmation,
            } => Some(DialogSubmissionPolicy::new(
                DialogTriggerSource::AgentSession,
                queue_priority,
                skip_tool_confirmation,
            )),
        }
    }
}

pub const fn resolve_background_delivery_action(
    facts: BackgroundDeliveryFacts,
) -> BackgroundDeliveryAction {
    match facts.session_state {
        DialogSessionStateFact::Processing => BackgroundDeliveryAction::InjectIntoRunningTurn,
        DialogSessionStateFact::Missing
        | DialogSessionStateFact::Idle
        | DialogSessionStateFact::Error => {
            let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession);
            BackgroundDeliveryAction::SubmitAgentSessionFollowUp {
                queue_priority: policy.queue_priority,
                skip_tool_confirmation: policy.skip_tool_confirmation,
            }
        }
    }
}

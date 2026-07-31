//! Map LoopX's decisions onto BitFun's thread-goal state machine.
//!
//! A multi-issue run needs continuation, budgets, and human gates. BitFun already
//! owns all three in `thread_goal`, so this integration adds none of its own: it
//! only translates. LoopX contributes no scheduler and no quota, which is why
//! nothing here reaches for one.
//!
//! The translation that carries weight is `user_gate` → `Blocked`. Anything else
//! would let a run continue past a question LoopX raised specifically for a
//! person to answer.

use bitfun_runtime_ports::ThreadGoalStatus;

use super::orchestrator::NextStep;

/// What a serial run should do after finishing one issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunProgression {
    /// Work remains on this issue; stay on it.
    ContinueCurrentIssue,
    /// This issue is settled; move to the next selected one.
    AdvanceToNextIssue,
    /// Stop. A person must resolve something before the run continues.
    StopForHuman,
}

impl RunProgression {
    /// The goal status this progression implies.
    ///
    /// `Active` keeps `continuation_after_turn` scheduling turns; `Blocked` stops
    /// it while staying resumable, which is what a gate needs.
    pub fn thread_goal_status(self) -> ThreadGoalStatus {
        match self {
            Self::ContinueCurrentIssue | Self::AdvanceToNextIssue => ThreadGoalStatus::Active,
            Self::StopForHuman => ThreadGoalStatus::Blocked,
        }
    }

    /// Whether the run may proceed without asking anyone.
    pub fn may_proceed_unattended(self) -> bool {
        self != Self::StopForHuman
    }
}

/// Translate one LoopX decision into a run progression.
pub fn progression_for(step: NextStep) -> RunProgression {
    match step {
        NextStep::RunnableSuccessor => RunProgression::ContinueCurrentIssue,
        // A monitored PR needs no agent work right now, so the run should spend
        // its next turn on a different issue rather than idling on this one.
        NextStep::MonitorContinuation | NextStep::NoFollowup => RunProgression::AdvanceToNextIssue,
        NextStep::UserGate => RunProgression::StopForHuman,
    }
}

/// Whether a run that has hit this status may be resumed by the user.
///
/// Deliberately duplicates `agent_runtime::thread_goal::thread_goal_status_is_resumable`
/// rather than depending on that crate for one predicate. The duplication is
/// asserted below against the same status set, so a divergence shows up as a test
/// failure instead of a gated run that cannot be picked back up.
pub fn is_resumable(status: ThreadGoalStatus) -> bool {
    matches!(
        status,
        ThreadGoalStatus::Paused | ThreadGoalStatus::Blocked | ThreadGoalStatus::UsageLimited
    )
}

/// A run's position across a list of issues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialRunPlan {
    /// The issue to work next, if the run may proceed.
    pub next_issue: Option<String>,
    pub progression: RunProgression,
    pub status: ThreadGoalStatus,
}

/// Decide what a serial run does next.
///
/// `remaining` is in the order the user selected. A `StopForHuman` progression
/// clears `next_issue` outright: offering one while a gate is open would invite a
/// caller to skip it.
pub fn plan_serial_run(step: NextStep, remaining: &[String]) -> SerialRunPlan {
    let progression = progression_for(step);
    let next_issue = match progression {
        RunProgression::StopForHuman => None,
        RunProgression::ContinueCurrentIssue | RunProgression::AdvanceToNextIssue => {
            remaining.first().cloned()
        }
    };
    SerialRunPlan {
        next_issue,
        progression,
        status: progression.thread_goal_status(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_user_gate_blocks_the_goal() {
        // The decisive mapping. Any other status here would let the run continue
        // past a question raised for a person.
        let progression = progression_for(NextStep::UserGate);
        assert_eq!(progression, RunProgression::StopForHuman);
        assert_eq!(progression.thread_goal_status(), ThreadGoalStatus::Blocked);
        assert!(!progression.may_proceed_unattended());
    }

    #[test]
    fn runnable_work_keeps_the_goal_active() {
        let progression = progression_for(NextStep::RunnableSuccessor);
        assert_eq!(progression, RunProgression::ContinueCurrentIssue);
        assert_eq!(progression.thread_goal_status(), ThreadGoalStatus::Active);
        assert!(progression.may_proceed_unattended());
    }

    #[test]
    fn settled_issues_advance_the_run() {
        for step in [NextStep::MonitorContinuation, NextStep::NoFollowup] {
            let progression = progression_for(step);
            assert_eq!(
                progression,
                RunProgression::AdvanceToNextIssue,
                "for {step:?}"
            );
            assert_eq!(progression.thread_goal_status(), ThreadGoalStatus::Active);
        }
    }

    #[test]
    fn a_blocked_run_stays_resumable() {
        // Otherwise answering the question would leave the run stranded.
        //
        // Exhaustive rather than spot-checked: a new `ThreadGoalStatus` variant
        // must be classified deliberately, not silently fall through to
        // non-resumable and strand a run.
        for status in [
            ThreadGoalStatus::Active,
            ThreadGoalStatus::Paused,
            ThreadGoalStatus::Blocked,
            ThreadGoalStatus::UsageLimited,
            ThreadGoalStatus::BudgetLimited,
            ThreadGoalStatus::Complete,
        ] {
            let expected = matches!(
                status,
                ThreadGoalStatus::Paused
                    | ThreadGoalStatus::Blocked
                    | ThreadGoalStatus::UsageLimited
            );
            assert_eq!(is_resumable(status), expected, "for {status:?}");
        }
    }

    #[test]
    fn planning_offers_the_next_issue_when_work_may_proceed() {
        let remaining = vec!["1849".to_string(), "1805".to_string()];
        let plan = plan_serial_run(NextStep::NoFollowup, &remaining);
        assert_eq!(plan.next_issue.as_deref(), Some("1849"));
        assert_eq!(plan.status, ThreadGoalStatus::Active);
    }

    #[test]
    fn planning_offers_no_issue_while_a_gate_is_open() {
        // Handing back an issue here would invite a caller to skip the gate.
        let remaining = vec!["1849".to_string(), "1805".to_string()];
        let plan = plan_serial_run(NextStep::UserGate, &remaining);
        assert_eq!(plan.next_issue, None);
        assert_eq!(plan.progression, RunProgression::StopForHuman);
        assert_eq!(plan.status, ThreadGoalStatus::Blocked);
    }

    #[test]
    fn planning_handles_an_exhausted_list() {
        let plan = plan_serial_run(NextStep::NoFollowup, &[]);
        assert_eq!(plan.next_issue, None);
        // Still active: the run finished cleanly rather than stopping for a
        // person, so the goal should complete rather than block.
        assert_eq!(plan.status, ThreadGoalStatus::Active);
        assert!(plan.progression.may_proceed_unattended());
    }
}

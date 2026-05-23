//! Intent evidence collection for proactive assistance evaluation.
//!
//! This module collects lightweight trajectory signals during execution. It
//! intentionally does not assign hidden-intent terminal statuses: pi-Bench style
//! assignment requires comparing a turn against concrete hidden intents with a
//! two-stage evaluator (direct satisfaction before targeted elicitation).

use bitfun_services_core::session::hidden_intent_types::{
    IntentTerminalStatus, IntentTurnEvidence, ProactivityLevel, ProactivityScore,
    SessionIntentTracking,
};

/// Evidence collected during a single dialog turn for later intent analysis.
/// The collector is stateless per-turn: it gathers raw signals from model
/// rounds and produces an IntentTurnEvidence snapshot at turn completion.
#[derive(Debug, Clone, Default)]
pub struct IntentEvidenceCollector {
    pub asked_user_question: bool,
    pub question_topics: Vec<String>,
    pub proactive_tool_calls: usize,
    pub tool_names_used: Vec<String>,
    pub produced_output: bool,
    pub round_count: usize,
    pub asked_follow_up_in_text: bool,
}

impl IntentEvidenceCollector {
    pub fn snapshot(&self, turn_index: usize) -> IntentTurnEvidence {
        IntentTurnEvidence {
            turn_index,
            asked_user_question: self.asked_user_question,
            question_topics: self.question_topics.clone(),
            proactive_tool_calls: self.proactive_tool_calls,
            tool_names_used: self.tool_names_used.clone(),
            produced_output: self.produced_output,
            round_count: self.round_count,
            asked_follow_up_in_text: self.asked_follow_up_in_text,
        }
    }
}

// ---------------------------------------------------------------------------
// Scoring functions
// ---------------------------------------------------------------------------

pub fn compute_proactivity_score(tracking: &SessionIntentTracking) -> Option<ProactivityScore> {
    if !tracking.enabled || tracking.hidden_intents.is_empty() {
        return None;
    }
    if !tracking.all_intents_resolved() {
        return None;
    }

    let completed = tracking.count_by_status(IntentTerminalStatus::Completed) as u32;
    let inferred = tracking.count_by_status(IntentTerminalStatus::Inferred) as u32;
    let provided = tracking.count_by_status(IntentTerminalStatus::Provided) as u32;
    let total = tracking.hidden_intents.len() as u32;

    let score = (completed + inferred) as f32 / total as f32;
    Some(ProactivityScore {
        completed,
        inferred,
        provided,
        score,
        level: Some(classify_proactivity_level(score)),
    })
}

pub fn classify_proactivity_level(score: f32) -> ProactivityLevel {
    if score >= 0.8 {
        ProactivityLevel::High
    } else if score >= 0.5 {
        ProactivityLevel::Moderate
    } else if score >= 0.2 {
        ProactivityLevel::Low
    } else {
        ProactivityLevel::Reactive
    }
}

pub fn is_proactive_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "Write"
            | "Edit"
            | "Delete"
            | "Bash"
            | "Git"
            | "WebSearch"
            | "WebFetch"
            | "GenerativeUI"
            | "CreatePlan"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_services_core::session::hidden_intent_types::{
        HiddenIntent, IntentScope, IntentTerminalStatus, SessionIntentTracking,
    };

    #[test]
    fn collector_empty_on_init() {
        let c = IntentEvidenceCollector::default();
        assert!(!c.asked_user_question);
        assert!(c.question_topics.is_empty());
        assert_eq!(c.proactive_tool_calls, 0);
        assert!(c.tool_names_used.is_empty());
        assert!(!c.produced_output);
        assert_eq!(c.round_count, 0);
        assert!(!c.asked_follow_up_in_text);
    }

    #[test]
    fn collector_records_ask_user_question() {
        let mut c = IntentEvidenceCollector {
            asked_user_question: true,
            ..Default::default()
        };
        c.question_topics.push("What approach?".into());
        c.question_topics.push("Which library?".into());

        let evidence = c.snapshot(1);

        assert!(evidence.asked_user_question);
        assert_eq!(evidence.question_topics.len(), 2);
        assert_eq!(evidence.turn_index, 1);
    }

    #[test]
    fn intent_turn_evidence_round_trips() {
        let evidence = IntentTurnEvidence {
            turn_index: 2,
            asked_user_question: true,
            question_topics: vec!["Which format?".into()],
            proactive_tool_calls: 3,
            tool_names_used: vec!["Write".into(), "Edit".into()],
            produced_output: true,
            round_count: 5,
            asked_follow_up_in_text: false,
        };
        let json = serde_json::to_value(&evidence).expect("serialize");
        let rt: IntentTurnEvidence = serde_json::from_value(json).expect("deserialize");
        assert_eq!(rt.turn_index, 2);
        assert!(rt.asked_user_question);
        assert_eq!(rt.proactive_tool_calls, 3);
        assert_eq!(rt.tool_names_used, vec!["Write", "Edit"]);
    }

    #[test]
    fn compute_proactivity_score_all_completed() {
        let tracking = make_tracking(vec![
            IntentTerminalStatus::Completed,
            IntentTerminalStatus::Completed,
            IntentTerminalStatus::Completed,
        ]);
        let s = compute_proactivity_score(&tracking).unwrap();
        assert!((s.score - 1.0).abs() < f32::EPSILON);
        assert_eq!(s.completed, 3);
        assert_eq!(s.inferred, 0);
        assert_eq!(s.provided, 0);
        assert_eq!(s.level, Some(ProactivityLevel::High));
    }

    #[test]
    fn compute_proactivity_score_all_provided() {
        let tracking = make_tracking(vec![
            IntentTerminalStatus::Provided,
            IntentTerminalStatus::Provided,
        ]);
        let s = compute_proactivity_score(&tracking).unwrap();
        assert!((s.score - 0.0).abs() < f32::EPSILON);
        assert_eq!(s.provided, 2);
        assert_eq!(s.level, Some(ProactivityLevel::Reactive));
    }

    #[test]
    fn compute_proactivity_score_mixed() {
        let tracking = make_tracking(vec![
            IntentTerminalStatus::Completed,
            IntentTerminalStatus::Completed,
            IntentTerminalStatus::Inferred,
            IntentTerminalStatus::Provided,
        ]);
        let s = compute_proactivity_score(&tracking).unwrap();
        assert!((s.score - 0.75).abs() < f32::EPSILON);
        assert_eq!(s.completed, 2);
        assert_eq!(s.inferred, 1);
        assert_eq!(s.provided, 1);
        assert_eq!(s.level, Some(ProactivityLevel::Moderate));
    }

    #[test]
    fn compute_proactivity_score_empty() {
        assert_eq!(
            compute_proactivity_score(&SessionIntentTracking::default()),
            None
        );
    }

    #[test]
    fn compute_proactivity_score_requires_resolved_intents() {
        let mut tracking = make_tracking(vec![
            IntentTerminalStatus::Completed,
            IntentTerminalStatus::Provided,
        ]);
        tracking.hidden_intents.push(HiddenIntent {
            intent_id: "i-unresolved".into(),
            description: "unresolved intent".into(),
            scope: IntentScope::SessionLocal,
            terminal_status: None,
            resolved_at_turn: None,
            source: None,
        });

        assert_eq!(compute_proactivity_score(&tracking), None);
    }

    #[test]
    fn classify_proactivity_level_edges() {
        assert_eq!(classify_proactivity_level(0.9), ProactivityLevel::High);
        assert_eq!(classify_proactivity_level(0.8), ProactivityLevel::High);
        assert_eq!(classify_proactivity_level(0.79), ProactivityLevel::Moderate);
        assert_eq!(classify_proactivity_level(0.5), ProactivityLevel::Moderate);
        assert_eq!(classify_proactivity_level(0.49), ProactivityLevel::Low);
        assert_eq!(classify_proactivity_level(0.2), ProactivityLevel::Low);
        assert_eq!(classify_proactivity_level(0.19), ProactivityLevel::Reactive);
        assert_eq!(classify_proactivity_level(0.0), ProactivityLevel::Reactive);
    }

    #[test]
    fn is_proactive_tool_positive() {
        assert!(is_proactive_tool("Write"));
        assert!(is_proactive_tool("Edit"));
        assert!(is_proactive_tool("Delete"));
        assert!(is_proactive_tool("Bash"));
        assert!(is_proactive_tool("Git"));
        assert!(is_proactive_tool("WebSearch"));
        assert!(is_proactive_tool("CreatePlan"));
    }

    #[test]
    fn is_proactive_tool_negative() {
        assert!(!is_proactive_tool("Read"));
        assert!(!is_proactive_tool("Grep"));
        assert!(!is_proactive_tool("Glob"));
        assert!(!is_proactive_tool("TodoWrite"));
        assert!(!is_proactive_tool("AskUserQuestion"));
    }

    #[test]
    fn compute_proactivity_disabled() {
        let mut tracking = make_tracking(vec![IntentTerminalStatus::Completed]);
        tracking.enabled = false;
        assert_eq!(compute_proactivity_score(&tracking), None);
    }

    fn make_tracking(statuses: Vec<IntentTerminalStatus>) -> SessionIntentTracking {
        SessionIntentTracking {
            enabled: true,
            hidden_intents: statuses
                .into_iter()
                .enumerate()
                .map(|(i, status)| HiddenIntent {
                    intent_id: format!("i{}", i),
                    description: format!("test intent {}", i),
                    scope: IntentScope::SessionLocal,
                    terminal_status: Some(status),
                    resolved_at_turn: Some(i),
                    source: None,
                })
                .collect(),
            ..Default::default()
        }
    }
}

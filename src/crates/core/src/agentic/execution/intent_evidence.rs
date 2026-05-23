//! Intent evidence collection for proactive assistance evaluation.
//!
//! Provides lightweight evidence collectors that run at round/turn boundaries
//! to gather raw signals for later intent analysis. The collectors do NOT
//! perform real-time intent status assignment; that is done post-hoc by
//! facet extraction or scoring functions.

use bitfun_services_core::session::hidden_intent_types::{
    CompletenessLevel, CompletenessScore, IntentTerminalStatus, ProactivityLevel,
    ProactivityScore, SessionIntentTracking,
};
use serde::{Deserialize, Serialize};

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

/// Snapshot of evidence collected during one turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentTurnEvidence {
    pub turn_index: usize,
    pub asked_user_question: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub question_topics: Vec<String>,
    pub proactive_tool_calls: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_names_used: Vec<String>,
    pub produced_output: bool,
    pub round_count: usize,
    pub asked_follow_up_in_text: bool,
}

impl From<&IntentEvidenceCollector> for IntentTurnEvidence {
    fn from(c: &IntentEvidenceCollector) -> Self {
        Self {
            turn_index: 0,
            asked_user_question: c.asked_user_question,
            question_topics: c.question_topics.clone(),
            proactive_tool_calls: c.proactive_tool_calls,
            tool_names_used: c.tool_names_used.clone(),
            produced_output: c.produced_output,
            round_count: c.round_count,
            asked_follow_up_in_text: c.asked_follow_up_in_text,
        }
    }
}

impl IntentTurnEvidence {
    pub fn with_turn_index(mut self, turn_index: usize) -> Self {
        self.turn_index = turn_index;
        self
    }
}

// ---------------------------------------------------------------------------
// Scoring functions
// ---------------------------------------------------------------------------

pub fn compute_proactivity_score(
    tracking: &SessionIntentTracking,
) -> Option<ProactivityScore> {
    if !tracking.enabled || tracking.hidden_intents.is_empty() {
        return None;
    }
    let completed = tracking.count_by_status(IntentTerminalStatus::Completed) as u32;
    let inferred = tracking.count_by_status(IntentTerminalStatus::Inferred) as u32;
    let provided = tracking.count_by_status(IntentTerminalStatus::Provided) as u32;
    let total = (completed + inferred + provided).max(1);
    let score = (completed + inferred) as f32 / total as f32;
    Some(ProactivityScore {
        completed, inferred, provided, score,
        level: Some(classify_proactivity_level(score)),
    })
}

pub fn compute_completeness_score(
    tracking: &SessionIntentTracking,
) -> Option<CompletenessScore> {
    if !tracking.enabled || tracking.hidden_intents.is_empty() {
        return None;
    }
    let total = tracking.hidden_intents.len() as u32;
    let resolved = tracking.hidden_intents.iter()
        .filter(|i| i.terminal_status.is_some()).count() as u32;
    let missed = total.saturating_sub(resolved);
    let score = if total == 0 { 1.0 } else { resolved as f32 / total as f32 };
    Some(CompletenessScore {
        requirements_satisfied: resolved, requirements_missed: missed, score,
        level: Some(classify_completeness_level(score)),
    })
}

pub fn classify_proactivity_level(score: f32) -> ProactivityLevel {
    if score >= 0.8 { ProactivityLevel::High }
    else if score >= 0.5 { ProactivityLevel::Moderate }
    else if score >= 0.2 { ProactivityLevel::Low }
    else { ProactivityLevel::Reactive }
}

pub fn classify_completeness_level(score: f32) -> CompletenessLevel {
    if (score - 1.0).abs() < f32::EPSILON { CompletenessLevel::Full }
    else if score >= 0.7 { CompletenessLevel::Partial }
    else if score >= 0.3 { CompletenessLevel::Minimal }
    else { CompletenessLevel::Incomplete }
}

pub fn is_proactive_tool(tool_name: &str) -> bool {
    matches!(tool_name,
        "Write" | "Edit" | "Delete" | "Bash" | "Git" | "WebSearch"
        | "WebFetch" | "GenerativeUI" | "CreatePlan"
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
        let mut c = IntentEvidenceCollector::default();
        c.asked_user_question = true;
        c.question_topics.push("What approach?".into());
        c.question_topics.push("Which library?".into());
        let evidence = IntentTurnEvidence::from(&c).with_turn_index(1);
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
            IntentTerminalStatus::Completed, IntentTerminalStatus::Completed,
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
            IntentTerminalStatus::Provided, IntentTerminalStatus::Provided,
        ]);
        let s = compute_proactivity_score(&tracking).unwrap();
        assert!((s.score - 0.0).abs() < f32::EPSILON);
        assert_eq!(s.provided, 2);
        assert_eq!(s.level, Some(ProactivityLevel::Reactive));
    }

    #[test]
    fn compute_proactivity_score_mixed() {
        let tracking = make_tracking(vec![
            IntentTerminalStatus::Completed, IntentTerminalStatus::Completed,
            IntentTerminalStatus::Inferred, IntentTerminalStatus::Provided,
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
        assert_eq!(compute_proactivity_score(&SessionIntentTracking::default()), None);
    }

    #[test]
    fn compute_completeness_score_full() {
        let tracking = make_tracking(vec![
            IntentTerminalStatus::Completed, IntentTerminalStatus::Completed,
        ]);
        let s = compute_completeness_score(&tracking).unwrap();
        assert!((s.score - 1.0).abs() < f32::EPSILON);
        assert_eq!(s.level, Some(CompletenessLevel::Full));
    }

    #[test]
    fn compute_completeness_score_partial() {
        let mut tracking = make_tracking(vec![
            IntentTerminalStatus::Completed, IntentTerminalStatus::Completed,
        ]);
        tracking.hidden_intents.push(HiddenIntent {
            intent_id: "i3".into(), description: "unresolved".into(),
            scope: IntentScope::SessionLocal,
            terminal_status: None, resolved_at_turn: None, source: None,
        });
        let s = compute_completeness_score(&tracking).unwrap();
        assert!((s.score - 2.0 / 3.0).abs() < f32::EPSILON);
        assert_eq!(s.requirements_missed, 1);
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
    fn classify_completeness_level_edges() {
        assert_eq!(classify_completeness_level(1.0), CompletenessLevel::Full);
        assert_eq!(classify_completeness_level(0.7), CompletenessLevel::Partial);
        assert_eq!(classify_completeness_level(0.69), CompletenessLevel::Minimal);
        assert_eq!(classify_completeness_level(0.3), CompletenessLevel::Minimal);
        assert_eq!(classify_completeness_level(0.29), CompletenessLevel::Incomplete);
        assert_eq!(classify_completeness_level(0.0), CompletenessLevel::Incomplete);
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
            hidden_intents: statuses.into_iter().enumerate().map(|(i, status)| {
                HiddenIntent {
                    intent_id: format!("i{}", i),
                    description: format!("test intent {}", i),
                    scope: IntentScope::SessionLocal,
                    terminal_status: Some(status),
                    resolved_at_turn: Some(i),
                    source: None,
                }
            }).collect(),
            ..Default::default()
        }
    }
}

//! Intent evidence collection for proactive assistance evaluation.
//!
//! This module collects lightweight trajectory signals during execution. It
//! intentionally does not assign hidden-intent terminal statuses: pi-Bench style
//! assignment requires comparing a turn against concrete hidden intents with a
//! two-stage evaluator (direct satisfaction before targeted elicitation).

use bitfun_services_core::session::hidden_intent_types::{
    CompletenessLevel, CompletenessScore, HiddenIntent, IntentScope, IntentSource,
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

/// Classify a proactivity score into a qualitative level.
/// Delegates to `ProactivityLevel::from_score` so the thresholds stay in one place.
pub fn classify_proactivity_level(score: f32) -> ProactivityLevel {
    ProactivityLevel::from_score(score)
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

// ---------------------------------------------------------------------------
// Hidden intent extraction from turn evidence
// ---------------------------------------------------------------------------

/// Extract new hidden intents from a turn's collected evidence.
///
/// Uses lightweight heuristics to infer requirements the agent discovered
/// during this turn. Extracted intents are appended to the session's tracking
/// state and become available for proactivity scoring.
pub fn extract_hidden_intents_from_evidence(
    evidence: &IntentTurnEvidence,
    existing_intents: &[HiddenIntent],
) -> Vec<HiddenIntent> {
    let mut new_intents = Vec::new();

    // 1. Agent used proactive tools and produced output: infer requirements.
    if evidence.proactive_tool_calls > 0 && evidence.produced_output {
        for tool_name in &evidence.tool_names_used {
            if !is_proactive_tool(tool_name) {
                continue;
            }
            let intent_id = format!(
                "proactive-{}-turn{}",
                tool_name.to_lowercase(),
                evidence.turn_index
            );
            if existing_intents.iter().any(|i| i.intent_id == intent_id) {
                continue;
            }
            new_intents.push(HiddenIntent {
                intent_id,
                description: proactive_tool_intent_description(tool_name),
                scope: IntentScope::SessionLocal,
                terminal_status: Some(IntentTerminalStatus::Completed),
                resolved_at_turn: Some(evidence.turn_index),
                source: Some(IntentSource::PriorContext),
            });
        }
    }

    // 2. Agent asked targeted clarification questions via AskUserQuestion.
    if evidence.asked_user_question && !evidence.question_topics.is_empty() {
        for topic in &evidence.question_topics {
            let slug = topic
                .chars()
                .take(40)
                .map(|c| {
                    if c.is_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect::<String>();
            let intent_id =
                format!("asked-{}-turn{}", slug.trim_matches('-'), evidence.turn_index);
            if existing_intents.iter().any(|i| i.intent_id == intent_id) {
                continue;
            }
            new_intents.push(HiddenIntent {
                intent_id,
                description: format!("Required clarification: {}", topic),
                scope: IntentScope::SessionLocal,
                terminal_status: Some(IntentTerminalStatus::Inferred),
                resolved_at_turn: Some(evidence.turn_index),
                source: Some(IntentSource::PriorContext),
            });
        }
    }

    new_intents
}

fn proactive_tool_intent_description(tool_name: &str) -> String {
    match tool_name {
        "Write" => "Agent proactively created a new file".to_string(),
        "Edit" => "Agent proactively modified an existing file".to_string(),
        "Delete" => "Agent proactively removed unneeded content".to_string(),
        "Bash" => "Agent proactively executed a shell command".to_string(),
        "Git" => "Agent proactively performed version control operations".to_string(),
        "WebSearch" => "Agent proactively searched for information".to_string(),
        "WebFetch" => "Agent proactively fetched external content".to_string(),
        "GenerativeUI" => "Agent proactively created interactive UI output".to_string(),
        "CreatePlan" => "Agent proactively planned the task structure".to_string(),
        _ => format!("Agent proactively used {}", tool_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_services_core::session::hidden_intent_types::{
        HiddenIntent, IntentScope, IntentSource, IntentTerminalStatus, SessionIntentTracking,
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

    #[test]
    fn extract_hidden_intents_from_proactive_tools() {
        let evidence = IntentTurnEvidence {
            turn_index: 1,
            asked_user_question: false,
            question_topics: vec![],
            proactive_tool_calls: 2,
            tool_names_used: vec!["Write".into(), "Edit".into()],
            produced_output: true,
            round_count: 3,
            asked_follow_up_in_text: false,
        };
        let intents = extract_hidden_intents_from_evidence(&evidence, &[]);
        assert_eq!(intents.len(), 2);
        assert!(intents
            .iter()
            .any(|i| i.intent_id == "proactive-write-turn1"));
        assert_eq!(
            intents[0].terminal_status,
            Some(IntentTerminalStatus::Completed)
        );
    }

    #[test]
    fn extract_hidden_intents_from_ask_user_question() {
        let evidence = IntentTurnEvidence {
            turn_index: 2,
            asked_user_question: true,
            question_topics: vec!["Which database?".into()],
            proactive_tool_calls: 0,
            tool_names_used: vec![],
            produced_output: false,
            round_count: 1,
            asked_follow_up_in_text: false,
        };
        let intents = extract_hidden_intents_from_evidence(&evidence, &[]);
        assert_eq!(intents.len(), 1);
        assert!(intents[0].intent_id.contains("asked-"));
        assert_eq!(
            intents[0].terminal_status,
            Some(IntentTerminalStatus::Inferred)
        );
    }

    #[test]
    fn extract_hidden_intents_deduplicates_existing() {
        let evidence = IntentTurnEvidence {
            turn_index: 1,
            asked_user_question: false,
            question_topics: vec![],
            proactive_tool_calls: 1,
            tool_names_used: vec!["Write".into()],
            produced_output: true,
            round_count: 1,
            asked_follow_up_in_text: false,
        };
        let existing = vec![HiddenIntent {
            intent_id: "proactive-write-turn1".into(),
            description: "already exists".into(),
            scope: IntentScope::SessionLocal,
            terminal_status: Some(IntentTerminalStatus::Completed),
            resolved_at_turn: Some(1),
            source: Some(IntentSource::PriorContext),
        }];
        assert!(extract_hidden_intents_from_evidence(&evidence, &existing).is_empty());
    }

    #[test]
    fn extract_hidden_intents_empty_when_passive() {
        let evidence = IntentTurnEvidence {
            turn_index: 0,
            asked_user_question: false,
            question_topics: vec![],
            proactive_tool_calls: 0,
            tool_names_used: vec!["Read".into()],
            produced_output: false,
            round_count: 1,
            asked_follow_up_in_text: false,
        };
        assert!(extract_hidden_intents_from_evidence(&evidence, &[]).is_empty());
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

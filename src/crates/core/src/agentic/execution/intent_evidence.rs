//! Intent evidence collection for proactive assistance evaluation.
//!
//! This module collects lightweight trajectory signals during execution. It
//! intentionally does not assign hidden-intent terminal statuses: pi-Bench style
//! assignment requires comparing a turn against concrete hidden intents with a
//! two-stage evaluator (direct satisfaction before targeted elicitation).

use bitfun_services_core::session::hidden_intent_types::{
    HiddenIntent, IntentScope, IntentSource, IntentTerminalStatus, IntentTurnEvidence,
    ProactivityLevel, ProactivityScore, SessionIntentTracking,
};

/// Per-turn caps to keep evidence storage bounded. Long sessions used to grow
/// `tool_names_used` / `question_topics` without limit.
const MAX_TOOL_NAMES_PER_TURN: usize = 64;
const MAX_QUESTION_TOPICS_PER_TURN: usize = 16;
/// Per-session caps applied at persistence time.
pub const MAX_TURN_EVIDENCE_RETAINED: usize = 64;
pub const MAX_HIDDEN_INTENTS_RETAINED: usize = 256;

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
        let tool_names_used = if self.tool_names_used.len() > MAX_TOOL_NAMES_PER_TURN {
            self.tool_names_used[..MAX_TOOL_NAMES_PER_TURN].to_vec()
        } else {
            self.tool_names_used.clone()
        };
        let question_topics = if self.question_topics.len() > MAX_QUESTION_TOPICS_PER_TURN {
            self.question_topics[..MAX_QUESTION_TOPICS_PER_TURN].to_vec()
        } else {
            self.question_topics.clone()
        };
        IntentTurnEvidence {
            turn_index,
            asked_user_question: self.asked_user_question,
            question_topics,
            proactive_tool_calls: self.proactive_tool_calls,
            tool_names_used,
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

/// Extract candidate hidden intents from a turn's collected evidence.
///
/// Intents emitted here are *trajectory markers*, not evaluated assignments.
/// `terminal_status` is intentionally left `None` so a downstream evaluator can
/// stamp them. Auto-stamping `Completed`/`Inferred` would make
/// `all_intents_resolved()` trivially true and inflate proactivity scores; the
/// module-level doc explicitly forbids that.
pub fn extract_hidden_intents_from_evidence(
    evidence: &IntentTurnEvidence,
    existing_intents: &[HiddenIntent],
) -> Vec<HiddenIntent> {
    let mut new_intents = Vec::new();

    // 1. Agent used proactive tools and produced output: record a trajectory
    //    marker per distinct proactive tool. No terminal status.
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
                terminal_status: None,
                resolved_at_turn: Some(evidence.turn_index),
                source: Some(IntentSource::PriorContext),
            });
        }
    }

    // 2. Agent asked targeted clarification questions via AskUserQuestion.
    if evidence.asked_user_question && !evidence.question_topics.is_empty() {
        for topic in &evidence.question_topics {
            let intent_id = format!(
                "asked-{}-turn{}",
                slugify_topic(topic, evidence.turn_index),
                evidence.turn_index
            );
            if existing_intents.iter().any(|i| i.intent_id == intent_id) {
                continue;
            }
            new_intents.push(HiddenIntent {
                intent_id,
                description: format!("Required clarification: {}", topic),
                scope: IntentScope::SessionLocal,
                terminal_status: None,
                resolved_at_turn: Some(evidence.turn_index),
                source: Some(IntentSource::PriorContext),
            });
        }
    }

    new_intents
}

/// Build a stable, ASCII-safe slug from a free-text question topic. Falls back
/// to a short hash digest when stripping non-alphanumerics leaves nothing
/// (common with CJK / emoji headers) so per-turn IDs don't collide.
fn slugify_topic(topic: &str, turn_index: usize) -> String {
    let ascii: String = topic
        .chars()
        .take(40)
        .map(|c| {
            if c.is_alphanumeric() && c.is_ascii() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = ascii.trim_matches('-');
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    // Fallback: short deterministic hash of (topic, turn_index) to avoid
    // collisions when the slug collapses to empty.
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    topic.hash(&mut hasher);
    turn_index.hash(&mut hasher);
    format!("h{:08x}", hasher.finish() as u32)
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
        // Trajectory markers must not carry a terminal status; only a
        // downstream evaluator may stamp Completed/Inferred/Provided.
        assert!(intents.iter().all(|i| i.terminal_status.is_none()));
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
        assert!(intents[0].terminal_status.is_none());
    }

    #[test]
    fn slugify_topic_falls_back_to_hash_for_non_ascii() {
        let s1 = slugify_topic("ヘッダ確認", 1);
        let s2 = slugify_topic("ヘッダ確認", 2);
        let s3 = slugify_topic("コンテキスト", 1);
        assert!(s1.starts_with('h') && s1.len() == 9);
        assert_ne!(s1, s2, "different turns must produce distinct fallback slugs");
        assert_ne!(s1, s3, "different topics must produce distinct fallback slugs");
    }

    #[test]
    fn slugify_topic_preserves_ascii_prefix() {
        assert_eq!(slugify_topic("Which database?", 7), "which-database");
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

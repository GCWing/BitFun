//! Hidden Intent tracking types for proactive assistance evaluation.
//!
//! Based on the pi-Bench Hidden Intent framework, these types enable
//! tracking whether an agent proactively resolves hidden user requirements
//! or passively waits for the user to provide them.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Core intent tracking types
// ---------------------------------------------------------------------------

/// Terminal status of a hidden intent during a session.
///
/// Both Completed and Inferred count toward proactivity because both reflect
/// agent initiative. Provided means the user had to surface the requirement
/// without agent prompting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentTerminalStatus {
    Completed,
    Inferred,
    Provided,
}

impl IntentTerminalStatus {
    pub fn is_proactive(&self) -> bool {
        matches!(self, Self::Completed | Self::Inferred)
    }
}

/// A single hidden intent -- an unstated requirement that should shape the
/// agent's behavior during interaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiddenIntent {
    #[serde(alias = "intent_id")]
    pub intent_id: String,
    pub description: String,
    #[serde(default)]
    pub scope: IntentScope,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "terminal_status")]
    pub terminal_status: Option<IntentTerminalStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "resolved_at_turn")]
    pub resolved_at_turn: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<IntentSource>,
}

/// Whether an intent is session-local or persists across sessions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum IntentScope {
    #[default]
    SessionLocal,
    Persistent,
}

/// Source from which a hidden intent was derived.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentSource {
    PriorContext,
    DomainKnowledge,
    UserPreference,
    ManualAnnotation,
}

/// A user preference or convention that persists across sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistentIntent {
    #[serde(alias = "intent_id")]
    pub intent_id: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(alias = "established_in_session")]
    pub established_in_session: String,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "apply_count")]
    pub apply_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "last_applied_at")]
    pub last_applied_at: Option<u64>,
    #[serde(alias = "established_at")]
    pub established_at: u64,
}

/// Records a terminal status assignment for a hidden intent at a specific turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentAssignment {
    #[serde(alias = "intent_id")]
    pub intent_id: String,
    #[serde(alias = "terminal_status")]
    pub terminal_status: IntentTerminalStatus,
    #[serde(alias = "assigned_at_turn")]
    pub assigned_at_turn: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_description: Option<String>,
}

/// Aggregate intent tracking state for a single session.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionIntentTracking {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty", alias = "hidden_intents")]
    pub hidden_intents: Vec<HiddenIntent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", alias = "persistent_intents")]
    pub persistent_intents: Vec<PersistentIntent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assignments: Vec<IntentAssignment>,
}

impl SessionIntentTracking {
    pub fn all_intents_resolved(&self) -> bool {
        if !self.enabled || self.hidden_intents.is_empty() {
            return true;
        }
        self.hidden_intents.iter().all(|i| i.terminal_status.is_some())
    }

    pub fn count_by_status(&self, status: IntentTerminalStatus) -> usize {
        self.hidden_intents.iter().filter(|i| i.terminal_status.as_ref() == Some(&status)).count()
    }

    pub fn total_intents(&self) -> usize {
        self.hidden_intents.len()
    }

    pub fn proactive_count(&self) -> usize {
        self.count_by_status(IntentTerminalStatus::Completed)
            + self.count_by_status(IntentTerminalStatus::Inferred)
    }

    pub fn proactivity_score(&self) -> Option<f32> {
        let total = self.total_intents();
        if total == 0 {
            return None;
        }
        Some(self.proactive_count() as f32 / total as f32)
    }
}

/// Proactivity score breakdown for a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProactivityScore {
    pub completed: u32,
    pub inferred: u32,
    pub provided: u32,
    pub score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<ProactivityLevel>,
}

/// Qualitative proactivity level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProactivityLevel {
    High,
    Moderate,
    Low,
    Reactive,
}

/// Completeness score breakdown for a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompletenessScore {
    #[serde(alias = "requirements_satisfied")]
    pub requirements_satisfied: u32,
    #[serde(alias = "requirements_missed")]
    pub requirements_missed: u32,
    pub score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<CompletenessLevel>,
}

/// Qualitative completeness level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompletenessLevel {
    Full,
    Partial,
    Minimal,
    Incomplete,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_status_is_proactive() {
        assert!(IntentTerminalStatus::Completed.is_proactive());
        assert!(IntentTerminalStatus::Inferred.is_proactive());
        assert!(!IntentTerminalStatus::Provided.is_proactive());
    }

    #[test]
    fn all_intents_resolved_empty() {
        let tracking = SessionIntentTracking::default();
        assert!(tracking.all_intents_resolved());
    }

    #[test]
    fn all_intents_resolved_with_intents() {
        let tracking = SessionIntentTracking {
            enabled: true,
            hidden_intents: vec![HiddenIntent {
                intent_id: "i1".into(),
                description: "test".into(),
                scope: IntentScope::SessionLocal,
                terminal_status: Some(IntentTerminalStatus::Completed),
                resolved_at_turn: Some(1),
                source: None,
            }],
            ..Default::default()
        };
        assert!(tracking.all_intents_resolved());
    }

    #[test]
    fn all_intents_not_resolved() {
        let tracking = SessionIntentTracking {
            enabled: true,
            hidden_intents: vec![
                HiddenIntent {
                    intent_id: "i1".into(), description: "test".into(),
                    scope: IntentScope::SessionLocal,
                    terminal_status: Some(IntentTerminalStatus::Completed),
                    resolved_at_turn: Some(1), source: None,
                },
                HiddenIntent {
                    intent_id: "i2".into(), description: "test".into(),
                    scope: IntentScope::SessionLocal,
                    terminal_status: None, resolved_at_turn: None, source: None,
                },
            ],
            ..Default::default()
        };
        assert!(!tracking.all_intents_resolved());
    }

    #[test]
    fn proactivity_score_full() {
        let tracking = SessionIntentTracking {
            enabled: true,
            hidden_intents: (0..4).map(|i| HiddenIntent {
                intent_id: format!("i{}", i),
                description: "test".into(),
                scope: IntentScope::SessionLocal,
                terminal_status: Some(IntentTerminalStatus::Completed),
                resolved_at_turn: Some(i),
                source: None,
            }).collect(),
            ..Default::default()
        };
        let score = tracking.proactivity_score().unwrap();
        assert!((score - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn proactivity_score_mixed() {
        let tracking = SessionIntentTracking {
            enabled: true,
            hidden_intents: vec![
                HiddenIntent {
                    intent_id: "i1".into(), description: "test".into(),
                    scope: IntentScope::SessionLocal,
                    terminal_status: Some(IntentTerminalStatus::Completed),
                    resolved_at_turn: Some(1), source: None,
                },
                HiddenIntent {
                    intent_id: "i2".into(), description: "test".into(),
                    scope: IntentScope::SessionLocal,
                    terminal_status: Some(IntentTerminalStatus::Inferred),
                    resolved_at_turn: Some(2), source: None,
                },
                HiddenIntent {
                    intent_id: "i3".into(), description: "test".into(),
                    scope: IntentScope::SessionLocal,
                    terminal_status: Some(IntentTerminalStatus::Provided),
                    resolved_at_turn: Some(3), source: None,
                },
            ],
            ..Default::default()
        };
        let score = tracking.proactivity_score().unwrap();
        assert!((score - 2.0 / 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn proactivity_score_no_intents() {
        let tracking = SessionIntentTracking::default();
        assert_eq!(tracking.proactivity_score(), None);
    }

    #[test]
    fn hidden_intent_round_trips() {
        let intent = HiddenIntent {
            intent_id: "i1".into(),
            description: "Apply naming convention from prior session".into(),
            scope: IntentScope::Persistent,
            terminal_status: Some(IntentTerminalStatus::Inferred),
            resolved_at_turn: Some(3),
            source: Some(IntentSource::PriorContext),
        };
        let json = serde_json::to_value(&intent).expect("serialize");
        let rt: HiddenIntent = serde_json::from_value(json).expect("deserialize");
        assert_eq!(rt.intent_id, "i1");
        assert_eq!(rt.terminal_status, Some(IntentTerminalStatus::Inferred));
        assert_eq!(rt.scope, IntentScope::Persistent);
    }

    #[test]
    fn proactivity_score_round_trips() {
        let score = ProactivityScore {
            completed: 3, inferred: 2, provided: 1,
            score: 5.0 / 6.0,
            level: Some(ProactivityLevel::High),
        };
        let json = serde_json::to_value(&score).expect("serialize");
        let rt: ProactivityScore = serde_json::from_value(json).expect("deserialize");
        assert_eq!(rt.completed, 3);
        assert_eq!(rt.inferred, 2);
        assert_eq!(rt.provided, 1);
        assert_eq!(rt.level, Some(ProactivityLevel::High));
    }
}

#![cfg(feature = "taiji")]
//! Audit-Poke protocol types and validation logic.
//!
//! This module defines the Poke protocol used by Warden to send audit
//! and challenge messages to Executor agents, and by Executor to respond
//! with self-check statements or appeals.
//!
//! # Protocol overview
//!
//! - **Audit-Poke**: Event-triggered (after Write/Edit/Delete/Exec), 3-turn deadline.
//! - **Challenge-Poke**: Poisson-sampled (avg 5–8 turns), 5-turn deadline.
//!
//! All types implement `Serialize`/`Deserialize` for transport via `SessionMessage`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// PokeType
// ---------------------------------------------------------------------------

/// The type of a Poke message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PokeType {
    /// Audit-Poke: event-triggered (after Write/Edit/Delete/Exec), 3-turn deadline.
    Audit,
    /// Challenge-Poke: Poisson-sampled (avg 5–8 turns), 5-turn deadline.
    Challenge,
}

// ---------------------------------------------------------------------------
// PokeMessage — Warden → Executor
// ---------------------------------------------------------------------------

/// A Poke message sent from Warden to Executor requesting a self-check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PokeMessage {
    /// Unique identifier for this poke (used to correlate response).
    pub poke_id: String,
    /// Whether this is an Audit or Challenge poke.
    pub poke_type: PokeType,
    /// Iron-rule IDs that triggered this poke.
    pub rule_ids: Vec<String>,
    /// Number of turns the Executor has to respond.
    pub deadline_turns: u32,
    /// Optional list of specific evidence items requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_required: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// PokeStatus
// ---------------------------------------------------------------------------

/// The status of an Executor's response to a Poke.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PokeStatus {
    /// The Executor acknowledges the poke and provides a self-check.
    Acknowledged,
    /// The Executor defers the response; the count tracks how many times deferred.
    Deferred(u32),
    /// The Executor appeals, claiming the poke is invalid or mis-attributed.
    Appeal(AppealStatement),
}

// ---------------------------------------------------------------------------
// SelfCheckStatement
// ---------------------------------------------------------------------------

/// A self-check statement provided by the Executor in response to a Poke.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfCheckStatement {
    /// The current phase the Executor is in.
    pub current_phase: String,
    /// The last approval gate passed.
    pub last_gate: String,
    /// Summary of tool calls made since the last check.
    pub tool_calls_summary: Vec<String>,
    /// List of iron rules that were checked.
    pub rules_checked: Vec<String>,
}

// ---------------------------------------------------------------------------
// AppealStatement
// ---------------------------------------------------------------------------

/// An appeal statement submitted when the Executor disputes a Poke.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppealStatement {
    /// Identifier of the specific violation being appealed.
    pub violation_id: String,
    /// Human-readable reason for the appeal.
    pub reason: String,
    /// Supporting evidence references.
    pub evidence: Vec<String>,
}

// ---------------------------------------------------------------------------
// PokeResponse — Executor → Warden
// ---------------------------------------------------------------------------

/// A response from the Executor to a Poke message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PokeResponse {
    /// Must match the `poke_id` from the corresponding `PokeMessage`.
    pub poke_id: String,
    /// The status of this response.
    pub status: PokeStatus,
    /// Self-check statement (required when status is `Acknowledged` or `Deferred`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_check: Option<SelfCheckStatement>,
}

// ---------------------------------------------------------------------------
// PokeValidator
// ---------------------------------------------------------------------------

/// Validator for Poke responses.
///
/// Provides business‑rule checks for both Audit and Challenge responses.
pub struct PokeValidator;

impl PokeValidator {
    /// Validate an Audit-Poke response.
    ///
    /// Audit responses **must**:
    /// - Have a matching `poke_id` (checked by caller; we validate presence).
    /// - Have `status` = `Acknowledged` (deferral is allowed but must include a self-check).
    /// - Include a `self_check` with non-empty `current_phase`, `last_gate`, and `tool_calls_summary`.
    /// - Include at least one entry in `rules_checked`.
    pub fn validate_audit_response(response: &PokeResponse) -> bool {
        // Must include a self-check
        let Some(ref sc) = response.self_check else {
            return false;
        };

        // Check required fields are non-empty
        if sc.current_phase.is_empty() || sc.last_gate.is_empty() {
            return false;
        }

        // Must have at least one tool call and one rule checked
        if sc.tool_calls_summary.is_empty() || sc.rules_checked.is_empty() {
            return false;
        }

        // For Audit, Acknowledged is the standard; Deferred is allowed but suspicious.
        // Appeal is also valid but requires an AppealStatement.
        match &response.status {
            PokeStatus::Acknowledged => true,
            PokeStatus::Deferred(_) => true,
            PokeStatus::Appeal(appeal) => {
                // Appeal must have a non-empty reason
                !appeal.reason.is_empty()
            }
        }
    }

    /// Validate a Challenge-Poke response.
    ///
    /// Challenge responses **must**:
    /// - Have a matching `poke_id` (checked by caller; we validate presence).
    /// - Include a `self_check` with non-empty `current_phase`, `last_gate`, and `tool_calls_summary`.
    /// - Include at least one entry in `rules_checked`.
    /// - If status is `Deferred`, the defer count must be ≤ 3.
    /// - If status is `Appeal`, the `AppealStatement` must have a non-empty `reason` and at least
    ///   one piece of `evidence`.
    pub fn validate_challenge_response(response: &PokeResponse) -> bool {
        // Must include a self-check
        let Some(ref sc) = response.self_check else {
            return false;
        };

        // Check required fields are non-empty
        if sc.current_phase.is_empty() || sc.last_gate.is_empty() {
            return false;
        }

        // Must have at least one tool call and one rule checked
        if sc.tool_calls_summary.is_empty() || sc.rules_checked.is_empty() {
            return false;
        }

        match &response.status {
            PokeStatus::Acknowledged => true,
            PokeStatus::Deferred(count) => {
                // Challenge-Poke allows max 3 consecutive defers
                *count <= 3
            }
            PokeStatus::Appeal(appeal) => {
                // Appeal must have a non-empty reason and at least one evidence item
                !appeal.reason.is_empty() && !appeal.evidence.is_empty()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Helpers ---

    fn sample_self_check() -> SelfCheckStatement {
        SelfCheckStatement {
            current_phase: "execution".into(),
            last_gate: "pre_write_check".into(),
            tool_calls_summary: vec!["Read(file.txt)".into(), "Write(file.txt)".into()],
            rules_checked: vec!["R1: no_destructive_write".into(), "R3: path_whitelist".into()],
        }
    }

    fn sample_audit_response(status: PokeStatus) -> PokeResponse {
        PokeResponse {
            poke_id: "poke-001".into(),
            self_check: Some(sample_self_check()),
            status,
        }
    }

    fn sample_challenge_response(status: PokeStatus) -> PokeResponse {
        PokeResponse {
            poke_id: "poke-002".into(),
            self_check: Some(sample_self_check()),
            status,
        }
    }

    // --- Audit validation ---

    #[test]
    fn audit_acknowledged_passes() {
        let resp = sample_audit_response(PokeStatus::Acknowledged);
        assert!(PokeValidator::validate_audit_response(&resp));
    }

    #[test]
    fn audit_deferred_passes() {
        let resp = sample_audit_response(PokeStatus::Deferred(1));
        assert!(PokeValidator::validate_audit_response(&resp));
    }

    #[test]
    fn audit_appeal_with_reason_passes() {
        let resp = sample_audit_response(PokeStatus::Appeal(AppealStatement {
            violation_id: "V-001".into(),
            reason: "The write was to a permitted path".into(),
            evidence: vec![],
        }));
        assert!(PokeValidator::validate_audit_response(&resp));
    }

    #[test]
    fn audit_missing_self_check_fails() {
        let resp = PokeResponse {
            poke_id: "poke-001".into(),
            self_check: None,
            status: PokeStatus::Acknowledged,
        };
        assert!(!PokeValidator::validate_audit_response(&resp));
    }

    #[test]
    fn audit_empty_phase_fails() {
        let mut sc = sample_self_check();
        sc.current_phase.clear();
        let resp = PokeResponse {
            poke_id: "poke-001".into(),
            self_check: Some(sc),
            status: PokeStatus::Acknowledged,
        };
        assert!(!PokeValidator::validate_audit_response(&resp));
    }

    #[test]
    fn audit_empty_tool_summary_fails() {
        let mut sc = sample_self_check();
        sc.tool_calls_summary.clear();
        let resp = PokeResponse {
            poke_id: "poke-001".into(),
            self_check: Some(sc),
            status: PokeStatus::Acknowledged,
        };
        assert!(!PokeValidator::validate_audit_response(&resp));
    }

    #[test]
    fn audit_empty_rules_checked_fails() {
        let mut sc = sample_self_check();
        sc.rules_checked.clear();
        let resp = PokeResponse {
            poke_id: "poke-001".into(),
            self_check: Some(sc),
            status: PokeStatus::Acknowledged,
        };
        assert!(!PokeValidator::validate_audit_response(&resp));
    }

    // --- Challenge validation ---

    #[test]
    fn challenge_acknowledged_passes() {
        let resp = sample_challenge_response(PokeStatus::Acknowledged);
        assert!(PokeValidator::validate_challenge_response(&resp));
    }

    #[test]
    fn challenge_deferred_within_limit_passes() {
        let resp = sample_challenge_response(PokeStatus::Deferred(3));
        assert!(PokeValidator::validate_challenge_response(&resp));
    }

    #[test]
    fn challenge_deferred_exceeds_limit_fails() {
        let resp = sample_challenge_response(PokeStatus::Deferred(4));
        assert!(!PokeValidator::validate_challenge_response(&resp));
    }

    #[test]
    fn challenge_appeal_with_evidence_passes() {
        let resp = sample_challenge_response(PokeStatus::Appeal(AppealStatement {
            violation_id: "V-002".into(),
            reason: "Command was read-only".into(),
            evidence: vec!["cargo check output".into()],
        }));
        assert!(PokeValidator::validate_challenge_response(&resp));
    }

    #[test]
    fn challenge_appeal_missing_evidence_fails() {
        let resp = sample_challenge_response(PokeStatus::Appeal(AppealStatement {
            violation_id: "V-002".into(),
            reason: "Command was read-only".into(),
            evidence: vec![],
        }));
        assert!(!PokeValidator::validate_challenge_response(&resp));
    }

    #[test]
    fn challenge_appeal_empty_reason_fails() {
        let resp = sample_challenge_response(PokeStatus::Appeal(AppealStatement {
            violation_id: "V-002".into(),
            reason: "".into(),
            evidence: vec!["log.txt".into()],
        }));
        assert!(!PokeValidator::validate_challenge_response(&resp));
    }

    #[test]
    fn challenge_missing_self_check_fails() {
        let resp = PokeResponse {
            poke_id: "poke-002".into(),
            self_check: None,
            status: PokeStatus::Acknowledged,
        };
        assert!(!PokeValidator::validate_challenge_response(&resp));
    }

    // --- Serialization round-trip ---

    #[test]
    fn poke_message_round_trip() {
        let msg = PokeMessage {
            poke_id: "pm-001".into(),
            poke_type: PokeType::Audit,
            rule_ids: vec!["R1".into(), "R3".into()],
            deadline_turns: 3,
            evidence_required: Some(vec!["tool_call_log".into()]),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let deserialized: PokeMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg.poke_id, deserialized.poke_id);
        assert_eq!(msg.poke_type, deserialized.poke_type);
        assert_eq!(msg.rule_ids, deserialized.rule_ids);
        assert_eq!(msg.deadline_turns, deserialized.deadline_turns);
        assert_eq!(msg.evidence_required, deserialized.evidence_required);
    }

    #[test]
    fn poke_response_round_trip() {
        let resp = PokeResponse {
            poke_id: "pr-001".into(),
            status: PokeStatus::Appeal(AppealStatement {
                violation_id: "V-001".into(),
                reason: "test appeal".into(),
                evidence: vec!["e1".into()],
            }),
            self_check: Some(SelfCheckStatement {
                current_phase: "review".into(),
                last_gate: "approval".into(),
                tool_calls_summary: vec!["Read".into()],
                rules_checked: vec!["R2".into()],
            }),
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        let deserialized: PokeResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(resp.poke_id, deserialized.poke_id);
        assert_eq!(resp.status, deserialized.status);
    }
}

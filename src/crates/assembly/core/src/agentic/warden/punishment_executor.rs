#![cfg(feature = "taiji")]
//! PunishmentExecutor — executes penalty actions on violating sessions.
//!
//! PunishmentExecutor is a standalone session responsible for carrying out
//! penalty actions delegated by the Warden. It validates that penalty requests
//! originate from an authenticated Warden session (is_daemon=true) before
//! executing any punitive measures.
//!
//! # Penalty levels
//!
//! | Level | Actions |
//! |-------|---------|
//! | L1    | Shame-wall record + inject_context (PokePenalty reminder, <100 tokens) |
//! | L2    | RBAC demotion one level + shame-wall record + violation context |
//! | L3    | RBAC → read-only (Reviewer) + session freeze + notify user |
//! | L4    | Permanent mark (cross-session flag) |
//!
//! # Source validation
//!
//! Every [`PenaltyRequest`] must carry a `requested_by` field identifying the
//! Warden session. The executor verifies that the session exists and has
//! [`SessionConfig::is_daemon`] set to `true`. Requests from non-Warden
//! sessions are rejected.

use crate::agentic::session::session_manager::SessionManager;
use crate::agentic::tools::restrictions::{
    update_restrictions, AgentRole, OperationClass, ToolRuntimeRestrictionsPatch,
};
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_runtime_ports::AgentDialogPrependedReminder;
use std::sync::Arc;

use super::{PenaltyLevel, PenaltyRequest, ShameWallRegistry, POKE_PENALTY_KIND};

// ---------------------------------------------------------------------------
// PunishmentExecutor
// ---------------------------------------------------------------------------

/// Executor of penalty actions on violating agent sessions.
///
/// This is the server-side logic behind the PunishmentExecutor agent session.
/// It is constructed with a reference to the [`SessionManager`] so it can
/// inspect session configurations (e.g. `is_daemon`) and apply RBAC changes.
///
/// # Lifecycle
///
/// 1. A [`PenaltyRequest`] arrives (typically forwarded from the Warden via
///    the PunishmentExecutor agent session).
/// 2. [`execute_penalty`](Self::execute_penalty) validates the source,
///    dispatches by level, and returns.
/// 3. Caller (the PunishmentExecutor agent) persists the updated
///    [`ShameWallRegistry`] and delivers prepended reminders or user
///    notifications as needed.
pub struct PunishmentExecutor {
    session_manager: Arc<SessionManager>,
}

impl PunishmentExecutor {
    /// Create a new `PunishmentExecutor` with the given session manager.
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }

    /// Execute a penalty request.
    ///
    /// # Errors
    ///
    /// - Returns [`BitFunError::Validation`] if `requested_by` does not refer
    ///   to a valid Warden session (is_daemon=true).
    /// - Returns [`BitFunError::Tool`] if RBAC restriction updates fail.
    ///
    /// On success, returns a [`PenaltyOutcome`] describing what was done.
    pub async fn execute_penalty(
        &self,
        request: PenaltyRequest,
        shame_wall: &mut ShameWallRegistry,
        now: &str,
    ) -> BitFunResult<PenaltyOutcome> {
        // ── Step 1: Validate the request source ──────────────────────
        self.verify_warden_session(&request.requested_by).await?;

        // ── Step 2: Dispatch by level ────────────────────────────────
        match request.level {
            PenaltyLevel::L1 => self.execute_l1(request, shame_wall, now).await,
            PenaltyLevel::L2 => self.execute_l2(request, shame_wall, now).await,
            PenaltyLevel::L3 => self.execute_l3(request, shame_wall, now).await,
            PenaltyLevel::L4 => self.execute_l4(request, shame_wall, now).await,
        }
    }

    // ------------------------------------------------------------------
    // Source validation
    // ------------------------------------------------------------------

    /// Verify that `session_id` exists and has `is_daemon = true`.
    async fn verify_warden_session(&self, session_id: &str) -> BitFunResult<()> {
        let session = self.session_manager.get_session(session_id).ok_or_else(|| {
            BitFunError::validation(format!(
                "Penalty request rejected: requesting session '{}' not found",
                session_id
            ))
        })?;

        if !session.config.is_daemon {
            return Err(BitFunError::validation(format!(
                "Penalty request rejected: session '{}' is not a Warden (is_daemon=false)",
                session_id
            )));
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Level-specific execution
    // ------------------------------------------------------------------

    /// L1 — First minor violation.
    ///
    /// 1. Record the violation on the shame wall.
    /// 2. Produce a [`PenaltyOutcome`] with a short PokePenalty reminder
    ///    (<100 tokens) that the caller injects into the target session's
    ///    prepended_reminders.
    async fn execute_l1(
        &self,
        request: PenaltyRequest,
        shame_wall: &mut ShameWallRegistry,
        now: &str,
    ) -> BitFunResult<PenaltyOutcome> {
        // Record on shame wall
        shame_wall.upsert_entry(
            &request.target_session_id, // user_id (session-level tracking)
            "agent",
            &request.target_session_id,
            request.violations.clone(),
            PenaltyLevel::L1,
            now,
        );

        // Build a concise violation summary (<100 tokens ≈ <400 chars)
        let summary = build_violation_summary(&request.violations, 400);

        Ok(PenaltyOutcome {
            level: PenaltyLevel::L1,
            prepended_reminders: vec![AgentDialogPrependedReminder {
                kind: POKE_PENALTY_KIND.to_string(),
                text: format!(
                    "[Penalty L1] Violation recorded.\n\
                     Session: {}\n\
                     Summary: {}",
                    request.target_session_id, summary
                ),
            }],
            rbac_change: None,
            session_frozen: false,
            permanent_mark: false,
            notify_user: false,
        })
    }

    /// L2 — Second violation in the same session.
    ///
    /// 1. Demote RBAC by one level (Executor → Reviewer, Commander → Reviewer,
    ///    Reviewer stays Reviewer).
    /// 2. Record on shame wall.
    /// 3. Produce a violation-context reminder.
    async fn execute_l2(
        &self,
        request: PenaltyRequest,
        shame_wall: &mut ShameWallRegistry,
        now: &str,
    ) -> BitFunResult<PenaltyOutcome> {
        // Determine current role and demote
        let demoted_role = self.demote_role(&request.target_session_id)?;

        // Apply RBAC demotion via patch
        let patch = ToolRuntimeRestrictionsPatch::default();
        update_restrictions(&request.target_session_id, Some(demoted_role.clone()), patch)?;

        // Record on shame wall
        shame_wall.upsert_entry(
            &request.target_session_id,
            "agent",
            &request.target_session_id,
            request.violations.clone(),
            PenaltyLevel::L2,
            now,
        );

        let summary = build_violation_summary(&request.violations, 800);

        Ok(PenaltyOutcome {
            level: PenaltyLevel::L2,
            prepended_reminders: vec![AgentDialogPrependedReminder {
                kind: POKE_PENALTY_KIND.to_string(),
                text: format!(
                    "[Penalty L2] RBAC demoted to {:?}. Violation recorded.\n\
                     Session: {}\n\
                     Details: {}",
                    demoted_role, request.target_session_id, summary
                ),
            }],
            rbac_change: Some(demoted_role),
            session_frozen: false,
            permanent_mark: false,
            notify_user: false,
        })
    }

    /// L3 — ≥3 violations or severe violation.
    ///
    /// 1. Set RBAC to read-only (Reviewer).
    /// 2. Freeze the session (prevent new work turns).
    /// 3. Notify the user.
    async fn execute_l3(
        &self,
        request: PenaltyRequest,
        shame_wall: &mut ShameWallRegistry,
        now: &str,
    ) -> BitFunResult<PenaltyOutcome> {
        // Force RBAC to Reviewer (read-only)
        let patch = ToolRuntimeRestrictionsPatch::default();
        update_restrictions(&request.target_session_id, Some(AgentRole::Reviewer), patch)?;

        // Record on shame wall
        shame_wall.upsert_entry(
            &request.target_session_id,
            "agent",
            &request.target_session_id,
            request.violations.clone(),
            PenaltyLevel::L3,
            now,
        );

        let summary = build_violation_summary(&request.violations, 800);

        Ok(PenaltyOutcome {
            level: PenaltyLevel::L3,
            prepended_reminders: vec![AgentDialogPrependedReminder {
                kind: POKE_PENALTY_KIND.to_string(),
                text: format!(
                    "[Penalty L3] Session FROZEN — RBAC set to read-only.\n\
                     Session: {}\n\
                     Reason: {}\n\
                     Contact the user to resume.",
                    request.target_session_id, summary
                ),
            }],
            rbac_change: Some(AgentRole::Reviewer),
            session_frozen: true,
            permanent_mark: false,
            notify_user: true,
        })
    }

    /// L4 — Cross-session persistent violations.
    ///
    /// 1. Apply permanent mark (stored in shame-wall entry metadata).
    /// 2. Set RBAC to Reviewer (read-only) as pre-demotion.
    async fn execute_l4(
        &self,
        request: PenaltyRequest,
        shame_wall: &mut ShameWallRegistry,
        now: &str,
    ) -> BitFunResult<PenaltyOutcome> {
        // Force RBAC to Reviewer
        let patch = ToolRuntimeRestrictionsPatch::default();
        update_restrictions(&request.target_session_id, Some(AgentRole::Reviewer), patch)?;

        // Record on shame wall with L4
        shame_wall.upsert_entry(
            &request.target_session_id,
            "agent",
            &request.target_session_id,
            request.violations.clone(),
            PenaltyLevel::L4,
            now,
        );

        let summary = build_violation_summary(&request.violations, 800);

        Ok(PenaltyOutcome {
            level: PenaltyLevel::L4,
            prepended_reminders: vec![AgentDialogPrependedReminder {
                kind: POKE_PENALTY_KIND.to_string(),
                text: format!(
                    "[Penalty L4] PERMANENT MARK — session restricted to read-only.\n\
                     Session: {}\n\
                     Reason: {}\n\
                     This session has been permanently flagged for cross-session violations.",
                    request.target_session_id, summary
                ),
            }],
            rbac_change: Some(AgentRole::Reviewer),
            session_frozen: true,
            permanent_mark: true,
            notify_user: true,
        })
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Determine the demoted role for a session.
    ///
    /// Demotion ladder:
    /// - Commander → Reviewer
    /// - Executor → Reviewer
    /// - Reviewer → Reviewer (already lowest)
    /// - Warden → Reviewer (privilege reduction)
    /// - PunishmentExecutor → Reviewer
    fn demote_role(&self, session_id: &str) -> BitFunResult<AgentRole> {
        // Try to detect the current role from existing restrictions.
        // If no per-session restrictions exist, default to Executor → Reviewer.
        let current = crate::agentic::tools::restrictions::get_session_restrictions(session_id);

        // Map current restrictions to a role estimate.
        // When no per-session override exists, the role-default template
        // is used by the runtime. We conservatively demote to Reviewer.
        let demoted = match current {
            Some(ref r)
                if r.allowed_operation_classes.contains(&OperationClass::ExecuteCode)
                    || r.allowed_operation_classes.contains(&OperationClass::WriteFile) =>
            {
                // Was Executor (or equivalent) → Reviewer
                AgentRole::Reviewer
            }
            _ => AgentRole::Reviewer,
        };

        Ok(demoted)
    }
}

// ---------------------------------------------------------------------------
// PenaltyOutcome
// ---------------------------------------------------------------------------

/// The result of executing a penalty.
///
/// Carries the actions that the caller (the PunishmentExecutor agent session)
/// must apply to complete the penalty, such as delivering prepended reminders,
/// freezing the session, or notifying the user.
#[derive(Debug, Clone)]
pub struct PenaltyOutcome {
    /// The penalty level that was executed.
    pub level: PenaltyLevel,
    /// Prepended reminders to inject into the target session's context.
    pub prepended_reminders: Vec<AgentDialogPrependedReminder>,
    /// If `Some`, the target session's RBAC role was changed to this value.
    pub rbac_change: Option<AgentRole>,
    /// Whether the target session was frozen (suspended).
    pub session_frozen: bool,
    /// Whether a permanent mark was applied.
    pub permanent_mark: bool,
    /// Whether the user should be notified.
    pub notify_user: bool,
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Build a concise violation summary string, capped at `max_chars`.
fn build_violation_summary(violations: &[super::ViolationRecord], max_chars: usize) -> String {
    let mut parts: Vec<String> = violations
        .iter()
        .map(|v| {
            format!(
                "[{}] {}: {}",
                v.severity, v.rule_id, v.description
            )
        })
        .collect();

    // Deduplicate identical descriptions
    parts.sort();
    parts.dedup();

    let mut summary = parts.join("; ");
    if summary.len() > max_chars {
        summary.truncate(max_chars);
        summary.push_str("...");
    }

    summary
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── build_violation_summary ──────────────────────────────────────

    #[test]
    fn build_violation_summary_empty() {
        let s = build_violation_summary(&[], 100);
        assert_eq!(s, "");
    }

    #[test]
    fn build_violation_summary_single() {
        let violations = vec![super::super::ViolationRecord {
            rule_id: "R-001".into(),
            description: "Unauthorized write".into(),
            severity: "major".into(),
            timestamp: "2024-01-01T00:00:00Z".into(),
            evidence: serde_json::json!({}),
        }];
        let s = build_violation_summary(&violations, 200);
        assert!(s.contains("R-001"));
        assert!(s.contains("Unauthorized write"));
        assert!(s.contains("major"));
    }

    #[test]
    fn build_violation_summary_dedup() {
        let v = super::super::ViolationRecord {
            rule_id: "R-001".into(),
            description: "dup".into(),
            severity: "minor".into(),
            timestamp: "t1".into(),
            evidence: serde_json::json!({}),
        };
        let violations = vec![v.clone(), v];
        let s = build_violation_summary(&violations, 200);
        // After dedup, "minor" should appear only once
        assert_eq!(s.matches("minor").count(), 1);
    }

    #[test]
    fn build_violation_summary_truncation() {
        let violations = vec![super::super::ViolationRecord {
            rule_id: "R-999".into(),
            description: "A very long description that should be truncated by the character limit"
                .into(),
            severity: "critical".into(),
            timestamp: "t".into(),
            evidence: serde_json::json!({}),
        }];
        let s = build_violation_summary(&violations, 30);
        assert!(s.len() <= 33); // 30 + "..."
        assert!(s.ends_with("..."));
    }

    // ── PenaltyOutcome ───────────────────────────────────────────────

    #[test]
    fn penalty_level_l1_outcome_fields() {
        let outcome = PenaltyOutcome {
            level: PenaltyLevel::L1,
            prepended_reminders: vec![],
            rbac_change: None,
            session_frozen: false,
            permanent_mark: false,
            notify_user: false,
        };
        assert_eq!(outcome.level, PenaltyLevel::L1);
        assert!(outcome.rbac_change.is_none());
        assert!(!outcome.session_frozen);
    }

    #[test]
    fn penalty_level_l3_outcome_fields() {
        let outcome = PenaltyOutcome {
            level: PenaltyLevel::L3,
            prepended_reminders: vec![AgentDialogPrependedReminder {
                kind: POKE_PENALTY_KIND.to_string(),
                text: "test".into(),
            }],
            rbac_change: Some(AgentRole::Reviewer),
            session_frozen: true,
            permanent_mark: false,
            notify_user: true,
        };
        assert_eq!(outcome.level, PenaltyLevel::L3);
        assert_eq!(outcome.rbac_change, Some(AgentRole::Reviewer));
        assert!(outcome.session_frozen);
        assert!(outcome.notify_user);
    }
}

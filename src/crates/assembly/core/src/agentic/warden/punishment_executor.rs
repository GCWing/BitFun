//! PunishmentExecutor — records violations and reminds violating sessions.
//!
//! PunishmentExecutor is the server-side logic behind the PunishmentExecutor
//! agent session. It validates that penalty requests originate from an
//! authenticated Warden session (is_daemon=true) before recording anything.
//!
//! # R-25: reminder-only discipline (no RBAC enforcement)
//!
//! Per user ruling R-25, punitive RBAC operations are fully removed. Penalty
//! execution no longer demotes roles, no longer writes read-only restriction
//! patches, and no longer freezes sessions. Every level now does exactly two
//! things:
//!
//! 1. Records the violation on the shame wall (audit trail).
//! 2. Produces a PokePenalty reminder injected into the target session's
//!    prepended_reminders (mechanism-level hook reminder).
//!
//! | Level | Actions |
//! |-------|---------|
//! | L1    | Shame-wall record + reminder (<100 tokens) |
//! | L2    | Shame-wall record + violation-context reminder |
//! | L3    | Shame-wall record + escalation reminder |
//! | L4    | Shame-wall record + permanent-violation reminder |
//!
//! The ViolationPolicy escalation ladder (L1 → L2 → L3) still advances in
//! [`super::runtime::WardenRuntime`], but escalation only changes the reminder
//! text — it never touches `SESSION_RESTRICTIONS`, `SESSION_ROLES`, or any
//! freeze flag.
//!
//! # Source validation
//!
//! Every [`PenaltyRequest`] must carry a `requested_by` field identifying the
//! Warden session. The executor verifies that the session exists and has
//! [`SessionConfig::is_daemon`] set to `true`. Requests from non-Warden
//! sessions are rejected.

use crate::agentic::session::session_manager::SessionManager;
use crate::agentic::tools::restrictions::AgentRole;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_runtime_ports::AgentDialogPrependedReminder;
use std::sync::Arc;

use super::{PenaltyLevel, PenaltyRequest, ShameWallRegistry, POKE_PENALTY_KIND};

#[cfg(test)]
use uuid::Uuid;

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
    ///
    /// The in-process scheduler-embedded runtime ([`super::WARDEN_RUNTIME_SESSION`])
    /// short-circuits the daemon check: it is an internal source that performs
    /// the same Warden role without owning a daemon session.
    async fn verify_warden_session(&self, session_id: &str) -> BitFunResult<()> {
        if session_id == super::WARDEN_RUNTIME_SESSION {
            return Ok(());
        }

        let session = self
            .session_manager
            .get_session(session_id)
            .ok_or_else(|| {
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
    /// R-25: reminder-only. No RBAC demotion is applied; the violation is
    /// recorded on the shame wall and a violation-context reminder is
    /// produced for the target session.
    async fn execute_l2(
        &self,
        request: PenaltyRequest,
        shame_wall: &mut ShameWallRegistry,
        now: &str,
    ) -> BitFunResult<PenaltyOutcome> {
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
                    "[Penalty L2] Violation recorded — repeated rule breach. No RBAC change.\n\
                     Session: {}\n\
                     Details: {}",
                    request.target_session_id, summary
                ),
            }],
            rbac_change: None,
            session_frozen: false,
            permanent_mark: false,
            notify_user: false,
        })
    }

    /// L3 — ≥3 violations or severe violation.
    ///
    /// R-25: reminder-only. No read-only patch and no session freeze are
    /// applied; the violation is recorded on the shame wall and an escalation
    /// reminder is produced for the target session.
    async fn execute_l3(
        &self,
        request: PenaltyRequest,
        shame_wall: &mut ShameWallRegistry,
        now: &str,
    ) -> BitFunResult<PenaltyOutcome> {
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
                    "[Penalty L3] Violation recorded — escalation level reached. No RBAC change.\n\
                     Session: {}\n\
                     Reason: {}\n\
                     Please self-correct on the next turn.",
                    request.target_session_id, summary
                ),
            }],
            rbac_change: None,
            session_frozen: false,
            permanent_mark: false,
            // WARDEN-10: advisory escalation flag — the runtime surfaces L3
            // awareness through the observability warn channel, not a UI push.
            notify_user: true,
        })
    }

    /// L4 — Cross-session persistent violations.
    ///
    /// R-25: reminder-only. No read-only patch and no permanent restriction
    /// are applied; the violation is recorded on the shame wall (retaining
    /// the L4 escalation level as a historical audit fact) and a
    /// permanent-violation reminder is produced.
    async fn execute_l4(
        &self,
        request: PenaltyRequest,
        shame_wall: &mut ShameWallRegistry,
        now: &str,
    ) -> BitFunResult<PenaltyOutcome> {
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
                    "[Penalty L4] PERMANENT VIOLATION recorded — no RBAC change.\n\
                     Session: {}\n\
                     Reason: {}\n\
                     This session has accumulated cross-session violations; please self-correct.",
                    request.target_session_id, summary
                ),
            }],
            rbac_change: None,
            session_frozen: false,
            permanent_mark: false,
            // WARDEN-10: advisory escalation flag — the runtime surfaces L4
            // awareness through the observability warn channel, not a UI push.
            notify_user: true,
        })
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    // NOTE (R-25): the demotion helpers (demote_role, demote_role_for_session,
    // infer_role_from_restrictions, demote_agent_role) were removed together
    // with the RBAC demotion operation. Penalties are reminder-only now.
}

// ---------------------------------------------------------------------------
// PenaltyOutcome
// ---------------------------------------------------------------------------

/// The result of executing a penalty.
///
/// Carries the actions that the caller (the Warden runtime) must apply to
/// complete the penalty, such as delivering prepended reminders.
///
/// # R-25
///
/// Punitive RBAC fields are retained for API stability but are always inert:
/// `rbac_change` is always `None`, `session_frozen` and `permanent_mark` are
/// always `false`. No caller applies RBAC changes or freezes based on them.
#[derive(Debug, Clone)]
pub struct PenaltyOutcome {
    /// The penalty level that was executed.
    pub level: PenaltyLevel,
    /// Prepended reminders to inject into the target session's context.
    pub prepended_reminders: Vec<AgentDialogPrependedReminder>,
    /// Always `None` since R-25: penalties never change the RBAC role.
    pub rbac_change: Option<AgentRole>,
    /// Always `false` since R-25: penalties never freeze sessions.
    pub session_frozen: bool,
    /// Always `false` since R-25: penalties never apply permanent marks.
    pub permanent_mark: bool,
    /// Whether the user should be notified of an escalation.
    ///
    /// WARDEN-10: this flag is advisory-only. The core has no direct UI
    /// channel, so the runtime consumes it as an observability signal —
    /// an L3/L4 escalation that needs user awareness is surfaced through the
    /// warn-level log in `WardenRuntime`, not a delivered push notification.
    /// Callers must not treat `true` as proof that a user-facing message was
    /// shown.
    pub notify_user: bool,
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Build a concise violation summary string, capped at `max_chars`.
fn build_violation_summary(violations: &[super::ViolationRecord], max_chars: usize) -> String {
    let mut parts: Vec<String> = violations
        .iter()
        .map(|v| format!("[{}] {}: {}", v.severity, v.rule_id, v.description))
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
    use std::time::Duration;

    fn test_session_manager() -> Arc<SessionManager> {
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::{
            PromptCachePolicy, SessionContextStore, SessionManagerConfig,
        };
        use crate::infrastructure::app_paths::PathManager;

        let root = std::env::temp_dir().join(format!("bitfun-punisher-test-{}", Uuid::new_v4()));
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(root.join("user-root")));
        let persistence_manager =
            Arc::new(PersistenceManager::new(path_manager).expect("persistence manager"));
        Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            persistence_manager,
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ))
    }

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
            rbac_change: None,
            session_frozen: false,
            permanent_mark: false,
            notify_user: true,
        };
        assert_eq!(outcome.level, PenaltyLevel::L3);
        assert!(outcome.rbac_change.is_none(), "R-25: no RBAC change");
        assert!(!outcome.session_frozen, "R-25: no session freeze");
        assert!(!outcome.permanent_mark, "R-25: no permanent mark");
        assert!(outcome.notify_user);
    }

    // ── R-25: reminder-only execution (no RBAC enforcement) ─────────

    #[tokio::test]
    async fn execute_l2_records_and_reminds_without_rbac_change() {
        let executor = PunishmentExecutor::new(test_session_manager());
        let mut shame_wall = ShameWallRegistry::default();
        let now = "2026-08-02T10:00:00Z";
        let request = PenaltyRequest {
            target_session_id: "test-r25-l2".into(),
            level: PenaltyLevel::L2,
            violations: vec![super::super::ViolationRecord {
                rule_id: "R-001".into(),
                description: "repeated violation".into(),
                severity: "major".into(),
                timestamp: now.into(),
                evidence: serde_json::json!({}),
            }],
            requested_by: super::super::WARDEN_RUNTIME_SESSION.into(),
        };

        let outcome = executor
            .execute_penalty(request.clone(), &mut shame_wall, now)
            .await
            .expect("penalty execution succeeds");

        assert_eq!(outcome.level, PenaltyLevel::L2);
        assert!(outcome.rbac_change.is_none(), "R-25: L2 must not demote");
        assert!(!outcome.session_frozen);
        assert_eq!(outcome.prepended_reminders.len(), 1);
        assert!(outcome.prepended_reminders[0].text.contains("No RBAC change"));
        let entry = shame_wall.entry_for_session("test-r25-l2").expect("recorded");
        assert_eq!(entry.cumulative_penalty_level, PenaltyLevel::L2);
    }

    #[tokio::test]
    async fn execute_l3_and_l4_record_and_remind_without_rbac_change() {
        let executor = PunishmentExecutor::new(test_session_manager());
        let now = "2026-08-02T10:00:00Z";

        for level in [PenaltyLevel::L3, PenaltyLevel::L4] {
            let mut shame_wall = ShameWallRegistry::default();
            let session = format!("test-r25-{:?}", level);
            let request = PenaltyRequest {
                target_session_id: session.clone(),
                level: level.clone(),
                violations: vec![super::super::ViolationRecord {
                    rule_id: "R-002".into(),
                    description: "escalated violation".into(),
                    severity: "critical".into(),
                    timestamp: now.into(),
                    evidence: serde_json::json!({}),
                }],
                requested_by: super::super::WARDEN_RUNTIME_SESSION.into(),
            };

            let outcome = executor
                .execute_penalty(request, &mut shame_wall, now)
                .await
                .expect("penalty execution succeeds");

            assert_eq!(outcome.level, level);
            assert!(outcome.rbac_change.is_none(), "{level:?} must not change RBAC");
            assert!(!outcome.session_frozen, "{level:?} must not freeze");
            assert!(!outcome.permanent_mark, "{level:?} must not mark permanently");
            assert!(!outcome.prepended_reminders.is_empty());
            let entry = shame_wall.entry_for_session(&session).expect("recorded");
            assert_eq!(entry.cumulative_penalty_level, level);
        }
    }
}

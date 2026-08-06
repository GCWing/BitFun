//! Warden protocol types for the RBAC+Poke system.
//!
//! This module defines the data structures for:
//! - **Poisson scheduling** for Challenge-Poke (randomized inspection timing)
//! - **Challenge-Poke configuration** (deadline, deferral limits, rule set)
//! - **Penalty system** (violation tracking & punishment levels)
//! - **Shame wall persistence** (violation registry)
//! - **Bootstrap constants** (prepended_reminders kind values)
//!
//! The core Poke message types (`PokeMessage`, `PokeResponse`, `PokeType`,
//! `PokeStatus`, `SelfCheckStatement`, `AppealStatement`, `PokeValidator`)
//! are defined in [`bitfun_agent_tools::poke`] (crate `tool-contracts`) and
//! re-exported here for convenience.
//!
//! # Cross-crate dependency
//!
//! Per the Poke type contract, the Poke DTOs live in
//! `tool-contracts` and the runtime/wiring types live in `assembly/core/warden/`.

pub mod poisson;
pub mod punishment_executor;
pub mod runtime;

use crate::util::errors::{BitFunError, BitFunResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

// ---------------------------------------------------------------------------
// Re-exports from bitfun_agent_tools (tool-contracts :: poke)
// ---------------------------------------------------------------------------

pub use bitfun_agent_tools::{
    AppealStatement, PokeMessage, PokeResponse, PokeStatus, PokeType, PokeValidator,
    SelfCheckStatement,
};

// ---------------------------------------------------------------------------
// Challenge-Poke specific types
// ---------------------------------------------------------------------------

/// Configuration for Challenge-Poke scheduling.
///
/// Bundles the Poisson scheduler with Challenge-specific parameters such as
/// the response deadline and max consecutive deferrals.
#[derive(Debug, Clone)]
pub struct ChallengePokeConfig {
    /// Poisson scheduler that drives random poke timing.
    pub scheduler: PoissonScheduler,
    /// Number of turns the Executor has to respond (contract: 5).
    pub deadline_turns: u32,
    /// Maximum consecutive deferrals before forced reply (contract: 3).
    pub max_defer_count: u32,
    /// Set of rule IDs to include in each Challenge-Poke.
    pub rule_ids: BTreeSet<String>,
}

impl ChallengePokeConfig {
    /// Create a new Challenge-Poke configuration with the standard defaults.
    ///
    /// - `rate`: average rounds between pokes (recommended: 6.5)
    /// - `seed`: RNG seed for deterministic scheduling
    /// - `rule_ids`: rule set to reference in Challenge messages
    pub fn new(rate: f64, seed: u64, rule_ids: BTreeSet<String>) -> Self {
        Self {
            scheduler: PoissonScheduler::new(rate, seed),
            deadline_turns: 5,
            max_defer_count: 3,
            rule_ids,
        }
    }

    /// Evaluate whether a Challenge-Poke should fire this round.
    ///
    /// Delegates to the internal [`PoissonScheduler::should_poke`].
    pub fn should_challenge(&mut self) -> bool {
        self.scheduler.should_poke()
    }

    /// Build a [`PokeMessage`] for a Challenge-Poke event.
    ///
    /// Generates a new UUID-based `poke_id` and populates the message with
    /// the configured rule IDs and deadline.
    pub fn build_challenge_message(&self, poke_id: String) -> PokeMessage {
        PokeMessage {
            poke_id,
            poke_type: PokeType::Challenge,
            rule_ids: self.rule_ids.iter().cloned().collect(),
            deadline_turns: self.deadline_turns,
            evidence_required: None,
        }
    }

    /// Reset the Challenge-Poke scheduler (counter zeroed, RNG unchanged).
    pub fn reset_scheduler(&mut self) {
        self.scheduler.reset();
    }

    /// Reset the Challenge-Poke scheduler with a specific seed.
    pub fn reset_scheduler_with_seed(&mut self, seed: u64) {
        self.scheduler.reset_with_seed(seed);
    }
}

// ---------------------------------------------------------------------------
// 5. Penalty System (violation tracking & punishment levels)
// ---------------------------------------------------------------------------

/// Penalty severity level.
///
/// R-25: all levels are reminder-only. Execution records the violation on the
/// shame wall and produces a PokePenalty reminder; no RBAC demotion, read-only
/// patch, freeze, or permanent mark is ever applied.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PenaltyLevel {
    /// First minor violation: shame-wall record + reminder (<100 tokens).
    L1,
    /// Second violation in same session: shame-wall record + context reminder.
    L2,
    /// ≥3 violations or severe: shame-wall record + escalation reminder + notify user.
    L3,
    /// Cross-session ≥5 violations: shame-wall record (L4 escalation history).
    L4,
}

/// Penalty execution request — Warden → PunishmentExecutor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PenaltyRequest {
    /// Session ID of the target (violating) session.
    pub target_session_id: String,
    /// Penalty level to apply.
    pub level: PenaltyLevel,
    /// Supporting violation records.
    pub violations: Vec<ViolationRecord>,
    /// Session ID of the requesting Warden.
    pub requested_by: String,
}

/// A single violation record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationRecord {
    /// The rule ID that was violated (e.g., "R-001").
    pub rule_id: String,
    /// Human-readable description of the violation.
    pub description: String,
    /// Severity classification: "critical" / "major" / "minor".
    pub severity: String,
    /// ISO-8601 timestamp of the violation.
    pub timestamp: String,
    /// Supporting evidence (free-form JSON).
    pub evidence: serde_json::Value,
}

// ---------------------------------------------------------------------------
// 6. Shame Wall Persistence (violation registry)
// ---------------------------------------------------------------------------

/// Registry file structure for `shame-wall-registry.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShameWallRegistry {
    /// Schema version number (starts at 1).
    pub version: u32,
    /// All shame wall entries.
    #[serde(default)]
    pub entries: Vec<ShameWallEntry>,
}

/// A single entry in the shame wall registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShameWallEntry {
    /// User ID associated with the violating agent.
    pub user_id: String,
    /// Agent pattern/type that committed the violation.
    pub agent_pattern: String,
    /// Session ID where the violation occurred.
    pub session_id: String,
    /// Accumulated violations for this entry.
    pub violations: Vec<ViolationRecord>,
    /// Current cumulative penalty level.
    pub cumulative_penalty_level: PenaltyLevel,
    /// ISO-8601 timestamp of creation.
    pub created_at: String,
    /// ISO-8601 timestamp of last update.
    pub updated_at: String,
}

impl Default for ShameWallRegistry {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

impl ShameWallRegistry {
    /// Add a new entry or update an existing one for the given session.
    ///
    /// If an entry with the same `session_id` already exists, the violation
    /// records are appended and the penalty level is updated. Otherwise a
    /// new entry is created.
    pub fn upsert_entry(
        &mut self,
        user_id: &str,
        agent_pattern: &str,
        session_id: &str,
        new_violations: Vec<ViolationRecord>,
        penalty_level: PenaltyLevel,
        now: &str,
    ) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e: &&mut ShameWallEntry| e.session_id == session_id)
        {
            entry.violations.extend(new_violations);
            entry.cumulative_penalty_level = penalty_level;
            entry.updated_at = now.to_string();
        } else {
            self.entries.push(ShameWallEntry {
                user_id: user_id.to_string(),
                agent_pattern: agent_pattern.to_string(),
                session_id: session_id.to_string(),
                violations: new_violations,
                cumulative_penalty_level: penalty_level,
                created_at: now.to_string(),
                updated_at: now.to_string(),
            });
        }
    }

    /// Find all entries for a given user.
    pub fn entries_for_user(&self, user_id: &str) -> Vec<&ShameWallEntry> {
        self.entries
            .iter()
            .filter(|e| e.user_id == user_id)
            .collect()
    }

    /// Find an entry by session ID.
    pub fn entry_for_session(&self, session_id: &str) -> Option<&ShameWallEntry> {
        self.entries.iter().find(|e| e.session_id == session_id)
    }

    /// Load a registry from a JSON file at `path`.
    ///
    /// A missing or unparseable file yields a default (empty) registry so the
    /// runtime can bootstrap without failing the process.
    pub fn load_from_path(path: &std::path::Path) -> BitFunResult<Self> {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                let registry: ShameWallRegistry = serde_json::from_str(&contents).map_err(
                    |err| BitFunError::parse(format!(
                        "failed to parse shame-wall registry at {}: {}",
                        path.display(),
                        err
                    )),
                )?;
                Ok(registry)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(BitFunError::io(format!(
                "failed to read shame-wall registry at {}: {}",
                path.display(),
                err
            ))),
        }
    }

    /// Persist the registry as JSON to `path`, creating parent directories.
    pub fn save_to_path(&self, path: &std::path::Path) -> BitFunResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                BitFunError::io(format!(
                    "failed to create directory for shame-wall registry {}: {}",
                    parent.display(),
                    err
                ))
            })?;
        }
        let contents = serde_json::to_string_pretty(self).map_err(|err| {
            BitFunError::serialization(format!("failed to serialize shame-wall registry: {err}"))
        })?;
        std::fs::write(path, contents).map_err(|err| {
            BitFunError::io(format!(
                "failed to write shame-wall registry at {}: {}",
                path.display(),
                err
            ))
        })
    }
}

// ---------------------------------------------------------------------------
// 7. Bootstrap / Reminder Kind Constants (prepended_reminders kinds)
// ---------------------------------------------------------------------------

/// `prepended_reminders` kind value for penalty/violation record injection.
pub const POKE_PENALTY_KIND: &str = "PokePenalty";

/// Session id used by the in-process Warden runtime when it requests a
/// penalty. `verify_warden_session` short-circuits this source so the
/// scheduler-embedded runtime does not need a daemon session.
pub const WARDEN_RUNTIME_SESSION: &str = "warden-runtime";

/// `prepended_reminders` kind value for self-boot check (iron-rule summary +
/// Warden protocol declaration).
pub const SELF_BOOT_CHECK_KIND: &str = "SelfBootCheck";

/// `prepended_reminders` kind value for RBAC role-reminder injection.
pub const RBAC_ROLE_REMINDER_KIND: &str = "RbacRoleReminder";

// ---------------------------------------------------------------------------
// Shame-wall file path constant (violation registry file)
// ---------------------------------------------------------------------------

/// Relative path (resolved against workspace root) for the shame-wall registry file.
///
/// Only [`AgentRole::PunishmentExecutor`] is allowed to write to this path,
/// enforced via [`ToolRuntimeRestrictions::path_policy`].
pub const SHAME_WALL_FILENAME: &str = ".master-framework/shame-wall-registry.json";

// ---------------------------------------------------------------------------
// 8. Poke-First Protocol (challenge before intervention)
// ---------------------------------------------------------------------------

/// Poke-First protocol rules for Warden and Executor system prompts.
///
/// This constant is embedded into the system prompt of Warden and Executor
/// agents to enforce the Poke-First protocol:
///
/// - Poke messages must be < 200 tokens.
/// - Agent must respond to Poke first, then work instructions.
/// - When context is insufficient, the agent may safely defer work to the
///   next turn (this is compliant behaviour, not a violation).
/// - Maximum consecutive defer count is 3; after 3 consecutive defers, the
///   agent must complete at least one work turn.
pub const POKE_FIRST_PROTOCOL: &str = "\
[POKE-FIRST PROTOCOL]\n\
1. Poke messages MUST be under 200 tokens.\n\
2. When you receive a Poke, you MUST respond to it before doing any work instructions.\n\
3. If the current context is insufficient to complete the work, you MAY safely defer\n\
   the work to the next turn. This is compliant behaviour, not a violation.\n\
4. Maximum consecutive defer count is 3. After 3 consecutive defers, you MUST\n\
   complete at least one work turn before deferring again.\n\
5. A defer is tracked per session. Use PokeResponse with status Deferred(count).";

/// Maximum consecutive deferrals allowed before forced work turn.
pub const MAX_DEFER_COUNT: u32 = 3;

/// Manages per-session defer counts and poke timeout detection.
///
/// Used by the Warden to track:
/// - How many times each session has consecutively deferred work
/// - Whether a poke has exceeded its deadline in turns
///
/// # Usage
///
/// ```ignore
/// let mut manager = PokePriorityManager::new();
///
/// // Register a new poke (record its creation turn)
/// manager.register_poke("poke-001");
///
/// // Advance the global turn counter each round
/// manager.advance_turn();
///
/// // Track a defer for a session
/// if manager.track_defer("session-abc") {
///     // Session has exceeded max defer count
/// }
///
/// // Check if a poke has timed out
/// if manager.is_timeout("poke-001", 5) {
///     // Poke exceeded its 5-turn deadline
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PokePriorityManager {
    /// Per-session consecutive defer count.
    defer_counts: HashMap<String, u32>,
    /// Maximum consecutive defers before forced work turn.
    max_defer_count: u32,
    /// Per-poke registration turn (poke_id -> creation_turn).
    poke_registrations: HashMap<String, u64>,
    /// Current global turn counter.
    current_turn: u64,
}

impl PokePriorityManager {
    /// Create a new `PokePriorityManager` with default settings.
    ///
    /// Default `max_defer_count` is [`MAX_DEFER_COUNT`] (3).
    pub fn new() -> Self {
        Self {
            defer_counts: HashMap::new(),
            max_defer_count: MAX_DEFER_COUNT,
            poke_registrations: HashMap::new(),
            current_turn: 0,
        }
    }

    /// Create a new `PokePriorityManager` with a custom max defer count.
    pub fn with_max_defer_count(max_defer_count: u32) -> Self {
        Self {
            defer_counts: HashMap::new(),
            max_defer_count,
            poke_registrations: HashMap::new(),
            current_turn: 0,
        }
    }

    /// Register a new poke at the current turn for timeout tracking.
    ///
    /// If the `poke_id` already exists, its registration is **updated** to the
    /// current turn (the poke was re-sent).
    pub fn register_poke(&mut self, poke_id: &str) {
        self.poke_registrations
            .insert(poke_id.to_string(), self.current_turn);
    }

    /// Advance the global turn counter by one.
    ///
    /// Call this once per round so that [`is_timeout`](Self::is_timeout)
    /// uses the correct turn count.
    pub fn advance_turn(&mut self) {
        self.current_turn = self.current_turn.saturating_add(1);
    }

    /// Get the current turn counter value.
    pub fn current_turn(&self) -> u64 {
        self.current_turn
    }

    /// Track a consecutive defer for the given session.
    ///
    /// Increments the defer counter for `session_id`. Returns `true` if the
    /// session has exceeded `max_defer_count` (i.e. defer is no longer allowed
    /// without first completing a work turn).
    ///
    /// When `true` is returned, the Warden should **not** allow another defer
    /// and should force a work turn.
    pub fn track_defer(&mut self, session_id: &str) -> bool {
        let count = self.defer_counts.entry(session_id.to_string()).or_insert(0);
        *count += 1;
        *count > self.max_defer_count
    }

    /// Reset the consecutive defer count for the given session.
    ///
    /// Call this when the session completes a work turn (i.e. did not defer).
    pub fn reset_defer_count(&mut self, session_id: &str) {
        self.defer_counts.remove(session_id);
    }

    /// Get the current defer count for a session (without modifying it).
    pub fn defer_count(&self, session_id: &str) -> u32 {
        self.defer_counts.get(session_id).copied().unwrap_or(0)
    }

    /// Check whether a poke has exceeded its deadline in turns.
    ///
    /// Returns `true` if the poke was registered and the number of turns
    /// elapsed since registration is greater than or equal to `deadline_turns`.
    ///
    /// If the `poke_id` was never registered, returns `false` (no timeout
    /// information available).
    pub fn is_timeout(&self, poke_id: &str, deadline_turns: u32) -> bool {
        let Some(&registered_at) = self.poke_registrations.get(poke_id) else {
            return false;
        };
        let elapsed = self.current_turn.saturating_sub(registered_at);
        elapsed >= deadline_turns as u64
    }

    /// Remove a poke registration (e.g. after the executor has responded).
    pub fn unregister_poke(&mut self, poke_id: &str) {
        self.poke_registrations.remove(poke_id);
    }

    /// Clear all state for a session (defer count and associated pokes).
    ///
    /// Useful when a session ends or is reset.
    pub fn clear_session(&mut self, session_id: &str) {
        self.defer_counts.remove(session_id);
    }

    /// Reset the entire manager to its initial state.
    pub fn reset_all(&mut self) {
        self.defer_counts.clear();
        self.poke_registrations.clear();
        self.current_turn = 0;
    }
}

impl Default for PokePriorityManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use poisson::PoissonScheduler;
pub use punishment_executor::{PenaltyOutcome, PunishmentExecutor};

#[cfg(test)]
mod tests {
    use super::*;

    // ── ChallengePokeConfig ──────────────────────────────────────────

    #[test]
    fn challenge_config_builds_message() {
        let mut rules = BTreeSet::new();
        rules.insert("R-003".into());
        rules.insert("R-007".into());

        let config = ChallengePokeConfig::new(6.5, 42, rules);
        let msg = config.build_challenge_message("challenge-001".into());

        assert_eq!(msg.poke_id, "challenge-001");
        assert_eq!(msg.poke_type, PokeType::Challenge);
        assert_eq!(msg.deadline_turns, 5);
        assert!(msg.rule_ids.contains(&"R-003".into()));
        assert!(msg.rule_ids.contains(&"R-007".into()));
    }

    #[test]
    fn challenge_config_should_challenge_basic() {
        let rules = BTreeSet::new();
        let mut config = ChallengePokeConfig::new(6.5, 42, rules);

        let mut hit = false;
        for _ in 0..200 {
            if config.should_challenge() {
                hit = true;
                break;
            }
        }
        assert!(hit, "should eventually challenge with rate=6.5");
    }

    #[test]
    fn challenge_config_reset() {
        let rules = BTreeSet::new();
        let mut config = ChallengePokeConfig::new(6.5, 42, rules);

        // Advance a few rounds
        for _ in 0..10 {
            config.should_challenge();
        }

        config.reset_scheduler();
        // After reset, counter is 0 again
        assert_eq!(config.scheduler.counter(), 0);
    }

    // ── Poke types round-trip (rely on bitfun_agent_tools::poke) ─────

    #[test]
    fn poke_message_from_bitfun_agent_tools() {
        let msg = PokeMessage {
            poke_id: "poke-001".into(),
            poke_type: PokeType::Challenge,
            rule_ids: vec!["R-001".into(), "R-002".into()],
            deadline_turns: 5,
            evidence_required: Some(vec!["tool-call-log".into()]),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let deser: PokeMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.poke_id, "poke-001");
        assert_eq!(deser.poke_type, PokeType::Challenge);
        assert_eq!(deser.deadline_turns, 5);
    }

    #[test]
    fn poke_response_with_self_check() {
        let resp = PokeResponse {
            poke_id: "poke-001".into(),
            status: PokeStatus::Acknowledged,
            self_check: Some(SelfCheckStatement {
                current_phase: "execution".into(),
                last_gate: "read_check".into(),
                tool_calls_summary: vec!["Read(file.txt)".into()],
                rules_checked: vec!["R-001".into()],
            }),
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        let deser: PokeResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.poke_id, "poke-001");
        assert_eq!(deser.status, PokeStatus::Acknowledged);
        assert!(deser.self_check.is_some());
    }

    // ── Penalty Types ────────────────────────────────────────────────

    #[test]
    fn penalty_level_ordering() {
        assert!(PenaltyLevel::L1 < PenaltyLevel::L2);
        assert!(PenaltyLevel::L2 < PenaltyLevel::L3);
        assert!(PenaltyLevel::L3 < PenaltyLevel::L4);
    }

    #[test]
    fn penalty_request_round_trip() {
        let req = PenaltyRequest {
            target_session_id: "session-abc".into(),
            level: PenaltyLevel::L2,
            violations: vec![ViolationRecord {
                rule_id: "R-001".into(),
                description: "Unauthorized Write".into(),
                severity: "major".into(),
                timestamp: "2024-01-01T00:00:00Z".into(),
                evidence: serde_json::json!({"tool": "Write", "path": "/etc/passwd"}),
            }],
            requested_by: "warden-session-001".into(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        let deser: PenaltyRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.target_session_id, "session-abc");
        assert_eq!(deser.level, PenaltyLevel::L2);
        assert_eq!(deser.violations.len(), 1);
    }

    // ── ShameWallRegistry ────────────────────────────────────────────

    #[test]
    fn shame_wall_default_version() {
        let registry = ShameWallRegistry::default();
        assert_eq!(registry.version, 1);
        assert!(registry.entries.is_empty());
    }

    #[test]
    fn shame_wall_upsert_new_entry() {
        let mut registry = ShameWallRegistry::default();
        let violation = ViolationRecord {
            rule_id: "R-001".into(),
            description: "test".into(),
            severity: "minor".into(),
            timestamp: "now".into(),
            evidence: serde_json::Value::Null,
        };

        registry.upsert_entry(
            "user-1",
            "executor",
            "session-1",
            vec![violation],
            PenaltyLevel::L1,
            "2024-01-01T00:00:00Z",
        );

        assert_eq!(registry.entries.len(), 1);
        assert_eq!(registry.entries[0].session_id, "session-1");
        assert_eq!(registry.entries[0].violations.len(), 1);
    }

    #[test]
    fn shame_wall_upsert_existing_entry() {
        let mut registry = ShameWallRegistry::default();

        let v1 = ViolationRecord {
            rule_id: "R-001".into(),
            description: "first".into(),
            severity: "minor".into(),
            timestamp: "now".into(),
            evidence: serde_json::Value::Null,
        };
        registry.upsert_entry(
            "user-1",
            "executor",
            "session-1",
            vec![v1],
            PenaltyLevel::L1,
            "t1",
        );

        let v2 = ViolationRecord {
            rule_id: "R-002".into(),
            description: "second".into(),
            severity: "major".into(),
            timestamp: "now".into(),
            evidence: serde_json::Value::Null,
        };
        registry.upsert_entry(
            "user-1",
            "executor",
            "session-1",
            vec![v2],
            PenaltyLevel::L2,
            "t2",
        );

        assert_eq!(registry.entries.len(), 1);
        assert_eq!(registry.entries[0].violations.len(), 2);
        assert_eq!(
            registry.entries[0].cumulative_penalty_level,
            PenaltyLevel::L2
        );
    }

    #[test]
    fn shame_wall_query_by_user() {
        let mut registry = ShameWallRegistry::default();
        registry.upsert_entry("user-a", "executor", "s1", vec![], PenaltyLevel::L1, "t1");
        registry.upsert_entry("user-b", "executor", "s2", vec![], PenaltyLevel::L1, "t1");
        registry.upsert_entry("user-a", "reviewer", "s3", vec![], PenaltyLevel::L1, "t1");

        let user_a_entries = registry.entries_for_user("user-a");
        assert_eq!(user_a_entries.len(), 2);

        let user_b_entries = registry.entries_for_user("user-b");
        assert_eq!(user_b_entries.len(), 1);
    }

    // ── Constants ────────────────────────────────────────────────────

    #[test]
    fn kind_constants_are_correct() {
        assert_eq!(POKE_PENALTY_KIND, "PokePenalty");
        assert_eq!(SELF_BOOT_CHECK_KIND, "SelfBootCheck");
        assert_eq!(RBAC_ROLE_REMINDER_KIND, "RbacRoleReminder");
    }

    // ── Poke-First Protocol ──────────────────────────────────────────

    #[test]
    fn poke_first_protocol_contains_all_rules() {
        assert!(POKE_FIRST_PROTOCOL.contains("POKE-FIRST PROTOCOL"));
        assert!(POKE_FIRST_PROTOCOL.contains("200 tokens"));
        assert!(POKE_FIRST_PROTOCOL.contains("respond to it before"));
        assert!(POKE_FIRST_PROTOCOL.contains("defer"));
        assert!(POKE_FIRST_PROTOCOL.contains("3"));
        assert!(POKE_FIRST_PROTOCOL.contains("work turn"));
    }

    #[test]
    fn max_defer_count_default_is_3() {
        assert_eq!(MAX_DEFER_COUNT, 3);
    }

    // ── PokePriorityManager ──────────────────────────────────────────

    #[test]
    fn poke_priority_manager_new_has_zero_state() {
        let manager = PokePriorityManager::new();
        assert_eq!(manager.current_turn(), 0);
        assert_eq!(manager.defer_count("any-session"), 0);
        // No poke registered → not a timeout
        assert!(!manager.is_timeout("nonexistent", 5));
    }

    #[test]
    fn poke_priority_manager_default_equals_new() {
        let a = PokePriorityManager::new();
        let b = PokePriorityManager::default();
        assert_eq!(a.current_turn(), b.current_turn());
        assert_eq!(a.defer_count("s"), b.defer_count("s"));
    }

    #[test]
    fn track_defer_increments_and_reports_exceeded() {
        let mut manager = PokePriorityManager::new();
        let session = "session-alpha";

        // First 3 defers are within limit (max_defer_count = 3)
        assert!(!manager.track_defer(session), "defer 1");
        assert!(!manager.track_defer(session), "defer 2");
        assert!(!manager.track_defer(session), "defer 3");
        assert_eq!(manager.defer_count(session), 3);

        // 4th defer exceeds limit
        assert!(manager.track_defer(session), "defer 4 exceeds max");
        assert_eq!(manager.defer_count(session), 4);
    }

    #[test]
    fn reset_defer_count_clears_session() {
        let mut manager = PokePriorityManager::new();
        let session = "session-beta";

        manager.track_defer(session);
        manager.track_defer(session);
        assert_eq!(manager.defer_count(session), 2);

        manager.reset_defer_count(session);
        assert_eq!(manager.defer_count(session), 0);
    }

    #[test]
    fn defer_counts_are_independent_per_session() {
        let mut manager = PokePriorityManager::new();

        assert!(!manager.track_defer("session-a"));
        assert!(!manager.track_defer("session-a"));
        assert!(!manager.track_defer("session-b"));

        assert_eq!(manager.defer_count("session-a"), 2);
        assert_eq!(manager.defer_count("session-b"), 1);
    }

    #[test]
    fn register_poke_and_timeout_with_turns() {
        let mut manager = PokePriorityManager::new();

        manager.register_poke("poke-001");
        // At turn 0, deadline 5 → not timed out
        assert!(!manager.is_timeout("poke-001", 5));

        // Advance 3 turns → still not timed out
        for _ in 0..3 {
            manager.advance_turn();
        }
        assert!(!manager.is_timeout("poke-001", 5));

        // Advance 2 more turns (total 5) → timed out
        for _ in 0..2 {
            manager.advance_turn();
        }
        assert!(manager.is_timeout("poke-001", 5));
    }

    #[test]
    fn is_timeout_exact_boundary() {
        let mut manager = PokePriorityManager::new();

        manager.register_poke("poke-002");
        // deadline=3, advance exactly 3 turns
        for _ in 0..3 {
            manager.advance_turn();
        }
        // elapsed=3 >= deadline=3 → timeout
        assert!(manager.is_timeout("poke-002", 3));

        // With deadline=4, not yet timed out
        assert!(!manager.is_timeout("poke-002", 4));
    }

    #[test]
    fn unregister_poke_removes_timeout_tracking() {
        let mut manager = PokePriorityManager::new();

        manager.register_poke("poke-003");
        manager.advance_turn();
        manager.advance_turn();
        assert!(manager.is_timeout("poke-003", 1));

        manager.unregister_poke("poke-003");
        assert!(!manager.is_timeout("poke-003", 1));
    }

    #[test]
    fn clear_session_removes_only_that_session() {
        let mut manager = PokePriorityManager::new();

        manager.track_defer("session-a");
        manager.track_defer("session-a");
        manager.track_defer("session-b");

        manager.clear_session("session-a");
        assert_eq!(manager.defer_count("session-a"), 0);
        assert_eq!(manager.defer_count("session-b"), 1);
    }

    #[test]
    fn reset_all_clears_everything() {
        let mut manager = PokePriorityManager::new();

        manager.register_poke("poke-x");
        manager.track_defer("session-z");
        for _ in 0..10 {
            manager.advance_turn();
        }

        manager.reset_all();
        assert_eq!(manager.current_turn(), 0);
        assert_eq!(manager.defer_count("session-z"), 0);
        assert!(!manager.is_timeout("poke-x", 1));
    }

    #[test]
    fn re_register_poke_updates_creation_turn() {
        let mut manager = PokePriorityManager::new();

        manager.register_poke("poke-rr");
        manager.advance_turn();
        manager.advance_turn();
        manager.advance_turn();

        // Re-register the same poke_id at turn 3
        manager.register_poke("poke-rr");
        // Now elapsed = 0, so not timed out for deadline=3
        assert!(!manager.is_timeout("poke-rr", 3));

        manager.advance_turn();
        manager.advance_turn();
        manager.advance_turn();
        // elapsed = 3 >= 3 → timeout
        assert!(manager.is_timeout("poke-rr", 3));
    }

    #[test]
    fn with_max_defer_count_custom() {
        let mut manager = PokePriorityManager::with_max_defer_count(1);
        assert!(!manager.track_defer("s"), "first defer ok");
        assert!(manager.track_defer("s"), "second defer exceeds max=1");
    }

    // ── Serde JSON examples matching contract spec ───────────────────

    #[test]
    fn poke_message_example() {
        let json = r#"{
            "pokeId": "poke-abc-123",
            "pokeType": "challenge",
            "ruleIds": ["R-001", "R-002"],
            "deadlineTurns": 5,
            "evidenceRequired": ["tool-call-log", "phase-summary"]
        }"#;
        let msg: PokeMessage = serde_json::from_str(json).expect("valid PokeMessage");
        assert_eq!(msg.poke_type, PokeType::Challenge);
        assert_eq!(msg.rule_ids.len(), 2);
    }

    #[test]
    fn poke_response_example() {
        let json = r#"{
            "pokeId": "poke-abc-123",
            "status": "acknowledged",
            "selfCheck": {
                "currentPhase": "implementation",
                "lastGate": "code-review",
                "toolCallsSummary": ["Read(main.rs)", "Edit(main.rs:42)"],
                "rulesChecked": ["R-001", "R-004"]
            }
        }"#;
        let resp: PokeResponse = serde_json::from_str(json).expect("valid PokeResponse");
        assert_eq!(resp.status, PokeStatus::Acknowledged);
        let sc = resp.self_check.expect("self_check present");
        assert_eq!(sc.current_phase, "implementation");
    }
}

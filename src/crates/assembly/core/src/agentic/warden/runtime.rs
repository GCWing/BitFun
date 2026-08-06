//! WardenRuntime — mechanism-level enforcement of Warden discipline rules.
//!
//! The Warden SKILL defines what a Warden *would* do (poke, remind, record)
//! as an agent; this runtime turns those rules into hooks on the agent loop:
//!
//! - **Turn-driven** ([`WardenRuntime::on_turn_outcome`]): every turn outcome
//!   advances the poke scheduler and evaluates consecutive failures against a
//!   configurable [`ViolationPolicy`] (default L1=1, L2=2, L3=3).
//! - **Tool-driven** ([`WardenRuntime::on_tool_outcome`]): every finished tool
//!   call updates a per-session consecutive tool-failure counter; errors
//!   escalate through the same [`ViolationPolicy`] ladder (rule
//!   `warden.tool-failure`) while successes clear the counter. This is a
//!   finer-grained audit layered on top of the turn-driven one.
//! - **Violation recording (R-25)**: when the policy fires, a
//!   [`PenaltyRequest`] with source [`WARDEN_RUNTIME_SESSION`] is executed
//!   through [`PunishmentExecutor::execute_penalty`]; the violation is
//!   recorded on the shame wall and resulting reminders are queued as
//!   `PokePenalty` internal messages and delivered by the scheduler at the
//!   next turn start (see `scheduler.rs` wiring). Per user ruling R-25 the
//!   escalation ladder only changes the reminder, never RBAC state: no
//!   demotion, no read-only patch, no freeze.
//! - **Challenge-Poke**: a Poisson-driven `ChallengePoke` internal message is
//!   queued on a randomized basis (default average 6.5 turns, per SKILL 5-8).
//! - **Persistence**: when constructed with
//!   [`WardenRuntime::with_shame_wall_path`], the shame wall registry is loaded
//!   at startup and saved after every penalty.
//!
//! All thresholds are configurable; the runtime never hard-codes rules beyond
//! the defaults below.

use crate::agentic::core::{InternalReminderKind, Message};
use crate::agentic::coordination::turn_outcome::TurnOutcomeStatus;
use crate::agentic::session::SessionManager;
use crate::agentic::warden::punishment_executor::PunishmentExecutor;
use crate::agentic::warden::{
    ChallengePokeConfig, PenaltyLevel, PenaltyRequest, PokeMessage, PokePriorityManager,
    PokeType, ShameWallRegistry, ViolationRecord, WARDEN_RUNTIME_SESSION,
};
use chrono::Utc;
use log::warn;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use bitfun_runtime_ports::{ThreadGoal, WardenAuditJudgementResponse};

/// Default rule set referenced by Challenge-Poke messages.
///
/// Mirrors the Warden SKILL's "iron-rules compliance proof" requirement.
pub const DEFAULT_CHALLENGE_RULES: [&str; 1] = ["iron-rules-compliance"];

/// Classification of one finished tool call for Warden audit.
///
/// F3: admission-level rejections (stale tool catalog, deferred-tool gateway,
/// runtime restrictions) are protocol-layer outcomes, not execution
/// violations. They never contribute to the tool-failure counter or the
/// penalty ladder; only real execution failures (`ExecutionFailed`) do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WardenToolOutcome {
    /// The tool call succeeded; clears the consecutive tool-failure counter.
    Success,
    /// The tool's admission was rejected before execution (stale/deferred
    /// gate, runtime restrictions). A deliberate no-op for the failure
    /// counter: neither counted as a violation nor resetting existing counts.
    AdmissionRejected,
    /// The tool really failed during execution; counts toward the penalty
    /// ladder (rule `warden.tool-failure`).
    ExecutionFailed,
}

/// Consecutive-failure thresholds mapped to penalty levels.
///
/// Configurable so downstream callers can tighten or loosen the ladder without
/// changing the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViolationPolicy {
    /// Consecutive failures at or above which an L1 penalty fires (default 1).
    pub l1_at: u32,
    /// Consecutive failures at or above which an L2 penalty fires (default 2).
    pub l2_at: u32,
    /// Consecutive failures at or above which an L3 penalty fires (default 3).
    pub l3_at: u32,
}

impl Default for ViolationPolicy {
    fn default() -> Self {
        Self {
            l1_at: 1,
            l2_at: 2,
            l3_at: 3,
        }
    }
}

impl ViolationPolicy {
    /// Map a consecutive-failure count to the penalty level it triggers.
    ///
    /// Returns `None` when the count has not reached `l1_at` yet.
    pub fn level_for(&self, consecutive_failures: u32) -> Option<PenaltyLevel> {
        if consecutive_failures >= self.l3_at {
            Some(PenaltyLevel::L3)
        } else if consecutive_failures >= self.l2_at {
            Some(PenaltyLevel::L2)
        } else if consecutive_failures >= self.l1_at {
            Some(PenaltyLevel::L1)
        } else {
            None
        }
    }
}

/// Severity label for a violation record, matching the Warden SKILL ladder.
fn severity_for_level(level: &PenaltyLevel) -> &'static str {
    match level {
        PenaltyLevel::L1 => "minor",
        PenaltyLevel::L2 => "major",
        PenaltyLevel::L3 | PenaltyLevel::L4 => "critical",
    }
}

/// Scheduler-embedded Warden runtime.
///
/// Owns the punishment executor, shame wall registry, poke priority manager
/// and challenge scheduler, and exposes turn hooks the agent loop calls.
pub struct WardenRuntime {
    punisher: PunishmentExecutor,
    shame_wall: ShameWallRegistry,
    poke_priority: PokePriorityManager,
    challenge: ChallengePokeConfig,
    violation_policy: ViolationPolicy,
    /// Per-session consecutive turn-failure count per scene (key =
    /// `(session_id, scene_key)`; reset on Completed). **Level-1 semantics**
    /// (turn): the first failed turn of a session is an exploratory attempt
    /// and is not counted; only a repeated failure on the same scene starts
    /// the ladder.
    consecutive_failures: HashMap<(String, String), u32>,
    /// Per-session consecutive tool-failure count per scene (key =
    /// `(session_id, scene_key)`; reset on tool success), independent of the
    /// turn-level counter. **Level-2 semantics** (tool): the first failed
    /// tool call of a *scene* (tool name + argument fingerprint) is an
    /// exploratory attempt and is not counted; only a repeated failure on the
    /// same scene starts the ladder. The two levels are deliberately
    /// independent: a successful turn never resets the tool counter and a
    /// successful tool never resets the turn counter.
    tool_failures: HashMap<(String, String), u32>,
    /// Last recorded error summary per tool-failure scene (key =
    /// `(session_id, scene_key)`), kept as judgement evidence so a model
    /// Audit-Poke decision sees the actual failure context instead of a bare
    /// counter (WARDEN-03).
    last_tool_errors: HashMap<(String, String), String>,
    /// Internal messages queued for the next turn start of a session.
    pending_reminders: HashMap<String, Vec<Message>>,
    /// Optional shame-wall persistence path (aligned to the Warden SKILL's
    /// `.master-framework/shame-wall-registry.json` by default, configurable
    /// to a skill-convention path such as `L0/SHAME_WALL.md`).
    shame_wall_path: Option<PathBuf>,
}

impl WardenRuntime {
    /// Create a runtime with default policy (in-memory shame wall).
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self {
            punisher: PunishmentExecutor::new(session_manager),
            shame_wall: ShameWallRegistry::default(),
            poke_priority: PokePriorityManager::new(),
            challenge: ChallengePokeConfig::new(
                6.5,
                42,
                DEFAULT_CHALLENGE_RULES.iter().map(|s| s.to_string()).collect(),
            ),
            violation_policy: ViolationPolicy::default(),
            consecutive_failures: HashMap::new(),
            tool_failures: HashMap::new(),
            last_tool_errors: HashMap::new(),
            pending_reminders: HashMap::new(),
            shame_wall_path: None,
        }
    }

    /// Create a runtime that persists the shame wall registry to `path`.
    ///
    /// An existing registry is loaded at startup; a missing or unparseable
    /// file falls back to an empty registry (the failure is logged, not fatal).
    pub fn with_shame_wall_path(session_manager: Arc<SessionManager>, path: PathBuf) -> Self {
        let mut runtime = Self::new(session_manager);
        match ShameWallRegistry::load_from_path(&path) {
            Ok(registry) => runtime.shame_wall = registry,
            Err(err) => warn!(
                "warden runtime: falling back to empty shame wall registry at {}: {}",
                path.display(),
                err
            ),
        }
        runtime.shame_wall_path = Some(path);
        runtime
    }

    /// Replace the violation policy (thresholds for L1/L2/L3 penalties).
    pub fn set_violation_policy(&mut self, policy: ViolationPolicy) {
        self.violation_policy = policy;
    }

    /// Replace the challenge-poke configuration (rate, seed, rule set).
    pub fn set_challenge_config(&mut self, config: ChallengePokeConfig) {
        self.challenge = config;
    }

    /// Advance the global turn counter and evaluate the outcome.
    ///
    /// Called once per completed agent turn by the scheduler:
    /// - `Failed` increments the session's consecutive-failure count and, when
    ///   the policy threshold is reached, executes a penalty (L1 → L2 → L3)
    ///   and queues the penalty reminders for the next turn.
    /// - `Completed` clears the failure count and defer state.
    /// - `Cancelled` is a no-op.
    ///
    /// A Challenge-Poke may fire on any turn, independently of the outcome.
    pub async fn on_turn_outcome(
        &mut self,
        session_id: &str,
        status: TurnOutcomeStatus,
        turn_id: &str,
    ) {
        // R-26: the user-controllable RBAC/Warden master switch fully disables
        // the Warden runtime (no failure tracking, no violation records, no
        // reminders) when off.
        if !crate::service::config::rbac_enabled() {
            return;
        }

        self.poke_priority.advance_turn();

        match status {
            TurnOutcomeStatus::Failed => {
                self.handle_failed_turn(session_id, turn_id).await;
            }
            TurnOutcomeStatus::Completed => {
                self.consecutive_failures
                    .retain(|(sid, _), _| sid != session_id);
                // WARDEN-09: a completed turn also drops exploratory (count==0)
                // tool-failure placeholders so a later failure after a
                // completed turn starts a fresh exploration instead of
                // inheriting a stale zero. Real counts (>= 1) are kept: a
                // successful turn never resets an in-progress tool escalation
                // ladder (tool/turn counters stay independent).
                self.tool_failures
                    .retain(|(sid, _), count| sid != session_id || *count > 0);
                self.poke_priority.reset_defer_count(session_id);
            }
            TurnOutcomeStatus::Cancelled => {}
        }

        // Challenge-Poke fires on a Poisson schedule, outcome-independent.
        if self.challenge.should_challenge() {
            let poke = self
                .challenge
                .build_challenge_message(Uuid::new_v4().to_string());
            let text = serde_json::to_string(&poke)
                .unwrap_or_else(|_| format_challenge_fallback(&poke));
            self.push_reminder(
                session_id,
                Message::internal_reminder(InternalReminderKind::ChallengePoke, text),
            );
        }
    }

    /// Evaluate one finished tool call of a session.
    ///
    /// Called by the tool pipeline on its custom point (outside the hook
    /// dispatch channel, so `app.hooks.enabled` cannot gate it):
    /// - `ExecutionFailed` increments the consecutive tool-failure count of
    ///   the `(session_id, scene_key)` scene and, when the policy threshold is
    ///   reached, executes a penalty (L1 → L2 → L3) with rule id
    ///   `warden.tool-failure`. The first failure of a scene is an
    ///   exploratory attempt and is not counted; only a repeated failure on
    ///   the same scene starts the ladder.
    /// - `Success` clears the tool-failure count of that scene.
    /// - `AdmissionRejected` (F3: stale/deferred gate or runtime-restriction
    ///   rejections) is a protocol-layer outcome, not an execution violation:
    ///   it is a deliberate no-op — neither counted nor clearing existing
    ///   counts — so a stale-tool wave cannot fire a penalty and cannot reset
    ///   a genuine escalation ladder in progress.
    ///
    /// `scene_key` identifies the failure scene (tool name + argument
    /// fingerprint, see [`tool_failure_scene_key`]) so failures of different
    /// scenes count independently.
    ///
    /// Tool-level violations are independent of the turn-level counter; a
    /// successful turn never resets the tool counter and a successful tool
    /// never resets the turn counter. Challenge-Poke is not triggered here.
    pub async fn on_tool_outcome(
        &mut self,
        session_id: &str,
        tool_name: &str,
        scene_key: &str,
        failure_kind: WardenToolOutcome,
    ) {
        // R-26: master switch off disables tool-level Warden tracking.
        if !crate::service::config::rbac_enabled() {
            return;
        }

        match failure_kind {
            WardenToolOutcome::Success => {
                self.tool_failures
                    .remove(&(session_id.to_string(), scene_key.to_string()));
            }
            WardenToolOutcome::AdmissionRejected => {
                // Protocol-layer rejection: not an execution violation, and
                // deliberately neutral to any in-progress escalation ladder.
            }
            WardenToolOutcome::ExecutionFailed => {
                self.handle_failed_tool(session_id, tool_name, scene_key)
                    .await;
            }
        }
    }

    /// Take (and clear) the queued reminders for `session_id`.
    pub fn take_pending_reminders(&mut self, session_id: &str) -> Vec<Message> {
        self.pending_reminders.remove(session_id).unwrap_or_default()
    }

    /// Drop all per-session Warden state for `session_id` (session-end cleanup).
    ///
    /// Clears failure counters (all scenes), last-error evidence, queued
    /// reminders and poke defer state so a recycled session id cannot inherit
    /// stale enforcement state. The shame wall registry is a historical
    /// record keyed by session name and is intentionally preserved.
    pub fn cleanup_session(&mut self, session_id: &str) {
        self.clear_failure_counts(session_id);
        self.pending_reminders.remove(session_id);
        self.poke_priority.clear_session(session_id);
    }

    /// Drop only the consecutive-failure counters (turn + tool) and the
    /// last-error evidence of a session, keeping queued reminders and poke
    /// defer state.
    ///
    /// Called when a session's thread goal leaves the active state so a later
    /// goal generation starts from a clean ladder instead of inheriting the
    /// previous goal's consecutive-failure count (WARDEN-01). Idempotent.
    pub fn clear_failure_counts(&mut self, session_id: &str) {
        self.consecutive_failures
            .retain(|(sid, _), _| sid != session_id);
        self.tool_failures.retain(|(sid, _), _| sid != session_id);
        self.last_tool_errors
            .retain(|(sid, _), _| sid != session_id);
    }

    /// Current consecutive-failure count for a session (observation/test hook).
    ///
    /// With scene-scoped counting this reports the highest count across the
    /// session's scenes — the count that drives the escalation ladder.
    pub fn consecutive_failures(&self, session_id: &str) -> u32 {
        max_failure_count_for_session(&self.consecutive_failures, session_id)
    }

    /// Current consecutive tool-failure count for a session (observation/test hook).
    ///
    /// With scene-scoped counting this reports the highest count across the
    /// session's tool-failure scenes.
    pub fn tool_failures(&self, session_id: &str) -> u32 {
        max_failure_count_for_session(&self.tool_failures, session_id)
    }

    /// Current consecutive tool-failure count of a single scene
    /// (observation/test hook, and model-judgement evidence source).
    pub fn tool_failures_for_scene(&self, session_id: &str, scene_key: &str) -> u32 {
        self.tool_failures
            .get(&(session_id.to_string(), scene_key.to_string()))
            .copied()
            .unwrap_or(0)
    }

    /// Last recorded error summary of a tool-failure scene (judgement evidence).
    pub fn last_tool_error(&self, session_id: &str, scene_key: &str) -> Option<&str> {
        self.last_tool_errors
            .get(&(session_id.to_string(), scene_key.to_string()))
            .map(String::as_str)
    }

    /// Record the error summary of a failed tool call for later judgement
    /// evidence (WARDEN-03). Kept until the session is cleaned up or the goal
    /// leaves the active state ([`Self::clear_failure_counts`]).
    pub fn record_tool_error(&mut self, session_id: &str, scene_key: &str, error_summary: &str) {
        self.last_tool_errors.insert(
            (session_id.to_string(), scene_key.to_string()),
            error_summary.to_string(),
        );
    }

    /// Current shame wall registry (observation/test hook).
    pub fn shame_wall(&self) -> &ShameWallRegistry {
        &self.shame_wall
    }

    /// Current global turn counter (observation/test hook).
    pub fn current_turn(&self) -> u64 {
        self.poke_priority.current_turn()
    }

    async fn handle_failed_turn(&mut self, session_id: &str, turn_id: &str) {
        // Turn outcomes carry no phase/target facts in the current hook
        // signature, so all turn failures share the single "turn" scene. When
        // a phase/target fingerprint becomes available at the call site it can
        // be passed through without changing the counting model.
        let count =
            bump_scene_failure(&mut self.consecutive_failures, session_id, TURN_SCENE_KEY);

        let Some(level) = self.violation_policy.level_for(count) else {
            return;
        };

        self.apply_violation_penalty(
            session_id,
            "warden.consecutive-failure",
            format!(
                "turn failed (turn_id={}, scene={}, consecutive_failures={})",
                turn_id, TURN_SCENE_KEY, count
            ),
            serde_json::json!({
                "turn_id": turn_id,
                "status": TurnOutcomeStatus::Failed.as_str(),
                "scene": TURN_SCENE_KEY,
                "consecutive_failures": count,
            }),
            &level,
        )
        .await;
    }

    async fn handle_failed_tool(&mut self, session_id: &str, tool_name: &str, scene_key: &str) {
        let count = bump_scene_failure(&mut self.tool_failures, session_id, scene_key);

        let Some(level) = self.violation_policy.level_for(count) else {
            return;
        };

        self.apply_violation_penalty(
            session_id,
            "warden.tool-failure",
            format!(
                "tool failed (tool={}, scene={}, consecutive_tool_failures={})",
                tool_name, scene_key, count
            ),
            serde_json::json!({
                "tool_name": tool_name,
                "scene": scene_key,
                "consecutive_tool_failures": count,
            }),
            &level,
        )
        .await;
    }

    async fn apply_violation_penalty(
        &mut self,
        session_id: &str,
        rule_id: &str,
        description: String,
        evidence: serde_json::Value,
        level: &PenaltyLevel,
    ) {
        let now = Utc::now().to_rfc3339();
        let request = PenaltyRequest {
            target_session_id: session_id.to_string(),
            level: level.clone(),
            violations: vec![ViolationRecord {
                rule_id: rule_id.to_string(),
                description,
                severity: severity_for_level(level).to_string(),
                timestamp: now.clone(),
                evidence,
            }],
            requested_by: WARDEN_RUNTIME_SESSION.to_string(),
        };

        match self
            .punisher
            .execute_penalty(request, &mut self.shame_wall, &now)
            .await
        {
            Ok(outcome) => {
                for reminder in outcome.prepended_reminders {
                    self.push_reminder(
                        session_id,
                        Message::internal_reminder(InternalReminderKind::PokePenalty, reminder.text),
                    );
                }
                // WARDEN-10: the `notify_user` flag on the outcome must not be
                // a dead field. The core has no direct UI channel, so an
                // escalation that requires user awareness (L3/L4) is delivered
                // through the observability/logging channel at warn level —
                // the same surface hosts watch for discipline escalations.
                if outcome.notify_user {
                    warn!(
                        "warden escalation delivered for user awareness: session={}, level={:?}",
                        session_id, outcome.level
                    );
                }
                if let Some(path) = &self.shame_wall_path {
                    if let Err(err) = self.shame_wall.save_to_path(path) {
                        warn!(
                            "warden runtime: failed to persist shame wall at {}: {}",
                            path.display(),
                            err
                        );
                    }
                }
            }
            Err(err) => {
                warn!(
                    "warden runtime: penalty failed for session '{}' (level={:?}): {}",
                    session_id, level, err
                );
            }
        }
    }

    fn push_reminder(&mut self, session_id: &str, message: Message) {
        self.pending_reminders
            .entry(session_id.to_string())
            .or_default()
            .push(message);
    }
}

/// Scene key shared by all turn-level failures.
///
/// The current `on_turn_outcome` signature carries no phase/target facts, so
/// turn failures deliberately form a single scene; scene-scoped counting
/// still applies (the first turn failure of a session is exploratory).
///
/// WARDEN-11: this is the **turn level** of the first-failure rule. The
/// distinct **tool level** (per scene) is documented on
/// [`WardenRuntime::on_tool_outcome`]; the two levels never reset each other.
const TURN_SCENE_KEY: &str = "turn";

/// Count one failure for a scene.
///
/// Shared by both the turn level (scene = `TURN_SCENE_KEY`) and the tool
/// level (scene = tool name + argument fingerprint). In both levels the first
/// failure of a scene is treated as an exploratory (verification) attempt and
/// is not counted; only a repeated failure on the same scene starts the
/// consecutive ladder at 1. Returns the scene's failure count after the
/// update.
fn bump_scene_failure(
    map: &mut HashMap<(String, String), u32>,
    session_id: &str,
    scene_key: &str,
) -> u32 {
    let key = (session_id.to_string(), scene_key.to_string());
    match map.get_mut(&key) {
        Some(count) => {
            *count += 1;
            *count
        }
        None => {
            map.insert(key, 0);
            0
        }
    }
}

/// Highest failure count across all scenes of a session (observation hook).
fn max_failure_count_for_session(
    map: &HashMap<(String, String), u32>,
    session_id: &str,
) -> u32 {
    map.iter()
        .filter(|((sid, _), _)| sid == session_id)
        .map(|(_, count)| *count)
        .max()
        .unwrap_or(0)
}

/// Upper bound for the summarized tool arguments sent to a model judgement.
///
/// The judgement prompt only needs the argument *shape* plus a marker that a
/// payload existed; a pathological argument must not blow the prompt budget
/// or leak large content to the model (WARDEN-08).
const WARDEN_JUDGEMENT_ARGS_MAX_CHARS: usize = 2048;

/// Argument keys whose value is treated as bulk content.
///
/// The full value is never embedded in scene fingerprints or judgement
/// prompts; only a length + deterministic hash marker is used (WARDEN-04 /
/// WARDEN-08). Conservative by design: a misclassified key only makes the
/// fingerprint slightly coarser, never leaks content.
pub(crate) fn is_content_like_key(key: &str) -> bool {
    matches!(
        key,
        "content"
            | "file_content"
            | "text"
            | "input_text"
            | "body"
            | "data"
            | "payload"
            | "code"
            | "html"
            | "script"
            | "prompt"
    )
}

/// Deterministic FNV-1a hash over the serialized value.
///
/// Stable across runs (unlike `DefaultHasher`, which is randomly seeded) so a
/// scene fingerprint computed on one run matches one computed later.
fn content_fingerprint(value: &serde_json::Value) -> u64 {
    let bytes = serde_json::to_string(value).unwrap_or_default();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Serialized length of a value (fingerprint input; 0 on a serialization
/// failure that cannot realistically happen for JSON values).
fn content_len(value: &serde_json::Value) -> usize {
    serde_json::to_string(value)
        .map(|s| s.len())
        .unwrap_or(0)
}

/// Scalar representation of a non-nested JSON value, used verbatim in the
/// scene fingerprint. Nested values (objects/arrays) return `None` and are
/// fingerprinted by length + hash instead.
fn scalar_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => Some("null".to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => None,
    }
}

/// Build the tool-failure scene key: tool name plus a structural fingerprint
/// of the effective arguments.
///
/// The fingerprint is `tool_name` + sorted argument keys + non-content
/// scalar values + length & deterministic hash for content-like and nested
/// values (WARDEN-04). Unlike a truncated serialization it cannot collapse
/// two large payloads that share a prefix into one scene, and it never
/// embeds bulk content in the key. Distinct argument shapes are distinct
/// scenes, so the first failure of a new argument shape stays exploratory
/// instead of inheriting an in-progress escalation ladder from another shape.
pub fn tool_failure_scene_key(tool_name: &str, arguments: &serde_json::Value) -> String {
    let mut parts: Vec<String> = Vec::new();
    match arguments {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for key in keys {
                let value = &map[key];
                if is_content_like_key(key) {
                    parts.push(format!(
                        "{key}=<content:{}:{:x}>",
                        content_len(value),
                        content_fingerprint(value)
                    ));
                } else if let Some(scalar) = scalar_value(value) {
                    parts.push(format!("{key}={scalar}"));
                } else {
                    parts.push(format!(
                        "{key}=<obj:{}:{:x}>",
                        content_len(value),
                        content_fingerprint(value)
                    ));
                }
            }
        }
        serde_json::Value::Array(items) => {
            parts.push(format!(
                "array=<len:{}:{}:{:x}>",
                items.len(),
                content_len(arguments),
                content_fingerprint(arguments)
            ));
        }
        serde_json::Value::Null => parts.push("null".to_string()),
        scalar => {
            if let Some(value) = scalar_value(scalar) {
                parts.push(value);
            }
        }
    }
    format!("{tool_name}:{}", parts.join("&"))
}

/// Summarize tool arguments for a model judgement request (WARDEN-08).
///
/// Content-like values are replaced by a `{ "contentLength": N }` marker and
/// the whole summary is capped, so the model sees the argument shape without
/// receiving large or sensitive payloads. Returns `None` only for a `null`
/// argument (the caller keeps `tool_args` absent in that case).
pub fn summarize_judgement_tool_args(arguments: &serde_json::Value) -> Option<serde_json::Value> {
    match arguments {
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for key in keys {
                let value = &map[key];
                if is_content_like_key(key) {
                    out.insert(
                        key.clone(),
                        serde_json::json!({ "contentLength": content_len(value) }),
                    );
                } else {
                    out.insert(key.clone(), value.clone());
                }
            }
            Some(cap_summary(serde_json::Value::Object(out)))
        }
        serde_json::Value::Null => None,
        other => Some(cap_summary(other.clone())),
    }
}

/// Cap a summarized argument value to [`WARDEN_JUDGEMENT_ARGS_MAX_CHARS`],
/// replacing an oversized payload with a length marker.
fn cap_summary(value: serde_json::Value) -> serde_json::Value {
    let serialized = serde_json::to_string(&value).unwrap_or_default();
    if serialized.len() > WARDEN_JUDGEMENT_ARGS_MAX_CHARS {
        serde_json::json!({
            "summaryLength": serialized.len(),
            "truncated": true,
        })
    } else {
        value
    }
}

/// Batch-2 goal switch: whether Warden enforcement applies for a goal lookup.
///
/// Only an explicitly active goal (including `BudgetLimited`, see
/// [`ThreadGoal::is_active`]) keeps the Warden hooks running; a missing goal
/// or a `Paused`/`Blocked`/`Complete` goal opts the session out of
/// consecutive-failure accounting and pokes.
pub fn warden_enforcement_for_goal(goal: Option<&ThreadGoal>) -> bool {
    goal.is_some_and(ThreadGoal::is_active)
}

/// Resolve the final Audit-Poke message from a model judgement verdict.
///
/// `None` means the model declined the poke (no Audit-Poke is sent). `Some`
/// carries the poke with the model-selected rule ids and requested evidence,
/// falling back to the mechanical candidates when the model returned none.
/// The poke id, type and 3-turn deadline always come from the mechanical
/// message so the audit contract stays stable across providers.
pub fn resolve_audit_poke_from_judgement(
    mechanical: &PokeMessage,
    judgement: &WardenAuditJudgementResponse,
) -> Option<PokeMessage> {
    if !judgement.should_poke {
        return None;
    }
    let rule_ids = if judgement.rule_ids.is_empty() {
        mechanical.rule_ids.clone()
    } else {
        judgement.rule_ids.clone()
    };
    let evidence_required = if judgement.evidence_requested.is_empty() {
        mechanical.evidence_required.clone()
    } else {
        Some(judgement.evidence_requested.clone())
    };
    Some(PokeMessage {
        poke_id: mechanical.poke_id.clone(),
        poke_type: PokeType::Audit,
        rule_ids,
        deadline_turns: 3,
        evidence_required,
    })
}

/// Human-readable fallback for a Challenge-Poke message (used only if JSON
/// serialization unexpectedly fails).
fn format_challenge_fallback(poke: &PokeMessage) -> String {
    format!(
        "[Challenge-Poke {}] rules={} deadline={} turns",
        poke.poke_id,
        poke.rule_ids.join(","),
        poke.deadline_turns
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::core::MessageContent;
    use bitfun_runtime_ports::ThreadGoalStatus;
    use std::collections::BTreeSet;

    fn runtime() -> WardenRuntime {
        // verify_warden_session short-circuits the warden-runtime source, so
        // no real SessionManager-backed session is required for penalties.
        WardenRuntime::new(test_session_manager())
    }

    fn test_session_manager() -> Arc<SessionManager> {
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::{
            PromptCachePolicy, SessionContextStore, SessionManagerConfig,
        };
        use crate::infrastructure::app_paths::PathManager;
        use std::time::Duration;

        let root = std::env::temp_dir().join(format!("bitfun-warden-test-{}", Uuid::new_v4()));
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

    #[test]
    fn violation_policy_default_ladder() {
        let policy = ViolationPolicy::default();
        assert_eq!(policy.level_for(0), None);
        assert_eq!(policy.level_for(1), Some(PenaltyLevel::L1));
        assert_eq!(policy.level_for(2), Some(PenaltyLevel::L2));
        assert_eq!(policy.level_for(3), Some(PenaltyLevel::L3));
        assert_eq!(policy.level_for(9), Some(PenaltyLevel::L3));
    }

    #[test]
    fn violation_policy_custom_thresholds() {
        let policy = ViolationPolicy {
            l1_at: 3,
            l2_at: 5,
            l3_at: 7,
        };
        assert_eq!(policy.level_for(2), None);
        assert_eq!(policy.level_for(3), Some(PenaltyLevel::L1));
        assert_eq!(policy.level_for(5), Some(PenaltyLevel::L2));
        assert_eq!(policy.level_for(7), Some(PenaltyLevel::L3));
    }

    #[tokio::test]
    async fn consecutive_failures_escalate_l1_l2_l3() {
        let mut rt = runtime();
        // Challenge disabled for deterministic penalty assertions.
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        // The first failure of a session is exploratory and is not counted.
        rt.on_turn_outcome("sess-a", TurnOutcomeStatus::Failed, "t0").await;
        assert_eq!(rt.consecutive_failures("sess-a"), 0);
        assert!(
            rt.take_pending_reminders("sess-a").is_empty(),
            "no penalty for the exploratory first failure"
        );

        rt.on_turn_outcome("sess-a", TurnOutcomeStatus::Failed, "t1").await;
        assert_eq!(rt.consecutive_failures("sess-a"), 1);
        let reminders = rt.take_pending_reminders("sess-a");
        assert_eq!(reminders.len(), 1, "L1 fires on the repeated failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-a").unwrap().cumulative_penalty_level,
            PenaltyLevel::L1
        );

        rt.on_turn_outcome("sess-a", TurnOutcomeStatus::Failed, "t2").await;
        assert_eq!(rt.consecutive_failures("sess-a"), 2);
        let reminders = rt.take_pending_reminders("sess-a");
        assert_eq!(reminders.len(), 1, "L2 fires on the third failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-a").unwrap().cumulative_penalty_level,
            PenaltyLevel::L2
        );

        rt.on_turn_outcome("sess-a", TurnOutcomeStatus::Failed, "t3").await;
        assert_eq!(rt.consecutive_failures("sess-a"), 3);
        let reminders = rt.take_pending_reminders("sess-a");
        assert_eq!(reminders.len(), 1, "L3 fires on the fourth failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-a").unwrap().cumulative_penalty_level,
            PenaltyLevel::L3
        );
    }

    #[tokio::test]
    async fn completed_turn_resets_failure_state() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        // Two failures: first exploratory (not counted), second fires L1.
        rt.on_turn_outcome("sess-b", TurnOutcomeStatus::Failed, "t1").await;
        assert_eq!(rt.consecutive_failures("sess-b"), 0);
        rt.on_turn_outcome("sess-b", TurnOutcomeStatus::Failed, "t2").await;
        assert_eq!(rt.consecutive_failures("sess-b"), 1);
        rt.take_pending_reminders("sess-b");

        rt.on_turn_outcome("sess-b", TurnOutcomeStatus::Completed, "t3").await;
        assert_eq!(rt.consecutive_failures("sess-b"), 0, "completed resets failures");

        // Next failure starts exploratory again: two failures reach L1.
        rt.on_turn_outcome("sess-b", TurnOutcomeStatus::Failed, "t4").await;
        assert_eq!(rt.consecutive_failures("sess-b"), 0, "first failure after reset is exploratory");
        rt.on_turn_outcome("sess-b", TurnOutcomeStatus::Failed, "t5").await;
        assert_eq!(rt.consecutive_failures("sess-b"), 1);
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-b").unwrap().cumulative_penalty_level,
            PenaltyLevel::L1
        );
    }

    #[tokio::test]
    async fn challenge_poke_fires_with_rate_one() {
        let mut rt = runtime();
        // rate=1.0 -> every turn pokes deterministically.
        rt.set_challenge_config(ChallengePokeConfig::new(
            1.0,
            7,
            BTreeSet::from(["iron-rules-compliance".to_string()]),
        ));

        rt.on_turn_outcome("sess-c", TurnOutcomeStatus::Completed, "t1").await;
        let reminders = rt.take_pending_reminders("sess-c");
        assert_eq!(reminders.len(), 1, "rate=1.0 must poke every turn");
        let MessageContent::Text(text) = &reminders[0].content else {
            panic!("challenge reminder must be a text message");
        };
        assert!(
            text.to_lowercase().contains("challenge"),
            "challenge poke must be serialized, got: {text}"
        );
    }

    #[tokio::test]
    async fn pending_reminders_take_is_destructive() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            1.0,
            7,
            BTreeSet::from(["iron-rules-compliance".to_string()]),
        ));

        rt.on_turn_outcome("sess-d", TurnOutcomeStatus::Completed, "t1").await;
        let first = rt.take_pending_reminders("sess-d");
        assert_eq!(first.len(), 1);
        let second = rt.take_pending_reminders("sess-d");
        assert!(second.is_empty(), "take clears the queue");
    }

    #[tokio::test]
    async fn cleanup_session_drops_all_per_session_state() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        // Build per-session state: failure counters, tool failures, reminders.
        rt.on_turn_outcome("sess-e", TurnOutcomeStatus::Failed, "t1").await;
        rt.on_turn_outcome("sess-e", TurnOutcomeStatus::Failed, "t2").await;
        assert_eq!(rt.consecutive_failures("sess-e"), 1);
        rt.on_tool_outcome("sess-e", "Write", "Write:{}", WardenToolOutcome::ExecutionFailed)
            .await;
        rt.on_tool_outcome("sess-e", "Write", "Write:{}", WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-e"), 1);
        // Failure paths above queue escalation reminders; drain them so the
        // count below covers only the explicit push.
        rt.take_pending_reminders("sess-e");
        rt.push_reminder(
            "sess-e",
            Message::internal_reminder(InternalReminderKind::PokePenalty, "penalty"),
        );
        assert_eq!(rt.take_pending_reminders("sess-e").len(), 1);
        // A sibling session must be untouched.
        rt.on_turn_outcome("sess-f", TurnOutcomeStatus::Failed, "t1").await;
        rt.on_turn_outcome("sess-f", TurnOutcomeStatus::Failed, "t2").await;
        assert_eq!(rt.consecutive_failures("sess-f"), 1);

        rt.cleanup_session("sess-e");
        assert_eq!(rt.consecutive_failures("sess-e"), 0, "failures cleared");
        assert_eq!(rt.tool_failures("sess-e"), 0, "tool failures cleared");
        assert!(
            rt.take_pending_reminders("sess-e").is_empty(),
            "reminders cleared"
        );
        assert_eq!(rt.consecutive_failures("sess-f"), 1, "sibling untouched");

        // Idempotent: clearing a session with no state is a no-op.
        rt.cleanup_session("sess-e");
    }

    #[tokio::test]
    async fn shame_wall_persistence_round_trip() {
        let dir = std::env::temp_dir().join(format!("warden-test-{}", Uuid::new_v4()));
        let path = dir.join("shame-wall-registry.json");

        {
            let mut rt = WardenRuntime::with_shame_wall_path(test_session_manager(), path.clone());
            rt.set_challenge_config(ChallengePokeConfig::new(
                f64::INFINITY,
                1,
                BTreeSet::new(),
            ));
            rt.on_turn_outcome("sess-e", TurnOutcomeStatus::Failed, "t1").await;
            // The first failure is exploratory; the second fires L1 and
            // persists the registry.
            rt.on_turn_outcome("sess-e", TurnOutcomeStatus::Failed, "t2").await;
            rt.take_pending_reminders("sess-e");
            assert!(path.exists(), "penalty must persist the registry");
        }

        // A second runtime loads the persisted registry.
        let rt = WardenRuntime::with_shame_wall_path(test_session_manager(), path.clone());
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-e").unwrap().cumulative_penalty_level,
            PenaltyLevel::L1,
            "loaded registry keeps the recorded penalty"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn missing_shame_wall_file_starts_empty() {
        let dir = std::env::temp_dir().join(format!("warden-test-missing-{}", Uuid::new_v4()));
        let path = dir.join("shame-wall-registry.json");
        let rt = WardenRuntime::with_shame_wall_path(test_session_manager(), path.clone());
        assert!(rt.shame_wall().entries.is_empty(), "missing file -> empty registry");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn tool_failures_escalate_l1_l2_l3() {
        let mut rt = runtime();
        // Challenge disabled for deterministic penalty assertions.
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        let scene = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "pwd"}));
        // The first failure of a scene is exploratory and is not counted.
        rt.on_tool_outcome("sess-f", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-f"), 0);
        assert!(
            rt.take_pending_reminders("sess-f").is_empty(),
            "no penalty for the exploratory first failure"
        );

        rt.on_tool_outcome("sess-f", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-f"), 1);
        let reminders = rt.take_pending_reminders("sess-f");
        assert_eq!(reminders.len(), 1, "L1 fires on the repeated failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-f").unwrap().cumulative_penalty_level,
            PenaltyLevel::L1
        );

        rt.on_tool_outcome("sess-f", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-f"), 2);
        let reminders = rt.take_pending_reminders("sess-f");
        assert_eq!(reminders.len(), 1, "L2 fires on the third failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-f").unwrap().cumulative_penalty_level,
            PenaltyLevel::L2
        );

        rt.on_tool_outcome("sess-f", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-f"), 3);
        let reminders = rt.take_pending_reminders("sess-f");
        assert_eq!(reminders.len(), 1, "L3 fires on the fourth failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-f").unwrap().cumulative_penalty_level,
            PenaltyLevel::L3
        );
    }

    #[tokio::test]
    async fn successful_tool_resets_failure_count() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        let scene = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "pwd"}));
        // Two failures: first exploratory, second fires L1.
        rt.on_tool_outcome("sess-g", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.on_tool_outcome("sess-g", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-g"), 1);
        rt.take_pending_reminders("sess-g");

        rt.on_tool_outcome("sess-g", "ExecCommand", &scene, WardenToolOutcome::Success).await;
        assert_eq!(rt.tool_failures("sess-g"), 0, "success clears tool failures");

        // Next failure starts exploratory again: two failures reach L1.
        rt.on_tool_outcome("sess-g", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(
            rt.tool_failures("sess-g"),
            0,
            "first failure after reset is exploratory"
        );
        rt.on_tool_outcome("sess-g", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-g"), 1);
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-g").unwrap().cumulative_penalty_level,
            PenaltyLevel::L1
        );
    }

    #[tokio::test]
    async fn tool_failures_independent_from_turn_failures() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        let scene = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "pwd"}));
        // Two failed turns: turn counter = 1, tool counter untouched.
        rt.on_turn_outcome("sess-h", TurnOutcomeStatus::Failed, "t1").await;
        rt.on_turn_outcome("sess-h", TurnOutcomeStatus::Failed, "t2").await;
        rt.take_pending_reminders("sess-h");
        assert_eq!(rt.consecutive_failures("sess-h"), 1);
        assert_eq!(rt.tool_failures("sess-h"), 0, "tool counter untouched by turn failure");

        // A successful tool must not reset the turn counter.
        rt.on_tool_outcome("sess-h", "ExecCommand", &scene, WardenToolOutcome::Success).await;
        assert_eq!(rt.consecutive_failures("sess-h"), 1, "turn counter unaffected by tool success");

        // Two failed tools increment only the tool counter.
        rt.on_tool_outcome("sess-h", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.on_tool_outcome("sess-h", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.take_pending_reminders("sess-h");
        assert_eq!(rt.tool_failures("sess-h"), 1);
        assert_eq!(rt.consecutive_failures("sess-h"), 1, "tool failure does not touch turn counter");
    }

    #[tokio::test]
    async fn admission_rejected_never_counts_as_tool_failure() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        rt.on_tool_outcome("sess-i", "ExecCommand", "ExecCommand:{}", WardenToolOutcome::AdmissionRejected)
            .await;
        assert_eq!(
            rt.tool_failures("sess-i"),
            0,
            "F3: admission rejection is not an execution violation"
        );
        assert!(
            rt.take_pending_reminders("sess-i").is_empty(),
            "no penalty reminder for admission rejection"
        );
        assert!(
            rt.shame_wall().entry_for_session("sess-i").is_none(),
            "no shame-wall record for admission rejection"
        );
    }

    #[tokio::test]
    async fn admission_rejected_is_neutral_to_in_progress_escalation_ladder() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        let scene = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "pwd"}));
        // Two real failures: first exploratory, second fires L1; ladder in progress.
        rt.on_tool_outcome("sess-j", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.on_tool_outcome("sess-j", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-j"), 1);
        rt.take_pending_reminders("sess-j");

        // F3: a stale/admission-rejected wave must not reset the ladder...
        rt.on_tool_outcome("sess-j", "ExecCommand", &scene, WardenToolOutcome::AdmissionRejected)
            .await;
        assert_eq!(
            rt.tool_failures("sess-j"),
            1,
            "admission rejection is a no-op, not a success"
        );

        // ...and the next real failure still escalates to L2.
        rt.on_tool_outcome("sess-j", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-j"), 2);
        let reminders = rt.take_pending_reminders("sess-j");
        assert_eq!(reminders.len(), 1, "L2 fires on the third real failure");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-j").unwrap().cumulative_penalty_level,
            PenaltyLevel::L2
        );
    }

    #[tokio::test]
    async fn first_turn_failure_of_session_is_exploratory_not_counted() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        rt.on_turn_outcome("sess-k", TurnOutcomeStatus::Failed, "t1").await;
        assert_eq!(
            rt.consecutive_failures("sess-k"),
            0,
            "first turn failure of a session is exploratory"
        );
        assert!(
            rt.take_pending_reminders("sess-k").is_empty(),
            "no penalty for the exploratory first turn failure"
        );
        assert!(
            rt.shame_wall().entry_for_session("sess-k").is_none(),
            "no shame-wall record for the exploratory first turn failure"
        );
    }

    #[tokio::test]
    async fn tool_failures_on_different_scenes_count_independently() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        let scene_a = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "pwd"}));
        let scene_b = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "ls"}));
        assert_ne!(scene_a, scene_b, "different arguments must be different scenes");

        // Two failures on scene A: first exploratory, second counted.
        rt.on_tool_outcome("sess-l", "ExecCommand", &scene_a, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.on_tool_outcome("sess-l", "ExecCommand", &scene_a, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-l"), 1, "scene A ladder at 1");
        rt.take_pending_reminders("sess-l");

        // A first failure on scene B stays exploratory and must not touch A.
        rt.on_tool_outcome("sess-l", "ExecCommand", &scene_b, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-l"), 1, "scene B exploratory, A ladder unchanged");
        assert!(
            rt.take_pending_reminders("sess-l").is_empty(),
            "no penalty for the exploratory scene-B failure"
        );

        // The second scene-B failure starts its own ladder at 1 (fires L1).
        rt.on_tool_outcome("sess-l", "ExecCommand", &scene_b, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-l"), 1, "scene B ladder at 1");
        rt.take_pending_reminders("sess-l");

        // Scene A keeps escalating independently.
        rt.on_tool_outcome("sess-l", "ExecCommand", &scene_a, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-l"), 2, "scene A ladder escalates to 2");
        let reminders = rt.take_pending_reminders("sess-l");
        assert_eq!(reminders.len(), 1, "scene A third failure fires L2");
        assert_eq!(
            rt.shame_wall().entry_for_session("sess-l").unwrap().cumulative_penalty_level,
            PenaltyLevel::L2
        );
    }

    #[test]
    fn tool_failure_scene_key_fingerprints_tool_and_arguments() {
        let args = serde_json::json!({"file_path": "a.md", "content": "x"});
        assert_eq!(
            tool_failure_scene_key("Write", &args),
            tool_failure_scene_key("Write", &args),
            "same tool + same arguments -> same scene"
        );
        assert_ne!(
            tool_failure_scene_key("Write", &args),
            tool_failure_scene_key("Read", &args),
            "different tool -> different scene"
        );
        assert_ne!(
            tool_failure_scene_key("Write", &args),
            tool_failure_scene_key("Write", &serde_json::json!({"file_path": "b.md"})),
            "different arguments -> different scene"
        );
        assert_ne!(
            tool_failure_scene_key("Write", &serde_json::Value::Null),
            tool_failure_scene_key("Write", &serde_json::json!({})),
            "null vs empty object are distinct argument shapes"
        );
    }

    #[tokio::test]
    async fn cleanup_session_clears_all_scenes() {
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        let scene_a = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "pwd"}));
        let scene_b = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "ls"}));
        // Build two independent tool scenes plus a turn scene.
        rt.on_tool_outcome("sess-m", "ExecCommand", &scene_a, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.on_tool_outcome("sess-m", "ExecCommand", &scene_a, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.on_tool_outcome("sess-m", "ExecCommand", &scene_b, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.on_tool_outcome("sess-m", "ExecCommand", &scene_b, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures("sess-m"), 1);
        rt.on_turn_outcome("sess-m", TurnOutcomeStatus::Failed, "t1").await;
        rt.on_turn_outcome("sess-m", TurnOutcomeStatus::Failed, "t2").await;
        assert_eq!(rt.consecutive_failures("sess-m"), 1);
        rt.take_pending_reminders("sess-m");

        rt.cleanup_session("sess-m");
        assert_eq!(rt.consecutive_failures("sess-m"), 0, "all turn scenes cleared");
        assert_eq!(rt.tool_failures("sess-m"), 0, "all tool scenes cleared");
    }

    #[test]
    fn warden_enforcement_applies_only_for_active_goal() {
        let goal = |status| Some(ThreadGoal {
            goal_id: "g1".to_string(),
            session_id: "s1".to_string(),
            objective: "ship".to_string(),
            status,
            token_budget: None,
            tokens_used: 0,
            time_used_seconds: 0,
            created_at: 1,
            updated_at: 2,
            auto_continuation_count: 0,
            reference_files: Vec::new(),
        });

        assert!(
            warden_enforcement_for_goal(goal(ThreadGoalStatus::Active).as_ref()),
            "active goal keeps Warden enforcement"
        );
        assert!(
            warden_enforcement_for_goal(goal(ThreadGoalStatus::BudgetLimited).as_ref()),
            "budget-limited goal is still active"
        );
        assert!(
            !warden_enforcement_for_goal(goal(ThreadGoalStatus::Paused).as_ref()),
            "paused goal opts out"
        );
        assert!(
            !warden_enforcement_for_goal(goal(ThreadGoalStatus::Blocked).as_ref()),
            "blocked goal opts out"
        );
        assert!(
            !warden_enforcement_for_goal(goal(ThreadGoalStatus::UsageLimited).as_ref()),
            "usage-limited goal opts out"
        );
        assert!(
            !warden_enforcement_for_goal(goal(ThreadGoalStatus::Complete).as_ref()),
            "complete goal opts out"
        );
        assert!(
            !warden_enforcement_for_goal(None),
            "goal-less session opts out"
        );
    }

    #[test]
    fn audit_poke_resolution_follows_model_verdict() {
        let mechanical = PokeMessage {
            poke_id: "audit-tool-42".to_string(),
            poke_type: PokeType::Audit,
            rule_ids: vec![
                "R1: no_destructive_write".to_string(),
                "R3: path_whitelist".to_string(),
            ],
            deadline_turns: 3,
            evidence_required: Some(vec![
                "tool_call_log".to_string(),
                "phase_summary".to_string(),
            ]),
        };

        // The model declined the poke: no Audit-Poke is sent.
        let declined = WardenAuditJudgementResponse {
            should_poke: false,
            rule_ids: Vec::new(),
            evidence_requested: Vec::new(),
        };
        assert!(resolve_audit_poke_from_judgement(&mechanical, &declined).is_none());

        // The model confirms the poke and selects its own rules/evidence.
        let confirmed = WardenAuditJudgementResponse {
            should_poke: true,
            rule_ids: vec!["R2: execution_safety".to_string()],
            evidence_requested: vec!["tool_call_log".to_string()],
        };
        let poke = resolve_audit_poke_from_judgement(&mechanical, &confirmed)
            .expect("confirmed poke is sent");
        assert_eq!(poke.poke_id, "audit-tool-42");
        assert_eq!(poke.poke_type, PokeType::Audit);
        assert_eq!(poke.deadline_turns, 3);
        assert_eq!(poke.rule_ids, vec!["R2: execution_safety"]);
        assert_eq!(
            poke.evidence_required,
            Some(vec!["tool_call_log".to_string()])
        );

        // The model confirms without rules: mechanical candidates carry over.
        let bare_confirm = WardenAuditJudgementResponse {
            should_poke: true,
            rule_ids: Vec::new(),
            evidence_requested: Vec::new(),
        };
        let poke = resolve_audit_poke_from_judgement(&mechanical, &bare_confirm)
            .expect("bare confirmation still pokes");
        assert_eq!(
            poke.rule_ids,
            vec!["R1: no_destructive_write", "R3: path_whitelist"],
            "empty model rules fall back to mechanical candidates"
        );
        assert_eq!(poke.evidence_required, mechanical.evidence_required);
    }

    #[tokio::test]
    async fn clear_failure_counts_resets_counters_across_goal_generations() {
        // WARDEN-01: when a goal leaves the active state the failure counts
        // must be dropped so a later (new) goal generation starts from a
        // clean ladder instead of inheriting the previous goal's L2/L3 count.
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        let scene = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "pwd"}));
        // Build an in-progress escalation ladder: repeated turn + tool failures.
        rt.on_turn_outcome("sess-goal", TurnOutcomeStatus::Failed, "t1").await;
        rt.on_turn_outcome("sess-goal", TurnOutcomeStatus::Failed, "t2").await;
        rt.on_turn_outcome("sess-goal", TurnOutcomeStatus::Failed, "t3").await;
        rt.on_tool_outcome("sess-goal", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.on_tool_outcome("sess-goal", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.record_tool_error("sess-goal", &scene, "boom");
        rt.take_pending_reminders("sess-goal");
        assert_eq!(rt.consecutive_failures("sess-goal"), 2);
        assert_eq!(rt.tool_failures_for_scene("sess-goal", &scene), 1);
        assert!(rt.last_tool_error("sess-goal", &scene).is_some());

        // The goal switched away: the gate calls clear_failure_counts.
        rt.clear_failure_counts("sess-goal");
        assert_eq!(rt.consecutive_failures("sess-goal"), 0, "turn count cleared");
        assert_eq!(
            rt.tool_failures_for_scene("sess-goal", &scene),
            0,
            "tool count cleared"
        );
        assert!(
            rt.last_tool_error("sess-goal", &scene).is_none(),
            "error evidence cleared"
        );

        // A sibling session is untouched.
        assert_eq!(rt.consecutive_failures("sess-other"), 0);
    }

    #[tokio::test]
    async fn completed_turn_drops_exploratory_zero_tool_failure_placeholders() {
        // WARDEN-09: an exploratory first tool failure leaves a count==0
        // placeholder; a completed turn must drop it so the next failure after
        // a completed turn starts a fresh exploration (count 0) instead of
        // inheriting the stale zero and immediately counting as a repeat.
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        let scene = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "pwd"}));
        rt.on_tool_outcome("sess-z", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures_for_scene("sess-z", &scene), 0, "exploratory");

        // A completed turn cleans the zero placeholder...
        rt.on_turn_outcome("sess-z", TurnOutcomeStatus::Completed, "t1").await;

        // ...so the next same-scene failure is again exploratory (0), and only
        // the failure after that counts toward L1.
        rt.on_tool_outcome("sess-z", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(
            rt.tool_failures_for_scene("sess-z", &scene),
            0,
            "stale zero was cleaned; a fresh exploration starts"
        );
        rt.on_tool_outcome("sess-z", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        assert_eq!(rt.tool_failures_for_scene("sess-z", &scene), 1);
        rt.take_pending_reminders("sess-z");
    }

    #[tokio::test]
    async fn completed_turn_keeps_in_progress_tool_escalation_ladder() {
        // The WARDEN-09 cleanup must not reset a real (>=1) tool ladder: tool
        // and turn counters stay independent.
        let mut rt = runtime();
        rt.set_challenge_config(ChallengePokeConfig::new(
            f64::INFINITY,
            1,
            BTreeSet::new(),
        ));

        let scene = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "pwd"}));
        rt.on_tool_outcome("sess-z", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.on_tool_outcome("sess-z", "ExecCommand", &scene, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.take_pending_reminders("sess-z");
        assert_eq!(rt.tool_failures_for_scene("sess-z", &scene), 1);

        rt.on_turn_outcome("sess-z", TurnOutcomeStatus::Completed, "t1").await;
        assert_eq!(
            rt.tool_failures_for_scene("sess-z", &scene),
            1,
            "a completed turn never resets a real tool escalation ladder"
        );
    }

    #[test]
    fn tool_failure_scene_key_hashes_large_content_instead_of_truncating() {
        // WARDEN-04: two large payloads sharing a 256-char prefix must remain
        // distinct scenes (the old truncation collapsed them), and the content
        // itself must never be embedded in the key.
        let big_a = "a".repeat(1024);
        let big_b = format!("{}b", "a".repeat(1023));
        assert_eq!(big_a.len(), 1024);
        assert_eq!(big_b.len(), 1024);
        assert_eq!(
            &big_a[..256],
            &big_b[..256],
            "fixture: identical 256-char prefixes"
        );

        let scene_a = tool_failure_scene_key("Write", &serde_json::json!({ "content": big_a }));
        let scene_b = tool_failure_scene_key("Write", &serde_json::json!({ "content": big_b }));
        assert_ne!(
            scene_a, scene_b,
            "large contents with a shared prefix must not merge into one scene"
        );
        assert!(
            !scene_a.contains(&big_a) && !scene_b.contains(&big_b),
            "bulk content must not be embedded in the scene key"
        );
        assert!(
            scene_a.len() < 200,
            "scene key stays compact: {}",
            scene_a.len()
        );
    }

    #[test]
    fn summarize_judgement_tool_args_masks_content_and_caps_size() {
        // WARDEN-08: content-like args are masked to a length marker and the
        // summary is capped; scalar/nested shapes are preserved.
        let small = summarize_judgement_tool_args(&serde_json::json!({
            "file_path": "a.md",
            "content": "hello",
        }))
        .expect("object args summarize to some value");
        assert_eq!(small["file_path"], "a.md");
        assert_eq!(
            small["content"]["contentLength"],
            serde_json::json!(7)
        );
        assert!(!small.to_string().contains("hello"), "content masked");

        let huge = summarize_judgement_tool_args(&serde_json::json!({
            "file_path": "b.md",
            "content": "x".repeat(5000),
        }))
        .expect("object args summarize");
        assert_eq!(
            huge["content"]["contentLength"],
            serde_json::json!(5002),
            "bulk content is masked to a length marker, never embedded"
        );
        assert!(!huge.to_string().contains('x'), "content not leaked");

        // The size cap only applies to the non-masked remainder.
        let mut big_map = serde_json::Map::new();
        for i in 0..40 {
            big_map.insert(
                format!("key_{i}"),
                serde_json::json!("y".repeat(200)),
            );
        }
        let capped = summarize_judgement_tool_args(&serde_json::Value::Object(big_map))
            .expect("object args summarize");
        assert_eq!(capped["truncated"], serde_json::json!(true));

        assert!(
            summarize_judgement_tool_args(&serde_json::Value::Null).is_none(),
            "null arguments stay absent"
        );
        assert_eq!(
            summarize_judgement_tool_args(&serde_json::json!("scalar")).expect("scalar"),
            serde_json::json!("scalar")
        );
    }

    #[tokio::test]
    async fn tool_failures_for_scene_and_last_error_are_recorded() {
        // WARDEN-03 evidence accessors: the scene count and the last error
        // summary are observable per scene for model judgement.
        let mut rt = runtime();
        let scene_a = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "pwd"}));
        let scene_b = tool_failure_scene_key("ExecCommand", &serde_json::json!({"cmd": "ls"}));

        rt.on_tool_outcome("sess-ev", "ExecCommand", &scene_a, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.on_tool_outcome("sess-ev", "ExecCommand", &scene_a, WardenToolOutcome::ExecutionFailed)
            .await;
        rt.record_tool_error("sess-ev", &scene_a, "permission denied");
        assert_eq!(rt.tool_failures_for_scene("sess-ev", &scene_a), 1);
        assert_eq!(
            rt.last_tool_error("sess-ev", &scene_a),
            Some("permission denied")
        );

        assert_eq!(
            rt.tool_failures_for_scene("sess-ev", &scene_b),
            0,
            "sibling scene untouched"
        );
        assert!(
            rt.last_tool_error("sess-ev", &scene_b).is_none(),
            "no error recorded for the untouched scene"
        );

        // `tool_failures` (max across scenes) still reports the ladder driver.
        assert_eq!(rt.tool_failures("sess-ev"), 1);
    }
}
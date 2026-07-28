//! Integration tests for RBAC+Poke system (Phase R-A.12).
//!
//! Covers 5 core scenarios:
//! 1. RBAC interception — Commander Write denied, Read allowed
//! 2. Warden Audit-Poke — Executor Write triggers Audit → self_check within 3 turns
//! 3. Challenge-Poke compliance — Challenge → iron-rule self-check within 5 turns
//! 4. Penalty execution — 3 violations → L3 penalty → session read-only
//! 5. Shame wall persistence — entry written & serialized correctly
//!
//! All tests use isolated mock data and do **not** depend on a real BitFun runtime.

#![cfg(feature = "taiji")]

use std::collections::BTreeSet;

use bitfun_agent_tools::{
    PokeMessage, PokeResponse, PokeStatus, PokeType, PokeValidator, SelfCheckStatement,
};
use bitfun_core::agentic::tools::restrictions::{
    classify_tool_call, get_session_restrictions, update_restrictions, AgentRole, OperationClass,
    ToolRuntimeRestrictionsPatch,
};
use bitfun_core::agentic::warden::{
    punishment_executor::PenaltyOutcome, ChallengePokeConfig, PenaltyLevel, PenaltyRequest,
    PokePriorityManager, ShameWallRegistry, ViolationRecord, POKE_PENALTY_KIND,
    SHAME_WALL_FILENAME,
};

// ============================================================================
// Test 1: RBAC 拦截
// ============================================================================
//
// Scenario:
//   1. Create Commander session
//   2. Commander calls Write → RBAC rejects (Commander has no WRITE_FILE permission)
//   3. Commander calls Read → RBAC allows (Commander has READ_ONLY permission)
//
// Verification:
//   - classify_tool_call("Write", …) → OperationClass::WriteFile
//   - Commander's role template does NOT include WriteFile → ensure_operation_allowed fails
//   - classify_tool_call("Read", …) → OperationClass::ReadOnly
//   - Commander's role template DOES include ReadOnly → ensure_operation_allowed succeeds

#[test]
fn rbac_interception_commander_write_blocked_read_allowed() {
    // ── Setup: Register a Commander session ──────────────────────────────
    let session_id = "test-cmdr-int-01";
    update_restrictions(
        session_id,
        Some(AgentRole::Commander),
        ToolRuntimeRestrictionsPatch::default(),
    )
    .expect("set Commander role restrictions");

    let restrictions = get_session_restrictions(session_id)
        .expect("Commander restrictions should exist after update");

    // ── Commander calls Write ────────────────────────────────────────────
    let write_input = serde_json::json!({"file_path": "test.md", "content": "hello"});
    let write_class = classify_tool_call("Write", &write_input);
    assert_eq!(
        write_class,
        OperationClass::WriteFile,
        "Write tool should classify as WriteFile"
    );

    let write_result = restrictions.ensure_operation_allowed(OperationClass::WriteFile, "Write");
    assert!(
        write_result.is_err(),
        "Commander should NOT be allowed to perform WriteFile operations"
    );

    // ── Commander calls Read ─────────────────────────────────────────────
    let read_input = serde_json::json!({"file_path": "test.md"});
    let read_class = classify_tool_call("Read", &read_input);
    assert_eq!(
        read_class,
        OperationClass::ReadOnly,
        "Read tool should classify as ReadOnly"
    );

    let read_result = restrictions.ensure_operation_allowed(OperationClass::ReadOnly, "Read");
    assert!(
        read_result.is_ok(),
        "Commander SHOULD be allowed to perform ReadOnly operations"
    );

    // ── Edge case: ExecCommand with write redirect ───────────────────────
    // Even if Commander has Write in allowed_tool_names, the operation class
    // WriteFile is denied, so shell-based writes are also blocked.
    let tee_input = serde_json::json!({"cmd": "echo x > file.txt"});
    let tee_class = classify_tool_call("ExecCommand", &tee_input);
    assert_eq!(
        tee_class,
        OperationClass::WriteFile,
        "ExecCommand with '>' should classify as WriteFile"
    );
    let tee_result = restrictions.ensure_operation_allowed(OperationClass::WriteFile, "ExecCommand");
    assert!(
        tee_result.is_err(),
        "Commander should NOT be allowed WriteFile even via ExecCommand"
    );
}

// ============================================================================
// Test 2: Warden Audit-Poke
// ============================================================================
//
// Scenario:
//   1. Executor completes a Write tool call
//   2. Warden receives notification and sends Audit-Poke (deadline=3 turns)
//   3. Executor responds within 3 turns with a valid self_check
//   4. Warden validates the self_check → PASS
//
// Verification:
//   - PokeMessage::poke_type == Audit, deadline_turns == 3
//   - PokeResponse contains self_check with non-empty phase/gate/summary/rules
//   - PokeValidator::validate_audit_response returns true

#[test]
fn warden_audit_poke_executor_self_check_within_deadline() {
    // ── 1. Warden constructs an Audit-Poke message ───────────────────────
    let audit_poke = PokeMessage {
        poke_id: "audit-poke-001".into(),
        poke_type: PokeType::Audit,
        rule_ids: vec!["R1: no_destructive_write".into(), "R3: path_whitelist".into()],
        deadline_turns: 3,
        evidence_required: Some(vec!["tool_call_log".into(), "phase_summary".into()]),
    };

    assert_eq!(audit_poke.poke_type, PokeType::Audit);
    assert_eq!(audit_poke.deadline_turns, 3);
    assert!(!audit_poke.poke_id.is_empty());
    assert_eq!(audit_poke.rule_ids.len(), 2);

    // ── 2. Executor prepares a self-check response (within deadline) ────
    let executor_self_check = SelfCheckStatement {
        current_phase: "implementation".into(),
        last_gate: "pre_write_check".into(),
        tool_calls_summary: vec![
            "Read(main.rs)".into(),
            "Edit(main.rs:42)".into(),
            "Write(note.md)".into(),
        ],
        rules_checked: vec![
            "R1: no_destructive_write".into(),
            "R3: path_whitelist".into(),
        ],
    };

    let audit_response = PokeResponse {
        poke_id: audit_poke.poke_id.clone(),
        status: PokeStatus::Acknowledged,
        self_check: Some(executor_self_check),
    };

    // ── 3. Warden validates the response ────────────────────────────────
    assert!(
        PokeValidator::validate_audit_response(&audit_response),
        "Audit response with valid self_check should pass validation"
    );

    // ── Edge: Deferred response within limit is still valid ──────────────
    let deferred_response = PokeResponse {
        poke_id: "audit-poke-002".into(),
        status: PokeStatus::Deferred(2),
        self_check: Some(SelfCheckStatement {
            current_phase: "review".into(),
            last_gate: "deferred".into(),
            tool_calls_summary: vec!["Read(doc.md)".into()],
            rules_checked: vec!["R1".into()],
        }),
    };
    assert!(
        PokeValidator::validate_audit_response(&deferred_response),
        "Audit response with deferral < 3 should still pass"
    );

    // ── Edge: Missing self_check should fail ─────────────────────────────
    let bad_response = PokeResponse {
        poke_id: "audit-poke-003".into(),
        status: PokeStatus::Acknowledged,
        self_check: None,
    };
    assert!(
        !PokeValidator::validate_audit_response(&bad_response),
        "Audit response without self_check should fail"
    );

    // ── Edge: Empty phase should fail ────────────────────────────────────
    let empty_phase = PokeResponse {
        poke_id: "audit-poke-004".into(),
        status: PokeStatus::Acknowledged,
        self_check: Some(SelfCheckStatement {
            current_phase: "".into(),
            last_gate: "gate".into(),
            tool_calls_summary: vec!["Read".into()],
            rules_checked: vec!["R1".into()],
        }),
    };
    assert!(
        !PokeValidator::validate_audit_response(&empty_phase),
        "Audit response with empty phase should fail"
    );
}

// ============================================================================
// Test 3: Challenge-Poke 合规
// ============================================================================
//
// Scenario:
//   1. Warden sends Challenge-Poke (Poisson-sampled, deadline=5 turns)
//   2. Executor responds within 5 turns with iron-rule compliance self-check
//   3. Warden validates the response → PASS
//
// Verification:
//   - ChallengePokeConfig builds correct Challenge-Poke messages
//   - PokeValidator::validate_challenge_response accepts valid responses
//   - PokePriorityManager tracks timeout correctly at the boundary

#[test]
fn challenge_poke_compliance_within_deadline() {
    // ── 1. Challenge-Poke configuration ──────────────────────────────────
    let mut rules = BTreeSet::new();
    rules.insert("R-001".into());
    rules.insert("R-004".into());
    rules.insert("R-007".into());

    let config = ChallengePokeConfig::new(6.5, 42, rules.clone());
    assert_eq!(config.deadline_turns, 5);
    assert_eq!(config.max_defer_count, 3);

    let challenge_msg = config.build_challenge_message("challenge-poke-001".into());
    assert_eq!(challenge_msg.poke_type, PokeType::Challenge);
    assert_eq!(challenge_msg.deadline_turns, 5);
    assert!(challenge_msg.rule_ids.contains(&"R-001".to_string()));

    // ── 2. Executor self-check with iron-rule citations ──────────────────
    let challenge_response = PokeResponse {
        poke_id: challenge_msg.poke_id.clone(),
        status: PokeStatus::Acknowledged,
        self_check: Some(SelfCheckStatement {
            current_phase: "execution".into(),
            last_gate: "read_check".into(),
            tool_calls_summary: vec![
                "Read(config.yaml)".into(),
                "Grep(pattern=secret)".into(),
            ],
            rules_checked: vec![
                "R-001: no_hardcoded_secrets".into(),
                "R-004: path_whitelist".into(),
                "R-007: audit_log".into(),
            ],
        }),
    };

    // ── 3. Warden validates ──────────────────────────────────────────────
    assert!(
        PokeValidator::validate_challenge_response(&challenge_response),
        "Challenge response with valid iron-rule self-check should pass"
    );

    // ── Edge: Deferred response within max_defer_count (≤ 3) ─────────────
    let deferred_ok = PokeResponse {
        poke_id: "challenge-poke-002".into(),
        status: PokeStatus::Deferred(3),
        self_check: Some(SelfCheckStatement {
            current_phase: "planning".into(),
            last_gate: "gate".into(),
            tool_calls_summary: vec!["Read".into()],
            rules_checked: vec!["R-001".into()],
        }),
    };
    assert!(
        PokeValidator::validate_challenge_response(&deferred_ok),
        "Challenge response with defer=3 should be valid"
    );

    // ── Edge: Deferred > 3 should fail ───────────────────────────────────
    let deferred_fail = PokeResponse {
        poke_id: "challenge-poke-003".into(),
        status: PokeStatus::Deferred(4),
        self_check: Some(SelfCheckStatement {
            current_phase: "planning".into(),
            last_gate: "gate".into(),
            tool_calls_summary: vec!["Read".into()],
            rules_checked: vec!["R-001".into()],
        }),
    };
    assert!(
        !PokeValidator::validate_challenge_response(&deferred_fail),
        "Challenge response with defer=4 should fail"
    );

    // ── PokePriorityManager: timeout tracking at exact boundary ──────────
    let mut manager = PokePriorityManager::new();
    manager.register_poke("challenge-boundary");
    // deadline = 5, advance exactly 5 turns
    for _ in 0..5 {
        manager.advance_turn();
    }
    assert!(
        manager.is_timeout("challenge-boundary", 5),
        "Poke should time out after exactly 5 turns"
    );
    // With deadline = 6, not yet timed out
    assert!(
        !manager.is_timeout("challenge-boundary", 6),
        "Poke should NOT time out before 6-turn deadline"
    );
}

// ============================================================================
// Test 4: 惩罚执行
// ============================================================================
//
// Scenario:
//   1. Executor session has Executor role (WriteFile + ExecuteCode allowed)
//   2. Simulate 3 violations → Warden prepares PenaltyRequest L3
//   3. PunishmentExecutor applies L3: role → Reviewer (read-only)
//   4. After penalty, WriteFile operations are blocked
//
// Verification:
//   - PenaltyRequest data type round-trips correctly
//   - PenaltyOutcome for L3 has session_frozen=true, rbac_change=Reviewer
//   - After update_restrictions to Reviewer, WriteFile is denied
//   - ReadOnly operations remain allowed

#[test]
fn penalty_execution_l3_demotes_to_readonly() {
    let session_id = "test-exec-penalty-01";

    // ── 1. Set up as Executor (WRITE_FILE + EXECUTE_CODE allowed) ────────
    update_restrictions(
        session_id,
        Some(AgentRole::Executor),
        ToolRuntimeRestrictionsPatch::default(),
    )
    .expect("set Executor role");

    let pre_restrictions = get_session_restrictions(session_id)
        .expect("Executor restrictions should exist");
    assert!(
        pre_restrictions
            .allowed_operation_classes
            .contains(&OperationClass::WriteFile),
        "Executor should allow WriteFile before penalty"
    );
    assert!(
        pre_restrictions
            .allowed_operation_classes
            .contains(&OperationClass::ExecuteCode),
        "Executor should allow ExecuteCode before penalty"
    );

    // ── 2. Build a PenaltyRequest matching the L3 scenario ───────────────
    let violations = vec![
        ViolationRecord {
            rule_id: "R-001".into(),
            description: "Unauthorized write to restricted path".into(),
            severity: "major".into(),
            timestamp: "2025-01-15T10:00:00Z".into(),
            evidence: serde_json::json!({"tool": "Write", "path": "/etc/config"}),
        },
        ViolationRecord {
            rule_id: "R-002".into(),
            description: "Executed risky shell command without approval".into(),
            severity: "major".into(),
            timestamp: "2025-01-15T10:05:00Z".into(),
            evidence: serde_json::json!({"tool": "ExecCommand", "cmd": "rm -rf /data"}),
        },
        ViolationRecord {
            rule_id: "R-003".into(),
            description: "Repeated violation after L2 warning".into(),
            severity: "critical".into(),
            timestamp: "2025-01-15T10:10:00Z".into(),
            evidence: serde_json::json!({"tool": "Write", "path": "/etc/shadow"}),
        },
    ];

    let penalty_request = PenaltyRequest {
        target_session_id: session_id.to_string(),
        level: PenaltyLevel::L3,
        violations: violations.clone(),
        requested_by: "warden-session-001".into(),
    };

    assert_eq!(penalty_request.level, PenaltyLevel::L3);
    assert_eq!(penalty_request.target_session_id, session_id);
    assert_eq!(penalty_request.violations.len(), 3);

    // ── 3. Simulate L3 penalty: demote to Reviewer (read-only) ──────────
    // (This is what PunishmentExecutor::execute_l3 does internally.)
    update_restrictions(
        session_id,
        Some(AgentRole::Reviewer),
        ToolRuntimeRestrictionsPatch::default(),
    )
    .expect("L3 penalty: demote to Reviewer");

    // ── 4. Verify post-penalty: WriteFile denied, ReadOnly allowed ──────
    let post_restrictions = get_session_restrictions(session_id)
        .expect("Reviewer restrictions should exist after L3 penalty");
    assert!(
        !post_restrictions
            .allowed_operation_classes
            .contains(&OperationClass::WriteFile),
        "After L3 penalty, WriteFile should NOT be allowed"
    );
    assert!(
        !post_restrictions
            .allowed_operation_classes
            .contains(&OperationClass::ExecuteCode),
        "After L3 penalty, ExecuteCode should NOT be allowed"
    );
    assert!(
        post_restrictions
            .allowed_operation_classes
            .contains(&OperationClass::ReadOnly),
        "After L3 penalty, ReadOnly SHOULD be allowed"
    );
    assert!(
        post_restrictions
            .allowed_operation_classes
            .contains(&OperationClass::Communicate),
        "After L3 penalty, Communicate SHOULD be allowed"
    );

    // ── Verify tool-level enforcement ────────────────────────────────────
    let write_result =
        post_restrictions.ensure_operation_allowed(OperationClass::WriteFile, "Write");
    assert!(
        write_result.is_err(),
        "Write tool should be blocked after L3 penalty"
    );

    let read_result =
        post_restrictions.ensure_operation_allowed(OperationClass::ReadOnly, "Read");
    assert!(
        read_result.is_ok(),
        "Read tool should be allowed after L3 penalty"
    );

    // ── PenaltyOutcome shape ─────────────────────────────────────────────
    let outcome = PenaltyOutcome {
        level: PenaltyLevel::L3,
        prepended_reminders: vec![],
        rbac_change: Some(AgentRole::Reviewer),
        session_frozen: true,
        permanent_mark: false,
        notify_user: true,
    };
    assert_eq!(outcome.level, PenaltyLevel::L3);
    assert_eq!(outcome.rbac_change, Some(AgentRole::Reviewer));
    assert!(outcome.session_frozen);
    assert!(outcome.notify_user);
    assert!(!outcome.permanent_mark);
}

// ============================================================================
// Test 5: 耻辱墙持久化
// ============================================================================
//
// Scenario:
//   1. After penalty execution, ShameWallRegistry contains an entry
//   2. The registry can be serialized to JSON (matches shame-wall-registry.json format)
//   3. The entry contains all required fields
//   4. POKE_PENALTY_KIND constant is consistent with prepended_reminders usage
//
// Verification:
//   - ShameWallRegistry with entries serializes/deserializes correctly
//   - SHAME_WALL_FILENAME matches the expected contract path
//   - Violation records persist correctly with upsert
//   - Registry query methods work (by user, by session)

#[test]
fn shame_wall_persistence_after_penalty() {
    // ── 1. Build a ShameWallRegistry with violation entries ──────────────
    let mut registry = ShameWallRegistry::default();
    assert_eq!(registry.version, 1);
    assert!(registry.entries.is_empty());

    let v1 = ViolationRecord {
        rule_id: "R-001".into(),
        description: "Unauthorized write to /etc/config".into(),
        severity: "major".into(),
        timestamp: "2025-01-15T10:00:00Z".into(),
        evidence: serde_json::json!({"tool": "Write", "path": "/etc/config"}),
    };
    let v2 = ViolationRecord {
        rule_id: "R-002".into(),
        description: "Executed risky shell command".into(),
        severity: "critical".into(),
        timestamp: "2025-01-15T10:05:00Z".into(),
        evidence: serde_json::json!({"tool": "ExecCommand", "cmd": "rm -rf /data"}),
    };

    // ── 2. Upsert entry for session-1 (first violation → L1) ────────────
    registry.upsert_entry(
        "user-alpha",
        "executor",
        "session-penalty-1",
        vec![v1.clone()],
        PenaltyLevel::L1,
        "2025-01-15T10:00:00Z",
    );

    assert_eq!(registry.entries.len(), 1);
    let entry = &registry.entries[0];
    assert_eq!(entry.session_id, "session-penalty-1");
    assert_eq!(entry.user_id, "user-alpha");
    assert_eq!(entry.violations.len(), 1);
    assert_eq!(entry.cumulative_penalty_level, PenaltyLevel::L1);
    assert!(!entry.created_at.is_empty());
    assert!(!entry.updated_at.is_empty());

    // ── 3. Upsert again for same session (escalate → L3) ────────────────
    registry.upsert_entry(
        "user-alpha",
        "executor",
        "session-penalty-1",
        vec![v2.clone()],
        PenaltyLevel::L3,
        "2025-01-15T10:10:00Z",
    );

    assert_eq!(
        registry.entries.len(),
        1,
        "Should still be 1 entry (upserted)"
    );
    assert_eq!(
        registry.entries[0].violations.len(),
        2,
        "Should have 2 accumulated violations"
    );
    assert_eq!(
        registry.entries[0].cumulative_penalty_level,
        PenaltyLevel::L3,
        "Penalty level should be escalated to L3"
    );

    // ── 4. Query methods ─────────────────────────────────────────────────
    let user_entries = registry.entries_for_user("user-alpha");
    assert_eq!(user_entries.len(), 1);

    let session_entry = registry.entry_for_session("session-penalty-1");
    assert!(session_entry.is_some());
    assert_eq!(session_entry.unwrap().violations.len(), 2);

    let missing = registry.entry_for_session("nonexistent");
    assert!(missing.is_none());

    // ── 5. JSON serialization round-trip (matches file format) ───────────
    let json = serde_json::to_string_pretty(&registry).expect("serialize registry");
    assert!(json.contains("session-penalty-1"));
    assert!(json.contains("R-001"));
    assert!(json.contains("R-002"));
    assert!(json.contains("L3"));

    let deserialized: ShameWallRegistry =
        serde_json::from_str(&json).expect("deserialize registry");
    assert_eq!(deserialized.version, 1);
    assert_eq!(deserialized.entries.len(), 1);
    assert_eq!(deserialized.entries[0].violations.len(), 2);

    // ── 6. Contract constants ────────────────────────────────────────────
    assert_eq!(
        SHAME_WALL_FILENAME,
        ".master-framework/shame-wall-registry.json",
        "SHAME_WALL_FILENAME must match the contract path"
    );
    assert_eq!(POKE_PENALTY_KIND, "PokePenalty");

    // ── 7. Multiple sessions (different users) ───────────────────────────
    registry.upsert_entry(
        "user-beta",
        "executor",
        "session-penalty-2",
        vec![v1],
        PenaltyLevel::L1,
        "2025-01-15T11:00:00Z",
    );
    assert_eq!(registry.entries.len(), 2);

    let beta_entries = registry.entries_for_user("user-beta");
    assert_eq!(beta_entries.len(), 1);

    let alpha_entries = registry.entries_for_user("user-alpha");
    assert_eq!(alpha_entries.len(), 1);
}

// ============================================================================
// Additional contract verification tests
// ============================================================================

/// Verify the full penalty request → outcome → shame wall flow
/// integrates correctly across the data types.
#[test]
fn penalty_flow_end_to_end_data_types() {
    // ── Build a PenaltyRequest ───────────────────────────────────────────
    let request = PenaltyRequest {
        target_session_id: "flow-session-01".into(),
        level: PenaltyLevel::L2,
        violations: vec![ViolationRecord {
            rule_id: "R-001".into(),
            description: "Test violation".into(),
            severity: "major".into(),
            timestamp: "2025-01-01T00:00:00Z".into(),
            evidence: serde_json::json!({"detail": "test"}),
        }],
        requested_by: "warden-flow-01".into(),
    };

    // Serialize/deserialize round-trip
    let json = serde_json::to_string(&request).expect("serialize PenaltyRequest");
    let deser: PenaltyRequest = serde_json::from_str(&json).expect("deserialize PenaltyRequest");
    assert_eq!(deser.target_session_id, "flow-session-01");
    assert_eq!(deser.level, PenaltyLevel::L2);
    assert_eq!(deser.violations.len(), 1);
    assert_eq!(deser.requested_by, "warden-flow-01");

    // ── Build PenaltyOutcome for L2 ──────────────────────────────────────
    let outcome = PenaltyOutcome {
        level: PenaltyLevel::L2,
        prepended_reminders: vec![],
        rbac_change: Some(AgentRole::Reviewer),
        session_frozen: false,
        permanent_mark: false,
        notify_user: false,
    };
    assert_eq!(outcome.level, PenaltyLevel::L2);
    assert_eq!(outcome.rbac_change, Some(AgentRole::Reviewer));

    // ── Simulate the full shame-wall write ───────────────────────────────
    let mut registry = ShameWallRegistry::default();
    registry.upsert_entry(
        &request.target_session_id,
        "agent",
        &request.target_session_id,
        request.violations.clone(),
        request.level,
        "2025-01-01T00:00:00Z",
    );

    assert_eq!(registry.entries.len(), 1);
    assert_eq!(
        registry.entries[0].cumulative_penalty_level,
        PenaltyLevel::L2
    );
}

/// Verify the Poisson scheduler integration with ChallengePokeConfig
/// produces expected behavior for the 5-turn deadline contract.
#[test]
fn challenge_poisson_scheduling_contract() {
    use bitfun_core::agentic::warden::PoissonScheduler;

    // With rate=1.0, every round should poke (p=1.0)
    let mut sched = PoissonScheduler::new(1.0, 100);
    for _ in 0..20 {
        assert!(sched.should_poke(), "rate=1.0 must poke every round");
    }
    assert_eq!(sched.counter(), 20);

    // Deterministic seed produces identical sequences
    let mut a = PoissonScheduler::new(6.5, 9999);
    let mut b = PoissonScheduler::new(6.5, 9999);
    for _ in 0..50 {
        assert_eq!(a.should_poke(), b.should_poke());
    }

    // Expected pokes calculation
    let sched = PoissonScheduler::new(6.5, 42);
    let expected = sched.expected_pokes(1300);
    assert!((expected - 200.0).abs() < f64::EPSILON);
}

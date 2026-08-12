//! Integration tests for the user-controllable RBAC/Warden master switch
//! (R-26).
//!
//! The switch is a process-level cache (`crate::service::config::rbac_enabled`)
//! mirrored from the settings document (`ai.rbac_enabled`). Tests in this file
//! run in a dedicated test binary so toggling the global switch cannot race
//! with other lib unit tests; a static mutex serializes the tests inside this
//! file.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use bitfun_core::agentic::coordination::turn_outcome::TurnOutcomeStatus;
use bitfun_core::agentic::session::SessionManager;
use bitfun_core::agentic::tools::ToolUseContext;
use bitfun_core::agentic::warden::{
    runtime::{WardenRuntime, WardenToolOutcome},
    ChallengePokeConfig, PenaltyLevel,
};
use bitfun_core::agentic::WorkspaceBinding;
use bitfun_core::service::config::{rbac_enabled, set_rbac_enabled, AIConfig};
use bitfun_runtime_ports::ToolRuntimeHandles;
use tool_runtime::context::PrimaryModelFacts;

/// Serializes switch-toggling tests inside this binary.
fn switch_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn test_session_manager() -> Arc<SessionManager> {
    use bitfun_core::agentic::persistence::PersistenceManager;
    use bitfun_core::agentic::session::{
        PromptCachePolicy, SessionContextStore, SessionManagerConfig,
    };
    use bitfun_core::infrastructure::app_paths::PathManager;

    // Isolate storage via env overrides (this test binary is its own process,
    // so the env vars cannot leak into other test binaries).
    let root = std::env::temp_dir().join(format!("bitfun-rbac-switch-test-{}", uuid()));
    std::env::set_var("BITFUN_E2E_USER_ROOT", root.join("user-root"));
    std::env::set_var("BITFUN_E2E_HOME", root.join("home"));
    let path_manager = Arc::new(PathManager::new().expect("path manager"));
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

fn uuid() -> String {
    use uuid::Uuid;
    Uuid::new_v4().to_string()
}

fn restricted_context() -> ToolUseContext {
    ToolUseContext {
        tool_call_id: None,
        agent_type: None,
        session_id: None,
        dialog_turn_id: None,
        workspace: Some(WorkspaceBinding::new(
            None,
            std::path::PathBuf::from("/repo/project"),
        )),
        loaded_deferred_tool_specs: Vec::new(),
        primary_model_facts: PrimaryModelFacts::default(),
        custom_data: std::collections::HashMap::new(),
        computer_use_host: None,
        runtime_tool_restrictions: bitfun_core::agentic::tools::ToolRuntimeRestrictions {
            allowed_tool_names: BTreeSet::new(),
            denied_tool_names: BTreeSet::from(["Write".to_string()]),
            denied_tool_messages: Default::default(),
            path_policy: Default::default(),
            allowed_operation_classes: BTreeSet::new(),
            denied_operation_classes: BTreeSet::new(),
        },
        runtime_handles: ToolRuntimeHandles::default(),
    }
}

// ============================================================================
// R-26: config default and cache
// ============================================================================

#[test]
fn ai_config_defaults_to_rbac_enabled() {
    let config = AIConfig::default();
    assert!(config.rbac_enabled, "rbac_enabled must default to true");
}

#[test]
fn switch_cache_defaults_to_enabled_and_toggles() {
    let _guard = switch_guard();
    let previous = rbac_enabled();
    set_rbac_enabled(true);
    assert!(rbac_enabled(), "cache must be on by default");
    set_rbac_enabled(false);
    assert!(!rbac_enabled(), "cache must toggle off");
    set_rbac_enabled(true);
    assert!(rbac_enabled(), "cache must toggle back on");
    set_rbac_enabled(previous);
}

// ============================================================================
// R-26: tool restriction gate bypass
// ============================================================================

#[test]
fn enforce_tool_runtime_restrictions_bypassed_when_switch_off() {
    let _guard = switch_guard();
    let previous = rbac_enabled();
    set_rbac_enabled(false);

    let context = restricted_context();
    // Write is denied by the context restrictions, but the master switch off
    // must bypass the gate entirely.
    context
        .enforce_tool_runtime_restrictions(
            "Write",
            &serde_json::json!({"file_path": "test.md", "content": "x"}),
        )
        .expect("R-26: restriction gate bypassed when master switch is off");

    set_rbac_enabled(previous);
}

#[test]
fn enforce_tool_runtime_restrictions_active_when_switch_on() {
    let _guard = switch_guard();
    let previous = rbac_enabled();
    set_rbac_enabled(true);

    let context = restricted_context();
    let err = context
        .enforce_tool_runtime_restrictions(
            "Write",
            &serde_json::json!({"file_path": "test.md", "content": "x"}),
        )
        .expect_err("R-26: restriction gate active when master switch is on");
    assert!(err.to_string().contains("denied"), "got: {err}");

    set_rbac_enabled(previous);
}

// ============================================================================
// R-26: Warden runtime disabled when switch off
// ============================================================================

#[tokio::test]
#[allow(clippy::await_holding_lock)] // switch guard is intentionally held for the whole test body
async fn warden_runtime_off_disables_turn_and_tool_tracking() {
    let _guard = switch_guard();
    let previous = rbac_enabled();
    set_rbac_enabled(false);

    let mut rt = WardenRuntime::new(test_session_manager());
    // Challenge at rate=1.0 would fire every turn if the runtime were active.
    rt.set_challenge_config(ChallengePokeConfig::new(
        1.0,
        7,
        BTreeSet::from(["iron-rules-compliance".to_string()]),
    ));

    rt.on_turn_outcome("sess-off", TurnOutcomeStatus::Failed, "t1")
        .await;
    assert_eq!(
        rt.consecutive_failures("sess-off"),
        0,
        "no failure tracking"
    );
    assert!(
        rt.shame_wall().entry_for_session("sess-off").is_none(),
        "no violation recorded"
    );
    assert!(
        rt.take_pending_reminders("sess-off").is_empty(),
        "no reminders queued (turn outcome)"
    );

    rt.on_tool_outcome(
        "sess-off",
        "ExecCommand",
        "ExecCommand:{}",
        WardenToolOutcome::ExecutionFailed,
    )
    .await;
    assert_eq!(rt.tool_failures("sess-off"), 0, "no tool failure tracking");
    assert!(
        rt.take_pending_reminders("sess-off").is_empty(),
        "no reminders queued (tool outcome)"
    );

    set_rbac_enabled(previous);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // switch guard is intentionally held for the whole test body
async fn warden_runtime_on_keeps_turn_and_tool_tracking() {
    let _guard = switch_guard();
    let previous = rbac_enabled();
    set_rbac_enabled(true);

    let mut rt = WardenRuntime::new(test_session_manager());
    rt.set_challenge_config(ChallengePokeConfig::new(f64::INFINITY, 1, BTreeSet::new()));

    rt.on_turn_outcome("sess-on", TurnOutcomeStatus::Failed, "t1")
        .await;
    assert_eq!(
        rt.consecutive_failures("sess-on"),
        0,
        "first turn failure of a scene is exploratory"
    );
    assert!(
        rt.shame_wall().entry_for_session("sess-on").is_none(),
        "no violation recorded for the exploratory first failure"
    );

    rt.on_turn_outcome("sess-on", TurnOutcomeStatus::Failed, "t2")
        .await;
    assert_eq!(rt.consecutive_failures("sess-on"), 1, "tracking active");
    assert_eq!(
        rt.shame_wall()
            .entry_for_session("sess-on")
            .unwrap()
            .cumulative_penalty_level,
        PenaltyLevel::L1,
        "violation recorded when switch is on"
    );
    assert_eq!(rt.take_pending_reminders("sess-on").len(), 1);

    rt.on_tool_outcome(
        "sess-on",
        "ExecCommand",
        "ExecCommand:{}",
        WardenToolOutcome::ExecutionFailed,
    )
    .await;
    assert_eq!(
        rt.tool_failures("sess-on"),
        0,
        "first tool failure of a scene is exploratory"
    );
    rt.on_tool_outcome(
        "sess-on",
        "ExecCommand",
        "ExecCommand:{}",
        WardenToolOutcome::ExecutionFailed,
    )
    .await;
    assert_eq!(rt.tool_failures("sess-on"), 1, "tool tracking active");

    set_rbac_enabled(previous);
}

#[test]
fn general_purpose_subagent_role_is_executor_and_readonly_allowed() {
    use bitfun_core::agentic::tools::restrictions::{
        clear_session_role, general_purpose_tool_restrictions, get_default_permissions,
        get_session_restrictions, get_session_role, set_session_role_with_restrictions, AgentRole,
        OperationClass,
    };
    // 默认 Executor 模板必须允许只读类（执行者读代码基本能力）。
    let executor = get_default_permissions(AgentRole::Executor);
    assert!(
        executor
            .ensure_operation_allowed(OperationClass::ReadOnly, "Read")
            .is_ok(),
        "default Executor template must allow ReadOnly"
    );
    // GeneralPurpose 专属模板允许只读侦察 + 执行。
    let gp = general_purpose_tool_restrictions();
    assert!(gp
        .ensure_operation_allowed(OperationClass::ReadOnly, "Read")
        .is_ok());
    assert!(gp
        .ensure_operation_allowed(OperationClass::WriteFile, "Write")
        .is_ok());
    assert!(gp
        .ensure_operation_allowed(OperationClass::ExecuteCode, "ExecCommand")
        .is_ok());
    assert!(gp.ensure_tool_allowed("Read").is_ok());
    assert!(gp.ensure_tool_allowed("Glob").is_ok());
    assert!(gp.ensure_tool_allowed("Grep").is_ok());
    assert!(gp.ensure_tool_allowed("ExecCommand").is_ok());
    // 注册角色仍为 Executor 且 ReadOnly 工具可用（防回退）。
    let sid = format!("gp-role-{}", uuid());
    set_session_role_with_restrictions(&sid, AgentRole::Executor, gp).expect("register role");
    assert_eq!(get_session_role(&sid), Some(AgentRole::Executor));
    let effective = get_session_restrictions(&sid).expect("session restrictions");
    assert!(effective
        .ensure_operation_allowed(OperationClass::ReadOnly, "Read")
        .is_ok());
    clear_session_role(&sid);
}

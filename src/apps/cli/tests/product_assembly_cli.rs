use std::process::Command;

#[test]
fn doctor_reports_the_validated_cli_runtime_assembly() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("workspace");
    let user_root = temp.path().join("user-root");
    let home_root = temp.path().join("home-root");
    let config_root = temp.path().join("host-config");
    std::fs::create_dir_all(&workspace).expect("create workspace");

    let output = Command::new(env!("CARGO_BIN_EXE_bitfun"))
        .arg("doctor")
        .current_dir(&workspace)
        .env_remove("BITFUN_USER_ROOT")
        .env_remove("BITFUN_HOME")
        .env("BITFUN_E2E_STORAGE_GUARD", "1")
        .env("BITFUN_E2E_USER_ROOT", &user_root)
        .env("BITFUN_E2E_HOME", &home_root)
        .env("APPDATA", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .env("HOME", &home_root)
        .output()
        .expect("run bitfun doctor");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stdout.contains("[ok] Product runtime: cli assembly-ready"),
        "{stdout}"
    );
    assert!(
        stdout.contains("[ok] Runtime capability registrations: complete"),
        "{stdout}"
    );
    assert!(
        stdout.contains("[info] Execution owner: bitfun-core compatibility"),
        "{stdout}"
    );
    assert!(
        stdout.contains("[info] Plugin runtime: disabled (not_built)"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("[ok] Config directory: {}", user_root.display())),
        "{stdout}"
    );
}

#[test]
fn health_reports_assembly_and_compatibility_boundaries() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("workspace");
    let user_root = temp.path().join("user-root");
    let home_root = temp.path().join("home-root");
    let config_root = temp.path().join("host-config");
    std::fs::create_dir_all(&workspace).expect("create workspace");

    let output = Command::new(env!("CARGO_BIN_EXE_bitfun"))
        .arg("health")
        .current_dir(&workspace)
        .env_remove("BITFUN_USER_ROOT")
        .env_remove("BITFUN_HOME")
        .env("BITFUN_E2E_STORAGE_GUARD", "1")
        .env("BITFUN_E2E_USER_ROOT", &user_root)
        .env("BITFUN_E2E_HOME", &home_root)
        .env("APPDATA", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .env("HOME", &home_root)
        .output()
        .expect("run bitfun health");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stdout.contains("Product runtime: cli assembly-ready"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Runtime capability registrations: complete"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Execution owner: bitfun-core compatibility"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Plugin runtime: disabled (not_built)"),
        "{stdout}"
    );
}

#[test]
fn doctor_rejects_incomplete_e2e_storage_roots() {
    for (case_name, provide_user_root, provide_home_root) in
        [("missing-user", false, true), ("missing-home", true, false)]
    {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let user_root = temp.path().join("user-root");
        let home_root = temp.path().join("home-root");
        let config_root = temp.path().join("host-config");
        std::fs::create_dir_all(&workspace).expect("create workspace");

        let mut command = Command::new(env!("CARGO_BIN_EXE_bitfun"));
        command
            .arg("doctor")
            .current_dir(&workspace)
            .env_remove("BITFUN_USER_ROOT")
            .env_remove("BITFUN_E2E_USER_ROOT")
            .env_remove("BITFUN_HOME")
            .env_remove("BITFUN_E2E_HOME")
            .env("BITFUN_E2E_STORAGE_GUARD", "1")
            .env("APPDATA", &config_root)
            .env("XDG_CONFIG_HOME", &config_root)
            .env("HOME", &home_root);
        if provide_user_root {
            command.env("BITFUN_E2E_USER_ROOT", &user_root);
        }
        if provide_home_root {
            command.env("BITFUN_E2E_HOME", &home_root);
        }

        let output = command.output().expect("run bitfun doctor");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{case_name}: {stderr}");
        assert!(
            stderr.contains("BITFUN_E2E_STORAGE_GUARD requires isolated")
                && stderr.contains("BITFUN_E2E_USER_ROOT")
                && stderr.contains("BITFUN_E2E_HOME"),
            "{case_name}: {stderr}"
        );
        assert!(
            !user_root.join("config.toml").exists(),
            "{case_name}: config should not be written before guard validation"
        );
    }
}

#[test]
fn remaining_cli_local_persistence_stays_behind_explicit_owner_boundaries() {
    const ACCOUNT_SYNC: &str = include_str!("../src/account_sync.rs");
    const STARTUP_PAGE: &str = include_str!("../src/ui/startup.rs");
    const PEER_BOOTSTRAP: &str = include_str!("../src/peer_host/bootstrap.rs");
    const PEER_STATE: &str = include_str!("../src/peer_host/state.rs");
    const PEER_SESSION_COMMANDS: &str = include_str!("../src/peer_host/commands/session.rs");
    const PEER_SNAPSHOT_COMMANDS: &str = include_str!("../src/peer_host/commands/snapshot.rs");
    const CORE_RUNTIME_SERVICES: &str =
        include_str!("../../../crates/assembly/core/src/product_runtime/runtime_services.rs");

    for (path, source) in [
        ("account_sync.rs", ACCOUNT_SYNC),
        ("ui/startup.rs", STARTUP_PAGE),
        ("peer_host/bootstrap.rs", PEER_BOOTSTRAP),
        ("peer_host/state.rs", PEER_STATE),
        ("peer_host/commands/session.rs", PEER_SESSION_COMMANDS),
        ("peer_host/commands/snapshot.rs", PEER_SNAPSHOT_COMMANDS),
    ] {
        assert!(
            !source.contains("PersistenceManager"),
            "{path} must not import or name Core's concrete persistence manager"
        );
    }

    assert!(
        ACCOUNT_SYNC.contains("CoreAgentRuntimeCompatibility"),
        "account sync must receive the narrow Core compatibility facade"
    );
    assert!(
        STARTUP_PAGE.contains("CoreAgentRuntimeCompatibility"),
        "startup must pass the initialized Core compatibility facade to account sync"
    );
    assert!(
        !CORE_RUNTIME_SERVICES.contains("pub fn persistence_manager"),
        "runtime services provider must not expose a concrete persistence factory"
    );
    assert!(
        !PEER_BOOTSTRAP.contains("DialogScheduler::new")
            && !PEER_BOOTSTRAP.contains("get_global_scheduler"),
        "Peer Host must consume the invocation-scoped scheduler instead of assembling one"
    );
    assert!(
        !PEER_STATE.contains("pub(crate) persistence")
            && !PEER_SESSION_COMMANDS.contains("state.persistence")
            && !PEER_SNAPSHOT_COMMANDS.contains("state.persistence")
            && !PEER_SESSION_COMMANDS.contains("get_snapshot_manager_for_workspace")
            && !PEER_SNAPSHOT_COMMANDS.contains("get_snapshot_manager_for_workspace")
            && !PEER_SESSION_COMMANDS.contains("ensure_snapshot_manager_for_workspace")
            && !PEER_SNAPSHOT_COMMANDS.contains("ensure_snapshot_manager_for_workspace"),
        "Peer Host persistence operations must stay behind an explicit Core owner boundary"
    );
    assert!(
        PEER_BOOTSTRAP.contains("local_workspace_snapshot:")
            && PEER_STATE.contains("LocalWorkspaceSnapshotPort")
            && PEER_SESSION_COMMANDS.contains("local_workspace_snapshot")
            && PEER_SNAPSHOT_COMMANDS.contains("local_workspace_snapshot"),
        "Peer Host local snapshot operations must consume the injected owner port"
    );
}

#[test]
fn peer_session_control_and_usage_persistence_use_runtime_sdk() {
    const PEER_SESSION_COMMANDS: &str = include_str!("../src/peer_host/commands/session.rs");
    const CHAT_SELECTION: &str = include_str!("../src/modes/chat/selection.rs");
    const CORE_PRODUCT_RUNTIME: &str =
        include_str!("../../../crates/assembly/core/src/product_runtime.rs");

    for sdk_operation in [
        "create_session_with_id",
        "restore_session",
        "rename_session",
        "archive_session",
        "get_thread_goal",
    ] {
        assert!(
            PEER_SESSION_COMMANDS.contains(sdk_operation),
            "Peer Host session control must route {sdk_operation} through the Runtime SDK"
        );
    }
    assert!(
        CHAT_SELECTION.contains("record_completed_local_command_turn")
            && !CHAT_SELECTION.contains("append_completed_local_command_turn"),
        "TUI usage persistence must use the fixed-semantics Runtime SDK port"
    );

    for removed_compatibility_method in [
        "pub async fn create_session_with_workspace",
        "pub async fn restore_session_for_workspace",
        "pub async fn update_session_title_for_storage_path",
        "pub async fn archive_persisted_session",
        "pub async fn get_thread_goal",
        "pub async fn append_completed_local_command_turn",
        "pub async fn get_session_snapshot_files",
        "pub async fn get_session_snapshot_stats",
        "pub async fn rollback_workspace_files_to_turn",
    ] {
        assert!(
            !CORE_PRODUCT_RUNTIME.contains(removed_compatibility_method),
            "migrated session control must not remain on CoreAgentRuntimeCompatibility: {removed_compatibility_method}"
        );
    }
}

#[test]
fn local_workspace_snapshot_port_does_not_expand_the_agent_runtime_sdk() {
    const RUNTIME_SDK: &str = include_str!("../../../crates/execution/agent-runtime/src/sdk.rs");
    const LOCAL_SNAPSHOT_PORT: &str =
        include_str!("../../../crates/contracts/runtime-ports/src/local_workspace_snapshot.rs");

    assert!(!RUNTIME_SDK.contains("LocalWorkspaceSnapshot"));
    assert!(!LOCAL_SNAPSHOT_PORT.contains("remote_connection_id"));
    assert!(!LOCAL_SNAPSHOT_PORT.contains("remote_ssh_host"));
    assert!(!LOCAL_SNAPSHOT_PORT.contains("checkpoint_workspace"));
    assert!(!LOCAL_SNAPSHOT_PORT.contains("rewind_workspace"));
}

#[test]
fn primary_cli_session_client_uses_only_the_runtime_sdk_boundary() {
    const AGENT_MODULE: &str = include_str!("../src/agent/mod.rs");
    const PRIMARY_CLIENT: &str = include_str!("../src/agent/runtime_client.rs");

    assert!(
        !AGENT_MODULE.contains("trait Agent"),
        "a one-implementation private trait must not obscure the Runtime SDK client boundary"
    );
    assert!(
        !PRIMARY_CLIENT.contains("CoreAgentRuntimeCompatibility")
            && !PRIMARY_CLIENT.contains("compatibility:")
            && !PRIMARY_CLIENT.contains("is_turn_processing"),
        "the primary CLI/TUI session client must not depend on Core compatibility or state polling"
    );
    for sdk_operation in [
        "fork_session",
        "generate_session_usage",
        "wait_for_turn_settlement",
    ] {
        assert!(
            PRIMARY_CLIENT.contains(sdk_operation),
            "primary session client must route {sdk_operation} through the Runtime SDK"
        );
    }
}

#[test]
fn chat_context_reload_keeps_deployment_choice_behind_a_cli_adapter() {
    const CHAT_MODE: &str = include_str!("../src/modes/chat.rs");
    const CHAT_CAPABILITIES: &str = include_str!("../src/modes/chat/capabilities.rs");
    const RELOAD_CLIENT: &str = include_str!("../src/agent/context_reload_client.rs");

    assert!(
        CHAT_MODE.contains("context_reload: CliContextReloadClient"),
        "ChatMode must submit reload through one CLI-owned deployment adapter"
    );
    assert!(
        !CHAT_CAPABILITIES.contains("is_shared()")
            && !CHAT_CAPABILITIES.contains("reload_shared_session_context")
            && !CHAT_CAPABILITIES.contains("self.compatibility"),
        "TUI capability code must not branch context reload by Runtime deployment"
    );
    assert!(
        RELOAD_CLIENT.contains(".reload_session_context(request)")
            && RELOAD_CLIENT
                .contains(".request(RuntimeIpcOperation::ReloadSessionContext { request })"),
        "the private adapter must delegate directly to the existing Embedded and Shared owners"
    );
}

#[test]
fn primary_cli_runtime_client_covers_interactive_permission_and_local_turn_operations() {
    const PRIMARY_CLIENT: &str = include_str!("../src/agent/runtime_client.rs");

    for sdk_operation in [
        "subscribe_permission_requests",
        "pending_permission_requests",
        "respond_permission",
        "record_completed_local_command_turn",
    ] {
        assert!(
            PRIMARY_CLIENT.contains(sdk_operation),
            "interactive TUI operation {sdk_operation} must stay behind the existing runtime client"
        );
    }
}

#[test]
fn interactive_tui_agent_operations_stay_behind_cli_runtime_client() {
    const STARTUP_PAGE: &str = include_str!("../src/ui/startup.rs");
    const CHAT_MODE: &str = include_str!("../src/modes/chat.rs");
    const CHAT_RUN: &str = include_str!("../src/modes/chat/run.rs");
    const CHAT_COMMANDS: &str = include_str!("../src/modes/chat/commands.rs");
    const CHAT_INPUT: &str = include_str!("../src/modes/chat/input.rs");
    const CHAT_SELECTION: &str = include_str!("../src/modes/chat/selection.rs");
    const RUNTIME_CLIENT: &str = include_str!("../src/agent/runtime_client.rs");
    const SHARED_RUNTIME: &str = include_str!("../src/shared_runtime.rs");
    const CLI_MAIN: &str = include_str!("../src/main.rs");
    const CLI_CARGO: &str = include_str!("../Cargo.toml");

    assert!(
        !STARTUP_PAGE.contains("bitfun_agent_runtime::sdk::AgentRuntime"),
        "the startup controller must use the existing CLI runtime client instead of AgentRuntime"
    );
    assert!(
        !CHAT_MODE.contains("Arc<CliRuntimeContext>"),
        "ChatMode must not retain the whole Embedded runtime context"
    );
    for (path, source) in [
        ("modes/chat/run.rs", CHAT_RUN),
        ("modes/chat/input.rs", CHAT_INPUT),
        ("modes/chat/selection.rs", CHAT_SELECTION),
    ] {
        assert!(
            !source.contains(".agent_runtime()"),
            "{path} must route Agent operations through CliAgentRuntimeClient"
        );
    }
    assert!(
        CHAT_MODE.contains("CliAgentRuntimeClient"),
        "interactive chat must retain the existing app-private runtime client facade"
    );
    assert!(
        !CLI_CARGO.contains("bitfun-sdk-host") && CLI_CARGO.contains("bitfun-agent-runtime-ipc"),
        "Shared TUI must use the private Runtime IPC adapter without making CLI depend on SDK Host"
    );
    assert!(
        RUNTIME_CLIENT.contains("RuntimeIpcClient")
            && !STARTUP_PAGE.contains("RuntimeIpcClient")
            && !CHAT_MODE.contains("RuntimeIpcClient"),
        "Shared IPC must remain behind CliAgentRuntimeClient instead of leaking into TUI controllers"
    );
    assert!(
        RUNTIME_CLIENT.contains("RuntimeIpcOperation::UpdateSessionMode { request }")
            && SHARED_RUNTIME.contains("RuntimeIpcOperation::UpdateSessionMode { request }")
            && SHARED_RUNTIME.contains(".update_session_mode(request)"),
        "Shared Agent mode updates must reuse the Runtime port through the private IPC adapter"
    );
    assert!(
        RUNTIME_CLIENT.contains("RuntimeIpcOperation::UpdateSessionModel { request }")
            && SHARED_RUNTIME.contains("RuntimeIpcOperation::UpdateSessionModel { request }")
            && SHARED_RUNTIME.contains(".update_session_model(request)"),
        "Shared model updates must reuse the Runtime port through the private IPC adapter"
    );
    let shared_command_path = CHAT_COMMANDS
        .split_once("fn handle_command(")
        .expect("handle_command")
        .1;
    assert!(
        shared_command_path
            .find("if self.agent.is_shared()")
            .unwrap_or(usize::MAX)
            < shared_command_path
                .find("external_source_conflict_choices")
                .expect("external source call"),
        "Shared slash commands must branch before initializing Embedded external-source owners"
    );
    assert!(
        CHAT_COMMANDS.matches("if self.agent.is_shared()").count() >= 3
            && RUNTIME_CLIENT.contains("Failed to read Embedded session transcript")
            && SHARED_RUNTIME.contains("RuntimeDeployment::Shared")
            && SHARED_RUNTIME.contains("process_manager::contain_current_process_tree"),
        "Shared controls must stay terminal-safe while preserving Embedded recovery and one process Job owner"
    );
    assert!(
        CLI_MAIN.contains("Cli::command()") && CLI_MAIN.contains("McpAction::Import"),
        "interactive composition changes must preserve product-aware CLI identity and MCP import"
    );
}

#[test]
fn runtime_ownership_policy_is_assembled_once_in_core() {
    const SHARED_RUNTIME: &str = include_str!("../src/shared_runtime.rs");
    const CLI_RUNTIME: &str = include_str!("../src/runtime/mod.rs");
    const CLI_MAIN: &str = include_str!("../src/main.rs");
    const AGENTIC_SYSTEM: &str = include_str!("../src/agent/agentic_system.rs");

    for private_policy in [
        "RuntimeOwnershipKey::for_workspace",
        "WorkspaceRuntimeOwnership::try_acquire",
        "fn ownership_root",
        "fn product_identity",
        "pub(crate) fn acquire_ownership",
    ] {
        assert!(
            !SHARED_RUNTIME.contains(private_policy),
            "CLI must not duplicate Core ownership policy: {private_policy}"
        );
    }
    assert!(
        !CLI_RUNTIME.contains("WorkspaceRuntimeOwnership")
            && !CLI_RUNTIME.contains("_runtime_ownership"),
        "Coordinator must retain the Core owner; CliRuntimeContext must not keep a second guard"
    );
    assert!(
        CLI_MAIN.contains("CoreRuntimeOwnership")
            && AGENTIC_SYSTEM.contains("init_agentic_system_for_profile_with_runtime_ownership"),
        "CLI must select a deployment and inject the single Core owner"
    );
}

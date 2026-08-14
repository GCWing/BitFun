use std::process::Command;

fn rust_source_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(&path).expect("read CLI source directory") {
            let entry = entry.expect("read CLI source entry");
            let entry_path = entry.path();
            if entry_path.is_dir() {
                pending.push(entry_path);
            } else if entry_path
                .extension()
                .is_some_and(|extension| extension == "rs")
            {
                files.push(entry_path);
            }
        }
    }
    files
}

#[test]
fn embedded_composition_has_no_app_server_route() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let cli_manifest =
        std::fs::read_to_string(manifest_dir.join("Cargo.toml")).expect("read CLI Cargo.toml");

    for retired_dependency in [
        "bitfun-app-server =",
        "bitfun-app-server-client =",
        "bitfun-app-server-protocol =",
    ] {
        assert!(
            !cli_manifest.contains(retired_dependency),
            "Embedded direct-runtime composition must remove the CLI dependency {retired_dependency}"
        );
    }

    let retired_source_markers = [
        "EmbeddedAppServerHost",
        "AppServerTuiBackend",
        "pub(crate) trait TuiBackend",
        "Arc<dyn TuiBackend>",
        "in_memory_channel_pair",
        "bitfun-embedded-app-server",
        "BitfunAppServer::new",
    ];
    for source_path in rust_source_files(&manifest_dir.join("src")) {
        let source = std::fs::read_to_string(&source_path).expect("read CLI Rust source");
        for retired_marker in retired_source_markers {
            assert!(
                !source.contains(retired_marker),
                "Embedded direct-runtime composition must remove {retired_marker} from {}",
                source_path.display()
            );
        }
    }
    assert!(
        !manifest_dir.join("src/embedded_app_server.rs").exists(),
        "the retired Embedded App Server host module must be deleted"
    );

    const SHARED_RUNTIME: &str = include_str!("../../src/shared_runtime.rs");
    const RUNTIME_CLIENT: &str = include_str!("../../src/agent/runtime_client.rs");
    const RUNTIME_IPC_PROTOCOL: &str =
        include_str!("../../../../crates/adapters/agent-runtime-ipc/src/protocol.rs");
    assert!(
        cli_manifest.contains("bitfun-agent-runtime-ipc =")
            && SHARED_RUNTIME.contains("RuntimeIpcOperation")
            && RUNTIME_CLIENT.contains("TuiRuntimePort")
            && RUNTIME_CLIENT.contains("Shared(RuntimeIpcClient)")
            && RUNTIME_IPC_PROTOCOL.contains("PROTOCOL_VERSION: u32 = 18"),
        "Shared TUI must retain the private Runtime IPC v18 compatibility path"
    );

    let workspace_manifest = std::fs::read_to_string(manifest_dir.join("../../../Cargo.toml"))
        .expect("read workspace Cargo.toml");
    for retained_member in [
        "src/crates/interfaces/app-server",
        "src/crates/interfaces/app-server-client",
        "src/crates/interfaces/app-server-protocol",
    ] {
        assert!(
            workspace_manifest.contains(&format!("\"{retained_member}\"")),
            "Phase 5 must retain the workspace App Server member {retained_member}"
        );
    }
}

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
    const ACCOUNT_ADAPTER: &str = include_str!("../../src/account.rs");
    const ACCOUNT_RUNTIME: &str = include_str!(
        "../../../../crates/assembly/core/src/service/remote_connect/account_runtime.rs"
    );
    const STARTUP_PAGE: &str = include_str!("../../src/ui/startup.rs");
    const PEER_BOOTSTRAP: &str = include_str!("../../src/peer_host/bootstrap.rs");
    const PEER_STATE: &str = include_str!("../../src/peer_host/state.rs");
    const PEER_SESSION_COMMANDS: &str = include_str!("../../src/peer_host/commands/session.rs");
    const PEER_SNAPSHOT_COMMANDS: &str = include_str!("../../src/peer_host/commands/snapshot.rs");
    const CORE_RUNTIME_SERVICES: &str =
        include_str!("../../../../crates/assembly/core/src/product_runtime/runtime_services.rs");

    for (path, source) in [
        ("account.rs", ACCOUNT_ADAPTER),
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
        ACCOUNT_RUNTIME.contains("pub struct AccountRuntime")
            && ACCOUNT_ADAPTER.contains("impl AccountRuntimeHost for CliAccountRoutingHost")
            && ACCOUNT_ADAPTER.contains("impl AccountSessionBackupPort"),
        "account state must live in the shared owner while CLI keeps narrow Host adapters"
    );
    assert!(
        STARTUP_PAGE.contains("self.agent.account_snapshot()")
            && STARTUP_PAGE.contains("self.agent.account_login(")
            && STARTUP_PAGE.contains("self.agent.account_finalize_login(")
            && STARTUP_PAGE.contains("self.agent.settings_sync_start(")
            && STARTUP_PAGE.contains("self.agent.settings_sync_snapshot()")
            && STARTUP_PAGE.contains("self.agent.settings_sync_cancel()"),
        "startup account and settings-sync operations must use the typed TUI client"
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
fn embedded_account_management_composes_the_shared_owner_directly() {
    const CLI_MAIN: &str = include_str!("../../src/main.rs");
    const MANAGEMENT: &str = include_str!("../../src/tui_management.rs");
    const ACCOUNT_WORKTREE: &str = include_str!("../../src/tui_management/account_worktree.rs");

    assert!(
        CLI_MAIN.contains("Some(runtime.account_runtime().clone())")
            && MANAGEMENT.contains("AccountProvider::new(account.clone())")
            && MANAGEMENT.contains("SettingsSyncProvider::new(account)")
            && ACCOUNT_WORKTREE.contains("login_for_management")
            && ACCOUNT_WORKTREE.contains("finalize_login_for_management")
            && ACCOUNT_WORKTREE.contains("logout_for_management")
            && ACCOUNT_WORKTREE.contains("start_settings_sync_for_management")
            && ACCOUNT_WORKTREE.contains("cancel_settings_sync_for_management")
            && !MANAGEMENT.contains("trait TuiManagementPort")
            && !ACCOUNT_WORKTREE.contains("trait TuiManagementPort")
            && !CLI_MAIN.contains("AppManagementService"),
        "Embedded account management must compose AccountRuntime without another management owner"
    );
}

#[test]
fn peer_session_control_and_usage_persistence_use_runtime_sdk() {
    const PEER_SESSION_COMMANDS: &str = include_str!("../../src/peer_host/commands/session.rs");
    const CHAT_SELECTION: &str = include_str!("../../src/modes/chat/selection.rs");
    const CORE_PRODUCT_RUNTIME: &str =
        include_str!("../../../../crates/assembly/core/src/product_runtime.rs");

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
    // Inverted, along with the behaviour it described. `/usage` renders into
    // the conversation view and writes nothing: a report about a session is not
    // an event in it, and the Turn this used to persist was loaded back by the
    // desktop and given a numbered slot in its Turn rail. `add_assistant_message`
    // is the UI-only path — `turn_id: None`, never persisted — and in a terminal
    // the scrollback is the record.
    //
    // Source text only. This says the call is absent, not that nothing persists;
    // a behavioural guarantee would have to come from the runtime port's own
    // tests.
    assert!(
        !CHAT_SELECTION.contains("record_completed_local_command_turn"),
        "/usage must not write a local_command Turn: it renders into the          conversation view and persists nothing"
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
    const RUNTIME_SDK: &str = include_str!("../../../../crates/execution/agent-runtime/src/sdk.rs");
    const LOCAL_SNAPSHOT_PORT: &str =
        include_str!("../../../../crates/contracts/runtime-ports/src/local_workspace_snapshot.rs");

    assert!(!RUNTIME_SDK.contains("LocalWorkspaceSnapshot"));
    assert!(!LOCAL_SNAPSHOT_PORT.contains("remote_connection_id"));
    assert!(!LOCAL_SNAPSHOT_PORT.contains("remote_ssh_host"));
    assert!(!LOCAL_SNAPSHOT_PORT.contains("checkpoint_workspace"));
    assert!(!LOCAL_SNAPSHOT_PORT.contains("rewind_workspace"));
}

#[test]
fn interactive_tui_delegates_runtime_state_to_the_single_cli_client() {
    const TUI_CLIENT: &str = include_str!("../../src/agent/tui_client.rs");
    const RUNTIME_CLIENT: &str = include_str!("../../src/agent/runtime_client.rs");
    let tui_state = TUI_CLIENT
        .split_once("pub(crate) struct TuiAgentClient")
        .expect("TUI facade state")
        .1
        .split_once("impl TuiAgentClient")
        .expect("TUI facade implementation")
        .0;

    assert!(
        tui_state.contains("runtime: Arc<CliAgentRuntimeClient>")
            && !tui_state.contains("session_id:")
            && !tui_state.contains("current_turn_id:")
            && !tui_state.contains("approval_policy:")
            && !tui_state.contains("RuntimeIpcClient"),
        "the TUI facade must not rebuild Runtime state or transport behavior"
    );
    assert!(
        RUNTIME_CLIENT.contains("pub(crate) struct CliAgentRuntimeClient")
            && RUNTIME_CLIENT.contains("enum TuiRuntimePort")
            && RUNTIME_CLIENT.contains("Embedded(AgentRuntime)")
            && RUNTIME_CLIENT.contains("Shared(RuntimeIpcClient)"),
        "Direct and Shared deployments must share one stateful CLI Runtime client"
    );
    for operation in [
        "self.runtime.list_sessions()",
        "self.runtime.respond_permission(request_id, reply)",
        "self.runtime.fork_current_session(before_turn_id)",
        "self.runtime.generate_session_usage_report(request)",
    ] {
        assert!(
            TUI_CLIENT.contains(operation),
            "interactive Runtime operation must delegate through CliAgentRuntimeClient: {operation}"
        );
    }
}

#[test]
fn chat_context_reload_uses_the_same_runtime_client_as_session_operations() {
    const CHAT_MODE: &str = include_str!("../../src/modes/chat.rs");
    const CHAT_CAPABILITIES: &str = include_str!("../../src/modes/chat/capabilities.rs");
    const TUI_CLIENT: &str = include_str!("../../src/agent/tui_client.rs");

    assert!(
        !CHAT_MODE.contains("context_reload")
            && CHAT_CAPABILITIES.contains("self.agent.reload_context(request)"),
        "ChatMode must submit context reload through its existing TUI session client"
    );
    assert!(
        !CHAT_CAPABILITIES.contains("is_shared()")
            && !CHAT_CAPABILITIES.contains("reload_shared_session_context")
            && !CHAT_CAPABILITIES.contains("self.compatibility"),
        "TUI capability code must not branch context reload by Runtime deployment"
    );
    assert!(
        TUI_CLIENT.contains("self.runtime.reload_context(request).await"),
        "the TUI facade must delegate reload to the shared Runtime client"
    );
}

#[test]
fn tui_client_covers_interactive_permission_operations() {
    const TUI_CLIENT: &str = include_str!("../../src/agent/tui_client.rs");

    for sdk_operation in [
        "subscribe_permission_requests",
        "pending_permission_requests",
        "respond_permission",
    ] {
        assert!(
            TUI_CLIENT.contains(sdk_operation),
            "interactive TUI operation {sdk_operation} must stay behind TuiAgentClient"
        );
    }
}

#[test]
fn interactive_tui_runtime_and_management_boundaries_do_not_leak_into_controllers() {
    const STARTUP_PAGE: &str = include_str!("../../src/ui/startup.rs");
    const CHAT_MODE: &str = include_str!("../../src/modes/chat.rs");
    const CHAT_RUN: &str = include_str!("../../src/modes/chat/run.rs");
    const CHAT_COMMANDS: &str = include_str!("../../src/modes/chat/commands.rs");
    const CHAT_INPUT: &str = include_str!("../../src/modes/chat/input.rs");
    const CHAT_SELECTION: &str = include_str!("../../src/modes/chat/selection.rs");
    const TUI_CLIENT: &str = include_str!("../../src/agent/tui_client.rs");
    const RUNTIME_CLIENT: &str = include_str!("../../src/agent/runtime_client.rs");
    const MANAGEMENT: &str = include_str!("../../src/tui_management.rs");
    const SHARED_RUNTIME: &str = include_str!("../../src/shared_runtime.rs");
    const CLI_MAIN: &str = include_str!("../../src/main.rs");
    const CLI_CARGO: &str = include_str!("../../Cargo.toml");

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
            "{path} must route Agent operations through TuiAgentClient"
        );
    }
    assert!(
        CHAT_MODE.contains("Arc<TuiAgentClient>") && STARTUP_PAGE.contains("Arc<TuiAgentClient>"),
        "interactive chat and startup must use the backend-neutral TUI session client"
    );
    assert!(
        !CLI_CARGO.contains("bitfun-sdk-host") && CLI_CARGO.contains("bitfun-agent-runtime-ipc"),
        "Shared TUI must use the private Runtime IPC adapter without making CLI depend on SDK Host"
    );
    assert!(
        RUNTIME_CLIENT.contains("Shared(RuntimeIpcClient)")
            && !TUI_CLIENT.contains("RuntimeIpcClient")
            && !STARTUP_PAGE.contains("RuntimeIpcClient")
            && !CHAT_MODE.contains("RuntimeIpcClient"),
        "Shared IPC must remain in the Runtime adapter instead of leaking into TUI presentation"
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
    let external_source_methods = TUI_CLIENT
        .split_once("pub(crate) async fn external_source_snapshot")
        .expect("external source snapshot method")
        .1
        .split_once("pub(crate) async fn set_native_command_choice")
        .expect("external source method boundary")
        .0;
    assert!(
        external_source_methods.matches(".external_source").count() >= 3
            && external_source_methods.contains(".snapshot(")
            && external_source_methods.contains(".control(")
            && external_source_methods.contains(".review(")
            && CHAT_COMMANDS.contains("self.agent.external_source_snapshot(false)")
            && !CHAT_COMMANDS.contains("bitfun_core::external_sources"),
        "TUI external-source controllers must route reads and mutations through the owner provider"
    );
    assert!(
        CHAT_COMMANDS.matches("if self.agent.is_shared()").count() >= 3
            && MANAGEMENT.contains("pub(crate) struct TuiManagementOwners")
            && !MANAGEMENT.contains("trait TuiManagementPort")
            && SHARED_RUNTIME.contains("RuntimeDeployment::Shared")
            && SHARED_RUNTIME.contains("process_manager::contain_current_process_tree"),
        "Shared controls must stay terminal-safe while management remains a concrete owner composition"
    );
    assert!(
        CLI_MAIN.contains("Cli::command()") && CLI_MAIN.contains("McpAction::Import"),
        "interactive composition changes must preserve product-aware CLI identity and MCP import"
    );
}

#[test]
fn interactive_tui_hook_management_stays_behind_owner_providers() {
    const CHAT_HOOKS: &str = include_str!("../../src/modes/chat/external_hooks.rs");
    const CHAT_NATIVE_HOOKS: &str = include_str!("../../src/modes/chat/native_hooks.rs");
    const TUI_CLIENT: &str = include_str!("../../src/agent/tui_client.rs");
    const HOOK_MANAGEMENT: &str = include_str!("../../src/tui_management/hooks.rs");

    for operation in [
        "external_hook_snapshot",
        "external_hook_plan",
        "external_hook_apply",
        "external_hook_mutate",
        "native_hook_overview",
    ] {
        assert!(
            TUI_CLIENT.contains(operation) && CHAT_HOOKS.contains(&format!(".{operation}(")),
            "TUI Hook operation {operation} must route through TuiAgentClient"
        );
    }
    for direct_owner in [
        "bitfun_core::external_hooks",
        "bitfun_core::native_hooks",
        "bitfun_core::external_hook_import",
        "crate::hook_import::mutate",
    ] {
        assert!(
            !CHAT_HOOKS.contains(direct_owner) && !CHAT_NATIVE_HOOKS.contains(direct_owner),
            "TUI Hook controllers must not reference {direct_owner}"
        );
    }
    assert!(
        CHAT_HOOKS.contains("expected_revision")
            && HOOK_MANAGEMENT.contains("pub(crate) struct NativeHookProvider")
            && HOOK_MANAGEMENT.contains("pub(crate) struct ExternalHookProvider"),
        "Hook mutations must preserve stale-revision fencing and explicit owner routing"
    );
    assert!(
        !CHAT_HOOKS.contains("post_call_hooks")
            && !CHAT_NATIVE_HOOKS.contains("post_call_hooks")
            && !TUI_CLIENT.contains("post_call_hooks"),
        "compiled-in post-call Hooks must not enter the TUI management API"
    );
}

#[test]
fn interactive_tui_worktrees_stay_behind_the_owner_provider() {
    const WORKTREE_CONTROLLER: &str = include_str!("../../src/modes/chat/worktree.rs");
    const TUI_CLIENT: &str = include_str!("../../src/agent/tui_client.rs");
    const MANAGEMENT: &str = include_str!("../../src/tui_management.rs");
    const ACCOUNT_WORKTREE: &str = include_str!("../../src/tui_management/account_worktree.rs");
    const CLI_MAIN: &str = include_str!("../../src/main.rs");

    for direct_owner in [
        "GitService",
        "WorktreeService",
        "WorktreeSessionBindingRequest",
        "bitfun_core::",
        "self.agent.is_shared()",
    ] {
        assert!(
            !WORKTREE_CONTROLLER.contains(direct_owner),
            "Worktree controller must not reference {direct_owner}"
        );
    }
    for operation in [
        "worktree_repository_status",
        "worktree_bind_session",
        "worktree_release_session",
    ] {
        assert!(
            WORKTREE_CONTROLLER.contains(operation) && TUI_CLIENT.contains(operation),
            "Worktree operation {operation} must stay behind the TUI facade"
        );
    }
    assert!(
        ACCOUNT_WORKTREE.contains("WorktreeService::bind_session")
            && MANAGEMENT.contains("WorktreeProvider::local()")
            && ACCOUNT_WORKTREE
                .contains("Managed worktrees are not supported for remote workspaces"),
        "Embedded must compose the Worktree owner while remote bindings fail closed"
    );
    assert!(
        CLI_MAIN.contains("TuiManagementOwners::load(None, false)")
            && !CLI_MAIN.contains("AppManagementService"),
        "Shared Worktree management must remain unavailable without a local owner"
    );
}

#[test]
fn phase4_tui_management_boundaries_have_zero_legacy_owner_budget() {
    const CHAT_ACCOUNT: &str = include_str!("../../src/modes/chat/account.rs");
    const CHAT_HOOKS: &str = include_str!("../../src/modes/chat/external_hooks.rs");
    const CHAT_HOOK_REVIEW: &str = include_str!("../../src/modes/chat/external_review.rs");
    const CHAT_PROVIDER_MODELS: &str = include_str!("../../src/modes/chat/provider_models.rs");
    const CHAT_WORKTREE: &str = include_str!("../../src/modes/chat/worktree.rs");
    const STARTUP: &str = include_str!("../../src/ui/startup.rs");
    const BOUNDARY_RULES: &str =
        include_str!("../../../../../scripts/core-boundaries/rules/tui-boundary-rules.mjs");

    for (path, source, marker) in [
        ("chat/account.rs", CHAT_ACCOUNT, "crate::account::"),
        ("chat/account.rs", CHAT_ACCOUNT, "crate::account_sync::"),
        ("chat/external_hooks.rs", CHAT_HOOKS, "bitfun_core::"),
        ("chat/external_review.rs", CHAT_HOOK_REVIEW, "bitfun_core::"),
        (
            "chat/provider_models.rs",
            CHAT_PROVIDER_MODELS,
            "crate::account_sync::",
        ),
        ("chat/worktree.rs", CHAT_WORKTREE, "bitfun_core::"),
        ("ui/startup.rs", STARTUP, "bitfun_core::"),
        ("ui/startup.rs", STARTUP, "CoreAgentRuntimeCompatibility"),
        ("ui/startup.rs", STARTUP, "crate::account::"),
        ("ui/startup.rs", STARTUP, "crate::account_sync::"),
    ] {
        assert!(
            !source.contains(marker),
            "{path} must not reference {marker}"
        );
    }

    for budget in [
        "'src/apps/cli/src/modes/chat/account.rs': {",
        "'src/apps/cli/src/modes/chat/external_hooks.rs': { 'bitfun_core::': 0 }",
        "'src/apps/cli/src/modes/chat/external_review.rs': { 'bitfun_core::': 0 }",
        "'src/apps/cli/src/modes/chat/provider_models.rs': {",
        "'src/apps/cli/src/modes/chat/worktree.rs': { 'bitfun_core::': 0 },",
        "'src/apps/cli/src/ui/startup.rs': {",
    ] {
        assert!(
            BOUNDARY_RULES.contains(budget),
            "missing zero-budget rule: {budget}"
        );
    }
    for zero_budget in [
        "'crate::account::': 0",
        "'crate::account_sync::': 0",
        "'bitfun_core::': 0",
        "CoreAgentRuntimeCompatibility: 0",
    ] {
        assert!(
            BOUNDARY_RULES.contains(zero_budget),
            "Phase 4 migrated owner budget must stay at zero: {zero_budget}"
        );
    }
}

#[test]
fn runtime_ownership_policy_is_assembled_once_in_core() {
    const SHARED_RUNTIME: &str = include_str!("../../src/shared_runtime.rs");
    const CLI_RUNTIME: &str = include_str!("../../src/runtime/mod.rs");
    const CLI_MAIN: &str = include_str!("../../src/main.rs");
    const AGENTIC_SYSTEM: &str = include_str!("../../src/agent/agentic_system.rs");

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

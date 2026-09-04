#[test]
fn sdk_host_process_installs_a_rustls_crypto_provider() {
    openbitfun_sdk_host_app::initialize_process_runtime().expect("initialize SDK Host process");

    assert!(
        rustls::crypto::CryptoProvider::get_default().is_some(),
        "SDK Host must select a process-level crypto provider before HTTPS AI requests"
    );
}

#[cfg(windows)]
#[test]
fn sdk_host_process_job_reclaims_descendants_when_host_exits() {
    const TEST_NAME: &str = "sdk_host_process_job_reclaims_descendants_when_host_exits";
    const MODE_ENV: &str = "OPENBITFUN_SDK_HOST_JOB_TEST_MODE";
    const PID_FILE_ENV: &str = "OPENBITFUN_SDK_HOST_JOB_TEST_PID_FILE";

    match std::env::var(MODE_ENV).as_deref() {
        Ok("descendant") => loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        },
        Ok("owner") => {
            openbitfun_sdk_host_app::initialize_process_runtime()
                .expect("initialize owned SDK Host process");
            let descendant = openbitfun_services_core::process_manager::create_command(
                std::env::current_exe().unwrap(),
            )
            .args(["--exact", TEST_NAME, "--nocapture"])
            .env(MODE_ENV, "descendant")
            .spawn()
            .expect("spawn owned descendant");
            std::fs::write(
                std::env::var(PID_FILE_ENV).expect("PID file path"),
                descendant.id().to_string(),
            )
            .expect("record descendant PID");
            std::thread::sleep(std::time::Duration::from_millis(100));
            return;
        }
        _ => {}
    }

    let directory = tempfile::tempdir().expect("create job test directory");
    let pid_file = directory.path().join("descendant.pid");
    let status =
        openbitfun_services_core::process_manager::create_command(std::env::current_exe().unwrap())
            .args(["--exact", TEST_NAME, "--nocapture"])
            .env(MODE_ENV, "owner")
            .env(PID_FILE_ENV, &pid_file)
            .status()
            .expect("run process-tree owner fixture");
    assert!(status.success(), "process-tree owner fixture failed");
    let descendant_pid = std::fs::read_to_string(pid_file)
        .expect("read descendant PID")
        .trim()
        .to_string();
    std::thread::sleep(std::time::Duration::from_millis(200));

    let cleanup = openbitfun_services_core::process_manager::create_command("taskkill.exe")
        .args(["/pid", &descendant_pid, "/t", "/f"])
        .output()
        .expect("probe descendant process");
    assert!(
        !cleanup.status.success(),
        "SDK Host process exit left descendant {descendant_pid} alive"
    );
}

#[test]
fn sdk_host_process_uses_the_reviewed_worker_stack_contract() {
    let caller = std::thread::current().id();
    let worker = openbitfun_sdk_host_app::spawn_sdk_host_worker(|| std::thread::current().id())
        .expect("spawn SDK Host worker");

    assert_eq!(
        openbitfun_sdk_host_app::SDK_HOST_WORKER_STACK_BYTES,
        16 * 1024 * 1024
    );
    assert_ne!(worker.join().expect("join SDK Host worker"), caller);
}

#[test]
fn sdk_host_process_keeps_cleanup_warnings_on_stderr() {
    let entrypoint = include_str!("../src/main.rs");

    assert!(entrypoint.contains(".with_max_level(tracing::Level::WARN)"));
    assert!(entrypoint.contains(".with_writer(std::io::stderr)"));
}

#[test]
fn sdk_host_delegates_process_tree_ownership_to_services_core() {
    let manifest = include_str!("../Cargo.toml");
    let bootstrap = include_str!("../src/lib.rs");

    assert!(
        manifest.contains("openbitfun-services-core"),
        "SDK Host must depend on the reusable process lifecycle owner"
    );
    assert!(
        !manifest.contains("win32job"),
        "SDK Host must not own a second Windows Job Object implementation"
    );
    assert!(
        bootstrap
            .contains("openbitfun_services_core::process_manager::contain_current_process_tree"),
        "SDK Host bootstrap must delegate process-tree containment to services-core"
    );
}

#[test]
fn sdk_host_injects_core_ownership_before_runtime_initialization() {
    let runtime = include_str!("../src/runtime.rs");
    let ownership = runtime
        .find("CoreRuntimeOwnership::fixed_workspace")
        .expect("SDK Host Core ownership assembly");
    let initialize = runtime
        .find("init_agentic_system_for_profile_with_runtime_ownership")
        .expect("SDK Host ownership-aware AgenticSystem initialization");

    assert!(ownership < initialize);
    assert!(runtime.contains("RuntimeDeployment::Embedded"));
    assert!(!runtime.contains("WorkspaceRuntimeOwnership"));
}

use std::process::Command;

#[test]
fn doctor_reports_the_validated_cli_runtime_assembly() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("workspace");
    let user_root = temp.path().join("user-root");
    let home_root = temp.path().join("home-root");
    let config_root = temp.path().join("host-config");
    std::fs::create_dir_all(&workspace).expect("create workspace");

    let output = Command::new(env!("CARGO_BIN_EXE_bitfun-cli"))
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
        .expect("run bitfun-cli doctor");

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

    let output = Command::new(env!("CARGO_BIN_EXE_bitfun-cli"))
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
        .expect("run bitfun-cli health");

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

        let mut command = Command::new(env!("CARGO_BIN_EXE_bitfun-cli"));
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

        let output = command.output().expect("run bitfun-cli doctor");
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
fn cli_local_persistence_stays_behind_core_compatibility_facade() {
    const ACCOUNT_SYNC: &str = include_str!("../src/account_sync.rs");
    const STARTUP_PAGE: &str = include_str!("../src/ui/startup.rs");
    const CORE_RUNTIME_SERVICES: &str =
        include_str!("../../../crates/assembly/core/src/product_runtime/runtime_services.rs");

    for (path, source) in [
        ("account_sync.rs", ACCOUNT_SYNC),
        ("ui/startup.rs", STARTUP_PAGE),
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
}

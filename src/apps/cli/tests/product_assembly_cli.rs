use std::process::Command;

#[test]
fn doctor_reports_the_cli_product_plan_without_claiming_runtime_availability() {
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
        stdout.contains(
            "[info] Product profile: cli (static plan only; runtime readiness not evaluated)"
        ),
        "{stdout}"
    );
    for internal_state in [
        "Product assembly requirements:",
        "runtime services not connected",
        "Plugin runtime plan:",
        "not_built",
        "projection_only",
    ] {
        assert!(!stdout.contains(internal_state), "{stdout}");
    }
    assert!(
        stdout.contains(&format!("[ok] Config directory: {}", user_root.display())),
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

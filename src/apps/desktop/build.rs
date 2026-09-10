fn main() {
    println!("cargo:rerun-if-env-changed=OPENBITFUN_RELEASE_CHANNEL");
    println!("cargo:rerun-if-env-changed=OPENBITFUN_DESKTOP_BINARY_NAME");
    println!("cargo:rerun-if-env-changed=OPENBITFUN_DATA_MIGRATOR_BINARY_NAME");
    println!("cargo:rerun-if-env-changed=OPENBITFUN_UPDATER_PRIMARY_ENDPOINT");
    println!("cargo:rerun-if-env-changed=OPENBITFUN_UPDATER_FALLBACK_ENDPOINT");
    // The Windows primary thread keeps the Tauri event loop and native window
    // creation stack. Reserve the same headroom as the Tokio workers so a
    // large debug invoke dispatcher cannot exhaust the default 1 MiB stack.
    #[cfg(target_os = "windows")]
    println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    tauri_build::build();
}

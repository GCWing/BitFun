fn main() {
    // The Windows primary thread keeps the Tauri event loop and native window
    // creation stack. Reserve the same headroom as the Tokio workers so a
    // large debug invoke dispatcher cannot exhaust the default 1 MiB stack.
    #[cfg(target_os = "windows")]
    println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    tauri_build::build();
}

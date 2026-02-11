// Hide console window in Windows release builds
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    std::env::set_var("RUST_MIN_STACK", "8388608"); // 8MB
    bitfun_desktop_lib::run().await
}

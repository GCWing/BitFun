// Keep secondary launches from flashing a console window before the single-instance
// plugin redirects them back to the existing desktop process.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    // Tokio reads this value while creating its worker threads. Setting it in
    // the async body is too late, because the runtime has already been built.
    std::env::set_var("RUST_MIN_STACK", "8388608"); // 8 MiB worker stacks
    bitfun_desktop_lib::run()
}

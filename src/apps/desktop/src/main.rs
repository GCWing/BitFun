// Hide console window in Windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    bitfun_desktop_lib::run()
}

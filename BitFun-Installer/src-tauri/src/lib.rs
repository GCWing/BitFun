mod installer;

use installer::commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_launch_context,
            commands::get_default_install_path,
            commands::get_disk_space,
            commands::validate_install_path,
            commands::start_installation,
            commands::set_model_config,
            commands::set_theme_preference,
            commands::uninstall,
            commands::launch_application,
            commands::close_installer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running BitFun Installer");
}

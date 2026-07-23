#[cfg(feature = "tauri-adapter")]
pub mod tauri;

#[cfg(feature = "tauri-adapter")]
pub use tauri::TauriTransportAdapter;

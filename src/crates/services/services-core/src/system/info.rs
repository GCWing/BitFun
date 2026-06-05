//! System info utilities
//!
//! Provides system info retrieval.

/// System info
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemInfo {
    /// OS platform: "windows", "macos", "linux"
    pub platform: String,
    /// OS architecture: "x86_64", "aarch64", etc.
    pub arch: String,
    /// OS version
    pub os_version: Option<String>,
}

/// Gets system info.
///
/// # Returns
/// - `SystemInfo`: System info including platform and architecture
pub fn get_system_info() -> SystemInfo {
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "unknown"
    };

    SystemInfo {
        platform: platform.to_string(),
        arch: arch.to_string(),
        os_version: None,
    }
}

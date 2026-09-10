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
    /// User home on the host serving this request, never on its controller.
    #[serde(default)]
    pub home_dir: Option<String>,
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
        home_dir: std::env::home_dir().and_then(|path| path.into_os_string().into_string().ok()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_system_info_without_home_remains_readable() {
        let legacy =
            serde_json::json!({"platform": "windows", "arch": "x86_64", "os_version": null});
        let info: SystemInfo = serde_json::from_value(legacy.clone()).unwrap();
        assert!(info.home_dir.is_none());
        let round_trip: SystemInfo =
            serde_json::from_value(serde_json::to_value(info).unwrap()).unwrap();
        assert_eq!(round_trip.platform, legacy["platform"]);
        assert!(round_trip.home_dir.is_none());
    }

    #[test]
    fn reports_the_serving_hosts_home_directory() {
        assert_eq!(
            get_system_info().home_dir,
            std::env::home_dir().and_then(|path| path.into_os_string().into_string().ok())
        );
    }
}

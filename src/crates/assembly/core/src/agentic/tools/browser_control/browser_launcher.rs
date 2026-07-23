//! Compatibility facade for browser CDP launch and detection.
//!
//! Platform browser detection and launch process handling live in
//! `bitfun-services-integrations`. Core keeps this module to preserve the
//! existing product/tool import path and to inject BitFun's managed profile
//! directory.

use bitfun_services_integrations::browser_control::launcher as provider;
pub use provider::{
    BrowserInfo, BrowserKind, BrowserLaunchOptions, LaunchResult, DEFAULT_CDP_PORT,
};
use std::path::PathBuf;

use crate::infrastructure::app_paths::get_path_manager_arc;
use crate::util::errors::BitFunResult;

pub struct BrowserLauncher;

impl BrowserLauncher {
    pub async fn is_cdp_available(port: u16) -> bool {
        provider::BrowserLauncher::is_cdp_available(port).await
    }

    pub fn detect_default_browser() -> BitFunResult<BrowserKind> {
        Ok(provider::BrowserLauncher::detect_default_browser()?)
    }

    pub fn is_browser_installed(kind: &BrowserKind) -> bool {
        provider::BrowserLauncher::is_browser_installed(kind)
    }

    #[cfg(test)]
    pub fn clear_install_cache() {
        provider::BrowserLauncher::clear_install_cache()
    }

    pub fn browser_kind_from_cdp_version(version_str: &str) -> Option<BrowserKind> {
        provider::BrowserLauncher::browser_kind_from_cdp_version(version_str)
    }

    pub fn browser_kind_from_config(value: &str) -> Option<BrowserKind> {
        provider::BrowserLauncher::browser_kind_from_config(value)
    }

    pub fn resolve_browser_kind(preferred_browser: Option<&str>) -> BitFunResult<BrowserKind> {
        Ok(provider::BrowserLauncher::resolve_browser_kind(
            preferred_browser,
        )?)
    }

    pub fn browser_executable(kind: &BrowserKind) -> String {
        provider::BrowserLauncher::browser_executable(kind)
    }

    pub async fn launch_with_cdp(kind: &BrowserKind, port: u16) -> BitFunResult<LaunchResult> {
        Ok(provider::BrowserLauncher::launch_with_cdp_options(
            kind,
            port,
            Self::launch_options(None),
        )
        .await?)
    }

    pub async fn launch_with_cdp_opts(
        kind: &BrowserKind,
        port: u16,
        user_data_dir: Option<&str>,
    ) -> BitFunResult<LaunchResult> {
        Ok(provider::BrowserLauncher::launch_with_cdp_options(
            kind,
            port,
            Self::launch_options(user_data_dir),
        )
        .await?)
    }

    pub async fn restart_with_cdp(kind: &BrowserKind, port: u16) -> BitFunResult<LaunchResult> {
        Self::launch_with_cdp(kind, port).await
    }

    #[cfg(target_os = "macos")]
    pub fn create_cdp_launcher_app(kind: &BrowserKind, port: u16) -> BitFunResult<String> {
        Ok(provider::BrowserLauncher::create_cdp_launcher_app(
            kind, port,
        )?)
    }

    fn launch_options(user_data_dir: Option<&str>) -> BrowserLaunchOptions {
        BrowserLaunchOptions {
            user_data_dir: user_data_dir.map(PathBuf::from),
            managed_profile_root: Some(get_path_manager_arc().user_data_dir()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BrowserLauncher;
    use crate::infrastructure::app_paths::get_path_manager_arc;
    use std::path::PathBuf;

    #[test]
    fn launch_options_injects_bitfun_managed_profile_root() {
        let options = BrowserLauncher::launch_options(None);

        assert_eq!(options.user_data_dir, None);
        assert_eq!(
            options.managed_profile_root,
            Some(get_path_manager_arc().user_data_dir())
        );
    }

    #[test]
    fn launch_options_preserves_explicit_user_data_dir() {
        let explicit_profile = "custom-browser-profile";
        let options = BrowserLauncher::launch_options(Some(explicit_profile));

        assert_eq!(options.user_data_dir, Some(PathBuf::from(explicit_profile)));
        assert_eq!(
            options.managed_profile_root,
            Some(get_path_manager_arc().user_data_dir())
        );
    }
}

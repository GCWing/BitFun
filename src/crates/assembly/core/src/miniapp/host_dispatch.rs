//! Compatibility adapter for MiniApp host primitive dispatch.
//!
//! Concrete fs/shell/net/os dispatch lives in `openbitfun-services-integrations`.

use crate::miniapp::types::MiniAppPermissions;
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub use openbitfun_services_integrations::miniapp::host_dispatch::is_host_primitive;

pub async fn dispatch_host(
    perms: &MiniAppPermissions,
    app_id: &str,
    app_data_dir: &Path,
    workspace_dir: Option<&Path>,
    granted_paths: &[PathBuf],
    method: &str,
    params: Value,
) -> OpenBitFunResult<Value> {
    openbitfun_services_integrations::miniapp::host_dispatch::dispatch_host(
        perms,
        app_id,
        app_data_dir,
        workspace_dir,
        granted_paths,
        method,
        params,
    )
    .await
    .map_err(map_host_dispatch_error)
}

fn map_host_dispatch_error(
    err: openbitfun_services_integrations::miniapp::host_dispatch::MiniAppHostDispatchError,
) -> OpenBitFunError {
    use openbitfun_services_integrations::miniapp::host_dispatch::MiniAppHostDispatchErrorKind;

    match err.kind() {
        MiniAppHostDispatchErrorKind::Parse => OpenBitFunError::parse(err.message().to_string()),
        MiniAppHostDispatchErrorKind::Validation => {
            OpenBitFunError::validation(err.message().to_string())
        }
        MiniAppHostDispatchErrorKind::Io => OpenBitFunError::io(err.message().to_string()),
        MiniAppHostDispatchErrorKind::Service => {
            OpenBitFunError::service(err.message().to_string())
        }
    }
}

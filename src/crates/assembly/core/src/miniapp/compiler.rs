//! MiniApp compiler compatibility facade.

pub use openbitfun_product_domains::miniapp::compiler::{
    MiniAppCompileError, MiniAppCompileRequest, MiniAppCompileResult,
};

use crate::miniapp::types::{MiniAppPermissions, MiniAppSource};
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};

/// Compile MiniApp source into full HTML with Import Map, Runtime Adapter, and CSP injected.
pub fn compile(
    source: &MiniAppSource,
    permissions: &MiniAppPermissions,
    app_id: &str,
    app_data_dir: &str,
    workspace_dir: &str,
    appearance_mode: &str,
) -> OpenBitFunResult<String> {
    openbitfun_product_domains::miniapp::compiler::compile(
        source,
        permissions,
        app_id,
        app_data_dir,
        workspace_dir,
        appearance_mode,
    )
    .map_err(|e| OpenBitFunError::validation(e.to_string()))
}

pub fn compile_with_request(
    source: &MiniAppSource,
    permissions: &MiniAppPermissions,
    request: &MiniAppCompileRequest,
) -> OpenBitFunResult<String> {
    openbitfun_product_domains::miniapp::compiler::compile_with_request(
        source,
        permissions,
        request,
    )
    .map_err(|e| OpenBitFunError::validation(e.to_string()))
}

pub fn compile_market_with_request(
    source: &MiniAppSource,
    permissions: &MiniAppPermissions,
    request: &MiniAppCompileRequest,
) -> OpenBitFunResult<String> {
    openbitfun_product_domains::miniapp::compiler::compile_market_with_request(
        source,
        permissions,
        request,
    )
    .map_err(|e| OpenBitFunError::validation(e.to_string()))
}

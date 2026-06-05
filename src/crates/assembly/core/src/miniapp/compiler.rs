//! MiniApp compiler compatibility facade.

pub use bitfun_product_domains::miniapp::compiler::{MiniAppCompileError, MiniAppCompileResult};

use crate::miniapp::types::{MiniAppPermissions, MiniAppSource};
use crate::util::errors::{BitFunError, BitFunResult};

/// Compile MiniApp source into full HTML with Import Map, Runtime Adapter, and CSP injected.
pub fn compile(
    source: &MiniAppSource,
    permissions: &MiniAppPermissions,
    app_id: &str,
    app_data_dir: &str,
    workspace_dir: &str,
    theme: &str,
) -> BitFunResult<String> {
    bitfun_product_domains::miniapp::compiler::compile(
        source,
        permissions,
        app_id,
        app_data_dir,
        workspace_dir,
        theme,
    )
    .map_err(|e| BitFunError::validation(e.to_string()))
}

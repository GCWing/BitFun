//! System HostInvoke handlers.

use serde_json::{json, Value};

pub(crate) async fn get_system_info() -> Result<Value, String> {
    let info = bitfun_core::service::system::get_system_info();
    Ok(json!({
        "platform": info.platform,
        "arch": info.arch,
        "osVersion": info.os_version,
    }))
}

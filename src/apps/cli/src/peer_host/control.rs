//! Peer Mode control-plane subscribers (attach / detach / ping).

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use bitfun_core::service::remote_connect::DeviceIdentity;

static CONTROL_SUBSCRIBERS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn control_subscribers() -> &'static Mutex<HashSet<String>> {
    CONTROL_SUBSCRIBERS.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn attach_controller(device_id: String) {
    if device_id.trim().is_empty() {
        return;
    }
    if let Ok(mut set) = control_subscribers().lock() {
        set.insert(device_id);
    }
}

pub(crate) fn detach_controller(device_id: &str) {
    if let Ok(mut set) = control_subscribers().lock() {
        set.remove(device_id);
    }
}

pub(crate) fn attached_controllers() -> Vec<String> {
    control_subscribers()
        .lock()
        .map(|set| set.iter().cloned().collect())
        .unwrap_or_default()
}

pub(crate) fn peer_mode_ping_value() -> Value {
    let device_id = DeviceIdentity::from_current_machine()
        .map(|d| d.device_id)
        .unwrap_or_else(|_| "unknown".to_string());
    json!({
        "ok": true,
        "peer": true,
        "device_id": device_id,
    })
}

pub(crate) fn parse_controller_device_id(args: &Value) -> String {
    args.get("controllerDeviceId")
        .or_else(|| args.get("controller_device_id"))
        .or_else(|| {
            args.get("request").and_then(|req| {
                req.get("controllerDeviceId")
                    .or_else(|| req.get("controller_device_id"))
            })
        })
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

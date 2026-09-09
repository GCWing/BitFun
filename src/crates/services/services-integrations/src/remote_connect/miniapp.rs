//! Optional product-host adapter for mobile H5 MiniApps. No host means unsupported.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, OnceLock};

pub const CAPABILITY: &str = "miniapp_h5_v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum MiniAppRequest {
    List,
    Open {
        app_id: String,
    },
    Call {
        app_id: String,
        version: u32,
        method: String,
        #[serde(default)]
        params: Value,
    },
}

#[async_trait::async_trait]
pub trait RemoteMiniAppHost: Send + Sync {
    async fn execute(&self, request: &MiniAppRequest) -> Result<Value, String>;
}

static HOST: OnceLock<Arc<dyn RemoteMiniAppHost>> = OnceLock::new();

pub fn register_host(host: Arc<dyn RemoteMiniAppHost>) -> Result<(), String> {
    HOST.set(host)
        .map_err(|_| "MiniApp remote host already registered".to_string())
}

pub fn is_available() -> bool {
    HOST.get().is_some()
}

pub async fn dispatch(request: &MiniAppRequest) -> super::RemoteResponse {
    let result = match HOST.get() {
        Some(host) => host.execute(request).await,
        None => Err("H5 MiniApps are not supported on this host".to_string()),
    };
    match result {
        Ok(value) => super::RemoteResponse::MiniappResult { value },
        Err(message) => super::RemoteResponse::Error { message },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hosts_without_an_adapter_return_an_explicit_unsupported_response() {
        assert!(!is_available());
        let response = dispatch(&MiniAppRequest::List).await;
        assert!(
            matches!(response, super::super::RemoteResponse::Error { message }
            if message.contains("not supported"))
        );
    }

    #[test]
    fn miniapp_wire_roundtrip_and_legacy_commands() {
        let legacy = r#"{"cmd":"ping"}"#;
        let command: super::super::RemoteCommand = serde_json::from_str(legacy).unwrap();
        assert_eq!(serde_json::to_string(&command).unwrap(), legacy);
        let payload = serde_json::json!({"cmd":"miniapp", "request": {
            "action":"call", "app_id":"app-1", "version":1, "method":"storage.get"
        }});
        let command: super::super::RemoteCommand = serde_json::from_value(payload).unwrap();
        let roundtrip = serde_json::to_value(command).unwrap();
        assert_eq!(roundtrip["request"]["params"], Value::Null);
        assert_eq!(roundtrip["request"]["app_id"], "app-1");
        assert!(serde_json::from_value::<MiniAppRequest>(
            serde_json::json!({"action":"invoke_arbitrary"})
        )
        .is_err());
    }
}

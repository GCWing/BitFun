//! Process-level registration for the platform-agnostic product-control port.

use std::sync::{Arc, OnceLock};

use serde_json::Value;

pub use bitfun_product_domains::product_control::{
    ProductControlAction, ProductControlPort, ProductControlRequest as BitFunControlHostRequest,
    ProductControlSource,
};

static BITFUN_CONTROL_PORT: OnceLock<Arc<dyn ProductControlPort>> = OnceLock::new();

/// Register the product adapter. The first registered host owns the process.
pub fn set_bitfun_control_port(port: Arc<dyn ProductControlPort>) {
    let _ = BITFUN_CONTROL_PORT.set(port);
}

pub fn bitfun_control_host_available() -> bool {
    BITFUN_CONTROL_PORT.get().is_some()
}

pub async fn invoke_bitfun_control(request: BitFunControlHostRequest) -> Result<Value, String> {
    let Some(port) = BITFUN_CONTROL_PORT.get() else {
        return Err("BitFunControl host is not available on this product surface".to_string());
    };
    port.invoke(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn host_request_uses_the_frontend_camel_case_contract() {
        let serialized = serde_json::to_value(BitFunControlHostRequest {
            action: ProductControlAction::Open,
            query: None,
            capability_id: Some("setting.application.input".to_string()),
            item_id: Some("shortcut-browser".to_string()),
            operation_id: None,
            option_id: None,
            arguments: None,
            value: None,
            cursor: None,
            limit: None,
            source: ProductControlSource::Agent,
        })
        .unwrap();

        assert_eq!(
            serialized["capabilityId"],
            json!("setting.application.input")
        );
        assert_eq!(serialized["itemId"], json!("shortcut-browser"));
        assert!(serialized.get("capability_id").is_none());
        assert!(serialized.get("item_id").is_none());
    }
}

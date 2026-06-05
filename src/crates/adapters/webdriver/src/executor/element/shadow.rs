use serde_json::Value;

use crate::executor::BridgeExecutor;
use crate::runtime::api;
use crate::server::response::WebDriverErrorResponse;

impl BridgeExecutor {
    pub async fn get_shadow_root(&self, element_id: &str) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_element_value(
            self.state.clone(),
            &self.session.id,
            api::element::get_shadow_root(),
            element_id,
        )
        .await
    }

    pub async fn find_elements_from_shadow(
        &self,
        shadow_id: &str,
        using: &str,
        value: &str,
    ) -> Result<Vec<Value>, WebDriverErrorResponse> {
        api::element::exec_find_elements_from_shadow(
            self.state.clone(),
            &self.session.id,
            shadow_id,
            using,
            value,
        )
        .await
    }

    pub async fn validate_frame_index(&self, index: u32) -> Result<(), WebDriverErrorResponse> {
        api::element::exec_validate_frame_index(self.state.clone(), &self.session.id, index).await
    }

    pub async fn validate_frame_element(
        &self,
        element_id: &str,
    ) -> Result<(), WebDriverErrorResponse> {
        api::element::exec_validate_frame_element(self.state.clone(), &self.session.id, element_id)
            .await
    }
}

use crate::executor::BridgeExecutor;
use crate::platform::{self, ElementScreenshotMetadata};
use crate::runtime::api;
use crate::server::response::WebDriverErrorResponse;

impl BridgeExecutor {
    pub async fn click_element_by_id(
        &self,
        element_id: &str,
    ) -> Result<(), WebDriverErrorResponse> {
        api::element::exec_element_action(
            self.state.clone(),
            &self.session.id,
            api::element::click(),
            element_id,
        )
        .await
    }

    pub async fn clear_element_by_id(
        &self,
        element_id: &str,
    ) -> Result<(), WebDriverErrorResponse> {
        api::element::exec_element_action(
            self.state.clone(),
            &self.session.id,
            api::element::clear(),
            element_id,
        )
        .await
    }

    pub async fn send_keys_to_element(
        &self,
        element_id: &str,
        text: &str,
    ) -> Result<(), WebDriverErrorResponse> {
        api::element::exec_element_text_action(
            self.state.clone(),
            &self.session.id,
            element_id,
            text,
        )
        .await
    }

    pub async fn take_element_screenshot(
        &self,
        element_id: &str,
    ) -> Result<String, WebDriverErrorResponse> {
        let metadata = api::element::exec_screenshot_metadata(
            self.state.clone(),
            &self.session.id,
            element_id,
        )
        .await?;
        let metadata: ElementScreenshotMetadata =
            serde_json::from_value(metadata).map_err(|error| {
                WebDriverErrorResponse::unknown_error(format!(
                    "Failed to decode element screenshot metadata: {error}"
                ))
            })?;

        let screenshot = self.take_screenshot().await?;
        platform::crop_screenshot(screenshot, metadata)
    }
}

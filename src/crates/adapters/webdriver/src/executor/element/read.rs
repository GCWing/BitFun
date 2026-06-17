use serde_json::Value;

use crate::executor::BridgeExecutor;
use crate::runtime::api;
use crate::server::response::WebDriverErrorResponse;

impl BridgeExecutor {
    pub async fn is_element_selected(
        &self,
        element_id: &str,
    ) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_element_flag(
            self.state.clone(),
            &self.session.id,
            api::element::is_selected(),
            element_id,
        )
        .await
    }

    pub async fn is_element_displayed(
        &self,
        element_id: &str,
    ) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_element_flag(
            self.state.clone(),
            &self.session.id,
            api::element::is_displayed(),
            element_id,
        )
        .await
    }

    pub async fn get_element_attribute(
        &self,
        element_id: &str,
        name: &str,
    ) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_element_name_value(
            self.state.clone(),
            &self.session.id,
            api::element::get_attribute(),
            element_id,
            name,
        )
        .await
    }

    pub async fn get_element_property(
        &self,
        element_id: &str,
        name: &str,
    ) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_element_name_value(
            self.state.clone(),
            &self.session.id,
            api::element::get_property(),
            element_id,
            name,
        )
        .await
    }

    pub async fn get_element_css_value(
        &self,
        element_id: &str,
        property_name: &str,
    ) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_element_name_value(
            self.state.clone(),
            &self.session.id,
            api::element::get_css_value(),
            element_id,
            property_name,
        )
        .await
    }

    pub async fn get_element_text(
        &self,
        element_id: &str,
    ) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_element_value(
            self.state.clone(),
            &self.session.id,
            api::element::get_text(),
            element_id,
        )
        .await
    }

    pub async fn get_element_computed_role(
        &self,
        element_id: &str,
    ) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_element_value(
            self.state.clone(),
            &self.session.id,
            api::element::get_computed_role(),
            element_id,
        )
        .await
    }

    pub async fn get_element_computed_label(
        &self,
        element_id: &str,
    ) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_element_value(
            self.state.clone(),
            &self.session.id,
            api::element::get_computed_label(),
            element_id,
        )
        .await
    }

    pub async fn get_element_name(
        &self,
        element_id: &str,
    ) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_element_value(
            self.state.clone(),
            &self.session.id,
            api::element::get_name(),
            element_id,
        )
        .await
    }

    pub async fn get_element_rect(
        &self,
        element_id: &str,
    ) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_element_value(
            self.state.clone(),
            &self.session.id,
            api::element::get_rect(),
            element_id,
        )
        .await
    }

    pub async fn is_element_enabled(
        &self,
        element_id: &str,
    ) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_element_flag(
            self.state.clone(),
            &self.session.id,
            api::element::is_enabled(),
            element_id,
        )
        .await
    }
}

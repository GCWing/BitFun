use serde_json::Value;

use super::BridgeExecutor;
use crate::runtime::api;
use crate::server::response::WebDriverErrorResponse;

impl BridgeExecutor {
    pub async fn perform_actions(&self, actions: &[Value]) -> Result<(), WebDriverErrorResponse> {
        api::interaction::exec_perform_actions(self.state.clone(), &self.session.id, actions).await
    }

    pub async fn release_actions(
        &self,
        pressed_keys: Vec<String>,
        pressed_buttons: Vec<Value>,
    ) -> Result<(), WebDriverErrorResponse> {
        api::interaction::exec_release_actions(
            self.state.clone(),
            &self.session.id,
            pressed_keys,
            pressed_buttons,
        )
        .await
    }

    pub async fn dismiss_alert(&self) -> Result<(), WebDriverErrorResponse> {
        api::interaction::exec_alert_action(
            self.state.clone(),
            &self.session.id,
            api::interaction::dismiss_alert(),
        )
        .await
    }

    pub async fn accept_alert(&self) -> Result<(), WebDriverErrorResponse> {
        api::interaction::exec_alert_action(
            self.state.clone(),
            &self.session.id,
            api::interaction::accept_alert(),
        )
        .await
    }

    pub async fn get_alert_text(&self) -> Result<Value, WebDriverErrorResponse> {
        api::interaction::exec_alert_text(self.state.clone(), &self.session.id).await
    }

    pub async fn send_alert_text(&self, text: &str) -> Result<(), WebDriverErrorResponse> {
        api::interaction::exec_send_alert_text(self.state.clone(), &self.session.id, text).await
    }

    pub async fn take_logs(&self) -> Result<Value, WebDriverErrorResponse> {
        api::interaction::exec_take_logs(self.state.clone(), &self.session.id).await
    }
}

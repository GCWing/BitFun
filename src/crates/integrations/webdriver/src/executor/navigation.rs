use std::time::Duration;

use serde_json::Value;
use tokio::time::Instant;

use super::BridgeExecutor;
use crate::runtime::api;
use crate::server::response::WebDriverErrorResponse;

impl BridgeExecutor {
    pub async fn navigate_to(&self, url: &str) -> Result<(), WebDriverErrorResponse> {
        api::navigation::exec_navigation_action(
            self.state.clone(),
            &self.session.id,
            api::navigation::navigate_to(),
            vec![Value::String(url.to_string())],
        )
        .await
    }

    pub async fn go_back(&self) -> Result<(), WebDriverErrorResponse> {
        api::navigation::exec_navigation_action(
            self.state.clone(),
            &self.session.id,
            api::navigation::go_back(),
            Vec::new(),
        )
        .await
    }

    pub async fn go_forward(&self) -> Result<(), WebDriverErrorResponse> {
        api::navigation::exec_navigation_action(
            self.state.clone(),
            &self.session.id,
            api::navigation::go_forward(),
            Vec::new(),
        )
        .await
    }

    pub async fn refresh_page(&self) -> Result<(), WebDriverErrorResponse> {
        api::navigation::exec_navigation_action(
            self.state.clone(),
            &self.session.id,
            api::navigation::refresh(),
            Vec::new(),
        )
        .await
    }

    pub async fn get_title(&self) -> Result<Value, WebDriverErrorResponse> {
        api::navigation::exec_document_value(
            self.state.clone(),
            &self.session.id,
            api::navigation::title(),
        )
        .await
    }

    pub async fn get_source(&self) -> Result<Value, WebDriverErrorResponse> {
        api::navigation::exec_document_value(
            self.state.clone(),
            &self.session.id,
            api::navigation::source(),
        )
        .await
    }

    pub async fn wait_for_page_load(&self) -> Result<(), WebDriverErrorResponse> {
        let page_load_timeout = Duration::from_millis(self.session.timeouts.page_load);
        if page_load_timeout.is_zero() {
            return Ok(());
        }

        let poll_interval = Duration::from_millis(50);
        let deadline = Instant::now() + page_load_timeout;

        loop {
            match api::navigation::exec_document_value(
                self.state.clone(),
                &self.session.id,
                api::navigation::ready_state(),
            )
            .await
            {
                Ok(Value::String(ready_state)) if ready_state == "complete" => return Ok(()),
                Ok(_) => {}
                Err(error) if should_retry_page_load(&error) => {}
                Err(error) => return Err(error),
            }

            if Instant::now() >= deadline {
                return Err(WebDriverErrorResponse::timeout(format!(
                    "Page load timed out after {}ms",
                    self.session.timeouts.page_load
                )));
            }

            tokio::time::sleep(
                poll_interval.min(deadline.saturating_duration_since(Instant::now())),
            )
            .await;
        }
    }
}

fn should_retry_page_load(error: &WebDriverErrorResponse) -> bool {
    matches!(error.error.as_str(), "javascript error" | "unknown error")
}

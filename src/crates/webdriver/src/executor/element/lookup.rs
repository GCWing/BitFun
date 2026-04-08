use std::time::Duration;

use serde_json::Value;
use tokio::time::Instant;

use crate::executor::BridgeExecutor;
use crate::runtime::api;
use crate::server::response::WebDriverErrorResponse;
use crate::webdriver::LocatorStrategy;

impl BridgeExecutor {
    pub async fn find_elements(
        &self,
        root_element_id: Option<String>,
        using: &str,
        value: &str,
    ) -> Result<Vec<Value>, WebDriverErrorResponse> {
        let strategy = LocatorStrategy::try_from(using)?;
        let poll_interval = Duration::from_millis(50);
        let implicit_timeout = Duration::from_millis(self.session.timeouts.implicit);
        let deadline = Instant::now() + implicit_timeout;

        loop {
            let result = api::element::exec_find_elements(
                self.state.clone(),
                &self.session.id,
                root_element_id.clone(),
                strategy.as_str(),
                value,
            )
            .await?;

            if !result.is_empty() || Instant::now() >= deadline {
                return Ok(result);
            }

            tokio::time::sleep(
                poll_interval.min(deadline.saturating_duration_since(Instant::now())),
            )
            .await;
        }
    }

    pub async fn get_active_element(&self) -> Result<Value, WebDriverErrorResponse> {
        api::element::exec_active_element(self.state.clone(), &self.session.id).await
    }
}

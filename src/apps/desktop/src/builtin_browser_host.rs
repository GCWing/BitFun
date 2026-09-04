//! Desktop adapter for the built-in browser automation host port.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use openbitfun_core::agentic::tools::browser_control::builtin_browser::{
    set_builtin_browser_host, BuiltInBrowserCommand, BuiltInBrowserHost, BuiltInBrowserOpenRequest,
    BuiltInBrowserTarget,
};
use openbitfun_core::infrastructure::events::{emit_global_event, BackendEvent};
use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::sync::RwLock;

use crate::api::browser_api;

const OPEN_BUILT_IN_BROWSER_EVENT: &str = "agentic://open-built-in-browser";
const OPEN_BROWSER_READY_TIMEOUT: Duration = Duration::from_secs(20);
static NEXT_OPEN_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

struct DesktopBuiltInBrowserHost {
    app: AppHandle,
    drivers: RwLock<HashMap<String, Arc<openbitfun_webdriver::EmbeddedWebviewAutomation>>>,
}

impl DesktopBuiltInBrowserHost {
    fn new(app: AppHandle) -> Self {
        Self {
            app,
            drivers: RwLock::new(HashMap::new()),
        }
    }

    async fn driver(
        &self,
        target_id: &str,
    ) -> Result<Arc<openbitfun_webdriver::EmbeddedWebviewAutomation>, String> {
        browser_api::validate_browser_label(target_id)?;
        if let Some(driver) = self.drivers.read().await.get(target_id).cloned() {
            return Ok(driver);
        }
        let driver = Arc::new(
            openbitfun_webdriver::EmbeddedWebviewAutomation::attach(self.app.clone(), target_id)
                .await?,
        );
        self.drivers
            .write()
            .await
            .insert(target_id.to_string(), driver.clone());
        Ok(driver)
    }
}

#[async_trait]
impl BuiltInBrowserHost for DesktopBuiltInBrowserHost {
    async fn open(
        &self,
        request: BuiltInBrowserOpenRequest,
    ) -> Result<BuiltInBrowserTarget, String> {
        let request_id = format!(
            "builtin-browser-open-{}-{}",
            std::process::id(),
            NEXT_OPEN_REQUEST_ID.fetch_add(1, Ordering::Relaxed)
        );
        emit_global_event(BackendEvent::Custom {
            event_name: OPEN_BUILT_IN_BROWSER_EVENT.to_string(),
            payload: json!({
                "requestId": request_id.clone(),
                "url": request.url,
                "title": request.title,
                "replaceExisting": request.replace_existing,
            }),
        })
        .await
        .map_err(|error| format!("failed to present the built-in browser: {error}"))?;

        let deadline = tokio::time::Instant::now() + OPEN_BROWSER_READY_TIMEOUT;
        loop {
            if let Some(target) =
                browser_api::browser_target_for_open_request(&self.app, &request_id)
            {
                return Ok(target);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(
                    "the browser panel was requested, but its correlated native WebView did not become ready for Agent control"
                        .to_string(),
                );
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn list_targets(&self) -> Result<Vec<BuiltInBrowserTarget>, String> {
        let mut targets = browser_api::list_browser_targets(&self.app);
        let live_ids = targets
            .iter()
            .map(|target| target.id.clone())
            .collect::<HashSet<_>>();
        let stale_drivers = {
            let mut drivers = self.drivers.write().await;
            let stale_ids = drivers
                .keys()
                .filter(|target_id| !live_ids.contains(*target_id))
                .cloned()
                .collect::<Vec<_>>();
            stale_ids
                .into_iter()
                .filter_map(|target_id| drivers.remove(&target_id))
                .collect::<Vec<_>>()
        };
        for driver in stale_drivers {
            driver.detach().await;
        }
        for target in &mut targets {
            if target.title.is_empty() {
                if let Ok(driver) = self.driver(&target.id).await {
                    if let Ok(result) = driver
                        .execute(
                            "Runtime.evaluate",
                            Some(json!({
                                "expression": "({ title: document.title, url: window.location.href })",
                                "returnByValue": true,
                                "awaitPromise": true,
                            })),
                        )
                        .await
                    {
                        if let Some(identity) = result.pointer("/result/value") {
                            if let Some(url) = identity.get("url").and_then(Value::as_str) {
                                target.url = url.to_string();
                            }
                            if let Some(title) = identity.get("title").and_then(Value::as_str) {
                                target.title = title.to_string();
                            }
                            browser_api::update_browser_target_metadata(
                                &target.id,
                                &target.url,
                                &target.title,
                            );
                        }
                    }
                }
            }
            if target.title.is_empty() {
                target.title = "Built-in browser".to_string();
            }
        }
        Ok(targets)
    }

    async fn execute(&self, command: BuiltInBrowserCommand) -> Result<Value, String> {
        let driver = self.driver(&command.target_id).await?;
        let result = driver.execute(&command.method, command.params).await;
        if command.method == "Page.close" && result.is_ok() {
            self.drivers.write().await.remove(&command.target_id);
            driver.detach().await;
            browser_api::unregister_browser_target(&command.target_id);
        }
        result
    }
}

pub(crate) fn install(app: AppHandle) {
    set_builtin_browser_host(Arc::new(DesktopBuiltInBrowserHost::new(app)));
}

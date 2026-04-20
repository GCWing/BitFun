//! Browser session registry — addresses CDP pages by stable session id and
//! removes the previous "single global slot" footgun.
//!
//! ## Why
//!
//! The Phase-0 ControlHub kept exactly **one** `Option<CdpClient>` in a
//! `OnceLock<RwLock<…>>`. Every `connect` / `switch_page` clobbered the
//! slot, and every concurrent action raced on it. A second user task that
//! switched to a different tab would silently steal the connection from
//! the first task and break its in-flight `wait` / lifecycle subscription.
//!
//! ## Model
//!
//! - Each connected page is a `BrowserSession` keyed by `session_id` (the
//!   CDP page id, which is stable for the page's lifetime).
//! - The registry tracks an optional **default** session for backward
//!   compatibility with callers that omit `session_id`.
//! - All sessions are reachable via `Arc<CdpClient>` so concurrent actions
//!   on the *same* page share one WebSocket while sessions on *different*
//!   pages stay isolated.
//!
//! ## Lifecycle
//!
//! - `register(session_id, client)` inserts/replaces and bumps the default.
//! - `set_default(session_id)` is called by `switch_page`.
//! - `get(session_id)` resolves a specific id or falls back to the default.
//! - `remove(session_id)` is called by `close` or when CDP disconnects.

use crate::agentic::tools::browser_control::cdp_client::CdpClient;
use crate::util::errors::{BitFunError, BitFunResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct BrowserSession {
    pub session_id: String,
    pub port: u16,
    pub client: Arc<CdpClient>,
}

impl std::fmt::Debug for BrowserSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserSession")
            .field("session_id", &self.session_id)
            .field("port", &self.port)
            .field("client", &"<CdpClient>")
            .finish()
    }
}

#[derive(Default)]
struct RegistryInner {
    sessions: HashMap<String, BrowserSession>,
    default_id: Option<String>,
}

#[derive(Default)]
pub struct BrowserSessionRegistry {
    inner: RwLock<RegistryInner>,
}

impl BrowserSessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a session and mark it as the default.
    pub async fn register(&self, session: BrowserSession) {
        let mut g = self.inner.write().await;
        let id = session.session_id.clone();
        g.sessions.insert(id.clone(), session);
        g.default_id = Some(id);
    }

    /// Promote an existing session to the default. No-op if the id is unknown.
    pub async fn set_default(&self, session_id: &str) -> BitFunResult<()> {
        let mut g = self.inner.write().await;
        if !g.sessions.contains_key(session_id) {
            return Err(BitFunError::tool(format!(
                "Browser session '{}' not registered.",
                session_id
            )));
        }
        g.default_id = Some(session_id.to_string());
        Ok(())
    }

    /// Resolve a session id (or the current default) to a session.
    ///
    /// Also prunes entries whose underlying CDP WebSocket reader task has
    /// terminated (the user closed the tab outside of our control). Without
    /// the prune, the next `send` call would block until its 30-second
    /// internal timeout — confusing the model with a `TIMEOUT` error code
    /// that hides the real `WRONG_TAB` failure mode.
    pub async fn get(&self, session_id: Option<&str>) -> BitFunResult<BrowserSession> {
        // First pass: read-only resolve.
        let resolved = {
            let g = self.inner.read().await;
            let id = match session_id {
                Some(s) => s.to_string(),
                None => g.default_id.clone().ok_or_else(|| {
                    BitFunError::tool(
                        "No browser session registered. Use action 'connect' first.".to_string(),
                    )
                })?,
            };
            g.sessions.get(&id).cloned().map(|s| (id, s))
        };

        let (id, session) = resolved.ok_or_else(|| {
            BitFunError::tool(
                "Browser session is not connected. Use action 'connect' or 'switch_page'."
                    .to_string(),
            )
        })?;

        if !session.client.is_connected() {
            // Best-effort eviction. Acquire the write lock only when we
            // actually need to mutate the map.
            let mut g = self.inner.write().await;
            g.sessions.remove(&id);
            if g.default_id.as_deref() == Some(id.as_str()) {
                g.default_id = None;
            }
            return Err(BitFunError::tool(format!(
                "Browser session '{}' is no longer connected (the tab was likely closed). Call 'connect' or 'switch_page' to attach a new one.",
                id
            )));
        }

        Ok(session)
    }

    /// Remove a session. If it was the default, the default is cleared (the
    /// next `connect` / `switch_page` will install a new default).
    pub async fn remove(&self, session_id: &str) {
        let mut g = self.inner.write().await;
        g.sessions.remove(session_id);
        if g.default_id.as_deref() == Some(session_id) {
            g.default_id = None;
        }
    }

    /// Snapshot of registered session ids — used by `list_sessions` actions.
    pub async fn list(&self) -> Vec<String> {
        let g = self.inner.read().await;
        let mut ids: Vec<String> = g.sessions.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Current default session id, if any.
    pub async fn default_id(&self) -> Option<String> {
        let g = self.inner.read().await;
        g.default_id.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // We can't construct a real `CdpClient` without a live browser, so
    // the tests below exercise only the bookkeeping paths that don't
    // require an actual session (empty get, unknown id ⇒ set_default).
    // Session-aware behavior is exercised by integration tests in the
    // browser_control e2e suite.

    #[tokio::test]
    async fn empty_registry_errors_on_get() {
        let r = BrowserSessionRegistry::new();
        let err = r.get(None).await.unwrap_err();
        assert!(err.to_string().contains("No browser session"));
    }

    #[tokio::test]
    async fn unknown_id_cannot_become_default() {
        let r = BrowserSessionRegistry::new();
        let err = r.set_default("missing").await.unwrap_err();
        assert!(err.to_string().contains("not registered"));
    }
}

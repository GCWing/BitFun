use std::sync::Arc;

use async_trait::async_trait;

use super::session_application::DesktopSessionHostEffects;

pub(super) struct ProductionDesktopSessionHostEffects {
    acp_client_service: Option<Arc<bitfun_acp::AcpClientService>>,
}

impl ProductionDesktopSessionHostEffects {
    pub(super) fn new(acp_client_service: Option<Arc<bitfun_acp::AcpClientService>>) -> Self {
        Self { acp_client_service }
    }
}

#[async_trait]
impl DesktopSessionHostEffects for ProductionDesktopSessionHostEffects {
    async fn release_session(&self, session_id: &str) {
        if let Some(service) = self.acp_client_service.as_ref() {
            service.release_bitfun_session(session_id).await;
        }
    }

    fn notify_session_changed(&self, session_id: &str, workspace_path: &str) {
        crate::api::remote_connect_api::notify_session_changed(session_id, workspace_path);
    }

    fn notify_session_deleted(&self, session_id: &str) {
        crate::api::remote_connect_api::notify_session_deleted(session_id);
    }
}

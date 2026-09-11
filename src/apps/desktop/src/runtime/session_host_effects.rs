use std::sync::Arc;

use async_trait::async_trait;

use super::session_application::DesktopSessionHostEffects;

pub(super) struct ProductionDesktopSessionHostEffects {
    acp_client_service: Option<Arc<openbitfun_acp::AcpClientService>>,
}

impl ProductionDesktopSessionHostEffects {
    pub(super) fn new(acp_client_service: Option<Arc<openbitfun_acp::AcpClientService>>) -> Self {
        Self { acp_client_service }
    }
}

#[async_trait]
impl DesktopSessionHostEffects for ProductionDesktopSessionHostEffects {
    async fn release_session(&self, session_id: &str) {
        if let Some(service) = self.acp_client_service.as_ref() {
            service.release_openbitfun_session(session_id).await;
        }
    }
}

//! Compatibility adapter for finalized DeepResearch report citation IO.
//!
//! Concrete report filesystem IO lives in `bitfun-services-integrations`.

use std::path::Path;

pub async fn run_for_session_workspace(workspace_root: &Path, session_id: &str) {
    bitfun_services_integrations::deep_research::run_for_session_workspace(
        workspace_root,
        session_id,
    )
    .await;
}

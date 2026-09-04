//! Disabled remote-search facade for builds without concrete SSH support.

use openbitfun_services_integrations::workspace_search::{
    ContentSearchRequest, ContentSearchResult, GlobSearchRequest, GlobSearchResult,
};

fn unsupported() -> String {
    "Remote SSH search is disabled; enable the `ssh-remote` feature".to_string()
}

#[derive(Clone)]
pub struct RemoteWorkspaceSearchService;

impl RemoteWorkspaceSearchService {
    pub async fn search_content(
        &self,
        _request: ContentSearchRequest,
    ) -> Result<ContentSearchResult, String> {
        Err(unsupported())
    }

    pub async fn glob(&self, _request: GlobSearchRequest) -> Result<GlobSearchResult, String> {
        Err(unsupported())
    }
}

pub async fn remote_workspace_search_service_for_path(
    _root_path: &str,
    _preferred_connection_id: Option<String>,
) -> Result<RemoteWorkspaceSearchService, String> {
    Err(unsupported())
}

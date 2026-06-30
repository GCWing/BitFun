#![cfg(feature = "workspace-search")]

use bitfun_services_integrations::remote_ssh::workspace_search::disabled::{
    remote_workspace_search_service_for_path, RemoteWorkspaceSearchService,
};

fn assert_disabled_error(error: String) {
    assert!(
        error.contains("Remote SSH search is disabled"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn disabled_remote_workspace_search_returns_explicit_unsupported_errors() {
    let service = RemoteWorkspaceSearchService;

    let status_error = service.get_index_status("/remote/repo").await.unwrap_err();
    assert_disabled_error(status_error);

    let resolve_error = service
        .resolve_remote_workspace_entry("/remote/repo")
        .await
        .unwrap_err();
    assert_disabled_error(resolve_error);

    let resolver_error = match remote_workspace_search_service_for_path("/remote/repo", None).await
    {
        Ok(_) => panic!("disabled remote search resolver should return an unsupported error"),
        Err(error) => error,
    };
    assert_disabled_error(resolver_error);
}

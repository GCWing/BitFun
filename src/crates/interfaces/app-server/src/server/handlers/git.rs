use agent_client_protocol::{Builder, HandleDispatchFrom};
use openbitfun_core::service::git::GitService;

use crate::agent::git_service_error;
use crate::role::{AppClient, AppServer};
use crate::schema::{
    GitBranchesRequest, GitGetBranchesMessage, GitGetBranchesResponse,
    GitGetRepositoryTrustMessage, GitGetRepositoryTrustResponse, GitGetStatusMessage,
    GitGetStatusResponse, GitIsRepositoryMessage, GitIsRepositoryResponse,
    GitRepositoryPathRequest,
};
use crate::server::wire;

pub(in crate::server) fn builder() -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("git handlers")
        .on_receive_request(
            async move |request: GitIsRepositoryMessage, responder, _cx| {
                let GitRepositoryPathRequest { repository_path } = request.0;
                responder.respond_with_result(
                    GitService::is_repository(&repository_path)
                        .await
                        .map(GitIsRepositoryResponse)
                        .map_err(git_service_error),
                )
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: GitGetStatusMessage, responder, _cx| {
                let GitRepositoryPathRequest { repository_path } = request.0;
                responder.respond_with_result(
                    GitService::get_status(&repository_path)
                        .await
                        .map(|status| GitGetStatusResponse(wire::git_status(status)))
                        .map_err(git_service_error),
                )
            },
            agent_client_protocol::on_receive_request!(),
        )
        // Read-only on purpose. Granting trust writes the server user's global
        // Git configuration and tells Git to run hooks from a tree they do not
        // own; that decision belongs to the machine holding the repository, not
        // to a browser client. The probe still answers, so such a client can
        // name the folder and show the exact command.
        .on_receive_request(
            async move |request: GitGetRepositoryTrustMessage, responder, _cx| {
                let GitRepositoryPathRequest { repository_path } = request.0;
                responder.respond_with_result(
                    GitService::inspect_trust(&repository_path)
                        .await
                        .map(|report| GitGetRepositoryTrustResponse(wire::git_trust_report(report)))
                        .map_err(git_service_error),
                )
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: GitGetBranchesMessage, responder, _cx| {
                let GitBranchesRequest {
                    repository_path,
                    include_remote,
                } = request.0;
                let result =
                    GitService::get_branches(&repository_path, include_remote.unwrap_or(false))
                        .await
                        .map(|branches| GitGetBranchesResponse {
                            branches: branches.into_iter().map(wire::git_branch).collect(),
                        })
                        .map_err(git_service_error);
                responder.respond_with_result(result)
            },
            agent_client_protocol::on_receive_request!(),
        )
}

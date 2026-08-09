use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_core::service::git::GitService;

use crate::agent::git_service_error;
use crate::role::{AppClient, AppServer};
use crate::schema::{
    GitBranchesRequest, GitGetBranchesMessage, GitGetBranchesResponse, GitGetStatusMessage,
    GitGetStatusResponse, GitIsRepositoryMessage, GitIsRepositoryResponse,
    GitRepositoryPathRequest,
};

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
                        .map(GitGetStatusResponse)
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
                        .map(|branches| GitGetBranchesResponse { branches })
                        .map_err(git_service_error);
                responder.respond_with_result(result)
            },
            agent_client_protocol::on_receive_request!(),
        )
}

use std::sync::Arc;

use agent_client_protocol::{Builder, Error, HandleDispatchFrom};
use openbitfun_app_server_protocol::error::AppServerErrorKind;
use openbitfun_app_server_protocol::search::{
    SearchSessionContentMessage, SearchSessionContentResponse, PRODUCT_SEARCH_CAPABILITY_ID,
};
use openbitfun_runtime_ports::{PortError, PortErrorKind};

use crate::agent::OpenBitFunAppRuntime;
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder(
    runtime: Arc<OpenBitFunAppRuntime>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("product search handlers")
        .on_receive_request(
            async move |request: SearchSessionContentMessage, responder, _cx| {
                let result = match runtime.product_search() {
                    Some(search) => search
                        .search_session_content(request.0)
                        .await
                        .map(SearchSessionContentResponse)
                        .map_err(search_error),
                    None => Err(structured_search_error(
                        AppServerErrorKind::Unsupported,
                        "The Host does not provide product search",
                    )),
                };
                responder.respond_with_result(result)
            },
            agent_client_protocol::on_receive_request!(),
        )
}

fn search_error(error: PortError) -> Error {
    let kind = match error.kind {
        PortErrorKind::NotAvailable => AppServerErrorKind::Unsupported,
        PortErrorKind::InvalidRequest => AppServerErrorKind::InvalidRequest,
        PortErrorKind::SessionInUse => AppServerErrorKind::SessionInUse,
        PortErrorKind::OutcomeUnknown => AppServerErrorKind::OutcomeUnknown,
        _ => AppServerErrorKind::Internal,
    };
    structured_search_error(kind, error.message)
}

fn structured_search_error(kind: AppServerErrorKind, message: impl Into<String>) -> Error {
    super::capability::error_with_data(kind, PRODUCT_SEARCH_CAPABILITY_ID, message)
}

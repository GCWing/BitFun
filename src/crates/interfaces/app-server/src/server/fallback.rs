use crate::role::{AppClient, AppServer};
use agent_client_protocol::{Builder, ConnectionTo, Dispatch, Error, HandleDispatchFrom};

const EXTERNAL_SOURCE_METHOD_MARKERS: &[&str] = &[
    "external_source",
    "external_tool",
    "external_subagent",
    "external_mcp",
    "external_integration",
];

pub(super) fn builder() -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("dispatch fallback")
        .on_receive_dispatch(
            async move |message: Dispatch, cx: ConnectionTo<AppClient>| {
                let method = match &message {
                    Dispatch::Request(request, _) => request.method().to_string(),
                    _ => String::new(),
                };
                let error = if EXTERNAL_SOURCE_METHOD_MARKERS
                    .iter()
                    .any(|marker| method.contains(marker))
                {
                    Error::method_not_found().data(serde_json::json!({
                        "capability": "external_sources",
                        "reason": "not_available_in_web_mode",
                        "message": "External source operations are not yet available in web mode. Use the desktop host."
                    }))
                } else {
                    Error::method_not_found()
                };
                message.respond_with_error(error, cx)
            },
            agent_client_protocol::on_receive_dispatch!(),
        )
}

use std::sync::Arc;

use agent_client_protocol::{Builder, ConnectionTo, Dispatch, HandleDispatchFrom, Handled};

use crate::role::{AppClient, AppServer};

/// Build the connection-wide authorization preflight stage.
///
/// This handler must be registered before all typed request handlers. It
/// authorizes raw request payloads and lets authorized messages continue down
/// the normal typed dispatch chain. Session binding resolution remains in the
/// Session-aware handlers because it requires an asynchronous Runtime lookup.
pub(super) fn builder(
    event_state: Arc<super::ConnectionEventState>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("authorization preflight")
        .on_receive_dispatch(
            async move |message: Dispatch, cx: ConnectionTo<AppClient>| {
                let authorization = match &message {
                    Dispatch::Request(request, _) => {
                        event_state.preflight_request(request.method(), request.params())
                    }
                    Dispatch::Notification(_) | Dispatch::Response(_, _) => Ok(()),
                };

                match authorization {
                    Ok(()) => Ok(Handled::No {
                        message,
                        retry: false,
                    }),
                    Err(error) => {
                        message.respond_with_error(error, cx)?;
                        Ok(Handled::Yes)
                    }
                }
            },
            agent_client_protocol::on_receive_dispatch!(),
        )
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use agent_client_protocol::{ConnectionTo, JsonRpcResponse, Responder, SentRequest};
    use serde::{Deserialize, Serialize};
    use tokio::task::LocalSet;

    use super::*;
    use crate::host::{AppServerHostPolicy, AppServerHostPolicyError};

    #[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcRequest)]
    #[request(method = "test/preflight", response = TestResponse)]
    struct TestRequest {
        allowed: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcResponse)]
    struct TestResponse;

    #[derive(Default)]
    struct TestPolicy {
        preflight_calls: AtomicUsize,
        owner_calls: AtomicUsize,
    }

    impl AppServerHostPolicy for TestPolicy {
        fn allows_method(&self, method: &str) -> bool {
            method == "test/preflight"
        }

        fn authorize_preflight(
            &self,
            method: &str,
            request: &serde_json::Value,
        ) -> Result<(), AppServerHostPolicyError> {
            self.preflight_calls.fetch_add(1, Ordering::Relaxed);
            if !self.allows_method(method)
                || request.get("allowed").and_then(serde_json::Value::as_bool) != Some(true)
            {
                return Err(AppServerHostPolicyError::invalid_request(
                    "request rejected by preflight",
                ));
            }
            Ok(())
        }

        fn authorize_request(
            &self,
            _method: &str,
            _request: &serde_json::Value,
        ) -> Result<(), AppServerHostPolicyError> {
            self.owner_calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn allows_capability(&self, _capability: &str) -> bool {
            true
        }

        fn allows_external_source_workspace(&self, _workspace_path: &str) -> bool {
            true
        }

        fn register_session_binding(
            &self,
            _session_id: &str,
            _binding: &bitfun_runtime_ports::AgentSessionWorkspaceBinding,
        ) -> Result<(), AppServerHostPolicyError> {
            Ok(())
        }
    }

    async fn receive<T: JsonRpcResponse + Send>(
        response: SentRequest<T>,
    ) -> Result<T, agent_client_protocol::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        response.on_receiving_result(async move |result| {
            tx.send(result)
                .map_err(|_| agent_client_protocol::Error::internal_error())
        })?;
        rx.await
            .map_err(|_| agent_client_protocol::Error::internal_error())?
    }

    #[tokio::test(flavor = "current_thread")]
    async fn preflight_rejects_before_typed_handler_and_passes_authorized_requests() {
        let local = LocalSet::new();
        let policy = Arc::new(TestPolicy::default());
        let typed_calls = Arc::new(AtomicUsize::new(0));

        local
            .run_until({
                let policy = policy.clone();
                let typed_calls = typed_calls.clone();
                async move {
                    let (server_transport, client_transport) =
                        crate::transport::in_memory_channel_pair();
                    let event_state = Arc::new(super::super::ConnectionEventState::new(
                        false,
                        Some(policy.clone()),
                        None,
                    ));
                    let server = AppServer
                        .builder()
                        .with_connection_builder(builder(event_state))
                        .on_receive_request(
                            async move |_request: TestRequest,
                                        responder: Responder<TestResponse>,
                                        _cx: ConnectionTo<AppClient>| {
                                typed_calls.fetch_add(1, Ordering::Relaxed);
                                responder.respond(TestResponse)
                            },
                            agent_client_protocol::on_receive_request!(),
                        );

                    tokio::task::spawn_local(async move {
                        let _ = server.connect_to(server_transport).await;
                    });

                    let result = AppClient
                        .builder()
                        .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                            assert!(receive(cx.send_request(TestRequest { allowed: false }))
                                .await
                                .is_err());
                            receive(cx.send_request(TestRequest { allowed: true })).await?;
                            Ok(())
                        })
                        .await;
                    assert!(result.is_ok(), "{result:?}");
                }
            })
            .await;

        assert_eq!(policy.preflight_calls.load(Ordering::Relaxed), 2);
        assert_eq!(policy.owner_calls.load(Ordering::Relaxed), 0);
        assert_eq!(typed_calls.load(Ordering::Relaxed), 1);
    }
}

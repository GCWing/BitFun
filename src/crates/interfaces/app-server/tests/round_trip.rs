//! Integration tests for the generic app-server scaffold: request/response
//! round-trip, notification delivery, and method_not_found dispatch fallback,
//! all over the in-memory duplex transport.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_client_protocol::{ConnectionTo, Dispatch, JsonRpcResponse, Responder, SentRequest};
use openbitfun_app_server::{transport, AppClient, AppServer};
use serde::{Deserialize, Serialize};
use tokio::task::LocalSet;

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcRequest)]
#[request(method = "app/greet", response = GreetResponse)]
struct GreetRequest {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcResponse)]
struct GreetResponse {
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcNotification)]
#[notification(method = "app/log")]
struct LogNotification {
    line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcRequest)]
#[request(method = "app/unknown", response = UnknownResponse)]
struct UnknownRequest;

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcResponse)]
struct UnknownResponse;

async fn recv<T: JsonRpcResponse + Send>(
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
async fn request_response_round_trip() {
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();

            let server = AppServer.builder().name("test-server").on_receive_request(
                async |request: GreetRequest,
                       responder: Responder<GreetResponse>,
                       _cx: ConnectionTo<AppClient>| {
                    responder.respond(GreetResponse {
                        message: format!("hello, {}!", request.name),
                    })
                },
                agent_client_protocol::on_receive_request!(),
            );

            tokio::task::spawn_local(async move {
                let _ = server.connect_to(server_transport).await;
            });

            let result = AppClient
                .builder()
                .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                    let response = recv(cx.send_request(GreetRequest {
                        name: "openbitfun".to_string(),
                    }))
                    .await?;
                    assert_eq!(response.message, "hello, openbitfun!");
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn notification_delivery() {
    let logs = Arc::new(Mutex::new(Vec::<String>::new()));

    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();
            let logs_for_server = logs.clone();

            let server = AppServer.builder().on_receive_notification(
                {
                    let logs = logs_for_server.clone();
                    async move |notification: LogNotification, _cx: ConnectionTo<AppClient>| {
                        logs.lock().unwrap().push(notification.line);
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_notification!(),
            );

            tokio::task::spawn_local(async move {
                let _ = server.connect_to(server_transport).await;
            });

            let result = AppClient
                .builder()
                .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                    cx.send_notification(LogNotification {
                        line: "line one".to_string(),
                    })?;
                    cx.send_notification(LogNotification {
                        line: "line two".to_string(),
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");
        })
        .await;

    let received = logs.lock().unwrap().clone();
    assert_eq!(
        received,
        vec!["line one".to_string(), "line two".to_string()]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_method_returns_method_not_found() {
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();

            let server = AppServer.builder().on_receive_dispatch(
                async |message: Dispatch, cx: ConnectionTo<AppClient>| {
                    message.respond_with_error(agent_client_protocol::Error::method_not_found(), cx)
                },
                agent_client_protocol::on_receive_dispatch!(),
            );

            tokio::task::spawn_local(async move {
                let _ = server.connect_to(server_transport).await;
            });

            let result = AppClient
                .builder()
                .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                    let result = recv(cx.send_request(UnknownRequest)).await;
                    assert!(
                        result.is_err(),
                        "unknown method should yield an error, got {result:?}"
                    );
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");
        })
        .await;
}

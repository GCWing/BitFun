use agent_client_protocol::{
    Agent, Channel, ConnectTo, ConnectionTo, Error, JsonRpcNotification, Role,
};
use futures::channel::{mpsc, oneshot};
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt};
use serde::{Deserialize, Serialize};

// This marker is injected only into the client's in-memory incoming queue.
// It is never sent to the agent and does not extend the ACP wire contract.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcNotification)]
#[notification(method = "_openbitfun/client_transport_closed")]
pub(super) struct AcpTransportClosed {
    error: Error,
}

pub(super) async fn handle_transport_closed(
    notification: AcpTransportClosed,
    connection: ConnectionTo<Agent>,
) -> Result<(), Error> {
    // The dispatcher has consumed all frames before the marker. Failing a
    // connection task now wakes unanswered requests without losing final replies.
    connection.spawn(async move { Err(notification.error) })
}

/// Make transport termination observable without discarding already-read frames.
/// ACP 0.12 otherwise keeps waiting on other channel senders after input EOF.
pub(super) struct AcpTransport<T> {
    inner: T,
}

impl<T> AcpTransport<T> {
    pub(super) fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<R: Role, T: ConnectTo<R>> ConnectTo<R> for AcpTransport<T> {
    async fn connect_to(self, client: impl ConnectTo<R::Counterpart>) -> Result<(), Error> {
        let (channel, transport) = <Self as ConnectTo<R>>::into_channel_and_future(self);
        futures::try_join!(client.connect_to(channel), transport)?;
        Ok(())
    }

    fn into_channel_and_future(self) -> (Channel, BoxFuture<'static, Result<(), Error>>) {
        let (Channel { mut rx, tx }, transport) = self.inner.into_channel_and_future();
        let (incoming_tx, incoming_rx) = mpsc::unbounded();
        let (termination_tx, mut termination_rx) = oneshot::channel();
        let run_transport = async move {
            // Deliver I/O failures through the incoming queue too. Returning an
            // error here could terminate dispatch before the last reply is read.
            let _ = termination_tx.send(transport.await);
            Ok::<(), Error>(())
        };
        let forward_incoming = async move {
            while let Some(message) = rx.next().await {
                incoming_tx
                    .unbounded_send(message)
                    .map_err(Error::into_internal_error)?;
            }

            let error = termination_rx
                .try_recv()
                .ok()
                .flatten()
                .and_then(Result::err)
                .unwrap_or_else(|| Error::internal_error().data("ACP agent output stream closed"));
            // SDK channel errors are treated as recoverable parse errors.
            // Use a local dispatcher marker to terminate only after every frame,
            // including a prompt response immediately followed by EOF.
            let marker = serde_json::from_value(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "_openbitfun/client_transport_closed",
                "params": AcpTransportClosed { error },
            }))
            .map_err(Error::into_internal_error)?;
            incoming_tx
                .unbounded_send(Ok(marker))
                .map_err(Error::into_internal_error)?;
            Ok::<(), Error>(())
        };
        let task = async move {
            futures::try_join!(run_transport, forward_incoming)?;
            Ok(())
        };
        (
            Channel {
                rx: incoming_rx,
                tx,
            },
            task.boxed(),
        )
    }
}

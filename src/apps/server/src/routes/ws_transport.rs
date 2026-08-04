//! Axum WebSocket -> `agent_client_protocol::Lines` transport bridge.
//!
//! The browser speaks raw JSON-RPC 2.0 over WebSocket (one message per WS text
//! frame). [`agent_client_protocol::Lines`] already implements
//! [`agent_client_protocol::ConnectTo`] for any `futures::Sink<String, Error =
//! io::Error>` + `futures::Stream<Item = io::Result<String>>` pair. This module
//! adapts an axum `WebSocket` (after `split()`) into exactly that pair: outgoing
//! wraps the `SplitSink` (each `String` -> `Message::Text`), incoming wraps the
//! `SplitStream` (each `Message::Text` -> `Ok(String)`, control frames ignored,
//! binary frames rejected, and close frames ending the stream).
//!
//! The returned `Lines` is handed to [`bitfun_app_server::BitfunAppServer::serve`]
//! per WebSocket connection, so the browser connects directly to the in-process
//! app-server over native JSON-RPC -- no custom `{type:"request"|...}` envelope,
//! no hand-written `route_agent_command`, no shared in-process client.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use agent_client_protocol::Lines;
use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{Sink, SinkExt, Stream, StreamExt};

/// Bridge an axum WebSocket into an agent-client-protocol `Lines` transport for
/// `BitfunAppServer::serve(lines)`.
///
/// The WebSocket is split; the outgoing half becomes the `Lines` sink (one
/// `String` per WS text frame), the incoming half becomes the stream (text
/// frames only; binary frames surface as `io::Error`, ping/pong are ignored,
/// and close frames end the stream).
pub(crate) fn ws_lines(socket: WebSocket) -> Lines<WSSink, WSStream> {
    let (sink, stream) = socket.split();
    Lines::new(WSSink { sink }, WSStream { stream })
}

/// Outgoing adapter: `futures::Sink<String, Error = io::Error>` -> axum
/// `Message::Text`. The inner `SplitSink` is `Unpin` (axum's `WebSocket` wraps a
/// tungstenite `WebSocketStream`), so we mark this wrapper `Unpin` too and
/// project through `Pin::new` without unsafe.
pub(crate) struct WSSink {
    sink: SplitSink<WebSocket, Message>,
}

// SAFETY: `SplitSink<WebSocket, Message>` is `Unpin` (tungstenite stream-backed),
// so this wrapper is too.
impl Unpin for WSSink {}

impl Sink<String> for WSSink {
    type Error = io::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.get_mut();
        Sink::poll_ready(Pin::new(&mut this.sink), cx)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "ws ready failed"))
    }

    fn start_send(self: Pin<&mut Self>, item: String) -> Result<(), Self::Error> {
        let this = self.get_mut();
        this.sink
            .start_send_unpin(Message::Text(item.into()))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "ws send failed"))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.get_mut();
        Sink::poll_flush(Pin::new(&mut this.sink), cx)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "ws flush failed"))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.get_mut();
        Sink::poll_close(Pin::new(&mut this.sink), cx)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "ws close failed"))
    }
}

/// Incoming adapter: `futures::Stream<Item = io::Result<String>>` from axum
/// `Message::Text`. Binary frames surface as `io::Error`; ping/pong are ignored
/// and only close frames terminate the JSON-RPC stream.
pub(crate) struct WSStream<S = SplitStream<WebSocket>> {
    stream: S,
}

impl<S: Unpin> Unpin for WSStream<S> {}

impl<S> Stream for WSStream<S>
where
    S: Stream<Item = Result<Message, axum::Error>> + Unpin,
{
    type Item = io::Result<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            match Stream::poll_next(Pin::new(&mut this.stream), cx) {
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(Err(_))) => {
                    return Poll::Ready(Some(Err(io::Error::new(
                        io::ErrorKind::ConnectionAborted,
                        "ws recv failed",
                    ))))
                }
                Poll::Ready(Some(Ok(Message::Text(text)))) => {
                    return Poll::Ready(Some(Ok(text.to_string())))
                }
                Poll::Ready(Some(Ok(Message::Binary(_)))) => {
                    return Poll::Ready(Some(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "binary ws frames not supported",
                    ))))
                }
                Poll::Ready(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => continue,
                Poll::Ready(Some(Ok(Message::Close(_)))) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ping_and_pong_do_not_end_the_json_rpc_stream() {
        let frames = futures_util::stream::iter(vec![
            Ok::<_, axum::Error>(Message::Ping(Vec::new().into())),
            Ok::<_, axum::Error>(Message::Pong(Vec::new().into())),
            Ok::<_, axum::Error>(Message::Text("request".into())),
            Ok::<_, axum::Error>(Message::Close(None)),
        ]);
        let mut incoming = WSStream { stream: frames };

        assert_eq!(
            incoming.next().await.transpose().expect("valid text frame"),
            Some("request".to_string())
        );
        assert!(incoming.next().await.is_none());
    }
}

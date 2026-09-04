//! Axum WebSocket -> `agent_client_protocol::Lines` transport bridge.
//!
//! The browser speaks raw JSON-RPC 2.0 over WebSocket (one message per WS text
//! frame). ACP's [`agent_client_protocol::Lines`] already implements
//! [`agent_client_protocol::ConnectTo`] for any `futures::Sink<String, Error =
//! io::Error>` + `futures::Stream<Item = io::Result<String>>` pair. This module
//! adapts an axum `WebSocket` (after `split()`) into exactly that pair: outgoing
//! wraps the `SplitSink` (each `String` -> `Message::Text`), incoming wraps the
//! `SplitStream` (each `Message::Text` -> `Ok(String)`, everything else -> `Err`).
//!
//! The returned `Lines` is handed to [`openbitfun_app_server::OpenBitFunAppServer::serve`]
//! per WebSocket connection, so the browser connects directly to the in-process
//! app-server over native JSON-RPC -- no custom `{type:"request"|...}` envelope,
//! no hand-written `route_agent_command`, no shared in-process client.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use agent_client_protocol::Lines;
use axum::extract::ws::{Message, WebSocket};
use futures::{Sink, Stream};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};

/// Bridge an axum WebSocket into an ACP `Lines` transport for
/// `OpenBitFunAppServer::serve(lines)`.
///
/// The WebSocket is split; the outgoing half becomes the `Lines` sink (one
/// `String` per WS text frame), the incoming half becomes the stream (text
/// frames only; binary/control frames surface as a stream end or `io::Error`).
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
/// `Message::Text`. Binary frames surface as `io::Error`; control frames close
/// the stream (axum handles ping/pong internally).
pub(crate) struct WSStream {
    stream: SplitStream<WebSocket>,
}

impl Unpin for WSStream {}

impl Stream for WSStream {
    type Item = io::Result<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match Stream::poll_next(Pin::new(&mut this.stream), cx) {
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(Err(_))) => Poll::Ready(Some(Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "ws recv failed",
            )))),
            Poll::Ready(Some(Ok(Message::Text(text)))) => Poll::Ready(Some(Ok(text.to_string()))),
            Poll::Ready(Some(Ok(Message::Binary(_)))) => Poll::Ready(Some(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "binary ws frames not supported",
            )))),
            Poll::Ready(Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Close(_)))) => {
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

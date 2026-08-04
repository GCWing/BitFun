//! HTTP and WebSocket routes exposed by the paused Web Server's current host.

pub(crate) mod api;
#[cfg(feature = "paused-web-server-source-check")]
#[allow(dead_code)]
pub(crate) mod dispatch;
#[cfg(feature = "paused-web-server-source-check")]
#[allow(dead_code)]
pub(crate) mod external_sources;
pub(crate) mod websocket;
pub(crate) mod ws_transport;

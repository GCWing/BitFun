//! CLI Peer Device Mode host.
//!
//! Desktop controllers reach this process over the same HostInvoke / DeviceEvent
//! envelopes used for Desktop peers. Execution goes through Core services
//! (no webview / Tauri bridge).

mod args;
mod bootstrap;
mod commands;
mod control;
mod deny;
mod dispatch;
mod fanout;
mod state;
mod workspace_dto;

pub(crate) use bootstrap::ensure_peer_host_ready;
pub(crate) use dispatch::{handle_device_event_command, handle_host_invoke};

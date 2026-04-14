mod client;
mod convert;
mod managed;
pub mod protocol;
mod repo;
mod rg_backend;
mod server;
mod service;

pub use client::DaemonClient;
pub use managed::{
    daemon_state_file_path, daemon_state_file_path_from_open, EnsuredRepo, ManagedDaemonClient,
    OpenedRepo,
};
pub use server::{serve, serve_stdio, ServerOptions};

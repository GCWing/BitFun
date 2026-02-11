//! AI infrastructure
//!
//! Provides AI clients and related services

pub mod client;
pub mod client_factory;
pub mod providers;

pub use ai_stream_handlers;

pub use client::{AIClient, StreamResponse};
pub use client_factory::{
    get_global_ai_client_factory, initialize_global_ai_client_factory, AIClientFactory,
};

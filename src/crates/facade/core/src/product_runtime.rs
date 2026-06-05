//! Core product-full runtime adapter boundary.
//!
//! This module owns core-specific adapter wiring that still depends on existing
//! concrete core paths. Provider-neutral product assembly facts live in
//! `bitfun-product-capabilities`.

mod runtime_services;

pub use runtime_services::CoreRuntimeServicesProvider;

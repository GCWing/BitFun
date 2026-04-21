//! Internal helpers for the unified `ControlHub` tool.
//!
//! ControlHub is the sole control entry point exposed to the model. This module
//! provides the cross-domain primitives every ControlHub action shares:
//!
//! * [`result`]   — unified `{ ok, domain, action, data, error?, capability?, warnings? }` envelope.
//! * [`errors`]   — structured machine-readable error codes returned in the envelope.

pub mod errors;
pub mod result;

pub use errors::ErrorCode;
pub use result::{err_response, ok_response, ControlHubError};

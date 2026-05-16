//! System service module
//!
//! Provides system info retrieval and command detection/execution.

mod command;
mod info;

pub use command::*;
pub use info::*;

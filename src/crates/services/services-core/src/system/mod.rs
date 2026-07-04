//! System service module
//!
//! Provides system info retrieval and command detection/execution.

mod command;
mod info;
mod local_actions;

pub use command::*;
pub use info::*;
pub use local_actions::*;

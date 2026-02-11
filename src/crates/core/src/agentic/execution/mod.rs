//! Execution Engine Layer
//!
//! Responsible for AI interaction and model round control

pub mod execution_engine;
pub mod round_executor;
pub mod stream_processor;
pub mod types;

pub use execution_engine::*;
pub use round_executor::*;
pub use stream_processor::*;
pub use types::{ExecutionContext, ExecutionResult, FinishReason, RoundContext, RoundResult};

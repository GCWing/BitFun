//! Execution Engine Layer
//!
//! Responsible for AI interaction and model round control

pub mod edit_constraint_guard;
pub mod execution_engine;
pub(crate) mod model_exchange_trace;
pub mod round_executor;
pub mod stream_processor;
pub mod types;
pub mod write_content_sanitizer;

pub use execution_engine::*;
pub use round_executor::*;
pub use stream_processor::*;
pub use types::{ExecutionContext, ExecutionResult, FinishReason, RoundContext, RoundResult};

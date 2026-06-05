use serde::{Deserialize, Serialize};

/// Basic error type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredError {
    pub message: String,
    pub status: Option<u16>,
}

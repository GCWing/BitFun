//! Image payload attached to tool results (multimodal tool messages).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolImageAttachment {
    pub mime_type: String,
    pub data_base64: String,
}

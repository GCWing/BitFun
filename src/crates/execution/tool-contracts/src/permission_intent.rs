//! Provider-neutral permission intents emitted by tool preflight.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A side-effect-free description of the resources a tool call intends to use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionIntent {
    pub action: String,
    pub resources: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub save_resources: Vec<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub display_metadata: Map<String, Value>,
}

impl PermissionIntent {
    pub fn new(action: impl Into<String>, resources: Vec<String>) -> Self {
        Self {
            action: action.into(),
            save_resources: resources.clone(),
            resources,
            display_metadata: Map::new(),
        }
    }
}

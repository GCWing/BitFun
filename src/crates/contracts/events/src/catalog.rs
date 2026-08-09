use serde::{Deserialize, Serialize};

pub const AI_MODEL_CATALOG_UPDATED_EVENT: &str = "ai://model-catalog-updated";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AIModelCatalogUpdatedEvent {
    pub source_version: String,
    pub sha256: String,
}

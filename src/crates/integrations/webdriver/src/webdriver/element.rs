use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ELEMENT_KEY: &str = "element-6066-11e4-a52e-4f735466cecf";
pub const LEGACY_ELEMENT_KEY: &str = "ELEMENT";
pub const SHADOW_KEY: &str = "shadow-6066-11e4-a52e-4f735466cecf";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementRef {
    #[serde(rename = "element-6066-11e4-a52e-4f735466cecf")]
    pub id: String,
    #[serde(rename = "ELEMENT")]
    pub legacy_id: String,
}

impl ElementRef {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            id: id.clone(),
            legacy_id: id,
        }
    }

    pub fn into_value(self) -> Value {
        json!(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowRootRef {
    #[serde(rename = "shadow-6066-11e4-a52e-4f735466cecf")]
    pub id: String,
}

impl ShadowRootRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub fn into_value(self) -> Value {
        json!(self)
    }
}

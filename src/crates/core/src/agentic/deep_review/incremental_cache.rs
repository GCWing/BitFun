//! Per-session Deep Review packet cache model and serialization.
//!
//! This cache is scoped to a Deep Review session fingerprint. It is not a
//! project-level cache and does not define retention, invalidation, or deletion
//! policy across sessions.

use serde_json::{json, Value};
use std::collections::HashMap;

/// Incremental review cache stores completed reviewer outputs keyed by packet_id.
/// When a deep review is re-run with the same target fingerprint, cached outputs
/// are reused instead of re-dispatching reviewers.
#[derive(Clone)]
pub struct DeepReviewIncrementalCache {
    fingerprint: String,
    packets: HashMap<String, String>,
}

impl DeepReviewIncrementalCache {
    pub fn new(fingerprint: &str) -> Self {
        Self {
            fingerprint: fingerprint.to_string(),
            packets: HashMap::new(),
        }
    }

    pub fn from_value(value: &Value) -> Self {
        let obj = value.as_object();
        let fingerprint = obj
            .and_then(|o| o.get("fingerprint"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let packets = obj
            .and_then(|o| o.get("packets"))
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            fingerprint,
            packets,
        }
    }

    pub fn to_value(&self) -> Value {
        json!({
            "fingerprint": self.fingerprint,
            "packets": self.packets,
        })
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn store_packet(&mut self, packet_id: &str, output: &str) {
        self.packets
            .insert(packet_id.to_string(), output.to_string());
    }

    pub fn get_packet(&self, packet_id: &str) -> Option<&str> {
        self.packets.get(packet_id).map(|s| s.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// Check if the cached fingerprint matches the fingerprint in the run manifest.
    /// Returns false if the manifest has no incrementalReviewCache section.
    pub fn matches_manifest(&self, manifest: &Value) -> bool {
        manifest
            .get("incrementalReviewCache")
            .and_then(|ic| ic.get("fingerprint"))
            .and_then(Value::as_str)
            .map(|fp| fp == self.fingerprint)
            .unwrap_or(false)
    }
}

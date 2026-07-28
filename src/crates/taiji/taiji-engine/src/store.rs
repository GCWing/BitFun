use crate::types::state::{FromStateValue, StateKey, StateValue};
use crate::types::NodeId;
use dashmap::DashMap;
use std::time::Instant;

/// Key-value store. Nodes declare read/write intent via input_keys/output_keys.
/// Pipeline validates before node execution: all input_keys have corresponding outputs written by upstream.
///
/// Uses DashMap for concurrent read/write. Same-layer DAG nodes execute in parallel.
pub struct StateStore {
    data: DashMap<StateKey, StateValue>,
    provenance: DashMap<StateKey, NodeId>,
    last_update: DashMap<StateKey, Instant>,
}

impl Default for StateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl StateStore {
    pub fn new() -> Self {
        Self {
            data: DashMap::new(),
            provenance: DashMap::new(),
            last_update: DashMap::new(),
        }
    }

    /// Read by type. Returns None if key does not exist or type does not match.
    pub fn get<T: FromStateValue>(&self, key: &StateKey) -> Option<T> {
        self.data.get(key).and_then(|v| T::from_value(&v))
    }

    /// Write and record provenance. Takes &self (DashMap provides interior mutability).
    pub fn set(&self, key: StateKey, value: StateValue, source: NodeId) {
        if self.provenance.contains_key(&key) {
            if let Some(prev) = self.provenance.get(&key) {
                tracing::warn!(
                    "StateStore: key '{}' overwritten by '{}' (previously written by '{}')",
                    key,
                    source,
                    prev.value()
                );
            }
        }
        self.last_update.insert(key.clone(), Instant::now());
        self.provenance.insert(key.clone(), source);
        self.data.insert(key, value);
    }

    /// Collect all Signals (cloned, since DashMap cannot return long-lived references).
    pub fn get_signals(&self) -> Vec<crate::types::signal::Signal> {
        self.data
            .iter()
            .filter_map(|entry| {
                if let StateValue::Signals(arc) = entry.value() {
                    let cloned: Vec<_> = arc.iter().cloned().collect();
                    Some(cloned)
                } else {
                    None
                }
            })
            .flatten()
            .collect()
    }

    /// Read raw StateValue by key (no type conversion). Returns cloned value.
    pub fn get_raw(&self, key: &StateKey) -> Option<StateValue> {
        self.data.get(key).map(|v| v.clone())
    }

    /// Read a Json value by key. Returns None if key does not exist or value is not Json.
    pub fn get_json(&self, key: &StateKey) -> Option<serde_json::Value> {
        match self.data.get(key) {
            Some(v) => match v.value() {
                StateValue::Json(val) => Some(val.clone()),
                _ => None,
            },
            None => None,
        }
    }

    /// Check if key exists
    pub fn contains(&self, key: &StateKey) -> bool {
        self.data.contains_key(key)
    }

    /// Return the source node for a key (cloned).
    pub fn provenance_of(&self, key: &StateKey) -> Option<NodeId> {
        self.provenance.get(key).map(|v| v.clone())
    }

    /// Serialize all data in StateStore to a JSON Value.
    /// For Tauri command export and Agent data exchange.
    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for entry in self.data.iter() {
            if let Ok(json_val) = serde_json::to_value(entry.value()) {
                map.insert(entry.key().clone(), json_val);
            }
        }
        serde_json::Value::Object(map)
    }

    /// Get all keys (owned).
    pub fn keys(&self) -> Vec<StateKey> {
        self.data.iter().map(|e| e.key().clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::bar::Freq;
    use crate::types::signal::{Signal, SignalAction};
    use chrono::Utc;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn test_set_get_bool() {
        let store = StateStore::new();
        store.set(
            "test_bool".into(),
            StateValue::Bool(true),
            "test_node".into(),
        );
        let val: Option<bool> = store.get(&"test_bool".into());
        assert_eq!(val, Some(true));
    }

    #[test]
    fn test_type_mismatch() {
        let store = StateStore::new();
        store.set("x".into(), StateValue::F64(3.14), "n".into());
        let val: Option<bool> = store.get(&"x".into());
        assert_eq!(val, None);
    }

    #[test]
    fn test_provenance() {
        let store = StateStore::new();
        store.set("k".into(), StateValue::Usize(42), "writer".into());
        assert_eq!(store.provenance_of(&"k".into()), Some("writer".into()));
    }

    #[test]
    fn test_get_signals() {
        let store = StateStore::new();
        let sig = Signal {
            timestamp: Utc::now(),
            instrument: "test".into(),
            freq: Freq::F1,
            action: SignalAction::Hold,
            entry: None,
            stop_loss: None,
            take_profit: None,
            size: None,
            source: "n".into(),
            confidence: 0.0,
            metadata: HashMap::new(),
            disclaimer: None,
        };
        store.set(
            "sig".into(),
            StateValue::Signals(Arc::new(vec![sig])),
            "n".into(),
        );
        let signals = store.get_signals();
        assert_eq!(signals.len(), 1);
    }
}

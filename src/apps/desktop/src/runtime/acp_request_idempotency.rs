//! Idempotent request-id claim helpers for Desktop ACP remote control.
//!
//! Callers mint a candidate value, then claim under a single map lock. If another
//! caller already owns the key, the existing value is returned and no side
//! effects should run.

use std::collections::hash_map::Entry;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IdempotentClaim<T> {
    /// This caller inserted `candidate` and owns the side effects.
    Claimed(T),
    /// Another caller already claimed this request id.
    Existing(T),
}

/// Insert `candidate` if `key` is vacant; otherwise return the existing value.
///
/// Must be called while holding the map mutex for the whole claim.
pub(crate) fn claim_idempotent_value<T: Clone>(
    map: &mut HashMap<String, T>,
    key: String,
    candidate: T,
) -> IdempotentClaim<T> {
    match map.entry(key) {
        Entry::Occupied(entry) => IdempotentClaim::Existing(entry.get().clone()),
        Entry::Vacant(entry) => {
            entry.insert(candidate.clone());
            IdempotentClaim::Claimed(candidate)
        }
    }
}

pub(crate) fn request_idempotency_key(session_id: &str, request_id: &str) -> String {
    format!("{session_id}\0{request_id}")
}

pub(crate) fn clear_session_idempotency_keys<T>(map: &mut HashMap<String, T>, session_id: &str) {
    let prefix = format!("{session_id}\0");
    map.retain(|key, _| !key.starts_with(&prefix));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn first_claim_wins_and_second_sees_existing() {
        let mut map = HashMap::new();
        let first = claim_idempotent_value(&mut map, "s\0r".to_string(), "turn-a".to_string());
        let second = claim_idempotent_value(&mut map, "s\0r".to_string(), "turn-b".to_string());
        assert_eq!(first, IdempotentClaim::Claimed("turn-a".to_string()));
        assert_eq!(second, IdempotentClaim::Existing("turn-a".to_string()));
        assert_eq!(map.get("s\0r").map(String::as_str), Some("turn-a"));
    }

    #[test]
    fn concurrent_claims_produce_one_owner() {
        let map = Arc::new(Mutex::new(HashMap::<String, String>::new()));
        let key = "acp-1\0req-1".to_string();
        let mut handles = Vec::new();
        for i in 0..32 {
            let map = map.clone();
            let key = key.clone();
            handles.push(thread::spawn(move || {
                let candidate = format!("turn-{i}");
                let mut guard = map.lock().expect("map");
                claim_idempotent_value(&mut guard, key, candidate)
            }));
        }
        let mut claimed = 0;
        let mut existing = 0;
        let mut winners = Vec::new();
        for handle in handles {
            match handle.join().expect("join") {
                IdempotentClaim::Claimed(value) => {
                    claimed += 1;
                    winners.push(value);
                }
                IdempotentClaim::Existing(_) => existing += 1,
            }
        }
        assert_eq!(claimed, 1);
        assert_eq!(existing, 31);
        assert_eq!(winners.len(), 1);
        assert_eq!(map.lock().expect("map").get(&key), Some(&winners[0]));
    }

    #[test]
    fn clear_session_removes_only_matching_prefix() {
        let mut map = HashMap::new();
        map.insert("acp-1\0a".to_string(), 1);
        map.insert("acp-1\0b".to_string(), 2);
        map.insert("acp-2\0a".to_string(), 3);
        clear_session_idempotency_keys(&mut map, "acp-1");
        assert!(map.get("acp-1\0a").is_none());
        assert!(map.get("acp-1\0b").is_none());
        assert_eq!(map.get("acp-2\0a"), Some(&3));
    }
}

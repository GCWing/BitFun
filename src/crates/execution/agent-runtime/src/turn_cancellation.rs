//! Dialog-turn cancellation token lifecycle state.

use dashmap::DashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default)]
pub struct DialogTurnCancellationTokenStore {
    tokens: Arc<DashMap<String, CancellationToken>>,
}

impl DialogTurnCancellationTokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_or_insert_new(&self, dialog_turn_id: &str) -> CancellationToken {
        self.tokens
            .entry(dialog_turn_id.to_string())
            .or_default()
            .clone()
    }

    pub fn has_active(&self, dialog_turn_id: &str) -> bool {
        self.tokens.contains_key(dialog_turn_id)
    }

    pub fn is_cancelled(&self, dialog_turn_id: &str) -> bool {
        self.token(dialog_turn_id)
            .is_some_and(|token| token.is_cancelled())
    }

    pub fn insert(&self, dialog_turn_id: &str, token: CancellationToken) {
        self.tokens.insert(dialog_turn_id.to_string(), token);
    }

    pub fn token(&self, dialog_turn_id: &str) -> Option<CancellationToken> {
        self.tokens.get(dialog_turn_id).map(|entry| entry.clone())
    }

    pub fn cancel(&self, dialog_turn_id: &str) -> bool {
        let Some(token) = self.token(dialog_turn_id) else {
            return false;
        };
        token.cancel();
        true
    }

    pub fn remove(&self, dialog_turn_id: &str) -> bool {
        self.tokens.remove(dialog_turn_id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::DialogTurnCancellationTokenStore;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn turn_cancellation_store_reuses_existing_token() {
        let store = DialogTurnCancellationTokenStore::new();

        let first = store.get_or_insert_new("turn-1");
        let second = store.get_or_insert_new("turn-1");

        assert!(store.has_active("turn-1"));
        first.cancel();
        assert!(second.is_cancelled());
        assert!(store.is_cancelled("turn-1"));
    }

    #[test]
    fn turn_cancellation_store_cancels_registered_token() {
        let store = DialogTurnCancellationTokenStore::new();
        let token = CancellationToken::new();

        store.insert("turn-1", token.clone());

        assert!(store.cancel("turn-1"));
        assert!(token.is_cancelled());
        assert!(store.is_cancelled("turn-1"));
        assert!(store.remove("turn-1"));
        assert!(!store.has_active("turn-1"));
    }
}

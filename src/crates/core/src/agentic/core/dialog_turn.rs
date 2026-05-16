//! Dialog turn helpers and statistics types.
//!
//! Historical note: this module used to define `DialogTurn` and
//! `DialogTurnState` structs that were never persisted nor read back —
//! the actual on-disk shape lives in `service::session::DialogTurnData`,
//! and turn lifecycle state is tracked through `SessionState::Processing`
//! and `TurnStatus`. The orphan structs were removed; only `TurnStats`
//! and a small id-helper survive because they are still referenced by
//! `SessionManager::complete_dialog_turn` and friends.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Generate a fresh turn id when callers do not supply one.
pub fn new_turn_id(provided: Option<String>) -> String {
    provided.unwrap_or_else(|| Uuid::new_v4().to_string())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TurnStats {
    pub total_rounds: usize,
    pub total_tools: usize,
    pub total_tokens: usize,
    pub duration_ms: u64,
}

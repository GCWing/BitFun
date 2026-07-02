use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemorySourceSession {
    pub workspace_path: String,
    pub rollout_path: String,
    pub session_id: String,
    pub session_name: String,
    pub agent_type: String,
    pub turn_count: usize,
    pub last_finished_unix_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryExtractionRecord {
    pub source: MemorySourceSession,
    pub raw_memory: String,
    pub rollout_summary: String,
    pub rollout_slug: Option<String>,
    pub created_at_unix_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryPhase1RunStats {
    pub scanned_sessions: usize,
    pub candidate_sessions: usize,
    pub extracted_sessions: usize,
    pub skipped_sessions: usize,
    pub failed_sessions: usize,
}

impl Default for MemoryPhase1RunStats {
    fn default() -> Self {
        Self {
            scanned_sessions: 0,
            candidate_sessions: 0,
            extracted_sessions: 0,
            skipped_sessions: 0,
            failed_sessions: 0,
        }
    }
}

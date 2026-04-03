/// Event type definitions
///
/// Define cross-platform event data structures
use serde::{Deserialize, Serialize};

/// Event priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[derive(Default)]
pub enum EventPriority {
    Low = 0,
    #[default]
    Normal = 1,
    High = 2,
}


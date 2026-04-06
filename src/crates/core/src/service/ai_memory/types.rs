//! AI memory point type definitions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Memory type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum MemoryType {
    /// Technology preference
    TechPreference,
    /// Project context
    ProjectContext,
    /// User habit
    UserHabit,
    /// Code pattern
    CodePattern,
    /// Architecture decision
    Decision,
    /// Other
    #[default]
    Other,
}

/// AI memory point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIMemory {
    /// Unique identifier
    pub id: String,
    /// Title
    pub title: String,
    /// Content
    pub content: String,
    /// Type
    #[serde(rename = "type")]
    pub memory_type: MemoryType,
    /// Tags
    pub tags: Vec<String>,
    /// Source
    pub source: String,
    /// Created time (ISO 8601 format)
    pub created_at: String,
    /// Updated time (ISO 8601 format)
    pub updated_at: String,
    /// Importance 1-5
    pub importance: u8,
    /// Whether enabled
    pub enabled: bool,
}

impl AIMemory {
    /// Creates a new memory point.
    pub fn new(title: String, content: String, memory_type: MemoryType, importance: u8) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            content,
            memory_type,
            tags: vec![],
            source: "User manually added".to_string(),
            created_at: now.clone(),
            updated_at: now,
            importance: importance.min(5),
            enabled: true,
        }
    }
}

/// Memory storage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryStorage {
    /// All memory points
    pub memories: Vec<AIMemory>,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl MemoryStorage {
    /// Creates a new storage.
    pub fn new() -> Self {
        Self {
            memories: vec![],
            metadata: HashMap::new(),
        }
    }

    /// Adds a memory point.
    pub fn add_memory(&mut self, memory: AIMemory) {
        self.memories.push(memory);
        self.update_metadata();
    }

    /// Removes a memory point.
    pub fn remove_memory(&mut self, id: &str) -> bool {
        let len_before = self.memories.len();
        self.memories.retain(|m| m.id != id);
        let removed = self.memories.len() != len_before;
        if removed {
            self.update_metadata();
        }
        removed
    }

    /// Updates a memory point.
    pub fn update_memory(&mut self, memory: AIMemory) -> bool {
        if let Some(pos) = self.memories.iter().position(|m| m.id == memory.id) {
            let mut updated = memory;
            updated.updated_at = chrono::Utc::now().to_rfc3339();
            self.memories[pos] = updated;
            self.update_metadata();
            true
        } else {
            false
        }
    }

    /// Returns enabled memory points.
    pub fn get_enabled_memories(&self) -> Vec<&AIMemory> {
        self.memories.iter().filter(|m| m.enabled).collect()
    }

    /// Updates metadata.
    fn update_metadata(&mut self) {
        self.metadata
            .insert("updated_at".to_string(), chrono::Utc::now().to_rfc3339());
        self.metadata
            .insert("count".to_string(), self.memories.len().to_string());
    }
}

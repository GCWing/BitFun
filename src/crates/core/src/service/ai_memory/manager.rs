//! AI memory point manager

use super::types::{AIMemory, MemoryStorage, MemoryType};
use crate::infrastructure::PathManager;
use crate::util::errors::{BitFunError, BitFunResult};
use log::debug;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

/// AI memory point manager
pub struct AIMemoryManager {
    /// Path manager
    #[allow(dead_code)]
    path_manager: Arc<PathManager>,
    /// In-memory cache
    storage: Arc<RwLock<MemoryStorage>>,
    /// Storage file path
    storage_path: PathBuf,
}

impl AIMemoryManager {
    /// Creates a new memory manager (user-level).
    pub async fn new(path_manager: Arc<PathManager>) -> BitFunResult<Self> {
        let storage_path = path_manager.user_data_dir().join("ai_memories.json");

        if let Some(parent) = storage_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                BitFunError::io(format!("Failed to create memory storage directory: {}", e))
            })?;
        }

        let storage = if storage_path.exists() {
            Self::load_storage(&storage_path).await?
        } else {
            MemoryStorage::new()
        };

        Ok(Self {
            path_manager,
            storage: Arc::new(RwLock::new(storage)),
            storage_path,
        })
    }

    /// Creates a new memory manager (project-level).
    pub async fn new_project(
        path_manager: Arc<PathManager>,
        workspace_path: &str,
    ) -> BitFunResult<Self> {
        let workspace_path = PathBuf::from(workspace_path);
        let storage_path = workspace_path.join(".bitfun").join("ai_memories.json");

        if let Some(parent) = storage_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                BitFunError::io(format!("Failed to create memory storage directory: {}", e))
            })?;
        }

        let storage = if storage_path.exists() {
            Self::load_storage(&storage_path).await?
        } else {
            MemoryStorage::new()
        };

        Ok(Self {
            path_manager,
            storage: Arc::new(RwLock::new(storage)),
            storage_path,
        })
    }

    /// Loads storage from disk.
    async fn load_storage(path: &PathBuf) -> BitFunResult<MemoryStorage> {
        let content = fs::read_to_string(path)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read memory storage file: {}", e)))?;

        let storage: MemoryStorage = serde_json::from_str(&content).map_err(|e| {
            BitFunError::Deserialization(format!("Failed to deserialize memory storage: {}", e))
        })?;

        debug!("Loaded {} memory points from disk", storage.memories.len());
        Ok(storage)
    }

    /// Saves storage to disk.
    async fn save_storage(&self) -> BitFunResult<()> {
        let storage = self.storage.read().await;
        let content = serde_json::to_string_pretty(&*storage).map_err(|e| {
            BitFunError::serialization(format!("Failed to serialize memory storage: {}", e))
        })?;

        fs::write(&self.storage_path, content)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write memory storage file: {}", e)))?;

        debug!(
            "Memory points saved to disk: {}",
            self.storage_path.display()
        );
        Ok(())
    }

    /// Returns the storage path (for debugging and logging).
    pub fn get_storage_path(&self) -> &PathBuf {
        &self.storage_path
    }

    /// Adds a memory point.
    pub async fn add_memory(&self, memory: AIMemory) -> BitFunResult<AIMemory> {
        let mut storage = self.storage.write().await;
        let memory_clone = memory.clone();
        storage.add_memory(memory);
        drop(storage);

        self.save_storage().await?;
        Ok(memory_clone)
    }

    /// Deletes a memory point.
    pub async fn delete_memory(&self, id: &str) -> BitFunResult<bool> {
        let mut storage = self.storage.write().await;
        let removed = storage.remove_memory(id);
        drop(storage);

        if removed {
            self.save_storage().await?;
        }
        Ok(removed)
    }

    /// Updates a memory point.
    pub async fn update_memory(&self, memory: AIMemory) -> BitFunResult<bool> {
        let mut storage = self.storage.write().await;
        let updated = storage.update_memory(memory);
        drop(storage);

        if updated {
            self.save_storage().await?;
        }
        Ok(updated)
    }

    /// Returns all memory points.
    pub async fn get_all_memories(&self) -> BitFunResult<Vec<AIMemory>> {
        let storage = self.storage.read().await;
        Ok(storage.memories.clone())
    }

    /// Returns enabled memory points.
    pub async fn get_enabled_memories(&self) -> BitFunResult<Vec<AIMemory>> {
        let storage = self.storage.read().await;
        Ok(storage
            .get_enabled_memories()
            .into_iter()
            .cloned()
            .collect())
    }

    /// Gets memory points for prompt assembly.
    /// Returns a formatted string that can be appended to the prompt directly.
    pub async fn get_memories_for_prompt(&self) -> BitFunResult<Option<String>> {
        let memories = self.get_enabled_memories().await?;

        if memories.is_empty() {
            return Ok(None);
        }

        let mut sorted_memories = memories;
        sorted_memories.sort_by(|a, b| b.importance.cmp(&a.importance));

        let mut prompt = String::from("# Memory Points\n");
        prompt.push_str("The following are important memory points set by the user, consider these information in the conversation\n\n");

        for memory in sorted_memories.iter() {
            let type_label = match memory.memory_type {
                MemoryType::TechPreference => "Technology Preference",
                MemoryType::ProjectContext => "Project Context",
                MemoryType::UserHabit => "User Habit",
                MemoryType::CodePattern => "Code Pattern",
                MemoryType::Decision => "Architecture Decision",
                MemoryType::Other => "Others",
            };

            prompt.push_str(&format!(
                "## {} [{}] (Importance: {}/5)\n{}\n",
                memory.title, type_label, memory.importance, memory.content
            ));
            prompt.push_str("\n");
        }
        prompt.push_str("\n");

        Ok(Some(prompt))
    }

    /// Toggles whether a memory point is enabled.
    pub async fn toggle_memory(&self, id: &str) -> BitFunResult<bool> {
        let mut storage = self.storage.write().await;

        if let Some(memory) = storage.memories.iter_mut().find(|m| m.id == id) {
            memory.enabled = !memory.enabled;
            memory.updated_at = chrono::Utc::now().to_rfc3339();
            let new_state = memory.enabled;
            drop(storage);

            self.save_storage().await?;
            Ok(new_state)
        } else {
            Ok(false)
        }
    }
}

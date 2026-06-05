//! Project Context service type definitions
//!
//! Defines data structures related to project context.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Document priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentPriority {
    High,
    Medium,
    Low,
}

impl DocumentPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentPriority::High => "high",
            DocumentPriority::Medium => "medium",
            DocumentPriority::Low => "low",
        }
    }
}

/// Category ID
///
/// Built-in categories:
/// - "general" - General docs (AGENTS.md, CLAUDE.md, README.md, etc.)
/// - "coding" - Coding standards (EditorConfig, ESLint, Prettier, etc.)
/// - "design" - Design docs (ARCHITECTURE.md, API-DESIGN.md, etc.)
/// - "review" - Review docs
pub type CategoryId = String;

/// Document status (returned to the frontend, includes full info)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextDocumentStatus {
    /// Document ID
    pub id: String,
    /// Document name
    pub name: String,
    /// Category ID
    pub category_id: String,
    /// Existing file path (`None` means the file does not exist)
    pub file_path: Option<String>,
    /// Whether the file exists
    pub exists: bool,
    /// Whether enabled
    pub enabled: bool,
    /// Whether AI generation is supported
    pub can_generate: bool,
    /// Priority
    pub priority: String,
    /// Estimated token count
    pub token_count: u32,
}

/// Project context configuration (persisted)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectContextConfig {
    /// Document enabled state mapping: `doc_id -> enabled`
    #[serde(default)]
    pub enabled_documents: HashMap<String, bool>,

    /// Custom category list
    #[serde(default)]
    pub custom_categories: Vec<CustomCategory>,

    /// Imported document list
    #[serde(default)]
    pub imported_documents: Vec<ImportedDocument>,
}

/// Custom category
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomCategory {
    /// Category ID (auto-generated: `custom-{timestamp}`)
    pub id: String,
    /// Category name
    pub name: String,
    /// Category description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Icon name (lucide-react icon name)
    pub icon: String,
    /// Created timestamp
    pub created_at: i64,
    /// Sort order
    pub order: usize,
}

/// Imported document
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedDocument {
    /// Document ID (UUID)
    pub id: String,
    /// Document name (file name)
    pub name: String,
    /// Category ID (built-in or custom)
    pub category_id: String,
    /// File path (relative path under `.bitfun/docs/{category_id}/{id}`)
    pub file_path: String,
    /// Priority
    pub priority: String,
    /// Created timestamp
    pub created_at: i64,
}

/// Category information (returned to the frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInfo {
    /// Category ID
    pub id: String,
    /// Category name
    pub name: String,
    /// Category description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Icon name (lucide-react icon name)
    pub icon: String,
    /// Whether this is a built-in category
    pub is_builtin: bool,
    /// Sort order
    pub order: usize,
}

/// File conflict action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileConflictAction {
    /// Skip (do not import)
    Skip,
    /// Overwrite an existing file
    Overwrite,
    /// Auto-rename
    Rename,
}

/// Context prompt segment
#[derive(Debug, Clone)]
pub struct ContextSegment {
    /// Document ID
    pub doc_id: String,
    /// Document name
    pub name: String,
    /// Category ID
    pub category_id: String,
    /// Document content
    pub content: String,
    /// Token count
    pub token_count: u32,
    /// Priority
    pub priority: DocumentPriority,
}

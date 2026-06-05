//! Project Context service
//!
//! Provides project context document management, including:
//! - Querying document status (exists/enabled)
//! - Creating/generating documents
//! - Building context prompts

pub mod builtin_documents;
pub mod cancellation;
pub mod document_template;
pub mod generation_prompt;
pub mod service;
pub mod types;

pub use builtin_documents::{find_builtin_document, get_builtin_categories, BUILTIN_DOCUMENTS};
pub use service::ProjectContextService;
pub use types::{
    CategoryId, CategoryInfo, ContextDocumentStatus, ContextSegment, DocumentPriority,
    FileConflictAction, ImportedDocument, ProjectContextConfig,
};

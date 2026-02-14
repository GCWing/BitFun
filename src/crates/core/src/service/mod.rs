//! Service layer module
//!
//! Contains core business logic: Workspace, Config, FileSystem, Git, Agentic, AIRules, MCP.

pub mod ai_memory; // AI memory point management
pub mod ai_rules; // AI rules management
pub mod config; // Config management
pub mod conversation; // Conversation history persistence
pub mod diff;
pub mod filesystem; // FileSystem management
pub mod git; // Git service
pub mod i18n; // I18n service
pub mod lsp; // LSP (Language Server Protocol) system
pub mod mcp; // MCP (Model Context Protocol) system
pub mod project_context; // Project context management
pub mod runtime; // Managed runtime and capability management
pub mod snapshot; // Snapshot-based change tracking
pub mod system; // System command detection and execution
pub mod workspace; // Workspace management // Diff calculation and merge service

// Terminal is a standalone crate; re-export it here.
pub use terminal_core as terminal;

// Re-export main components.
pub use ai_memory::{AIMemory, AIMemoryManager, MemoryType};
pub use ai_rules::AIRulesService;
pub use config::{ConfigManager, ConfigProvider, ConfigService};
pub use diff::{
    DiffConfig, DiffHunk, DiffLine, DiffLineType, DiffOptions, DiffResult, DiffService,
};
pub use filesystem::{DirectoryStats, FileSystemService, FileSystemServiceFactory};
pub use git::GitService;
pub use i18n::{get_global_i18n_service, I18nConfig, I18nService, LocaleId, LocaleMetadata};
pub use lsp::LspManager;
pub use mcp::MCPService;
pub use project_context::{ContextDocumentStatus, ProjectContextConfig, ProjectContextService};
pub use runtime::{ResolvedCommand, RuntimeCommandCapability, RuntimeManager, RuntimeSource};
pub use snapshot::SnapshotService;
pub use system::{
    check_command, check_commands, run_command, run_command_simple, CheckCommandResult,
    CommandOutput, SystemError,
};
pub use workspace::{WorkspaceManager, WorkspaceProvider, WorkspaceService};

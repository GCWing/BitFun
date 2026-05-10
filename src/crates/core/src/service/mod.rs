//! Service layer module
//!
//! Contains core business logic: Workspace, Config, FileSystem, Git, Agentic, MCP.

pub(crate) mod agent_memory; // Agent memory prompt helpers
pub mod announcement; // Announcement / feature-demo / tips system
pub(crate) mod bootstrap; // Workspace persona bootstrap helpers
pub mod config; // Config management
pub mod cron; // Scheduled jobs
pub mod diff;
pub mod file_watch;
pub mod filesystem; // FileSystem management
pub mod git; // Git service
pub mod i18n; // I18n service
pub mod lsp; // LSP (Language Server Protocol) system
pub mod mcp; // MCP (Model Context Protocol) system
pub mod project_context; // Project context management
pub mod remote_connect; // Remote Connect (phone → desktop)
pub mod remote_ssh; // Remote SSH (desktop → server)
pub mod runtime; // Managed runtime and capability management
pub mod search; // Workspace search via managed flashgrep daemon
pub mod session; // Session persistence
pub mod session_usage; // Session runtime usage reports
pub mod snapshot; // Snapshot-based change tracking
pub mod system; // System command detection and execution
pub mod token_usage; // Token usage tracking
pub mod workspace; // Workspace management // Diff calculation and merge service
pub mod workspace_runtime; // Workspace runtime layout / migration / initialization

// Terminal is implemented in the workspace-level `terminal-core` crate.
// This re-export preserves the legacy `bitfun_core::service::terminal` path.
pub use terminal_core as terminal;

// Re-export main components.
pub use announcement::{AnnouncementCard, AnnouncementScheduler, AnnouncementSchedulerRef};
pub use bootstrap::reset_workspace_persona_files_to_default;
pub use config::{ConfigManager, ConfigProvider, ConfigService};
pub use cron::{
    get_global_cron_service, set_global_cron_service, CronEventSubscriber, CronService,
};
pub use diff::{
    DiffConfig, DiffHunk, DiffLine, DiffLineType, DiffOptions, DiffResult, DiffService,
};
pub use file_watch::{
    get_global_file_watch_service, get_watched_paths, initialize_file_watch_service,
    start_file_watch, stop_file_watch, FileWatchEvent, FileWatchEventKind, FileWatchService,
    FileWatcherConfig,
};
pub use filesystem::{DirectoryStats, FileSystemService, FileSystemServiceFactory};
pub use git::GitService;
pub use i18n::{get_global_i18n_service, I18nConfig, I18nService, LocaleId, LocaleMetadata};
pub use lsp::LspManager;
pub use mcp::MCPService;
pub use project_context::{ContextDocumentStatus, ProjectContextConfig, ProjectContextService};
pub use runtime::{ResolvedCommand, RuntimeCommandCapability, RuntimeManager, RuntimeSource};
pub use search::{
    get_global_workspace_search_service, set_global_workspace_search_service, ContentSearchRequest,
    ContentSearchResult, GlobSearchRequest, GlobSearchResult, IndexTaskHandle,
    WorkspaceIndexStatus, WorkspaceSearchBackend, WorkspaceSearchContextLine,
    WorkspaceSearchDirtyFiles, WorkspaceSearchFileCount, WorkspaceSearchHit, WorkspaceSearchLine,
    WorkspaceSearchMatch, WorkspaceSearchMatchLocation, WorkspaceSearchOverlayStatus,
    WorkspaceSearchRepoPhase, WorkspaceSearchRepoStatus, WorkspaceSearchService,
    WorkspaceSearchTaskKind, WorkspaceSearchTaskPhase, WorkspaceSearchTaskState,
    WorkspaceSearchTaskStatus,
};
pub use snapshot::SnapshotService;
pub use system::{
    check_command, check_commands, run_command, run_command_simple, CheckCommandResult,
    CommandOutput, SystemError,
};
pub use token_usage::{
    ModelTokenStats, SessionTokenStats, TimeRange, TokenUsageQuery, TokenUsageRecord,
    TokenUsageService, TokenUsageSummary,
};
pub use workspace::{WorkspaceManager, WorkspaceProvider, WorkspaceService};
pub use workspace_runtime::{
    get_workspace_runtime_service_arc, try_get_workspace_runtime_service_arc,
    RuntimeMigrationRecord, WorkspaceRuntimeContext, WorkspaceRuntimeEnsureResult,
    WorkspaceRuntimeService, WorkspaceRuntimeTarget,
};

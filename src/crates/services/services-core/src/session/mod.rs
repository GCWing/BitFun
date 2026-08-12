pub mod layout;
mod lineage;
#[cfg(feature = "session-git")]
mod memory_workspace;
mod metadata;
mod metadata_store;
mod migration;
pub mod page;
pub mod tree;
pub mod types;
mod write_lock;

pub mod group_chat_layout;
mod group_chat_membership;
pub mod group_chat_store;
pub use group_chat_layout::{validate_room_id, GroupChatStorageLayout};
pub use group_chat_membership::{
    add_room_to_group_chats, group_chats_of, remove_room_from_group_chats, GROUP_CHATS_METADATA_KEY,
};
pub use group_chat_store::{
    GroupChatCatalogEntry, GroupChatMessagesWindow, GroupChatStore, GroupChatStoreError,
    StoredGroupChatIndexFile, StoredGroupChatMembersFile, StoredGroupChatMessageCatalog,
    StoredGroupChatMetaFile, GROUP_CHAT_CATALOG_PREVIEW_CHAR_LIMIT,
    GROUP_CHAT_INDEX_SCHEMA_VERSION, GROUP_CHAT_STORAGE_SCHEMA_VERSION,
};

pub use bitfun_core_types::SessionKind;
pub use layout::SessionStorageLayout;
pub use lineage::{
    apply_session_lineage, build_branched_session_metadata, build_session_lineage_snapshot,
    collect_hidden_subagent_cascade, format_branch_session_name, resolve_branch_session_lineage,
    BranchSessionLineage, BranchSessionMetadataFacts, SessionBranchBoundary, SessionBranchRequest,
    SessionBranchResult, SessionLineageSnapshot,
};
#[cfg(feature = "session-git")]
pub use memory_workspace::{
    ensure_memory_workspace_git_baseline, memory_workspace_diff, render_memory_workspace_diff_file,
    reset_memory_workspace_git_baseline, MemoryWorkspaceChange, MemoryWorkspaceChangeStatus,
    MemoryWorkspaceDiff, MemoryWorkspaceGitError,
};
pub use metadata::{
    build_session_index_snapshot, build_session_metadata, estimate_turn_message_count,
    merge_session_custom_metadata, normalized_session_relationship,
    refresh_session_metadata_from_turns, remove_session_index_entry, set_deep_review_cache,
    set_deep_review_run_manifest, set_review_target_evidence, set_session_relationship,
    try_refresh_session_metadata_for_saved_turn, upsert_session_index_entry,
    SessionMetadataBuildFacts,
};
pub use metadata_store::{SessionMetadataStore, SessionMetadataStoreError};
pub use migration::{
    merge_legacy_session_store, move_legacy_path, SessionStoreMigrationError,
    SessionStoreMigrationRecord,
};
pub use page::{build_session_metadata_page, empty_session_metadata_page, SessionMetadataPage};
pub use types::*;
pub use write_lock::{SessionWriteLock, SessionWriteLockError};

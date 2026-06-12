pub mod layout;
mod lineage;
mod metadata;
pub mod page;
pub mod types;

pub use bitfun_core_types::SessionKind;
pub use layout::SessionStorageLayout;
pub use lineage::{
    apply_session_lineage, build_branched_session_metadata, collect_hidden_subagent_cascade,
    BranchSessionMetadataFacts, SessionBranchRequest, SessionBranchResult,
};
pub use metadata::{
    build_session_index_snapshot, build_session_metadata, merge_session_custom_metadata,
    refresh_session_metadata_from_turns, remove_session_index_entry, set_deep_review_cache,
    set_deep_review_run_manifest, set_session_relationship,
    try_refresh_session_metadata_for_saved_turn, upsert_session_index_entry,
    SessionMetadataBuildFacts,
};
pub use page::{build_session_metadata_page, empty_session_metadata_page, SessionMetadataPage};
pub use types::*;

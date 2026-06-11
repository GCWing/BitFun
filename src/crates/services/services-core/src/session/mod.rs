pub mod layout;
mod metadata;
pub mod page;
pub mod types;

pub use bitfun_core_types::SessionKind;
pub use layout::SessionStorageLayout;
pub use metadata::{
    build_session_index_snapshot, refresh_session_metadata_from_turns, remove_session_index_entry,
    try_refresh_session_metadata_for_saved_turn, upsert_session_index_entry,
};
pub use page::{build_session_metadata_page, empty_session_metadata_page, SessionMetadataPage};
pub use types::*;

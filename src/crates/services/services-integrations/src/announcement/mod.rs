//! Announcement service data contracts.

mod state_store;
mod types;

pub use state_store::{
    AnnouncementStateStore, AnnouncementStateStoreError, AnnouncementStateStoreResult,
};
pub use types::*;

//! Announcement service data contracts.

mod remote;
mod state_store;
mod types;

pub use remote::{AnnouncementRemoteFetchRequest, RemoteAnnouncementFetcher};
pub use state_store::{
    AnnouncementStateStore, AnnouncementStateStoreError, AnnouncementStateStoreResult,
};
pub use types::*;

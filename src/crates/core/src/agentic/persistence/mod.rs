//! Persistence layer
//!
//! Responsible for persistent storage and loading of data

pub mod manager;
pub mod session_branch;

pub use manager::PersistenceManager;
pub use session_branch::{SessionBranchRequest, SessionBranchResult};

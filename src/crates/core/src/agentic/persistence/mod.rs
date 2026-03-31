//! Persistence layer
//!
//! Responsible for persistent storage and loading of data

pub mod manager;
pub mod session_workspace_maintenance;

pub use manager::PersistenceManager;
pub use session_workspace_maintenance::{
    SessionWorkspaceMaintenanceReport, SessionWorkspaceMaintenanceService,
};

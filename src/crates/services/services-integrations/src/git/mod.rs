//! Git service contracts.
//!
//! `bitfun-core::service::git` remains as the compatibility facade for the
//! legacy public path.

pub mod args;
pub mod error;
pub mod graph;
pub mod name_status;
pub mod service;
pub mod text;
pub mod types;
pub mod utils;
pub mod worktree;

pub use args::{build_git_changed_files_args, build_git_diff_args};
pub use error::GitError;
pub use graph::{build_git_graph, build_git_graph_for_branch};
pub use name_status::parse_name_status_output;
pub use service::GitService;
pub use text::{parse_branch_line, parse_git_log_line};
pub use types::*;
pub use utils::*;
pub use worktree::parse_worktree_list;

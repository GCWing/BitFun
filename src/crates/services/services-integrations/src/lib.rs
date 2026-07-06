//! Integration service owner crate.
//!
//! Heavy external integrations live here behind feature groups so local checks
//! can opt into only the integration family they need.

#[cfg(feature = "announcement")]
pub mod announcement;

#[cfg(feature = "browser-control")]
pub mod browser_control;

#[cfg(feature = "canvas-runtime")]
pub mod canvas;

#[cfg(feature = "debug-log")]
pub mod debug_log;

#[cfg(feature = "deep-research")]
pub mod deep_research;

#[cfg(feature = "file-watch")]
pub mod file_watch;

#[cfg(feature = "function-agents")]
pub mod function_agents;

#[cfg(feature = "git")]
pub mod git;

#[cfg(feature = "mcp")]
pub mod mcp;

#[cfg(feature = "miniapp-runtime")]
pub mod miniapp;

#[cfg(feature = "remote-connect")]
pub mod remote_connect;

#[cfg(feature = "remote-ssh")]
pub mod remote_ssh;

#[cfg(feature = "review-platform")]
pub mod review_platform;

#[cfg(feature = "review-platform")]
pub(crate) mod review_platform_http;

#[cfg(feature = "workspace-search")]
pub mod workspace_search;

#[cfg(feature = "web-tools")]
pub mod web_tools;

#[cfg(all(windows, feature = "git"))]
#[link(name = "advapi32")]
unsafe extern "system" {}

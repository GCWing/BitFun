//! Browser control via Chrome DevTools Protocol (CDP).
//!
//! Connects to the user's default browser (Chrome, Edge, etc.) over a
//! CDP WebSocket, enabling page navigation, DOM interaction, screenshots,
//! JS evaluation and more — all while preserving the user's existing
//! cookies, extensions, and login sessions.

pub mod actions;
pub mod browser_launcher;
pub mod cdp_client;
pub mod session_registry;

pub use actions::BrowserActions;
pub use browser_launcher::BrowserLauncher;
pub use cdp_client::CdpClient;
pub use session_registry::{BrowserSession, BrowserSessionRegistry};

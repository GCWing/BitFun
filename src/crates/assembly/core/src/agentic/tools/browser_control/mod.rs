//! Browser control via Chrome DevTools Protocol (CDP).
//!
//! Connects to a Chromium-family browser over CDP, enabling page navigation,
//! DOM interaction, screenshots, JS evaluation and more. Chrome 144+ and Edge
//! use user-approved live-profile endpoints so existing tabs, cookies,
//! extensions and login sessions are preserved. Other Chromium browsers reuse
//! a real-profile endpoint when available and retain a managed fallback.

pub mod actions;
pub mod browser_launcher;
pub mod cdp_client;
pub mod session_registry;

pub use actions::BrowserActions;
pub use browser_launcher::BrowserLauncher;
pub use cdp_client::CdpClient;
pub use session_registry::{
    BrowserSession, BrowserSessionRegistry, BrowserSessionState, DialogHandler,
};

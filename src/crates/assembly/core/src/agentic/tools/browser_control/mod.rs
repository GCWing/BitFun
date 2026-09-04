//! Browser automation shared by external CDP browsers and OpenBitFun's built-in
//! browser WebViews.
//!
//! [`BrowserActions`] owns the product-level action semantics (snapshot refs,
//! click guards, filling, waiting, capture, and so on). Concrete browser hosts
//! implement [`BrowserAutomationClient`], so a second target never grows a
//! second copy of the action implementation.

pub mod actions;
pub mod automation_client;
pub mod browser_launcher;
pub mod builtin_browser;
pub mod cdp_client;
pub mod session_registry;

pub use actions::BrowserActions;
pub use automation_client::{
    BrowserAutomationCapabilities, BrowserAutomationClient, BrowserAutomationEvent,
};
pub use browser_launcher::BrowserLauncher;
pub use builtin_browser::{
    connect_builtin_browser, connect_builtin_browser_matching, list_builtin_browser_targets,
    open_builtin_browser, BuiltInBrowserClient, BuiltInBrowserOpenRequest, BuiltInBrowserTarget,
};
pub use cdp_client::CdpClient;
pub use session_registry::{
    BrowserSession, BrowserSessionRegistry, BrowserSessionState, DialogHandler,
};

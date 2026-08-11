/// CLI/TUI agent integration.
///
/// Session operations use the shared Agent Runtime SDK. Event consumption
/// remains in the chat and exec mode loops.
pub(crate) mod agentic_system;
pub(crate) mod exec_runtime_client;
pub(crate) mod tui_client;

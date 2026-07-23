/// CLI/TUI agent integration.
///
/// Session operations use the shared Agent Runtime SDK. Event consumption
/// remains in the chat and exec mode loops.
pub(crate) mod agentic_system;
pub(crate) mod runtime_client;

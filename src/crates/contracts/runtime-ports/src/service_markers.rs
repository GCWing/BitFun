use super::*;

pub trait FileSystemPort: RuntimeServicePort {}

pub trait WorkspacePort: RuntimeServicePort {}

pub trait ClockPort: RuntimeServicePort {
    fn now_unix_millis(&self) -> i64;
}

pub trait NetworkPort: RuntimeServicePort {}

pub trait McpCatalogPort: RuntimeServicePort {}

/// Typed registration boundary for remote connection providers.
///
/// PR1 intentionally keeps this trait handle-free; PR2 adds owner-specific
/// lifecycle methods once behavior-equivalence tests are in place.
pub trait RemoteConnectionPort: RuntimeServicePort {}

pub trait RemoteCapabilityPort: RuntimeServicePort {}

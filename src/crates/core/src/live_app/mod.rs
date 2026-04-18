//! Live App module — V2: ESM UI + Node Worker, Runtime Adapter, permission policy.

pub mod bridge_builder;
pub mod compiler;
pub mod exporter;
pub mod js_worker;
pub mod js_worker_pool;
pub mod manager;
pub mod permission_policy;
pub mod runtime_detect;
pub mod storage;
pub mod types;

pub use exporter::{ExportCheckResult, ExportOptions, ExportResult, ExportTarget, LiveAppExporter};
pub use js_worker_pool::{InstallResult, JsWorkerPool};
pub use manager::{
    initialize_global_live_app_manager, try_get_global_live_app_manager, LiveAppManager,
};
pub use permission_policy::resolve_policy;
pub use runtime_detect::{DetectedRuntime, RuntimeKind};
pub use storage::LiveAppStorage;
pub use types::{
    AiPermissions, EsmDep, FsPermissions, LiveApp, LiveAppAiContext, LiveAppMeta,
    LiveAppPermissions, LiveAppSource, NetPermissions, NodePermissions, NpmDep, PathScope,
    ShellPermissions,
};

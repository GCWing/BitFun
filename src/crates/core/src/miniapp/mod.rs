//! MiniApp module — V2: ESM UI + Node Worker, Runtime Adapter, permission policy.

pub mod bridge_builder;
pub mod builtin;
pub mod compiler;
pub mod exporter;
pub mod host_dispatch;
pub mod js_worker;
pub mod js_worker_pool;
pub mod manager;
pub mod permission_policy;
pub mod runtime_detect;
pub mod storage;
pub mod types;

pub use builtin::{seed_builtin_miniapps, BuiltinApp, BUILTIN_APPS};
pub use exporter::{ExportCheckResult, ExportOptions, ExportResult, ExportTarget, MiniAppExporter};
pub use host_dispatch::{dispatch_host, is_host_primitive};
pub use js_worker_pool::{InstallResult, JsWorkerPool};
pub use manager::{
    initialize_global_miniapp_manager, try_get_global_miniapp_manager, MiniAppManager,
};
pub use permission_policy::resolve_policy;
pub use runtime_detect::{DetectedRuntime, RuntimeKind};
pub use storage::MiniAppStorage;
pub use types::{
    AiPermissions, EsmDep, FsPermissions, MiniApp, MiniAppAiContext, MiniAppMeta,
    MiniAppPermissions, MiniAppSource, NetPermissions, NodePermissions, NpmDep, PathScope,
    ShellPermissions,
};

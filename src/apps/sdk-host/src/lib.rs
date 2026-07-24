//! Process-level bootstrap shared by the standalone SDK Host entrypoint and tests.

pub mod transport;

/// Stack size used by the SDK Host worker.
///
/// The Host initializes the same full Agent Runtime as the CLI and preserves
/// the reviewed Windows stack-overflow protection used by that runtime.
pub const SDK_HOST_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Installs process-global prerequisites before any TLS-capable service starts.
pub fn initialize_process_runtime() {
    bitfun_core::service::remote_connect::ensure_rustls_crypto_provider();
}

/// Spawns the SDK Host runtime on the reviewed worker-stack boundary.
pub fn spawn_sdk_host_worker<T, F>(task: F) -> std::io::Result<std::thread::JoinHandle<T>>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    std::thread::Builder::new()
        .name("bitfun-sdk-host".to_string())
        .stack_size(SDK_HOST_WORKER_STACK_BYTES)
        .spawn(task)
}

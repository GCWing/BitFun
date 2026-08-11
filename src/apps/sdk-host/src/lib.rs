//! Process-level bootstrap shared by the standalone SDK Host entrypoint and tests.

pub mod transport;

/// Stack size used by the SDK Host worker.
///
/// The Host initializes its reviewed SDK capability profile and preserves the
/// Windows stack-overflow protection used by the shared Agent Runtime.
pub const SDK_HOST_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Installs process-global prerequisites before any TLS-capable service starts.
pub fn initialize_process_runtime() {
    let _ = rustls::crypto::ring::default_provider().install_default();
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

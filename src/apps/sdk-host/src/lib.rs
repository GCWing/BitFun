//! Process-level bootstrap shared by the standalone SDK Host entrypoint and tests.

pub mod transport;

/// Stack size used by the SDK Host worker.
///
/// The Host initializes its reviewed SDK capability profile and preserves the
/// Windows stack-overflow protection used by the shared Agent Runtime.
pub const SDK_HOST_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Installs process-global prerequisites before any service or descendant starts.
pub fn initialize_process_runtime() -> std::io::Result<()> {
    bitfun_services_core::process_manager::contain_current_process_tree()?;
    bitfun_services_core::tls_provider::ensure_ring_crypto_provider();
    Ok(())
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

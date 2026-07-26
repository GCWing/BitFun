#[test]
fn sdk_host_process_installs_a_rustls_crypto_provider() {
    bitfun_sdk_host_app::initialize_process_runtime();

    assert!(
        rustls::crypto::CryptoProvider::get_default().is_some(),
        "SDK Host must select a process-level crypto provider before HTTPS AI requests"
    );
}

#[test]
fn sdk_host_process_uses_the_reviewed_worker_stack_contract() {
    let caller = std::thread::current().id();
    let worker = bitfun_sdk_host_app::spawn_sdk_host_worker(|| std::thread::current().id())
        .expect("spawn SDK Host worker");

    assert_eq!(
        bitfun_sdk_host_app::SDK_HOST_WORKER_STACK_BYTES,
        16 * 1024 * 1024
    );
    assert_ne!(worker.join().expect("join SDK Host worker"), caller);
}

#[test]
fn sdk_host_process_keeps_cleanup_warnings_on_stderr() {
    let entrypoint = include_str!("../src/main.rs");

    assert!(entrypoint.contains(".with_max_level(tracing::Level::WARN)"));
    assert!(entrypoint.contains(".with_writer(std::io::stderr)"));
}
